//! Application state and events.

use common::domain::Username;
use common::protocol::{ClientMessage, ServerMessage};

use crate::domain::{AuthMode, ConnectionForm, PendingAction, Screen};

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
    /// The pure model of the connection form.
    pub connection_form: ConnectionForm,
    /// Whether the client started via the connection form.
    pub uses_connection_form: bool,
}

impl AppState {
    /// Creates the initial state.
    ///
    /// If both `server_url` and `guest_name` are fully resolved (e.g. via CLI flags or env vars),
    /// starts on `Screen::Connecting`. Otherwise, starts on `Screen::Connection`.
    pub fn with_config(
        server_url: Option<&str>,
        guest_name: Option<&str>,
        config: &crate::config::ClientConfig,
    ) -> Self {
        let mut form = ConnectionForm::new(config);
        if let Some(url) = &server_url
            && let Ok(u) = url::Url::parse(url)
        {
            let host_str = u.host_str().unwrap_or_default();
            let port_str = u.port().map(|p| format!(":{p}")).unwrap_or_default();
            let path_str = u.path();
            let path_str = if path_str == "/" { "" } else { path_str };
            form.host = format!("{host_str}{port_str}{path_str}");
            form.use_tls = u.scheme() == "wss" || u.scheme() == "https";
        }
        if let Some(name) = &guest_name {
            form.guest_name = name.to_string();
        }

        let is_fully_resolved = server_url.is_some() && guest_name.is_some();
        let display_name = guest_name.unwrap_or(form.guest_name.as_str()).to_string();

        let screen = if is_fully_resolved {
            Screen::Connecting
        } else {
            Screen::Connection
        };

        Self {
            screen,
            display_name,
            authenticated_as: None,
            status: String::from("connecting..."),
            should_quit: false,
            connection_form: form,
            uses_connection_form: !is_fully_resolved,
        }
    }

    /// Returns `true` when the session is authenticated.
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        self.authenticated_as.is_some()
    }

    /// Convenience constructor used in tests: creates state with a given
    /// display name and no pre-resolved URL, so the connection form is shown.
    #[cfg(test)]
    pub fn new(display_name: &str) -> Self {
        Self::with_config(
            None,
            Some(display_name),
            &crate::config::ClientConfig::default(),
        )
    }
}

/// An event the state machine reacts to.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A message arrived from the server.
    Server(ServerMessage),
    /// The server closed the connection.
    Disconnected,
    /// Connection failed with a specific reason.
    ConnectionFailed { reason: String },
    /// The user pressed `q` or `Esc` outside of the auth/connection screens.
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
    /// A character typed on the connection screen.
    Input(char),
    /// Backspace on the connection screen.
    PopChar,
    /// Tab on the connection screen.
    Tab,
    /// Shift-Tab on the connection screen.
    ShiftTab,
    /// Enter on the connection screen.
    Submit,
    /// F3 on the connection screen.
    ToggleTls,
    /// Esc on the connection screen.
    Cancel,
    /// The terminal was resized; the main loop should redraw.
    ///
    /// This event never changes state; it only wakes the main loop so that
    /// the next iteration calls `terminal.draw` with the new dimensions.
    Redraw,
    /// A user requested that we send an arbitrary message; used by tests.
    Send(ClientMessage),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_starts_connecting_if_resolved() {
        let state = AppState::with_config(
            Some("ws://test"),
            Some("alice"),
            &crate::config::ClientConfig::default(),
        );
        assert_eq!(state.display_name, "alice");
        assert!(matches!(state.screen, Screen::Connecting));
        assert!(!state.should_quit);
        assert!(!state.is_authenticated());
    }

    #[test]
    fn new_state_starts_on_connection_if_not_resolved() {
        let state = AppState::with_config(
            None,
            Some("alice"),
            &crate::config::ClientConfig::default(),
        );
        assert!(matches!(state.screen, Screen::Connection));
    }
}
