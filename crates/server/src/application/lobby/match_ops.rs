//! Match-related operations on [`LobbyService`].
//!
//! This file implements the public methods that drive the match lifecycle
//! (`list_matches`, `create_match`, `join_match`, `make_move`,
//! `leave_match`, `spectate`, `leave_spectate`) and the free helpers that
//! mutate match state:
//!
//! - [`detach_from_match`] removes a client from a match, notifies every
//!   remaining participant and spectator, and drops the match.
//! - [`detach_spectator_from_match`] removes a single spectator from a
//!   match that keeps running.
//! - [`apply_move`] mutates the board and returns everything the caller
//!   needs to broadcast.
//! - [`finish_match`] broadcasts the final state, records the winner, and
//!   releases both players.

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
    ///
    /// Sends every match, full or not. The client filters according to its
    /// current mode. This keeps the server stateless with respect to the
    /// user's intent and lets a spectator see matches that already have two
    /// players.
    pub fn list_matches(&self, client: ClientId) {
        let state = self.lock();
        let Some(session) = state.sessions.get(&client) else {
            return;
        };
        let matches = state
            .matches
            .values()
            .map(|m| common::protocol::MatchSummary {
                id: m.id,
                host: state
                    .sessions
                    .get(&m.host)
                    .and_then(|s| s.display_name.clone())
                    .unwrap_or_else(|| String::from("unknown")),
                spectator_count: m.spectator_count(),
                is_full: m.is_full(),
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
        if session.is_spectating() {
            session.try_send(ServerMessage::Error {
                code: ErrorCode::AlreadySpectating,
                message: String::from("leave the spectated match first"),
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
                    Some(m) if m.host == client => {
                        Some((ErrorCode::CannotJoinOwnMatch, "you already host this match"))
                    }
                    _ if session.current_match.is_some() => {
                        Some((ErrorCode::InvalidState, "already in a match"))
                    }
                    _ if session.is_spectating() => Some((
                        ErrorCode::AlreadySpectating,
                        "leave the spectated match first",
                    )),
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
        let spectators: Vec<ClientId> = state
            .matches
            .get(&match_id)
            .map(|m| m.spectators.clone())
            .unwrap_or_default();

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
                opponent: guest_display.clone(),
                your_mark: Player::X,
                board,
                current_turn: first_turn,
            });
        }
        if let Some(guest_session) = state.sessions.get(&client) {
            guest_session.try_send(ServerMessage::MatchReady {
                match_id,
                opponent: host_display.clone(),
                your_mark: Player::O,
                board,
                current_turn: first_turn,
            });
        }

        // The guest slot is now filled, so a spectator that was watching an
        // empty match needs a fresh snapshot with the guest name populated.
        let update = ServerMessage::SpectateStarted {
            match_id,
            host_name: host_display,
            guest_name: guest_display,
            board,
            current_turn: first_turn,
            status: GameStatus::InProgress,
            spectator_count: u32::try_from(spectators.len()).unwrap_or(u32::MAX),
        };
        for spectator in &spectators {
            if let Some(session) = state.sessions.get(spectator) {
                session.try_send(update.clone());
            }
        }
    }

    /// Handles `Spectate`.
    pub fn spectate(&self, client: ClientId, match_id: MatchId) {
        let mut state = self.lock();

        if let Err((code, message)) = validate_spectate_request(&state, client, match_id) {
            if let Some(session) = state.sessions.get(&client) {
                session.try_send(ServerMessage::Error {
                    code,
                    message: String::from(message),
                });
            }
            return;
        }

        let display_name = state
            .sessions
            .get(&client)
            .and_then(|s| s.display_name.clone())
            .unwrap_or_else(|| String::from("spectator"));

        // Read the participant identities before touching the match, so the
        // immutable borrow of `state.sessions` does not overlap with the
        // mutable borrow of `state.matches`.
        let (host, guest) = {
            let Some(m) = state.matches.get(&match_id) else {
                return;
            };
            (m.host, m.guest)
        };
        let host_name = state
            .sessions
            .get(&host)
            .and_then(|s| s.display_name.clone())
            .unwrap_or_else(|| String::from("unknown"));
        let guest_name = guest
            .and_then(|id| state.sessions.get(&id))
            .and_then(|s| s.display_name.clone())
            .unwrap_or_default();

        let (board, current_turn, status, count) = {
            let Some(m) = state.matches.get_mut(&match_id) else {
                return;
            };
            if !m.add_spectator(client, display_name.clone()) {
                return;
            }
            (m.board, m.current_turn, m.status, m.spectator_count())
        };
        if let Some(session) = state.sessions.get_mut(&client) {
            session.spectating = Some(match_id);
        }

        if let Some(session) = state.sessions.get(&client) {
            session.try_send(ServerMessage::SpectateStarted {
                match_id,
                host_name,
                guest_name,
                board,
                current_turn,
                status,
                spectator_count: count,
            });
        }

        broadcast_spectator_joined(&state, match_id, client, display_name, count);
    }

    /// Handles `LeaveSpectate`.
    ///
    /// Idempotent: calling it when the session is not spectating any match
    /// is a silent no-op. The client sends this defensively every time it
    /// leaves the `Finished` screen, so the server must tolerate it even
    /// for sessions that never spectated or that already left.
    ///
    /// If the match still exists, the spectator is detached and the others
    /// are notified. If the match has already ended (for example, the game
    /// finished and the viewer is dismissing the result screen), only the
    /// session flag is cleared.
    pub fn leave_spectate(&self, client: ClientId) {
        let mut state = self.lock();
        let Some(match_id) = state.sessions.get(&client).and_then(|s| s.spectating) else {
            // Not spectating anything: nothing to detach.
            return;
        };
        if let Some(session) = state.sessions.get_mut(&client) {
            session.spectating = None;
        }
        if state.matches.contains_key(&match_id) {
            detach_spectator_from_match(&mut state, match_id, client);
        }
    }

    /// Handles `MakeMove`.
    ///
    /// When the move ends the match, the server broadcasts `MatchOver` with
    /// the winner's display name, releases both players from the match, and
    /// removes the match from the lobby. Spectators receive the same
    /// broadcasts as the players. The win is recorded in the ranking service
    /// after the lobby lock is released.
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
                broadcast_to_participants(&state, match_id, host, guest, &update);

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
    ///
    /// An explicit leave is a deliberate action by the player; there is
    /// nothing to recover from. The match is dissolved immediately and
    /// every remaining participant receives `MatchAbandoned`. Only
    /// unexpected disconnections (handled by
    /// [`LobbyService::disconnect`](LobbyService::disconnect)) go through
    /// the reconnection grace period.
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

    /// Called by the grace-period timer when a disconnected player did not
    /// come back in time.
    ///
    /// Closes the match, awards the win to the remaining player if any, and
    /// releases every associated session. Wins obtained this way are not
    /// recorded in the ranking: an abandonment is not a legitimate victory.
    pub fn expire_disconnection(&self, username: &Username) {
        let mut state = self.lock();
        let Some(pending) = state.pending_disconnections.remove(username) else {
            return;
        };
        state.active_sessions.remove(username);

        let Some(m) = state.matches.remove(&pending.match_id) else {
            return;
        };

        let opponent = m.opponent_of(pending.client_id);
        let winner_name = opponent
            .and_then(|id| state.sessions.get(&id))
            .and_then(|s| s.display_name.clone());

        let over = match winner_name {
            Some(name) => ServerMessage::MatchOver {
                board: m.board,
                status: GameStatus::Won(if pending.was_host {
                    Player::O
                } else {
                    Player::X
                }),
                winner_name: Some(name),
            },
            None => ServerMessage::MatchAbandoned {
                match_id: pending.match_id,
            },
        };

        if let Some(opponent_id) = opponent {
            if let Some(session) = state.sessions.get(&opponent_id) {
                session.try_send(over.clone());
            }
            if let Some(session) = state.sessions.get_mut(&opponent_id) {
                session.current_match = None;
                session.mark = None;
            }
        }
        for spectator in &m.spectators {
            if let Some(session) = state.sessions.get(spectator) {
                session.try_send(over.clone());
            }
            if let Some(session) = state.sessions.get_mut(spectator) {
                session.spectating = None;
            }
        }
    }
}

/// Checks whether a `Spectate` request is acceptable.
///
/// Returns `Ok(())` when the request may proceed, or the error pair the
/// caller should deliver to the client. A request for an unknown session is
/// treated as `Ok(())` so the caller's subsequent `state.sessions.get` will
/// simply find nothing and stop silently.
fn validate_spectate_request(
    state: &LobbyState,
    client: ClientId,
    match_id: MatchId,
) -> Result<(), (ErrorCode, &'static str)> {
    let Some(session) = state.sessions.get(&client) else {
        return Ok(());
    };
    if session.display_name.is_none() {
        return Err((ErrorCode::InvalidState, "send hello before spectating"));
    }
    if session.current_match.is_some() {
        return Err((ErrorCode::InvalidState, "cannot spectate while playing"));
    }
    if session.is_spectating() {
        return Err((ErrorCode::AlreadySpectating, "already spectating a match"));
    }
    match state.matches.get(&match_id) {
        None => Err((ErrorCode::MatchNotFound, "match not found")),
        Some(m) if m.is_player(client) => Err((
            ErrorCode::CannotJoinOwnMatch,
            "cannot spectate your own match",
        )),
        Some(m) if !m.has_room_for_spectator() => Err((
            ErrorCode::SpectatorLimitReached,
            "the match has reached the spectator limit",
        )),
        Some(_) => Ok(()),
    }
}

/// Notifies every participant of a match that a new spectator joined.
///
/// The new spectator does not receive this message; it already knows from
/// its `SpectateStarted` snapshot.
fn broadcast_spectator_joined(
    state: &LobbyState,
    match_id: MatchId,
    new_spectator: ClientId,
    display_name: String,
    count: u32,
) {
    let Some(m) = state.matches.get(&match_id) else {
        return;
    };
    let joined = ServerMessage::SpectatorJoined {
        username: display_name,
        spectator_count: count,
    };
    if let Some(session) = state.sessions.get(&m.host) {
        session.try_send(joined.clone());
    }
    if let Some(guest_id) = m.guest
        && let Some(session) = state.sessions.get(&guest_id)
    {
        session.try_send(joined.clone());
    }
    for spectator in &m.spectators {
        if *spectator != new_spectator
            && let Some(session) = state.sessions.get(spectator)
        {
            session.try_send(joined.clone());
        }
    }
}

/// Sends `message` to the two players of a match and to every spectator.
///
/// Used for `BoardUpdate`, where the same content goes to everyone.
fn broadcast_to_participants(
    state: &LobbyState,
    match_id: MatchId,
    host: ClientId,
    guest: Option<ClientId>,
    message: &ServerMessage,
) {
    if let Some(session) = state.sessions.get(&host) {
        session.try_send(message.clone());
    }
    if let Some(guest_id) = guest
        && let Some(session) = state.sessions.get(&guest_id)
    {
        session.try_send(message.clone());
    }
    if let Some(m) = state.matches.get(&match_id) {
        for spectator in &m.spectators {
            if let Some(session) = state.sessions.get(spectator) {
                session.try_send(message.clone());
            }
        }
    }
}

/// Removes `client` from `match_id` and eliminates the match.
///
/// The policy for this version is that a match ends as soon as one of its
/// players leaves. Every remaining participant (the other player, if any)
/// and every spectator receives `MatchAbandoned` and is returned to the
/// lobby. This replaces the earlier `OpponentLeft` message, which described
/// only the player-to-player perspective and left spectators without a
/// signal.
pub(super) fn detach_from_match(state: &mut LobbyState, match_id: MatchId, client: ClientId) {
    let Some(m) = state.matches.remove(&match_id) else {
        return;
    };
    let opponent = m.opponent_of(client);
    let spectators = m.spectators;

    let abandoned = ServerMessage::MatchAbandoned { match_id };
    if let Some(opponent_id) = opponent {
        if let Some(session) = state.sessions.get(&opponent_id) {
            session.try_send(abandoned.clone());
        }
        if let Some(session) = state.sessions.get_mut(&opponent_id) {
            session.current_match = None;
            session.mark = None;
        }
    }
    for spectator in spectators {
        if let Some(session) = state.sessions.get(&spectator) {
            session.try_send(abandoned.clone());
        }
        if let Some(session) = state.sessions.get_mut(&spectator) {
            session.spectating = None;
        }
    }
}

/// Removes a single spectator from a match that keeps running.
///
/// Called by `leave_spectate` and by `disconnect` when the departing client
/// was spectating.
pub(super) fn detach_spectator_from_match(
    state: &mut LobbyState,
    match_id: MatchId,
    client: ClientId,
) {
    let Some(m) = state.matches.get_mut(&match_id) else {
        return;
    };
    let Some(removed_name) = m.remove_spectator(client) else {
        return;
    };
    let count = m.spectator_count();
    let host = m.host;
    let guest = m.guest;
    let other_spectators: Vec<ClientId> = m
        .spectators
        .iter()
        .copied()
        .filter(|id| *id != client)
        .collect();

    let left = ServerMessage::SpectatorLeft {
        username: removed_name,
        spectator_count: count,
    };
    if let Some(session) = state.sessions.get(&host) {
        session.try_send(left.clone());
    }
    if let Some(guest_id) = guest
        && let Some(session) = state.sessions.get(&guest_id)
    {
        session.try_send(left.clone());
    }
    for id in other_spectators {
        if let Some(session) = state.sessions.get(&id) {
            session.try_send(left.clone());
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

/// Finalizes a match: broadcasts `MatchOver` to the players and the
/// spectators, releases the players so they can start a new one, returns
/// the spectators to the lobby, and drops the match.
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

    // Broadcast the final state to players and spectators.
    let over = ServerMessage::MatchOver {
        board,
        status,
        winner_name,
    };
    broadcast_to_participants(state, match_id, host, guest, &over);

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

    // The spectators stay attached to the match until they dismiss the
    // result screen. Their session keeps `spectating = Some(match_id)` so
    // the client can show the outcome; `LeaveSpectate` clears it.
    state.matches.remove(&match_id);

    winner
}
