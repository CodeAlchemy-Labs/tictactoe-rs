//! Server-message dispatch.

use common::domain::{Board, GameStatus, Player};
use common::protocol::{ClientMessage, ErrorCode, MatchId, ServerMessage};

use crate::app::state::AppState;
use crate::domain::{ActiveMatch, AuthMode, Screen};

use super::SideEffect;
use super::auth::{retry_pending, show_auth};
use super::lobby_status;
use super::spectator::apply_spectator_message;

/// Applies a message received from the server.
pub(super) fn apply_server(
    state: &mut AppState,
    message: ServerMessage,
    effects: &mut Vec<SideEffect>,
) {
    if apply_list_message(state, &message) {
        return;
    }
    if apply_spectator_message(state, &message, effects) {
        return;
    }
    if apply_in_game_opponent_notice(state, &message) {
        return;
    }
    match message {
        ServerMessage::Welcome { display_name, .. } => {
            state.display_name = display_name;
            state.screen = Screen::Lobby {
                matches: Vec::new(),
                spectator_mode: false,
            };
            state.status = String::from("connected as guest");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
        }
        ServerMessage::Registered { profile } => {
            state.authenticated_as = Some(profile.username.clone());
            state.display_name.clone_from(&profile.name);
            state.status = format!("registered as {}", profile.username);
            retry_pending(state, effects);
        }
        ServerMessage::LoginSucceeded { profile } => {
            state.authenticated_as = Some(profile.username.clone());
            state.display_name.clone_from(&profile.name);
            state.status = format!("logged in as {}", profile.username);
            retry_pending(state, effects);
        }
        ServerMessage::AuthenticationFailed { reason, message } => {
            super::auth::apply_auth_failure(state, reason, &message);
        }
        ServerMessage::MatchCreated { .. } => {
            state.status = String::from("waiting for an opponent...");
        }
        ServerMessage::MatchReady {
            match_id,
            opponent,
            your_mark,
            board,
            current_turn,
        } => {
            enter_game(state, match_id, opponent, your_mark, board, current_turn);
        }
        ServerMessage::BoardUpdate {
            board,
            current_turn,
            status,
        } => {
            if let Screen::InGame(active) = &mut state.screen {
                active.board = board;
                active.current_turn = current_turn;
                active.status = status;
            }
        }
        ServerMessage::MatchOver {
            board,
            status,
            winner_name,
        } => {
            state.screen = Screen::Finished {
                board,
                status,
                winner_name,
            };
            state.status = String::from("press Esc to return to the lobby");
        }
        ServerMessage::MatchAbandoned { .. } => {
            return_to_lobby(state, effects, "the match was abandoned");
        }
        ServerMessage::OpponentLeft { .. } => {
            return_to_lobby(state, effects, "opponent left the match");
        }
        ServerMessage::Error { code, message } => {
            apply_error(state, code, message);
        }
        // Handled earlier or intentionally no-op. The arms stay here to keep
        // the match exhaustive.
        ServerMessage::MatchList { .. }
        | ServerMessage::Ranking { .. }
        | ServerMessage::SpectateStarted { .. }
        | ServerMessage::SpectatorJoined { .. }
        | ServerMessage::SpectatorLeft { .. }
        | ServerMessage::OpponentDisconnected { .. }
        | ServerMessage::OpponentReconnected { .. }
        | ServerMessage::Pong => {}
    }
}

/// Handles the opponent notices that only apply while the local client is
/// playing a match.
///
/// Returns `true` when the message was consumed here. A message about a
/// different match is also consumed: the local client cannot act on it.
fn apply_in_game_opponent_notice(state: &mut AppState, message: &ServerMessage) -> bool {
    let Screen::InGame(active) = &state.screen else {
        return false;
    };
    let active_id = active.id;
    match message {
        ServerMessage::OpponentDisconnected {
            match_id,
            grace_seconds,
        } => {
            if *match_id == active_id {
                state.status = format!(
                    "opponent disconnected; waiting up to {grace_seconds}s for reconnection"
                );
            }
            true
        }
        ServerMessage::OpponentReconnected { match_id } => {
            if *match_id == active_id {
                state.status = String::from("opponent reconnected; game resumed");
            }
            true
        }
        _ => false,
    }
}

/// Switches the client into the in-game screen with the given state.
fn enter_game(
    state: &mut AppState,
    match_id: MatchId,
    opponent: String,
    your_mark: Player,
    board: Board,
    current_turn: Player,
) {
    state.screen = Screen::InGame(Box::new(ActiveMatch {
        id: match_id,
        opponent,
        your_mark,
        board,
        current_turn,
        status: GameStatus::InProgress,
    }));
    state.status = String::from("1-9 to play, q to leave");
}

/// Returns the client to the lobby and requests a fresh match list.
fn return_to_lobby(state: &mut AppState, effects: &mut Vec<SideEffect>, status: &str) {
    state.screen = Screen::Lobby {
        matches: Vec::new(),
        spectator_mode: false,
    };
    state.status = String::from(status);
    effects.push(SideEffect::Send(ClientMessage::ListMatches));
}

/// Applies `MatchList` and `Ranking`.
///
/// Returns `true` if the message was handled here. `apply_server` calls this
/// first so the two list-shaped messages stay together and do not grow
/// `apply_server` further.
fn apply_list_message(state: &mut AppState, message: &ServerMessage) -> bool {
    match message {
        ServerMessage::MatchList { matches } => {
            // Preserve the spectator mode across refreshes.
            let spectator_mode = matches!(
                &state.screen,
                Screen::Lobby {
                    spectator_mode: true,
                    ..
                }
            );
            state.screen = Screen::Lobby {
                matches: matches.clone(),
                spectator_mode,
            };
            state.status = if spectator_mode {
                String::from("spectator mode: press 1-9 to watch a match, Esc to cancel")
            } else {
                lobby_status()
            };
            true
        }
        ServerMessage::Ranking { entries } => {
            if matches!(state.screen, Screen::Ranking { .. }) {
                let count = entries.len();
                state.screen = Screen::Ranking {
                    entries: entries.clone(),
                };
                state.status = format!("{count} entries; press Esc to return");
            }
            true
        }
        _ => false,
    }
}

/// Applies a protocol error. `AuthenticationRequired` moves the client to
/// the auth screen when it is not already authenticated; any other code is
/// surfaced on the status line.
fn apply_error(state: &mut AppState, code: ErrorCode, message: String) {
    if code == ErrorCode::AuthenticationRequired && !state.is_authenticated() {
        show_auth(state, AuthMode::Login, None);
        if let Screen::Auth(form) = &mut state.screen {
            form.error = Some(message);
        }
    } else {
        state.status = format!("{code:?}: {message}");
    }
}
