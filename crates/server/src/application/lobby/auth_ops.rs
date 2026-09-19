//! Authentication-related operations on [`LobbyService`].
//!
//! This file implements the four public methods that manage the session
//! lifecycle for authentication:
//!
//! - [`LobbyService::hello`]: registers the display name and moves the
//!   session into the "guest" state.
//! - [`LobbyService::register_user`]: creates a new account and
//!   authenticates the session in one step.
//! - [`LobbyService::login_user`]: authenticates the session against an
//!   existing account, subject to the single-session invariant.
//! - [`LobbyService::pong`]: responds to a liveness probe.

use common::domain::{Age, Player, UserProfile, Username};
use common::protocol::{AuthFailureReason, ClientId, ServerMessage};

use super::{
    LobbyService, LobbyState, MAX_DISPLAY_NAME_LEN, MAX_PASSWORD_LEN, MIN_PASSWORD_LEN,
    reason_message,
};

impl LobbyService {
    /// Handles `Hello`.
    #[allow(clippy::significant_drop_tightening)] // reason: state is required to mutate session and send response
    pub fn hello(&self, client: ClientId, display_name: &str) {
        let mut state = self.lock();
        let Some(session) = state.sessions.get_mut(&client) else {
            return;
        };
        if session.display_name.is_some() {
            session.try_send(ServerMessage::Error {
                code: common::protocol::ErrorCode::InvalidState,
                message: String::from("client is already registered"),
            });
            return;
        }
        let trimmed = display_name.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_DISPLAY_NAME_LEN {
            session.try_send(ServerMessage::Error {
                code: common::protocol::ErrorCode::InvalidDisplayName,
                message: format!(
                    "display name must be 1 to {MAX_DISPLAY_NAME_LEN} non-empty characters"
                ),
            });
            return;
        }
        let owned = trimmed.to_string();
        session.display_name = Some(owned.clone());
        session.try_send(ServerMessage::Welcome {
            client_id: client,
            display_name: owned,
        });
    }

    /// Handles `Register`.
    pub async fn register_user(
        &self,
        client_id: ClientId,
        name: String,
        username: String,
        age: u8,
        password: String,
    ) {
        if !self.consume_auth_token(client_id) {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::RateLimited,
                reason_message(AuthFailureReason::RateLimited),
            );
            return;
        }

