//! Authentication-screen events and auth-failure handling.

use common::protocol::ClientMessage;

use crate::app::state::{AppEvent, AppState};
use crate::domain::{AuthForm, AuthMode, PendingAction, Screen};

use super::SideEffect;

/// Applies the auth-screen events.
///
/// Returns `true` if the event was an auth event and was handled here.
pub(super) fn apply_auth_event(
    state: &mut AppState,
    event: &AppEvent,
    effects: &mut Vec<SideEffect>,
) -> bool {
    match event {
        AppEvent::AuthInput(character) => {
            if let Screen::Auth(form) = &mut state.screen {
                form.push_char(*character);
            }
            true
        }
        AppEvent::AuthBackspace => {
            if let Screen::Auth(form) = &mut state.screen {
                form.pop_char();
            }
            true
        }
        AppEvent::AuthNextField => {
            if let Screen::Auth(form) = &mut state.screen {
                form.focus_next();
            }
            true
        }
        AppEvent::AuthPreviousField => {
            if let Screen::Auth(form) = &mut state.screen {
                form.focus_previous();
            }
            true
        }
        AppEvent::AuthSubmit => {
            if let Screen::Auth(form) = &mut state.screen {
                match form.build_message() {
                    Ok(message) => {
                        form.error = None;
                        effects.push(SideEffect::Send(message));
                    }
                    Err(reason) => {
                        form.error = Some(reason.to_string());
                    }
                }
            }
            true
        }
        AppEvent::AuthToggleMode => {
            if let Screen::Auth(form) = &mut state.screen {
                form.toggle_mode();
            }
            true
        }
        AppEvent::AuthToggleReveal => {
            if let Screen::Auth(form) = &mut state.screen {
                form.toggle_reveal_password();
            }
            true
        }
        AppEvent::AuthCancel => {
            state.screen = Screen::Lobby {
                matches: Vec::new(),
                spectator_mode: false,
            };
            state.status = String::from("returned to lobby");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
            true
        }
        _ => false,
    }
}

/// Applies an authentication failure: shows the error on the auth form when
/// the client is on that screen, otherwise on the status line.
pub(super) fn apply_auth_failure(
    state: &mut AppState,
    reason: common::protocol::AuthFailureReason,
    message: &str,
) {
    if let Screen::Auth(form) = &mut state.screen {
        form.error = Some(format!("{reason:?}: {message}"));
    } else {
        state.status = format!("{reason:?}: {message}");
    }
}

/// Switches the client to the auth screen with the given mode and pending
/// action.
pub(super) fn show_auth(state: &mut AppState, mode: AuthMode, pending: Option<PendingAction>) {
    state.screen = Screen::Auth(Box::new(AuthForm::new(mode, pending)));
}

/// Returns to the lobby and retries the action the user was attempting when
/// authentication was requested.
pub(super) fn retry_pending(state: &mut AppState, effects: &mut Vec<SideEffect>) {
    let pending = match &state.screen {
        Screen::Auth(form) => form.pending_action,
        _ => None,
    };
    state.screen = Screen::Lobby {
        matches: Vec::new(),
        spectator_mode: false,
    };
    effects.push(SideEffect::Send(ClientMessage::ListMatches));
    match pending {
        Some(PendingAction::CreateMatch) => {
            effects.push(SideEffect::Send(ClientMessage::CreateMatch));
        }
        Some(PendingAction::JoinMatch(match_id)) => {
            effects.push(SideEffect::Send(ClientMessage::JoinMatch { match_id }));
        }
        None => {}
    }
}
