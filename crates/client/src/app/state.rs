//! Application state and events.

use common::domain::Username;
use common::protocol::{ClientMessage, ServerMessage};

use crate::domain::{AuthMode, PendingAction, Screen};

/// Everything the client knows at any point in time.
pub struct AppState {
    /// The current screen.
    pub screen: Screen,
    /// The display name shown in the header.
    pub display_name: String,
    /// The username of the authenticated account, if any.
    pub authenticated_as: Option<Username>,
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
            authenticated_as: None,
            status: String::from("connecting..."),
            should_quit: false,
        }
    }

    /// Returns `true` when the session is authenticated.
    pub const fn is_authenticated(&self) -> bool {
        self.authenticated_as.is_some()
    }
}

/// An event the state machine reacts to.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A message arrived from the server.
    Server(ServerMessage),
    /// The server closed the connection.
    Disconnected,
    /// The user pressed `q` or `Esc` outside of the auth screen.
    Quit,
    /// The user asked to refresh the lobby list.
    RefreshLobby,
    /// The user asked to open the ranking screen.
    ShowRanking,
    /// The user asked to return to the lobby from a secondary screen.
    BackToLobby,
    /// The user asked to create a new match.
    CreateMatch,
    /// The user asked to join the match at the given 0-based index in the
    /// current lobby list.
    JoinMatchAt(usize),
    /// The user asked to enter or leave the lobby's spectator mode.
    ToggleSpectatorMode,
    /// The user asked to spectate the match at the given 0-based index in
    /// the current lobby list.
    SpectateAt(usize),
    /// The user asked to stop spectating the current match.
    LeaveSpectate,
    /// The user played a move at the given 1-based cell index.
    PlayMove(u8),
    /// The user asked to leave the current match.
    LeaveMatch,
    /// The user asked to see the auth screen.
    ShowAuth {
        /// Whether to start in login or register mode.
        mode: AuthMode,
        /// An action to retry after authentication succeeds.
        pending: Option<PendingAction>,
    },
    /// A character typed on the auth screen.
    AuthInput(char),
    /// Backspace on the auth screen.
    AuthBackspace,
    /// Tab on the auth screen.
    AuthNextField,
    /// Shift-Tab on the auth screen.
    AuthPreviousField,
    /// Enter on the auth screen.
    AuthSubmit,
    /// F2 on the auth screen: toggles between login and register.
    AuthToggleMode,
    /// F3 on the auth screen: toggles password visibility.
    AuthToggleReveal,
    /// Esc on the auth screen: returns to the lobby as a guest.
    AuthCancel,
    /// A no-op event used to wake the main loop without changing state.
    ///
    /// The keyboard thread emits this when a key produces no event, so the
    /// main loop can re-publish the current screen and the keyboard thread
    /// can read the next key.
    Noop,
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
        assert!(!state.is_authenticated());
    }
}