        if self.is_authenticated(client_id) {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::AlreadyAuthenticated,
                reason_message(AuthFailureReason::AlreadyAuthenticated),
            );
            return;
        }

        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::NameEmpty,
                reason_message(AuthFailureReason::NameEmpty),
            );
            return;
        }
        if trimmed_name.chars().count() > 64 {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::NameEmpty,
                "name must be at most 64 characters",
            );
            return;
        }

        let username = match Username::new(&username) {
            Ok(value) => value,
            Err(error) => {
                let rendered = error.to_string();
                self.send_auth_failure(client_id, AuthFailureReason::UsernameInvalid, &rendered);
                return;
            }
        };

        let age = match Age::new(age) {
            Ok(value) => value,
            Err(error) => {
                let rendered = error.to_string();
                self.send_auth_failure(client_id, AuthFailureReason::AgeOutOfRange, &rendered);
                return;
            }
        };

        let password_len = password.chars().count();
        if password_len < MIN_PASSWORD_LEN {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::PasswordTooShort,
                &format!("password must be at least {MIN_PASSWORD_LEN} characters"),
            );
            return;
        }
        if password_len > MAX_PASSWORD_LEN {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::PasswordTooShort,
                &format!("password must be at most {MAX_PASSWORD_LEN} characters"),
            );
            return;
        }

        let profile = UserProfile {
            name: trimmed_name.to_string(),
            username,
            age,
        };

        match self.auth.register(profile.clone(), password).await {
            Ok(()) => {
                let mut state = self.lock();
                state
                    .active_sessions
                    .insert(profile.username.clone(), client_id);
                if let Some(session) = state.sessions.get_mut(&client_id) {
                    session.authenticated_as = Some(profile.username.clone());
                    session.display_name = Some(profile.name.clone());
                    session.try_send(ServerMessage::Registered { profile });
                }
            }
            Err(reason) => {
                self.send_auth_failure(client_id, reason, reason_message(reason));
            }
        }
    }

    /// Handles `Login`.
    ///
    /// If the username has a pending disconnection, the login reclaims the
    /// slot instead of being rejected by the single-session rule.
    pub async fn login_user(&self, client_id: ClientId, username: String, password: String) {
        if !self.consume_auth_token(client_id) {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::RateLimited,
                reason_message(AuthFailureReason::RateLimited),
            );
            return;
        }

        if self.is_authenticated(client_id) {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::AlreadyAuthenticated,
                reason_message(AuthFailureReason::AlreadyAuthenticated),
            );
            return;
        }

        let Ok(username) = Username::new(&username) else {
            // Do not reveal whether the username is well-formed. From the
            // caller's perspective, an invalid username and a wrong
            // password are indistinguishable.
            self.send_auth_failure(
                client_id,
                AuthFailureReason::InvalidCredentials,
                reason_message(AuthFailureReason::InvalidCredentials),
            );
            return;
        };

        // A pending disconnection reserves the username but does not block
        // the legitimate owner from reclaiming it.
        let has_pending = self.has_pending_disconnection(&username);
        if !has_pending && self.is_username_active(&username) {
            self.send_auth_failure(
                client_id,
                AuthFailureReason::AlreadyLoggedIn,
                reason_message(AuthFailureReason::AlreadyLoggedIn),
            );
            return;
        }

        match self.auth.authenticate(&username, password).await {
            Ok(profile) => {
                let mut state = self.lock();
                if has_pending {
                    reconnect_pending(&mut state, &profile.username, client_id);
                } else {
                    state
                        .active_sessions
                        .insert(profile.username.clone(), client_id);
                }
                if let Some(session) = state.sessions.get_mut(&client_id) {
                    session.authenticated_as = Some(profile.username.clone());
                    session.display_name = Some(profile.name.clone());
                    session.try_send(ServerMessage::LoginSucceeded { profile });
                }
            }
            Err(reason) => {
                self.send_auth_failure(client_id, reason, reason_message(reason));
            }
        }
    }

    /// Handles `Ping`.
    pub fn pong(&self, client: ClientId) {
        let state = self.lock();
        if let Some(session) = state.sessions.get(&client) {
            session.try_send(ServerMessage::Pong);
        }
    }

    /// Returns `true` when the username has a pending disconnection.
    fn has_pending_disconnection(&self, username: &Username) -> bool {
        self.lock().pending_disconnections.contains_key(username)
    }

    /// Handles `ListRanking`.
    ///
    /// Returns the current top-10 ranking. The ranking is public, so guests
    /// and authenticated users can both request it. The list is computed
    /// before the lobby lock is taken, so the two locks are never held at
    /// the same time.
    pub fn list_ranking(&self, client: ClientId) {
        let entries = self.ranking.top(crate::application::ranking::TOP_N);
        let state = self.lock();
        if let Some(session) = state.sessions.get(&client) {
            session.try_send(ServerMessage::Ranking { entries });
        }
    }
}

/// Reconnects a freshly authenticated client to the match it was playing
/// before it disconnected.
///
/// Updates `active_sessions` to point at the new client, restores the slot
/// in the match, sets the session fields, and notifies the opponent and
/// the spectators with `OpponentReconnected`.
fn reconnect_pending(state: &mut LobbyState, username: &Username, new_client: ClientId) {
    let Some(pending) = state.pending_disconnections.remove(username) else {
        // No pending disconnection; treat as a fresh login.
        state.active_sessions.insert(username.clone(), new_client);
        return;
    };

    state.active_sessions.insert(username.clone(), new_client);

    let Some(m) = state.matches.get_mut(&pending.match_id) else {
        // The match vanished while the player was away. Nothing to restore.
        return;
    };
    if pending.was_host {
        m.host = new_client;
    } else {
        m.guest = Some(new_client);
    }

    let mark = if pending.was_host {
        Player::X
    } else {
        Player::O
    };
    let opponent = m.opponent_of(new_client);
    let reconnected = ServerMessage::OpponentReconnected {
        match_id: pending.match_id,
    };
    let spectators = m.spectators.clone();

    if let Some(session) = state.sessions.get_mut(&new_client) {
        session.current_match = Some(pending.match_id);
        session.mark = Some(mark);
    }

    if let Some(opponent_id) = opponent
        && let Some(session) = state.sessions.get(&opponent_id)
    {
        session.try_send(reconnected.clone());
    }
    for spectator in spectators {
        if let Some(session) = state.sessions.get(&spectator) {
            session.try_send(reconnected.clone());
        }
    }
}
