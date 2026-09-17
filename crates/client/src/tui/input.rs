//! Keyboard input handling.
//!
//! The translation from a raw `KeyCode` to an [`AppEvent`] is context
//! sensitive. In the lobby, the digits `1` through `9` select a match from
//! the list. In a game, the same digits play a move. The translator needs to
//! know which screen is currently shown, and that is why [`read_key_action`]
//! takes the current screen as an argument.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use thiserror::Error;

use crate::app::AppEvent;
use crate::domain::screen::Screen;

/// Errors raised while polling for keyboard events.
#[derive(Debug, Error)]
pub enum InputError {
    /// The underlying terminal reported an I/O error.
    #[error("terminal input error: {0}")]
    Io(#[from] io::Error),
    /// The keyboard thread has been shut down.
    #[error("keyboard thread has been shut down")]
    Closed,
}

/// A high-level action produced by a key press.
pub enum KeyAction {
    /// Forward the given event to the state machine.
    Event(AppEvent),
    /// Ignore the key press.
    Ignored,
}

/// Blocks until a key is pressed and returns the corresponding action,
/// translated according to the screen currently shown.
///
/// # Errors
///
/// Returns [`InputError::Io`] on terminal failures.
pub fn read_key_action(screen: &Screen) -> Result<KeyAction, InputError> {
    loop {
        let event = event::read()?;
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        return Ok(translate_key(key.code, screen));
    }
}

/// Translates a raw `KeyCode` into a [`KeyAction`] for the given screen.
///
/// Extracted from [`read_key_action`] so it can be unit-tested without a
/// terminal.
pub fn translate_key(code: KeyCode, screen: &Screen) -> KeyAction {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => KeyAction::Event(AppEvent::Quit),
        KeyCode::Char('r') => KeyAction::Event(AppEvent::RefreshLobby),
        KeyCode::Char('c') => KeyAction::Event(AppEvent::CreateMatch),
        KeyCode::Char('l') => KeyAction::Event(AppEvent::LeaveMatch),
        KeyCode::Char(digit @ '1'..='9') => {
            let value = digit as u8 - b'0';
            match screen {
                Screen::Lobby { .. } => {
                    KeyAction::Event(AppEvent::JoinMatchAt((value - 1) as usize))
                }
                Screen::InGame(_) => KeyAction::Event(AppEvent::PlayMove(value)),
                Screen::Connecting | Screen::Finished { .. } | Screen::Fatal(_) => {
                    KeyAction::Ignored
                }
            }
        }
        _ => KeyAction::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use common::domain::{Board, GameStatus, Player};
    use common::protocol::{MatchId, MatchSummary};

    use super::*;
    use crate::domain::screen::ActiveMatch;

    fn lobby() -> Screen {
        Screen::Lobby {
            matches: vec![MatchSummary {
                id: MatchId::new(0),
                host: String::from("alice"),
            }],
        }
    }

    fn in_game() -> Screen {
        Screen::InGame(Box::new(ActiveMatch {
            id: MatchId::new(0),
            opponent: String::from("bob"),
            your_mark: Player::X,
            board: Board::new(),
            current_turn: Player::X,
            status: GameStatus::InProgress,
        }))
    }

    #[test]
    fn digit_one_in_the_lobby_joins_the_first_match() {
        let action = translate_key(KeyCode::Char('1'), &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::JoinMatchAt(0))));
    }

    #[test]
    fn digit_nine_in_the_lobby_joins_the_ninth_match() {
        let action = translate_key(KeyCode::Char('9'), &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::JoinMatchAt(8))));
    }

    #[test]
    fn digit_one_in_a_game_plays_a_move() {
        let action = translate_key(KeyCode::Char('1'), &in_game());
        assert!(matches!(action, KeyAction::Event(AppEvent::PlayMove(1))));
    }

    #[test]
    fn digit_in_connecting_screen_is_ignored() {
        let action = translate_key(KeyCode::Char('1'), &Screen::Connecting);
        assert!(matches!(action, KeyAction::Ignored));
    }

    #[test]
    fn q_quits_from_anywhere() {
        let action = translate_key(KeyCode::Char('q'), &lobby());
        assert!(matches!(action, KeyAction::Event(AppEvent::Quit)));
    }

    #[test]
    fn unknown_key_is_ignored() {
        let action = translate_key(KeyCode::Char('z'), &lobby());
        assert!(matches!(action, KeyAction::Ignored));
    }
}
