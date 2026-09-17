//! Application state and events.

use common::protocol::{ClientMessage, ServerMessage};

use crate::domain::Screen;

/// Everything the client knows at any point in time.
pub struct AppState {
    /// The current screen.
    pub screen: Screen,
    /// The local player's display name.
    pub display_name: String,
    /// A transient status line shown at the bottom of the UI.
    pub status: String,
    /// Set to `true` when the main loop should exit.
    pub should_quit: bool,
}

impl AppState {
    /// Creates the initial state for the given display name.
    pub fn new(display_name: impl Into<String>) -> Self {
        Self {
            screen: Screen::Connecting,
            display_name: display_name.into(),
            status: String::from("connecting..."),
            should_quit: false,
        }
    }
}

/// An event the state machine reacts to.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A message arrived from the server.
    Server(ServerMessage),
    /// The server closed the connection.
    Disconnected,
    /// The user pressed `q` or `Esc`.
    Quit,
    /// The user asked to refresh the lobby list.
    RefreshLobby,
    /// The user asked to create a new match.
    CreateMatch,
    /// The user asked to join the match at the given 0-based index in the
    /// current lobby list.
    JoinMatchAt(usize),
    /// The user played a move at the given 1-based cell index.
    PlayMove(u8),
    /// The user asked to leave the current match.
    LeaveMatch,
    /// A user requested that we send an arbitrary message; used by tests.
    Send(ClientMessage),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_starts_connecting() {
        let state = AppState::new("alice");
        assert_eq!(state.display_name, "alice");
        assert!(matches!(state.screen, Screen::Connecting));
        assert!(!state.should_quit);
    }
}
