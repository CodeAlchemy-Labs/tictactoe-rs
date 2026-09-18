//! In-memory lobby and match coordination.
//!
//! [`LobbyService`] is the single entry point for every state change that
//! involves connected clients. It owns a `Mutex<LobbyState>` and every public
//! method locks it, mutates the state, and releases the lock before returning.
//! No method holds the lock across an `.await` point.
//!
//! Authentication is delegated to [`AuthService`], which owns the user
//! registry. `LobbyService` orchestrates the interaction: it validates the
//! raw input, awaits the hashing or verification, and updates the session
//! once the auth service confirms the operation.
//!
//! Only authenticated sessions may create or join matches. Guests can list
//! matches, ping, and (once implemented) spectate.
//!
//! A given username may be signed in on at most one connection at a time.
//! The `LobbyState::active_sessions` map tracks which `Username` is bound to
//! which `ClientId`. The invariant is maintained on both ends: registration
//! and login insert, `disconnect` removes. Since `SessionGuard` guarantees
//! that `disconnect` runs synchronously when the connection ends, there is
//! no window during which a dead session blocks a legitimate login.
//!
//! The implementation is split across three files:
//!
//! - `auth_ops` contains the session lifecycle for authentication
//!   (`hello`, `register_user`, `login_user`, `pong`).
//! - `match_ops` contains the match lifecycle (`list_matches`,
//!   `create_match`, `join_match`, `make_move`, `leave_match`) and the two
//!   free helpers that operate on matches.
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
}

/// Coordinates sessions and matches for all connected clients.
pub struct LobbyService {
    state: Mutex<LobbyState>,
    auth: Arc<AuthService>,
}

impl Default for LobbyService {
    fn default() -> Self {
        Self::new()
    }
}

impl LobbyService {
    /// Creates an empty lobby with a default-configured auth service.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(LobbyState::default()),
            auth: Arc::new(AuthService::default()),
        }
    }

    /// Creates an empty lobby with a caller-provided auth service.
    ///
    /// Intended for tests that want to use cheaper Argon2 parameters.
    pub fn with_auth(auth: Arc<AuthService>) -> Self {
        Self {
            state: Mutex::new(LobbyState::default()),
            auth,
        }
    }

    /// Registers a new client and returns its identifier.
    pub fn register_client(&self, sender: mpsc::UnboundedSender<ServerMessage>) -> ClientId {
        let mut state = self.lock();
        let id = ClientId::new(state.next_client_id);
        state.next_client_id += 1;
        state.sessions.insert(id, Session::new(id, sender));
        id
    }

    /// Removes the client and any match it was part of.
    ///
    /// Also releases the username from `active_sessions` so that the account
    /// can be used on a new connection.
    pub fn disconnect(&self, client: ClientId) {
        let mut state = self.lock();
        if let Some(session) = state.sessions.remove(&client) {
            tracing::debug!(client_id = %session.client_id, "session removed");
            if let Some(username) = &session.authenticated_as {
                // Only remove the entry if it still points to this client.
                // A racing re-login could have overwritten it.
                if state.active_sessions.get(username) == Some(&client) {
                    state.active_sessions.remove(username);
                }
            }
            if let Some(match_id) = session.current_match {
                match_ops::detach_from_match(&mut state, match_id, client);
            }
        }
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