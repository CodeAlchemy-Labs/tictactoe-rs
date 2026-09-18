//! Spectator-specific server message handling.
//!
//! Every branch only consumes the message when the client is on the
//! [`Screen::Spectating`] screen. Otherwise it returns `false` and the
//! general server handler takes over. This is what makes the same
//! `BoardUpdate` reach both players (via the general handler) and
//! spectators (via this module).

use common::domain::GameStatus;
use common::protocol::{ClientMessage, ServerMessage};

use crate::app::state::AppState;
use crate::domain::{Screen, SpectatedMatch};

use super::SideEffect;

/// Applies spectator-related messages.
///
/// Returns `true` when the message was handled here.
pub(super) fn apply_spectator_message(
    state: &mut AppState,
    message: &ServerMessage,
    effects: &mut Vec<SideEffect>,
) -> bool {
    match message {
        ServerMessage::SpectateStarted {
            match_id,
            host_name,
            guest_name,
            board,
            current_turn,
            status,
            spectator_count,
        } => {
            state.screen = Screen::Spectating(Box::new(SpectatedMatch {
                id: *match_id,
                host_name: host_name.clone(),
                guest_name: guest_name.clone(),
                board: *board,
                current_turn: *current_turn,
                status: *status,
                spectator_count: *spectator_count,
            }));
            state.status =
                format!("observing {match_id}; {spectator_count} spectator(s); Esc to leave");
            true
        }
        ServerMessage::SpectatorJoined {
            username,
            spectator_count,
        } => {
            // Only meaningful while spectating. Otherwise fall through so
            // the general handler can decide what to do.
            let Screen::Spectating(active) = &mut state.screen else {
                return false;
            };
            active.spectator_count = *spectator_count;
            state.status = format!("{username} joined as spectator; {spectator_count} watching");
            true
        }
        ServerMessage::SpectatorLeft {
            username,
            spectator_count,
        } => {
            let Screen::Spectating(active) = &mut state.screen else {
                return false;
            };
            active.spectator_count = *spectator_count;
            state.status = format!("{username} left the audience; {spectator_count} watching");
            true
        }
        ServerMessage::BoardUpdate {
            board,
            current_turn,
            status,
        } => {
            // The same message reaches players and spectators. Only the
            // spectator branch is handled here; players fall through to
            // `apply_server` so their own board is updated.
            let Screen::Spectating(active) = &mut state.screen else {
                return false;
            };
            active.board = *board;
            active.current_turn = *current_turn;
            active.status = *status;
            true
        }
        ServerMessage::MatchOver {
            board,
            status,
            winner_name,
        } => {
            let Screen::Spectating(_) = &state.screen else {
                return false;
            };
            // The match ended. Show the result on the spectating client
            // instead of pushing it straight back to the lobby. The user
            // dismisses the screen with Esc; the client then sends
            // `LeaveSpectate` which the server accepts even if the match no
            // longer exists.
            state.screen = Screen::Finished {
                board: *board,
                status: *status,
                winner_name: winner_name.clone(),
            };
            state.status = String::from("press Esc to return to the lobby");
            true
        }
        ServerMessage::MatchAbandoned { .. } => {
            if !matches!(state.screen, Screen::Spectating(_)) {
                return false;
            }
            state.screen = Screen::Lobby {
                matches: Vec::new(),
                spectator_mode: false,
            };
            state.status = String::from("the match was abandoned by both players");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
            true
        }
        ServerMessage::OpponentDisconnected {
            match_id,
            grace_seconds,
        } => {
            if let Screen::Spectating(active) = &state.screen
                && active.id == *match_id
            {
                state.status = format!(
                    "a player disconnected; waiting up to {grace_seconds}s for reconnection"
                );
            }
            true
        }
        ServerMessage::OpponentReconnected { match_id } => {
            if let Screen::Spectating(active) = &state.screen
                && active.id == *match_id
            {
                state.status = String::from("the player reconnected; the match continues");
            }
            true
        }
        _ => false,
    }
}
