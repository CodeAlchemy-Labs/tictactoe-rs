//! Match-related operations on [`LobbyService`].
//!
//! This file implements the public methods that drive the match lifecycle
//! (`list_matches`, `create_match`, `join_match`, `make_move`,
//! `leave_match`) and the two free helpers that mutate match state:
//!
//! - [`detach_from_match`] removes a client from a match, notifies the
//!   opponent, and drops the match when it becomes empty. It is called from
//!   both `leave_match` and `disconnect`.
//! - [`apply_move`] mutates the board and returns everything the caller
//!   needs to broadcast.

use common::domain::{Board, GameStatus, Player, Position, Username};
use common::protocol::{ClientId, ErrorCode, MatchId, ServerMessage};

use super::{LobbyService, LobbyState};
use crate::domain::Match;

/// Tuple returned by [`apply_move`] on success.
///
/// The fields are `(host, guest, board, next_turn, status)`.
type MoveOutcome = (ClientId, Option<ClientId>, Board, Player, GameStatus);

impl LobbyService {
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
            .map(|m| common::protocol::MatchSummary {
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
            } else {
                match state.matches.get(&match_id) {
                    None => Some((ErrorCode::MatchNotFound, "match not found")),
                    // Checked before `current_match` so that a host that
                    // tries to join its own match receives the specific
                    // error instead of the generic "already in a match".
                    Some(m) if m.host == client => {
                        Some((ErrorCode::CannotJoinOwnMatch, "you already host this match"))
                    }
                    _ if session.current_match.is_some() => {
                        Some((ErrorCode::InvalidState, "already in a match"))
                    }
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
    ///
    /// When the move ends the match, the server broadcasts `MatchOver` with
    /// the winner's display name, releases both players from the match, and
    /// removes the match from the lobby. The win is recorded in the ranking
    /// service after the lobby lock is released.
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

        let winner_for_ranking = match outcome {
            Err((code, message)) => {
                if let Some(session) = state.sessions.get(&client) {
                    session.try_send(ServerMessage::Error {
                        code,
                        message: String::from(message),
                    });
                }
                None
            }
            Ok((host, guest, board, next_turn, status)) => {
                let update = ServerMessage::BoardUpdate {
                    board,
                    current_turn: next_turn,
                    status,
                };
                if let Some(host_session) = state.sessions.get(&host) {
                    host_session.try_send(update.clone());
                }
                if let Some(guest_id) = guest
                    && let Some(guest_session) = state.sessions.get(&guest_id)
                {
                    guest_session.try_send(update);
                }

                if status.is_finished() {
                    finish_match(&mut state, match_id, host, guest, board, status)
                } else {
                    None
                }
            }
        };

        drop(state);

        if let Some((username, name)) = winner_for_ranking {
            self.ranking.record_win(&username, &name);
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
}

/// Removes `client` from `match_id` and eliminates the match.
///
/// The policy for this version is that a match ends as soon as one of its
/// players leaves, regardless of whether it is the host or the guest. That
/// keeps the lobby state simple: no match survives with a single player,
/// and the remaining player is returned to the lobby by the caller through
/// `OpponentLeft`. This avoids the confusing situation where the host sees
/// its own abandoned match still listed for other guests to join.
pub(super) fn detach_from_match(state: &mut LobbyState, match_id: MatchId, client: ClientId) {
    let Some(m) = state.matches.remove(&match_id) else {
        return;
    };
    let opponent = m.opponent_of(client);
    if let Some(opponent_id) = opponent {
        if let Some(opponent_session) = state.sessions.get(&opponent_id) {
            opponent_session.try_send(ServerMessage::OpponentLeft { match_id });
        }
        if let Some(opponent_session) = state.sessions.get_mut(&opponent_id) {
            opponent_session.current_match = None;
            opponent_session.mark = None;
        }
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

/// Returns the authenticated username and display name for a session, if
/// the session is authenticated.
fn winner_from_session(state: &LobbyState, client: ClientId) -> Option<(Username, String)> {
    let session = state.sessions.get(&client)?;
    let username = session.authenticated_as.clone()?;
    let name = session
        .display_name
        .clone()
        .unwrap_or_else(|| username.as_str().to_string());
    Some((username, name))
}

/// Finalizes a match: broadcasts `MatchOver`, releases both players so they
/// can start a new one, and drops the match from the lobby.
///
/// Returns the winner's authenticated identity for the ranking update, or
/// `None` when the match ended in a draw or the winner has no account.
fn finish_match(
    state: &mut LobbyState,
    match_id: MatchId,
    host: ClientId,
    guest: Option<ClientId>,
    board: Board,
    status: GameStatus,
) -> Option<(Username, String)> {
    // Winner display name for the `MatchOver` message.
    let winner_name = match status {
        GameStatus::Won(Player::X) => state
            .sessions
            .get(&host)
            .and_then(|s| s.display_name.clone()),
        GameStatus::Won(Player::O) => guest
            .and_then(|id| state.sessions.get(&id))
            .and_then(|s| s.display_name.clone()),
        GameStatus::InProgress | GameStatus::Draw => None,
    };

    // Winner identity for the ranking.
    let winner = match status {
        GameStatus::Won(Player::X) => winner_from_session(state, host),
        GameStatus::Won(Player::O) => guest.and_then(|id| winner_from_session(state, id)),
        GameStatus::InProgress | GameStatus::Draw => None,
    };

    // Broadcast the final state to both players.
    let over = ServerMessage::MatchOver {
        board,
        status,
        winner_name,
    };
    if let Some(session) = state.sessions.get(&host) {
        session.try_send(over.clone());
    }
    if let Some(guest_id) = guest
        && let Some(session) = state.sessions.get(&guest_id)
    {
        session.try_send(over);
    }

    // Release both players so they can create or join a new match.
    if let Some(session) = state.sessions.get_mut(&host) {
        session.current_match = None;
        session.mark = None;
    }
    if let Some(guest_id) = guest
        && let Some(session) = state.sessions.get_mut(&guest_id)
    {
        session.current_match = None;
        session.mark = None;
    }

    state.matches.remove(&match_id);

    winner
}
