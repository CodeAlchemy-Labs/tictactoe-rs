//! Pure update function that maps `(state, event)` to side effects.
//!
//! The side effects are expressed as values rather than being applied
//! directly. This makes the state machine fully testable: a test drives the
//! state through a sequence of events and inspects both the resulting state
//! and the list of [`SideEffect`] values produced.
//!
//! The implementation is split across four files:
//!
//! - `auth` handles the auth screen events and the auth-failure path.
//! - `server` dispatches server messages to the state.
//! - `spectator` handles the messages specific to the spectating screen.
//! - this file contains the public API (`SideEffect`, `apply_event`,
//!   `dispatch`) and the shared `lobby_status` helper.

mod auth;
mod server;
mod spectator;
#[cfg(test)]
mod tests;

use common::protocol::ClientMessage;
use tokio::sync::mpsc;

use crate::app::state::{AppEvent, AppState};
use crate::domain::{AuthMode, PendingAction, Screen};

use auth::{apply_auth_event, show_auth};
use server::apply_server;

/// A side effect the main loop must perform after applying an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideEffect {
    /// Send a message to the server.
    Send(ClientMessage),
    /// Connect to a new server.
    Connect {
        url: String,
        name: String,
        use_tls: bool,
    },
    /// Quit the process.
    Quit,
}

/// Applies `event` to `state`, pushing any resulting side effects into
/// `effects`.
pub fn apply_event(state: &mut AppState, event: AppEvent, effects: &mut Vec<SideEffect>) {
    // Auth events take the event by reference. If one of them handles it,
    // we are done; otherwise, `event` is still available for the main match.
    if apply_auth_event(state, &event, effects) {
        return;
    }

    match event {
        AppEvent::Server(message) => apply_server(state, message, effects),
        AppEvent::Disconnected => {
            state.screen = Screen::Fatal(String::from("server closed the connection"));
            state.should_quit = true;
        }
        AppEvent::ConnectionFailed { reason } => {
            state.screen = Screen::Connection;
            state.connection_form.error = Some(reason);
        }
        AppEvent::Quit => {
            state.should_quit = true;
        }
        AppEvent::RefreshLobby => {
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
        }
        AppEvent::ShowRanking => {
            state.screen = Screen::Ranking {
                entries: Vec::new(),
            };
            state.status = String::from("loading ranking...");
            effects.push(SideEffect::Send(ClientMessage::ListRanking));
        }
        AppEvent::BackToLobby => {
            state.screen = Screen::Lobby {
                matches: Vec::new(),
                spectator_mode: false,
            };
            state.status = lobby_status();
            // The client sends `LeaveSpectate` defensively every time it
            // returns to the lobby. If the session was spectating a match
            // that has already ended (typical after a `Finished` screen),
            // the server still holds `session.spectating = Some(...)` and
            // this message releases it. For sessions that were not
            // spectating, the server treats it as a no-op.
            effects.push(SideEffect::Send(ClientMessage::LeaveSpectate));
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
        }
        AppEvent::CreateMatch => {
            if state.is_authenticated() {
                effects.push(SideEffect::Send(ClientMessage::CreateMatch));
            } else {
                show_auth(state, AuthMode::Login, Some(PendingAction::CreateMatch));
            }
        }
        AppEvent::JoinMatchAt(index) => {
            if let Some(match_id) = state.screen.visible_matches().get(index).map(|s| s.id) {
                if state.is_authenticated() {
                    effects.push(SideEffect::Send(ClientMessage::JoinMatch { match_id }));
                } else {
                    show_auth(
                        state,
                        AuthMode::Login,
                        Some(PendingAction::JoinMatch(match_id)),
                    );
                }
            }
        }
        AppEvent::ToggleSpectatorMode => {
            if let Screen::Lobby { spectator_mode, .. } = &mut state.screen {
                *spectator_mode = !*spectator_mode;
                state.status = if *spectator_mode {
                    String::from("spectator mode: press 1-9 to watch a match, Esc to cancel")
                } else {
                    lobby_status()
                };
            }
        }
        AppEvent::SpectateAt(index) => {
            if let Some(match_id) = state.screen.visible_matches().get(index).map(|s| s.id) {
                state.status = String::from("connecting to the match...");
                effects.push(SideEffect::Send(ClientMessage::Spectate { match_id }));
            }
        }
        AppEvent::LeaveSpectate => {
            effects.push(SideEffect::Send(ClientMessage::LeaveSpectate));
        }
        AppEvent::PlayMove(cell) => {
            if let Some(zero_based) = cell.checked_sub(1)
                && let Ok(position) = common::domain::Position::new(zero_based)
            {
                effects.push(SideEffect::Send(ClientMessage::MakeMove { position }));
            }
        }
        AppEvent::LeaveMatch => {
            effects.push(SideEffect::Send(ClientMessage::LeaveMatch));
        }
        AppEvent::ShowAuth { mode, pending } => {
            show_auth(state, mode, pending);
        }
        AppEvent::Send(message) => {
            effects.push(SideEffect::Send(message));
        }
        // `Redraw` only wakes the main loop so it can redraw the frame; the
        // auth events are handled at the top of this function. Both are
        // intentional no-ops in this match, so they share an arm.
        AppEvent::Input(c) => {
            if let Screen::Connection = state.screen {
                state.connection_form.push_char(c);
            }
        }
        AppEvent::PopChar => {
            if let Screen::Connection = state.screen {
                state.connection_form.pop_char();
            }
        }
        AppEvent::Tab => {
            if let Screen::Connection = state.screen {
                state.connection_form.tab();
            }
        }
        AppEvent::ShiftTab => {
            if let Screen::Connection = state.screen {
                state.connection_form.shift_tab();
            }
        }
        AppEvent::ToggleTls => {
            if let Screen::Connection = state.screen {
                state.connection_form.toggle_tls();
            }
        }
        AppEvent::Submit => {
            if let Screen::Connection = state.screen {
                match state.connection_form.build_url() {
                    Ok(url) => {
                        effects.push(SideEffect::Connect {
                            url,
                            name: state.connection_form.guest_name.clone(),
                            use_tls: state.connection_form.use_tls,
                        });
                        state.screen = Screen::Connecting;
                        state.status = String::from("connecting...");
                    }
                    Err(msg) => {
                        state.connection_form.error = Some(msg);
                    }
                }
            }
        }
        AppEvent::Cancel => {
            if let Screen::Connection = state.screen {
                state.should_quit = true;
            }
        }
        AppEvent::Redraw
        | AppEvent::AuthInput(_)
        | AppEvent::AuthBackspace
        | AppEvent::AuthNextField
        | AppEvent::AuthPreviousField
        | AppEvent::AuthSubmit
        | AppEvent::AuthToggleMode
        | AppEvent::AuthToggleReveal
        | AppEvent::AuthCancel => {}
    }
}

/// Returns the standard lobby status line.
fn lobby_status() -> String {
    String::from("press a number to join, c to create, t for ranking, s to spectate")
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
            SideEffect::Connect { .. } => {}
        }
    }
}
