//! Keyboard input handling.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use thiserror::Error;

use crate::app::AppEvent;

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
///
/// Kept separate from [`AppEvent`] because a key may not always map to a
/// state transition (for example, an unrecognized key is ignored).
pub enum KeyAction {
    /// Forward the given event to the state machine.
    Event(AppEvent),
    /// Ignore the key press.
    Ignored,
}

/// Blocks until a key is pressed and returns the corresponding action.
///
/// # Errors
///
/// Returns [`InputError::Io`] on terminal failures.
pub fn read_key_action() -> Result<KeyAction, InputError> {
    loop {
        let event = event::read()?;
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        return Ok(match key.code {
            KeyCode::Char('q') | KeyCode::Esc => KeyAction::Event(AppEvent::Quit),
            KeyCode::Char('r') => KeyAction::Event(AppEvent::RefreshLobby),
            KeyCode::Char('c') => KeyAction::Event(AppEvent::CreateMatch),
            KeyCode::Char('l') => KeyAction::Event(AppEvent::LeaveMatch),
            KeyCode::Char(digit @ '1'..='9') => {
                let value = digit as u8 - b'0';
                KeyAction::Event(AppEvent::PlayMove(value))
            }
            _ => KeyAction::Ignored,
        });
    }
}
