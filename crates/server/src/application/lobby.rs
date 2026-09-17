//! In-memory lobby and match coordination.
//!
//! [`LobbyService`] is the single entry point for every state change that
//! involves connected clients. It owns a `Mutex<LobbyState>` and every public
//! method locks it, mutates the state, and releases the lock before returning.
//! No method holds the lock across an `.await` point.
//!
//! The service is the aggregate root for both [`Session`] and [`Match`].
//! Splitting it into two services would require sharing the same mutex and
//! would create artificial coupling; keeping it together makes the single
//! atomicity boundary explicit.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use common::domain::{GameStatus, Player, Position};
use common::protocol::{ClientId, ErrorCode, MatchId, MatchSummary, ServerMessage};
use tokio::sync::mpsc;

use crate::domain::{Match, Session};

/// Maximum length of a display name, in bytes.
const MAX_DISPLAY_NAME_LEN: usize = 32;

/// Internal state guarded by the lobby mutex.
#[derive(Default)]
struct LobbyState {
    next_client_id: u64,
    next_match_id: u64,
    sessions: HashMap<ClientId, Session>,
    matches: HashMap<MatchId, Match>,
}

/// Coordinates sessions and matches for all connected clients.
pub struct LobbyService {
    state: Mutex<LobbyState>,
}

impl Default for LobbyService {
    fn default() -> Self {
        Self::new()
    }
}

