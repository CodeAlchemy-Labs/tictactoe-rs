//! In-memory lobby and match coordination.
//!
//! [`LobbyService`] is the single entry point for every state change that
//! involves connected clients. It owns a `Mutex<LobbyState>` and every public
//! method locks it, mutates the state, and releases the lock before returning.
//! No method holds the lock across an `.await` point.
//!
//! Authentication is delegated to [`AuthService`], which owns the user
//! registry. The win counter is delegated to [`RankingService`].
//! `LobbyService` orchestrates the interaction between the two: it validates
//! input, awaits hashing or verification, updates the session, and records
//! wins when a match ends with a real victory.
//!
//! Only authenticated sessions may create or join matches. Guests can list
//! matches, view the ranking, and ping.
//!
//! A given username may be signed in on at most one connection at a time.
//! The `LobbyState::active_sessions` map tracks which `Username` is bound to
//! which `ClientId`. The invariant is maintained on both ends: registration
//! and login insert, `disconnect` removes. Since `SessionGuard` guarantees
//! that `disconnect` runs synchronously when the connection ends, there is
//! no window during which a dead session blocks a legitimate login.
//!
//! The implementation is split across four files:
//!
//! - `auth_ops` contains the session lifecycle for authentication
//!   (`hello`, `register_user`, `login_user`, `list_ranking`, `pong`) and
//!   the reconnection path that reclaims a pending disconnection.
//! - `match_ops` contains the match lifecycle (`list_matches`,
//!   `create_match`, `join_match`, `make_move`, `leave_match`, `spectate`,
//!   `leave_spectate`), the grace-period expiration, and the free helpers
//!   that operate on matches.
//! - `tests` contains the unit tests.
//! - this file contains the type definitions, the constructors, the client
//!   lifecycle (`register_client`, `disconnect`), and the shared helpers.

mod auth_ops;
mod match_ops;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use common::domain::Username;
use common::protocol::{AuthFailureReason, ClientId, MatchId, ServerMessage};
use tokio::sync::mpsc;

use crate::application::auth::AuthService;
use crate::application::ranking::RankingService;
use crate::domain::{Match, Session};

/// Maximum length of a display name, in bytes.
const MAX_DISPLAY_NAME_LEN: usize = 32;

/// Minimum length of a password, in characters.
const MIN_PASSWORD_LEN: usize = 8;

/// Maximum length of a password, in characters.
const MAX_PASSWORD_LEN: usize = 256;

/// Internal state guarded by the lobby mutex.
#[derive(Default)]
struct LobbyState {
    next_client_id: u64,
    next_match_id: u64,
    sessions: HashMap<ClientId, Session>,
    matches: HashMap<MatchId, Match>,
    /// Maps an authenticated username to the single client that is currently
    /// signed in with it. Enforces the "one session per account" invariant.
    active_sessions: HashMap<Username, ClientId>,
    /// Players that have disconnected from a live match and are waiting for
    /// the grace period to expire. Their username remains reserved.
    pending_disconnections: HashMap<Username, PendingDisconnection>,
}

/// A player that has been temporarily disconnected from a live match.
///
/// While a player is pending reconnection, the match stays alive, their
/// username is still reserved, and any attempt by a third party to use
/// that username is rejected. Only the original player (or someone who
/// knows their credentials) can reclaim the slot within the grace period.
#[derive(Debug, Clone)]
pub(super) struct PendingDisconnection {
    /// The match the player was in when they disconnected.
    pub match_id: MatchId,
    /// The client identifier the player had before disconnecting.
    pub client_id: ClientId,
    /// Whether the player was the host of the match.
    pub was_host: bool,
}

/// Information about a player that has just been scheduled for reconnection.
///
/// Returned by [`LobbyService::disconnect`] so the caller can start the
/// grace-period timer without holding the lobby lock.
pub struct Disconnection {
    /// The username of the disconnected player.
    pub username: Username,
}

/// Coordinates sessions and matches for all connected clients.
pub struct LobbyService {
    state: Mutex<LobbyState>,
    auth: Arc<AuthService>,
    ranking: Arc<RankingService>,
}

impl Default for LobbyService {
    fn default() -> Self {
        Self::new()
    }
}

impl LobbyService {
    /// Creates an empty lobby with default-configured services.
    pub fn new() -> Self {
        Self::with_services(
            Arc::new(AuthService::default()),
            Arc::new(RankingService::new()),
        )
    }

    /// Creates an empty lobby with a caller-provided auth service and a
    /// fresh ranking service.
    ///
    /// Intended for tests that want cheaper Argon2 parameters without
    /// caring about the ranking.
    pub fn with_auth(auth: Arc<AuthService>) -> Self {
        Self::with_services(auth, Arc::new(RankingService::new()))
    }

