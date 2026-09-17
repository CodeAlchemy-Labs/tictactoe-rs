//! Pure update function that maps `(state, event)` to side effects.
//!
//! The side effects are expressed as values rather than being applied
//! directly. This makes the state machine fully testable: a test drives the
//! state through a sequence of events and inspects both the resulting state
//! and the list of [`SideEffect`] values produced.

use common::domain::{Board, GameStatus};
use common::protocol::{ClientMessage, ErrorCode, MatchSummary, ServerMessage};
use tokio::sync::mpsc;

use crate::app::state::{AppEvent, AppState};
use crate::domain::screen::{ActiveMatch, Screen};

/// A side effect the main loop must perform after applying an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideEffect {
    /// Send a message to the server.
    Send(ClientMessage),
    /// Quit the process.
    Quit,
}

/// Applies `event` to `state`, pushing any resulting side effects into
/// `effects`.
pub fn apply_event(state: &mut AppState, event: AppEvent, effects: &mut Vec<SideEffect>) {
    match event {
        AppEvent::Server(message) => apply_server(state, message, effects),
        AppEvent::Disconnected => {
            state.screen = Screen::Fatal(String::from("server closed the connection"));
            state.should_quit = true;
        }
        AppEvent::Quit => {
            state.should_quit = true;
        }
        AppEvent::RefreshLobby => {
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
        }
        AppEvent::CreateMatch => {
            effects.push(SideEffect::Send(ClientMessage::CreateMatch));
        }
        AppEvent::JoinMatchAt(index) => {
            if let Screen::Lobby { matches } = &state.screen
                && let Some(summary) = matches.get(index)
            {
                effects.push(SideEffect::Send(ClientMessage::JoinMatch {
                    match_id: summary.id,
                }));
            }
        }
        AppEvent::PlayMove(cell) => {
            if let Ok(position) = common::domain::Position::new(cell - 1) {
                effects.push(SideEffect::Send(ClientMessage::MakeMove { position }));
            }
        }
        AppEvent::LeaveMatch => {
            effects.push(SideEffect::Send(ClientMessage::LeaveMatch));
        }
        AppEvent::Send(message) => {
            effects.push(SideEffect::Send(message));
        }
    }
}

fn apply_server(state: &mut AppState, message: ServerMessage, effects: &mut Vec<SideEffect>) {
    match message {
        ServerMessage::Welcome { display_name, .. } => {
            state.display_name = display_name;
            state.screen = Screen::Lobby {
                matches: Vec::new(),
            };
            state.status = String::from("connected");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
        }
        ServerMessage::MatchList { matches } => {
            state.screen = Screen::Lobby { matches };
            state.status = String::from("press a number to join, c to create, r to refresh");
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
        ServerMessage::MatchOver { board, status } => {
            state.screen = Screen::Finished { board, status };
            state.status = String::from("press q to exit");
        }
        ServerMessage::OpponentLeft { .. } => {
            state.screen = Screen::Lobby {
                matches: Vec::new(),
            };
            state.status = String::from("opponent left the match");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
        }
        ServerMessage::Error { code, message } => {
            state.status = format!("{code:?}: {message}");
        }
        ServerMessage::Pong => {}
    }
}

/// Convenience helper: sends a message through a channel.
pub fn dispatch(
    effects: Vec<SideEffect>,
    outgoing: &mpsc::UnboundedSender<ClientMessage>,
    should_quit: &mut bool,
) {
    for effect in effects {
        match effect {
            SideEffect::Send(message) => {
                let _ = outgoing.send(message);
            }
            SideEffect::Quit => *should_quit = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::protocol::{ClientId, MatchId};

    fn apply(state: &mut AppState, event: AppEvent) -> Vec<SideEffect> {
        let mut effects = Vec::new();
        apply_event(state, event, &mut effects);
        effects
    }

    #[test]
    fn welcome_transitions_to_lobby_and_requests_the_list() {
        let mut state = AppState::new("alice");
        let effects = apply(
            &mut state,
            AppEvent::Server(ServerMessage::Welcome {
                client_id: ClientId::new(1),
                display_name: String::from("alice"),
            }),
        );
        assert!(matches!(state.screen, Screen::Lobby { .. }));
        assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListMatches)]);
    }

    #[test]
    fn match_ready_transitions_to_in_game() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::Welcome {
                client_id: ClientId::new(1),
                display_name: String::from("alice"),
            }),
        );
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::MatchReady {
                match_id: MatchId::new(0),
                opponent: String::from("bob"),
                your_mark: common::domain::Player::X,
                board: Board::new(),
                current_turn: common::domain::Player::X,
            }),
        );
        assert!(matches!(state.screen, Screen::InGame(_)));
    }

    #[test]
    fn match_list_updates_the_lobby() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Lobby {
            matches: Vec::new(),
        };
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::MatchList {
                matches: vec![MatchSummary {
                    id: MatchId::new(0),
                    host: String::from("bob"),
                }],
            }),
        );
        match &state.screen {
            Screen::Lobby { matches } => assert_eq!(matches.len(), 1),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn join_at_selects_the_right_match() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Lobby {
            matches: vec![MatchSummary {
                id: MatchId::new(7),
                host: String::from("bob"),
            }],
        };
        let effects = apply(&mut state, AppEvent::JoinMatchAt(0));
        assert_eq!(
            effects,
            vec![SideEffect::Send(ClientMessage::JoinMatch {
                match_id: MatchId::new(7)
            })]
        );
    }

    #[test]
    fn join_at_ignores_out_of_range_index() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Lobby {
            matches: Vec::new(),
        };
        let effects = apply(&mut state, AppEvent::JoinMatchAt(3));
        assert!(effects.is_empty());
    }

    #[test]
    fn play_move_converts_one_based_to_zero_based() {
        let mut state = AppState::new("alice");
        let effects = apply(&mut state, AppEvent::PlayMove(5));
        assert_eq!(
            effects,
            vec![SideEffect::Send(ClientMessage::MakeMove {
                position: common::domain::Position::new(4).unwrap()
            })]
        );
    }

    #[test]
    fn play_move_ignores_zero_and_out_of_range() {
        let mut state = AppState::new("alice");
        assert!(apply(&mut state, AppEvent::PlayMove(0)).is_empty());
        assert!(apply(&mut state, AppEvent::PlayMove(10)).is_empty());
    }

    #[test]
    fn error_updates_the_status_line() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::Error {
                code: ErrorCode::MatchNotFound,
                message: String::from("no such match"),
            }),
        );
        assert!(state.status.contains("no such match"));
    }

    #[test]
    fn disconnect_is_fatal() {
        let mut state = AppState::new("alice");
        let _ = apply(&mut state, AppEvent::Disconnected);
        assert!(matches!(state.screen, Screen::Fatal(_)));
        assert!(state.should_quit);
    }

    #[test]
    fn quit_event_marks_should_quit() {
        let mut state = AppState::new("alice");
        let _ = apply(&mut state, AppEvent::Quit);
        assert!(state.should_quit);
    }
}