impl LobbyService {
    /// Creates an empty lobby.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(LobbyState::default()),
        }
    }

    /// Registers a new client and returns its identifier.
    ///
    /// The caller keeps ownership of the lifecycle by way of a
    /// [`SessionGuard`](crate::infrastructure::session_guard::SessionGuard);
    /// when the guard drops it calls [`LobbyService::disconnect`].
    pub fn register(&self, sender: mpsc::UnboundedSender<ServerMessage>) -> ClientId {
        let mut state = self.lock();
        let id = ClientId::new(state.next_client_id);
        state.next_client_id += 1;
        state.sessions.insert(id, Session::new(id, sender));
        id
    }

    /// Handles `Hello`.
    pub fn hello(&self, client: ClientId, display_name: String) {
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

        let join_error = {
            let Some(session) = state.sessions.get(&client) else {
                return;
            };
            if session.display_name.is_none() {
                Some((ErrorCode::InvalidState, "send hello before joining a match"))
            } else if session.current_match.is_some() {
                Some((ErrorCode::InvalidState, "already in a match"))
            } else if !state.matches.contains_key(&match_id) {
                Some((ErrorCode::MatchNotFound, "match not found"))
            } else if state.matches.get(&match_id).is_some_and(Match::is_full) {
                Some((ErrorCode::MatchFull, "match is already full"))
            } else {
                None
            }
        };

        if let Some((code, message)) = join_error {
            if let Some(session) = state.sessions.get(&client) {
                session.try_send(ServerMessage::Error {
                    code,
                    message: String::from(message),
                });
            }
            return;
        }

        // From here the match exists, is not full, and the client can join.
        let (host, host_display, guest_display) = {
            let m = state
                .matches
                .get_mut(&match_id)
                .expect("checked above");
            m.guest = Some(client);
            let host = m.host;
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
            (host, host_display, guest_display)
        };

        if let Some(session) = state.sessions.get_mut(&client) {
            session.current_match = Some(match_id);
            session.mark = Some(Player::O);
        }

        // Notify both players.
        let m = state.matches.get(&match_id).expect("checked above");
        let board = m.board;
        let first_turn = m.current_turn;
        let host_message = ServerMessage::MatchReady {
            match_id,
            opponent: guest_display,
            your_mark: Player::X,
            board,
            current_turn: first_turn,
        };
        let guest_message = ServerMessage::MatchReady {
            match_id,
            opponent: host_display,
            your_mark: Player::O,
            board,
            current_turn: first_turn,
        };
        if let Some(host) = state.sessions.get(&host) {
            host.try_send(host_message);
        }
        if let Some(guest) = state.sessions.get(&client) {
            guest.try_send(guest_message);
        }
    }

    /// Handles `MakeMove`.
    pub fn make_move(&self, client: ClientId, position: Position) {
        let mut state = self.lock();

        let match_id = match state.sessions.get(&client).and_then(|s| s.current_match) {
            Some(id) => id,
            None => {
                if let Some(session) = state.sessions.get(&client) {
                    session.try_send(ServerMessage::Error {
                        code: ErrorCode::NotInMatch,
                        message: String::from("not in a match"),
                    });
                }
                return;
            }
        };

        // Apply the move. We deliberately scope the mutable borrow of
        // `state.matches` so the compiler can prove disjointness with the
        // subsequent access to `state.sessions`.
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
                if let Some(guest_id) = guest {
                    if let Some(guest) = state.sessions.get(&guest_id) {
                        guest.try_send(update);
                    }
                }
                if status.is_finished() {
                    let over = ServerMessage::MatchOver { board, status };
                    if let Some(host) = state.sessions.get(&host) {
                        host.try_send(over.clone());
                    }
                    if let Some(guest_id) = guest {
                        if let Some(guest) = state.sessions.get(&guest_id) {
                            guest.try_send(over);
                        }
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
    /// This is the method invoked by
    /// [`SessionGuard`](crate::infrastructure::session_guard::SessionGuard)'s
    /// `Drop`, which is what makes cleanup deterministic: the guard runs
    /// synchronously when the connection handler returns, and there is no
    /// window during which a stale session is observable by other clients.
    pub fn disconnect(&self, client: ClientId) {
        let mut state = self.lock();
        if let Some(session) = state.sessions.remove(&client) {
            tracing::debug!(client_id = %session.client_id, "session removed");
            if let Some(match_id) = session.current_match {
                detach_from_match(&mut state, match_id, client);
            }
        }
    }

    /// Returns the number of currently registered sessions.
    ///
    /// Intended for observability and tests.
    pub fn session_count(&self) -> usize {
        self.lock().sessions.len()
    }

    /// Returns the number of currently open matches.
    ///
    /// Intended for observability and tests.
    pub fn match_count(&self) -> usize {
        self.lock().matches.len()
    }

    fn lock(&self) -> MutexGuard<'_, LobbyState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Handles `Ping`.
    pub fn pong(&self, client: ClientId) {
        let state = self.lock();
        if let Some(session) = state.sessions.get(&client) {
            session.try_send(ServerMessage::Pong);
        }
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
    // The host leaving ends the match; the guest leaving leaves the match
    // open for another opponent.
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
///
/// On success returns the (host, guest, board, next_turn, status) tuple the
/// caller needs to broadcast. On failure returns the `(ErrorCode, message)`
/// pair the caller should send to the offending client.
#[allow(clippy::type_complexity)]
fn apply_move(
    m: &mut Match,
    client: ClientId,
    position: Position,
) -> Result<(ClientId, Option<ClientId>, common::domain::Board, Player, GameStatus), (ErrorCode, &'static str)> {
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
    use super::*;

    fn lobby_with_client() -> (LobbyService, ClientId, mpsc::UnboundedReceiver<ServerMessage>) {
        let lobby = LobbyService::new();
        let (tx, rx) = mpsc::unbounded_channel();
        let id = lobby.register(tx);
        (lobby, id, rx)
    }

    #[test]
    fn register_increments_session_count() {
        let (lobby, _, _) = lobby_with_client();
        assert_eq!(lobby.session_count(), 1);
    }

    #[test]
    fn hello_sends_welcome() {
        let (lobby, id, mut rx) = lobby_with_client();
        lobby.hello(id, String::from("alice"));
        match rx.try_recv().unwrap() {
            ServerMessage::Welcome { display_name, .. } => assert_eq!(display_name, "alice"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn empty_display_name_is_rejected() {
        let (lobby, id, mut rx) = lobby_with_client();
        lobby.hello(id, String::from("   "));
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
    fn joining_a_match_notifies_both_players() {
        let lobby = LobbyService::new();
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register(host_tx);
        let guest = lobby.register(guest_tx);
        lobby.hello(host, String::from("host"));
        let _ = host_rx.try_recv(); // Welcome
        lobby.hello(guest, String::from("guest"));
        let _ = guest_rx.try_recv(); // Welcome

        lobby.create_match(host);
        let match_id = match host_rx.try_recv().unwrap() {
            ServerMessage::MatchCreated { match_id } => match_id,
            other => panic!("unexpected: {other:?}"),
        };
        lobby.join_match(guest, match_id);

        match host_rx.try_recv().unwrap() {
            ServerMessage::MatchReady { your_mark, opponent, .. } => {
                assert_eq!(your_mark, Player::X);
                assert_eq!(opponent, "guest");
            }
            other => panic!("unexpected: {other:?}"),
        }
        match guest_rx.try_recv().unwrap() {
            ServerMessage::MatchReady { your_mark, opponent, .. } => {
                assert_eq!(your_mark, Player::O);
                assert_eq!(opponent, "host");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn playing_a_full_game_ends_in_a_win() {
        let lobby = LobbyService::new();
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register(host_tx);
        let guest = lobby.register(guest_tx);
        lobby.hello(host, String::from("host"));
        let _ = host_rx.try_recv();
        lobby.hello(guest, String::from("guest"));
        let _ = guest_rx.try_recv();
        lobby.create_match(host);
        let match_id = match host_rx.try_recv().unwrap() {
            ServerMessage::MatchCreated { match_id } => match_id,
            _ => unreachable!(),
        };
        lobby.join_match(guest, match_id);
        let _ = host_rx.try_recv(); // MatchReady
        let _ = guest_rx.try_recv(); // MatchReady

        // X: 0, 1, 2 wins. O plays 3, 4.
        for (client, pos) in [
            (host, 0u8),
            (guest, 3),
            (host, 1),
            (guest, 4),
            (host, 2),
        ] {
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

    #[test]
    fn disconnect_removes_the_session_and_the_open_match() {
        let lobby = LobbyService::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let host = lobby.register(tx);
        lobby.hello(host, String::from("host"));
        lobby.create_match(host);
        assert_eq!(lobby.session_count(), 1);
        assert_eq!(lobby.match_count(), 1);

        lobby.disconnect(host);
        assert_eq!(lobby.session_count(), 0);
        assert_eq!(lobby.match_count(), 0);
    }

    #[test]
    fn disconnect_notifies_the_opponent() {
        let lobby = LobbyService::new();
        let (host_tx, mut host_rx) = mpsc::unbounded_channel();
        let (guest_tx, mut guest_rx) = mpsc::unbounded_channel();
        let host = lobby.register(host_tx);
        let guest = lobby.register(guest_tx);
        lobby.hello(host, String::from("host"));
        let _ = host_rx.try_recv();
        lobby.hello(guest, String::from("guest"));
        let _ = guest_rx.try_recv();
        lobby.create_match(host);
        let _ = host_rx.try_recv();
        let match_id = lobby
            .lock()
            .matches
            .keys()
            .copied()
            .next()
            .expect("match exists");
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
}