    /// Creates an empty lobby with caller-provided services.
    pub fn with_services(auth: Arc<AuthService>, ranking: Arc<RankingService>) -> Self {
        Self {
            state: Mutex::new(LobbyState::default()),
            auth,
            ranking,
        }
    }

    /// Returns the ranking service owned by this lobby.
    pub fn ranking(&self) -> &RankingService {
        &self.ranking
    }

    /// Registers a new client and returns its identifier.
    pub fn register_client(&self, sender: mpsc::UnboundedSender<ServerMessage>) -> ClientId {
        let mut state = self.lock();
        let id = ClientId::new(state.next_client_id);
        state.next_client_id += 1;
        state.sessions.insert(id, Session::new(id, sender));
        id
    }

    /// Removes the client and any spectate relationship it was part of.
    ///
    /// If the client was a player in a live match, the match stays alive
    /// and a pending disconnection is registered: the username remains
    /// reserved, the opponent and the spectators are notified, and the
    /// caller is expected to start the grace-period timer with the returned
    /// [`Disconnection`]. If the client was not in a match, the username is
    /// released immediately.
    pub fn disconnect(&self, client: ClientId) -> Option<Disconnection> {
        let mut state = self.lock();
        let session = state.sessions.remove(&client)?;
        tracing::debug!(client_id = %session.client_id, "session removed");

        let disconnection = match (&session.authenticated_as, session.current_match) {
            (Some(username), Some(match_id)) => {
                let was_host = state
                    .matches
                    .get(&match_id)
                    .is_some_and(|m| m.host == client);

                let notice = ServerMessage::OpponentDisconnected {
                    match_id,
                    grace_seconds: common::protocol::GRACE_PERIOD_SECS,
                };
                if let Some(m) = state.matches.get(&match_id) {
                    if let Some(opponent_id) = m.opponent_of(client)
                        && let Some(session) = state.sessions.get(&opponent_id)
                    {
                        session.try_send(notice.clone());
                    }
                    for spectator in &m.spectators {
                        if let Some(session) = state.sessions.get(spectator) {
                            session.try_send(notice.clone());
                        }
                    }
                }

                state.pending_disconnections.insert(
                    username.clone(),
                    PendingDisconnection {
                        match_id,
                        client_id: client,
                        was_host,
                    },
                );

                Some(Disconnection {
                    username: username.clone(),
                })
            }
            (Some(username), None) => {
                // Authenticated but not playing. Release the username.
                if state.active_sessions.get(username) == Some(&client) {
                    state.active_sessions.remove(username);
                }
                None
            }
            (None, _) => None,
        };

        if let Some(match_id) = session.spectating {
            match_ops::detach_spectator_from_match(&mut state, match_id, client);
        }

        disconnection
    }

    /// Returns the number of currently registered sessions.
    pub fn session_count(&self) -> usize {
        self.lock().sessions.len()
    }

    /// Returns the number of currently open matches.
    pub fn match_count(&self) -> usize {
        self.lock().matches.len()
    }

    fn is_authenticated(&self, client_id: ClientId) -> bool {
        let state = self.lock();
        state
            .sessions
            .get(&client_id)
            .is_some_and(Session::is_authenticated)
    }

    #[cfg(test)]
    fn is_spectating(&self, client_id: ClientId) -> bool {
        let state = self.lock();
        state
            .sessions
            .get(&client_id)
            .is_some_and(Session::is_spectating)
    }

    fn is_username_active(&self, username: &Username) -> bool {
        self.lock().active_sessions.contains_key(username)
    }

    fn send_auth_failure(&self, client_id: ClientId, reason: AuthFailureReason, message: &str) {
        let state = self.lock();
        if let Some(session) = state.sessions.get(&client_id) {
            session.try_send(ServerMessage::AuthenticationFailed {
                reason,
                message: message.to_string(),
            });
        }
    }

    fn lock(&self) -> MutexGuard<'_, LobbyState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Returns the default human-readable message for an auth failure reason.
pub(super) const fn reason_message(reason: AuthFailureReason) -> &'static str {
    match reason {
        AuthFailureReason::NameEmpty => "name must not be empty",
        AuthFailureReason::UsernameTaken => "username is already taken",
        AuthFailureReason::UsernameInvalid => "invalid username",
        AuthFailureReason::AgeOutOfRange => "age is out of range",
        AuthFailureReason::PasswordTooShort => "password is too short",
        AuthFailureReason::InvalidCredentials => "invalid username or password",
        AuthFailureReason::AlreadyAuthenticated => "this connection is already authenticated",
        AuthFailureReason::AlreadyLoggedIn => "this account is already signed in elsewhere",
        AuthFailureReason::InternalError => "internal server error",
    }
}
