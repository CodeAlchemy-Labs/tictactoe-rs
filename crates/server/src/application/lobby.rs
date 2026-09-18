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

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use common::domain::{Age, Board, GameStatus, Player, Position, UserProfile, Username};
use common::protocol::{
    AuthFailureReason, ClientId, ErrorCode, MatchId, MatchSummary, ServerMessage,
};
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

/// Tuple returned by [`apply_move`] on success.
type MoveOutcome = (ClientId, Option<ClientId>, Board, Player, GameStatus);

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

    /// Handles `Hello`.
    pub fn hello(&self, client: ClientId, display_name: &str) {
        let mut state = self.lock();
        let Some(session) = state.sessions.get_mut(&client) else {
            return;
        };
        if session.display_name.is_some() {
            session.try_send(ServerMessage::Error {
                code: ErrorCode::InvalidState,
                message: String::from("client is already registered"),
            });
            return;
        }
        let trimmed = display_name.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_DISPLAY_NAME_LEN {
            session.try_send(ServerMessage::Error {
                code: ErrorCode::InvalidDisplayName,
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
    pub async fn login_user(&self, client_id: ClientId, username: String, password: String) {
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

        // Check whether the account is already signed in on another
        // connection. This is done before authenticating so that a
        // legitimate account cannot be blocked by an attacker who does not
        // know the password.
        if self.is_username_active(&username) {
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
                state
                    .active_sessions
                    .insert(profile.username.clone(), client_id);
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

    /// Handles `ListMatches`.
    pub fn list_matches(&self, client: ClientId) {
        let state = self.lock();
        let Some(session) = state.sessions.get(&client) else {
            return;
        };
        let matches = state
            .matches
            .values()
            .filter(|m| !m.is_full())
            .map(|m| MatchSummary {
                id: m.id,
                host: state
                    .sessions
                    .get(&m.host)
                    .and_then(|s| s.display_name.clone())
                    .unwrap_or_else(|| String::from("unknown")),
            })
            .collect();
        session.try_send(ServerMessage::MatchList { matches });
    }

    /// Handles `CreateMatch`.
    pub fn create_match(&self, client: ClientId) {
        let mut state = self.lock();
        let Some(session) = state.sessions.get(&client) else {
            return;
        };
        if session.display_name.is_none() {
            session.try_send(ServerMessage::Error {
                code: ErrorCode::InvalidState,
                message: String::from("send hello before creating a match"),
            });
            return;
        }
        if !session.is_authenticated() {
            session.try_send(ServerMessage::Error {
                code: ErrorCode::AuthenticationRequired,
                message: String::from("register or log in before creating a match"),
            });
            return;
        }
        if session.current_match.is_some() {
            session.try_send(ServerMessage::Error {
                code: ErrorCode::InvalidState,
                message: String::from("already in a match"),
            });
            return;
        }
        let match_id = MatchId::new(state.next_match_id);
        state.next_match_id += 1;
        state.matches.insert(match_id, Match::new(match_id, client));
        if let Some(session) = state.sessions.get_mut(&client) {
            session.current_match = Some(match_id);
            session.mark = Some(Player::X);
            session.try_send(ServerMessage::MatchCreated { match_id });
        }
    }

    /// Handles `JoinMatch`.
    pub fn join_match(&self, client: ClientId, match_id: MatchId) {
        let mut state = self.lock();

        let validation: Option<(ErrorCode, &'static str)> = {
            let Some(session) = state.sessions.get(&client) else {
                return;
            };
            if session.display_name.is_none() {
                Some((ErrorCode::InvalidState, "send hello before joining a match"))
            } else if !session.is_authenticated() {
                Some((
                    ErrorCode::AuthenticationRequired,
                    "register or log in before joining a match",
                ))
            } else if session.current_match.is_some() {
                Some((ErrorCode::InvalidState, "already in a match"))
            } else {
                match state.matches.get(&match_id) {
                    None => Some((ErrorCode::MatchNotFound, "match not found")),
                    Some(m) if m.is_full() => Some((ErrorCode::MatchFull, "match is already full")),
                    Some(_) => None,
                }
            }
        };
        if let Some((code, message)) = validation {
            if let Some(session) = state.sessions.get(&client) {
                session.try_send(ServerMessage::Error {
                    code,
                    message: String::from(message),
                });
            }
            return;
        }

        let (host, board, first_turn) = {
            let Some(m) = state.matches.get(&match_id) else {
                return;
            };
            (m.host, m.board, m.current_turn)
        };
        let host_display = state
            .sessions
            .get(&host)
            .and_then(|s| s.display_name.clone())
            .unwrap_or_else(|| String::from("unknown"));
        let guest_display = state
            .sessions
            .get(&client)
            .and_then(|s| s.display_name.clone())
            .unwrap_or_else(|| String::from("unknown"));

        if let Some(m) = state.matches.get_mut(&match_id) {
            m.guest = Some(client);
        }
        if let Some(session) = state.sessions.get_mut(&client) {
            session.current_match = Some(match_id);
            session.mark = Some(Player::O);
        }

        if let Some(host_session) = state.sessions.get(&host) {
            host_session.try_send(ServerMessage::MatchReady {
                match_id,
                opponent: guest_display,
                your_mark: Player::X,
                board,
                current_turn: first_turn,
            });
        }
        if let Some(guest_session) = state.sessions.get(&client) {
            guest_session.try_send(ServerMessage::MatchReady {
                match_id,
                opponent: host_display,
                your_mark: Player::O,
                board,
                current_turn: first_turn,
            });
        }
    }

    /// Handles `MakeMove`.
    pub fn make_move(&self, client: ClientId, position: Position) {
        let mut state = self.lock();

        let match_id = {
            let Some(session) = state.sessions.get(&client) else {
                return;
            };
            if !session.is_authenticated() {
                session.try_send(ServerMessage::Error {
                    code: ErrorCode::AuthenticationRequired,
                    message: String::from("register or log in before making a move"),
                });
                return;
            }
            let Some(id) = session.current_match else {
                session.try_send(ServerMessage::Error {
                    code: ErrorCode::NotInMatch,
                    message: String::from("not in a match"),
                });
                return;
            };
            id
        };

        let outcome = {
            let Some(m) = state.matches.get_mut(&match_id) else {
                if let Some(session) = state.sessions.get(&client) {
                    session.try_send(ServerMessage::Error {
                        code: ErrorCode::MatchNotFound,
                        message: String::from("match not found"),
                    });
                }
                return;
            };
            apply_move(m, client, position)
        };

        match outcome {
            Err((code, message)) => {
                if let Some(session) = state.sessions.get(&client) {
                    session.try_send(ServerMessage::Error {
                        code,
                        message: String::from(message),
                    });
                }
            }
            Ok((host, guest, board, next_turn, status)) => {
                let update = ServerMessage::BoardUpdate {
                    board,
                    current_turn: next_turn,
                    status,
                };
                if let Some(host) = state.sessions.get(&host) {
                    host.try_send(update.clone());
                }
                if let Some(guest_id) = guest
                    && let Some(guest) = state.sessions.get(&guest_id)
                {
                    guest.try_send(update);
                }
                if status.is_finished() {
                    let over = ServerMessage::MatchOver { board, status };
                    if let Some(host) = state.sessions.get(&host) {
                        host.try_send(over.clone());
                    }
                    if let Some(guest_id) = guest
                        && let Some(guest) = state.sessions.get(&guest_id)
                    {
                        guest.try_send(over);
                    }
                }
            }
        }
    }

    /// Handles `LeaveMatch`.
    pub fn leave_match(&self, client: ClientId) {
        let mut state = self.lock();
        let Some(match_id) = state.sessions.get(&client).and_then(|s| s.current_match) else {
            if let Some(session) = state.sessions.get(&client) {
                session.try_send(ServerMessage::Error {
                    code: ErrorCode::NotInMatch,
                    message: String::from("not in a match"),
                });
            }
            return;
        };
        if let Some(session) = state.sessions.get_mut(&client) {
            session.current_match = None;
            session.mark = None;
        }
        detach_from_match(&mut state, match_id, client);
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
                detach_from_match(&mut state, match_id, client);
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
const fn reason_message(reason: AuthFailureReason) -> &'static str {
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

/// Removes `client` from `match_id`, notifies the opponent, and drops the
/// match when it is empty.
fn detach_from_match(state: &mut LobbyState, match_id: MatchId, client: ClientId) {
    let Some(m) = state.matches.get_mut(&match_id) else {
        return;
    };
    let opponent = m.opponent_of(client);
    let is_host = m.host == client;
    let drop_match = is_host || m.guest.is_none() || opponent.is_none();
    if let Some(opponent_id) = opponent {
        if let Some(opponent_session) = state.sessions.get(&opponent_id) {
            opponent_session.try_send(ServerMessage::OpponentLeft { match_id });
        }
        if let Some(opponent_session) = state.sessions.get_mut(&opponent_id) {
            opponent_session.current_match = None;
            opponent_session.mark = None;
        }
    }
    if drop_match {
        state.matches.remove(&match_id);
    } else if let Some(m) = state.matches.get_mut(&match_id) {
        m.guest = None;
    }
}

/// Applies `position` as a move by `client` to `m`.
fn apply_move(
    m: &mut Match,
    client: ClientId,
    position: Position,
) -> Result<MoveOutcome, (ErrorCode, &'static str)> {
    if m.status.is_finished() {
        return Err((ErrorCode::InvalidState, "match is already over"));
    }
    let Some(mark) = m.mark_of(client) else {
        return Err((ErrorCode::InvalidState, "not a player in this match"));
    };
    if !m.is_full() {
        return Err((ErrorCode::InvalidState, "waiting for an opponent"));
    }
    if m.current_turn != mark {
        return Err((ErrorCode::InvalidState, "not your turn"));
    }
    if m.board.place(position, mark).is_err() {
        return Err((ErrorCode::IllegalMove, "cell is already occupied"));
    }
    let status = m.board.status();
    m.status = status;
    let next_turn = m.current_turn.other();
    m.current_turn = next_turn;
    Ok((m.host, m.guest, m.board, next_turn, status))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use argon2::Params;

    use super::*;

    fn fast_lobby() -> LobbyService {
        let params = Params::new(8, 1, 1, None).expect("test parameters are within range");
        LobbyService::with_auth(Arc::new(AuthService::with_params(params)))
    }

    fn lobby_with_client() -> (
        LobbyService,
        ClientId,
        mpsc::UnboundedReceiver<ServerMessage>,
    ) {
        let lobby = fast_lobby();
        let (tx, rx) = mpsc::unbounded_channel();
        let id = lobby.register_client(tx);
        (lobby, id, rx)
    }

    async fn authenticate(lobby: &LobbyService, client: ClientId, username: &str) {
        lobby
            .register_user(
                client,
                format!("{username} name"),
                username.to_string(),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
    }

    #[test]
    fn register_increments_session_count() {
        let (lobby, _, _) = lobby_with_client();
        assert_eq!(lobby.session_count(), 1);
    }

    #[test]
    fn hello_sends_welcome() {
        let (lobby, id, mut rx) = lobby_with_client();
        lobby.hello(id, "alice");
        match rx.try_recv().unwrap() {
            ServerMessage::Welcome { display_name, .. } => assert_eq!(display_name, "alice"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn empty_display_name_is_rejected() {
        let (lobby, id, mut rx) = lobby_with_client();
        lobby.hello(id, "   ");
        match rx.try_recv().unwrap() {
            ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::InvalidDisplayName),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn create_match_requires_hello() {
        let (lobby, id, mut rx) = lobby_with_client();
        lobby.create_match(id);
        match rx.try_recv().unwrap() {
            ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::InvalidState),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn create_match_requires_authentication() {
        let (lobby, id, mut rx) = lobby_with_client();
        lobby.hello(id, "guest");
        let _ = rx.try_recv();
        lobby.create_match(id);
        match rx.try_recv().unwrap() {
            ServerMessage::Error { code, .. } => {
                assert_eq!(code, ErrorCode::AuthenticationRequired);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn authenticated_client_can_create_a_match() {
        let (lobby, id, mut rx) = lobby_with_client();
        authenticate(&lobby, id, "alice_99").await;
        let _ = rx.try_recv(); // Registered
        lobby.create_match(id);
        match rx.try_recv().unwrap() {
            ServerMessage::MatchCreated { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn joining_a_match_notifies_both_players() {
        let lobby = fast_lobby();
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register_client(host_tx);
        let guest = lobby.register_client(guest_tx);
        authenticate(&lobby, host, "host_99").await;
        let _ = host_rx.try_recv();
        authenticate(&lobby, guest, "guest_99").await;
        let _ = guest_rx.try_recv();

        lobby.create_match(host);
        let match_id = match host_rx.try_recv().unwrap() {
            ServerMessage::MatchCreated { match_id } => match_id,
            other => panic!("unexpected: {other:?}"),
        };
        lobby.join_match(guest, match_id);

        match host_rx.try_recv().unwrap() {
            ServerMessage::MatchReady {
                your_mark,
                opponent,
                ..
            } => {
                assert_eq!(your_mark, Player::X);
                assert_eq!(opponent, "guest_99 name");
            }
            other => panic!("unexpected: {other:?}"),
        }
        match guest_rx.try_recv().unwrap() {
            ServerMessage::MatchReady {
                your_mark,
                opponent,
                ..
            } => {
                assert_eq!(your_mark, Player::O);
                assert_eq!(opponent, "host_99 name");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn playing_a_full_game_ends_in_a_win() {
        let lobby = fast_lobby();
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register_client(host_tx);
        let guest = lobby.register_client(guest_tx);
        authenticate(&lobby, host, "host_99").await;
        let _ = host_rx.try_recv();
        authenticate(&lobby, guest, "guest_99").await;
        let _ = guest_rx.try_recv();
        lobby.create_match(host);
        let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
            unreachable!("first message must be MatchCreated")
        };
        lobby.join_match(guest, match_id);
        let _ = host_rx.try_recv();
        let _ = guest_rx.try_recv();

        for (client, pos) in [(host, 0u8), (guest, 3), (host, 1), (guest, 4), (host, 2)] {
            lobby.make_move(client, Position::new(pos).unwrap());
        }

        let mut saw_match_over = false;
        while let Ok(message) = host_rx.try_recv() {
            if let ServerMessage::MatchOver { status, .. } = message {
                assert_eq!(status, GameStatus::Won(Player::X));
                saw_match_over = true;
            }
        }
        assert!(saw_match_over);
    }

    #[tokio::test]
    async fn disconnect_removes_the_session_and_the_open_match() {
        let lobby = fast_lobby();
        let (tx, _rx) = mpsc::unbounded_channel();
        let host = lobby.register_client(tx);
        authenticate(&lobby, host, "host_99").await;
        lobby.create_match(host);
        assert_eq!(lobby.session_count(), 1);
        assert_eq!(lobby.match_count(), 1);

        lobby.disconnect(host);
        assert_eq!(lobby.session_count(), 0);
        assert_eq!(lobby.match_count(), 0);
    }

    #[tokio::test]
    async fn disconnect_notifies_the_opponent() {
        let lobby = fast_lobby();
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register_client(host_tx);
        let guest = lobby.register_client(guest_tx);
        authenticate(&lobby, host, "host_99").await;
        let _ = host_rx.try_recv();
        authenticate(&lobby, guest, "guest_99").await;
        let _ = guest_rx.try_recv();
        lobby.create_match(host);
        let ServerMessage::MatchCreated { match_id } = host_rx.try_recv().unwrap() else {
            unreachable!("first message must be MatchCreated")
        };
        lobby.join_match(guest, match_id);
        let _ = host_rx.try_recv();
        let _ = guest_rx.try_recv();

        lobby.disconnect(host);
        match guest_rx.try_recv().unwrap() {
            ServerMessage::OpponentLeft { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
        assert_eq!(lobby.session_count(), 1);
        assert_eq!(lobby.match_count(), 0);
    }

    #[tokio::test]
    async fn register_marks_the_session_as_authenticated() {
        let lobby = fast_lobby();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = lobby.register_client(tx);
        lobby
            .register_user(
                client,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        match rx.try_recv().unwrap() {
            ServerMessage::Registered { profile } => {
                assert_eq!(profile.username.as_str(), "alice_99");
            }
            other => panic!("unexpected: {other:?}"),
        }
        assert!(lobby.is_authenticated(client));
    }

    #[tokio::test]
    async fn register_rejects_short_password() {
        let lobby = fast_lobby();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = lobby.register_client(tx);
        lobby
            .register_user(
                client,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("short"),
            )
            .await;
        match rx.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::PasswordTooShort);
            }
            other => panic!("unexpected: {other:?}"),
        }
        assert!(!lobby.is_authenticated(client));
    }

    #[tokio::test]
    async fn register_rejects_duplicate_username() {
        let lobby = fast_lobby();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        let client1 = lobby.register_client(tx1);
        let client2 = lobby.register_client(tx2);
        lobby
            .register_user(
                client1,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = rx1.try_recv();
        lobby
            .register_user(
                client2,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("otherpassword"),
            )
            .await;
        match rx2.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::UsernameTaken);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn login_succeeds_after_registration() {
        let lobby = fast_lobby();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        let client1 = lobby.register_client(tx1);
        let client2 = lobby.register_client(tx2);
        lobby
            .register_user(
                client1,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = rx1.try_recv();

        // Close the first connection so the account is no longer active.
        lobby.disconnect(client1);

        lobby
            .login_user(
                client2,
                String::from("alice_99"),
                String::from("hunter2hunter2"),
            )
            .await;
        match rx2.try_recv().unwrap() {
            ServerMessage::LoginSucceeded { profile } => {
                assert_eq!(profile.username.as_str(), "alice_99");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn login_rejects_wrong_password() {
        let lobby = fast_lobby();
        let (tx1, _rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        let client1 = lobby.register_client(tx1);
        let client2 = lobby.register_client(tx2);
        lobby
            .register_user(
                client1,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        lobby
            .login_user(
                client2,
                String::from("alice_99"),
                String::from("wrongpassword"),
            )
            .await;
        match rx2.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::InvalidCredentials);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn double_registration_is_rejected() {
        let lobby = fast_lobby();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = lobby.register_client(tx);
        lobby
            .register_user(
                client,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = rx.try_recv();
        lobby
            .register_user(
                client,
                String::from("Bob"),
                String::from("bob_77"),
                25,
                String::from("hunter2hunter2"),
            )
            .await;
        match rx.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::AlreadyAuthenticated);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_rejects_invalid_username() {
        let lobby = fast_lobby();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = lobby.register_client(tx);
        lobby
            .register_user(
                client,
                String::from("Alice"),
                String::from("a!"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        match rx.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::UsernameInvalid);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_rejects_invalid_age() {
        let lobby = fast_lobby();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = lobby.register_client(tx);
        lobby
            .register_user(
                client,
                String::from("Alice"),
                String::from("alice_99"),
                91,
                String::from("hunter2hunter2"),
            )
            .await;
        match rx.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::AgeOutOfRange);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn login_rejects_when_account_is_already_active() {
        let lobby = fast_lobby();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        let client1 = lobby.register_client(tx1);
        let client2 = lobby.register_client(tx2);
        lobby
            .register_user(
                client1,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = rx1.try_recv();
        // The first session is still active; the second login must fail.

        lobby
            .login_user(
                client2,
                String::from("alice_99"),
                String::from("hunter2hunter2"),
            )
            .await;
        match rx2.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::AlreadyLoggedIn);
            }
            other => panic!("unexpected: {other:?}"),
        }
        assert!(!lobby.is_authenticated(client2));
    }

    #[tokio::test]
    async fn login_succeeds_after_the_first_session_disconnects() {
        let lobby = fast_lobby();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        let client1 = lobby.register_client(tx1);
        let client2 = lobby.register_client(tx2);
        lobby
            .register_user(
                client1,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = rx1.try_recv();

        // Close the first connection; the active entry must be released.
        lobby.disconnect(client1);

        lobby
            .login_user(
                client2,
                String::from("alice_99"),
                String::from("hunter2hunter2"),
            )
            .await;
        match rx2.try_recv().unwrap() {
            ServerMessage::LoginSucceeded { profile } => {
                assert_eq!(profile.username.as_str(), "alice_99");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_rejects_username_taken_by_an_active_session() {
        // The username already exists, so the register path returns
        // UsernameTaken even though the account is currently signed in.
        // AlreadyLoggedIn is reserved for the login path.
        let lobby = fast_lobby();
        let (tx1, mut rx1) = mpsc::unbounded_channel();
        let (tx2, mut rx2) = mpsc::unbounded_channel();
        let client1 = lobby.register_client(tx1);
        let client2 = lobby.register_client(tx2);
        lobby
            .register_user(
                client1,
                String::from("Alice"),
                String::from("alice_99"),
                30,
                String::from("hunter2hunter2"),
            )
            .await;
        let _ = rx1.try_recv();

        lobby
            .register_user(
                client2,
                String::from("Alice Impostor"),
                String::from("alice_99"),
                25,
                String::from("hunter2hunter2"),
            )
            .await;
        match rx2.try_recv().unwrap() {
            ServerMessage::AuthenticationFailed { reason, .. } => {
                assert_eq!(reason, AuthFailureReason::UsernameTaken);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
