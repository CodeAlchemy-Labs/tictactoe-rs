//! Pure update function that maps `(state, event)` to side effects.
//!
//! The side effects are expressed as values rather than being applied
//! directly. This makes the state machine fully testable: a test drives the
//! state through a sequence of events and inspects both the resulting state
//! and the list of [`SideEffect`] values produced.

use common::domain::GameStatus;
use common::protocol::{ClientMessage, ErrorCode, ServerMessage};
use tokio::sync::mpsc;

use crate::app::state::{AppEvent, AppState};
use crate::domain::{ActiveMatch, AuthForm, AuthMode, PendingAction, Screen, SpectatedMatch};

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
            if let Screen::Lobby { matches, .. } = &state.screen
                && let Some(summary) = matches.get(index)
            {
                let match_id = summary.id;
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
            if let Screen::Lobby { matches, .. } = &state.screen
                && let Some(summary) = matches.get(index)
            {
                let match_id = summary.id;
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
        // Auth events are handled at the top of this function; listing them
        // here keeps the match exhaustive.
        AppEvent::AuthInput(_)
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

/// Applies the auth-screen events.
///
/// Returns `true` if the event was an auth event and was handled here.
fn apply_auth_event(state: &mut AppState, event: &AppEvent, effects: &mut Vec<SideEffect>) -> bool {
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

fn apply_server(state: &mut AppState, message: ServerMessage, effects: &mut Vec<SideEffect>) {
    if apply_list_message(state, &message) {
        return;
    }
    if apply_spectator_message(state, &message, effects) {
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
            apply_auth_failure(state, reason, &message);
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
            state.screen = Screen::Lobby {
                matches: Vec::new(),
                spectator_mode: false,
            };
            state.status = String::from("the match was abandoned");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
        }
        ServerMessage::OpponentLeft { .. } => {
            state.screen = Screen::Lobby {
                matches: Vec::new(),
                spectator_mode: false,
            };
            state.status = String::from("opponent left the match");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
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
        | ServerMessage::Pong => {}
    }
}

/// Applies `MatchList` and `Ranking`.
///
/// Returns `true` if the message was handled here. The main server match
/// calls this first so the two list-shaped messages stay together and do
/// not grow `apply_server` further.
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

/// Applies spectator-related messages.
///
/// Returns `true` when the message was handled here. The main server match
/// calls this before the general match so the spectator state machine stays
/// in one place.
fn apply_spectator_message(
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
            if let Screen::Spectating(active) = &mut state.screen {
                active.spectator_count = *spectator_count;
                state.status =
                    format!("{username} joined as spectator; {spectator_count} watching");
            }
            true
        }
        ServerMessage::SpectatorLeft {
            username,
            spectator_count,
        } => {
            if let Screen::Spectating(active) = &mut state.screen {
                active.spectator_count = *spectator_count;
                state.status = format!("{username} left the audience; {spectator_count} watching");
            }
            true
        }
        ServerMessage::BoardUpdate {
            board,
            current_turn,
            status,
        } => {
            if let Screen::Spectating(active) = &mut state.screen {
                active.board = *board;
                active.current_turn = *current_turn;
                active.status = *status;
            }
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
            // The match ended. Return to the lobby and surface the result
            // in the status line.
            let outcome = match status {
                GameStatus::Won(_) => winner_name
                    .clone()
                    .map_or_else(|| String::from("the match ended"), |name| format!("{name} won")),
                GameStatus::Draw => String::from("the match ended in a draw"),
                GameStatus::InProgress => String::from("the match ended"),
            };
            let _ = board;
            state.screen = Screen::Lobby {
                matches: Vec::new(),
                spectator_mode: false,
            };
            state.status = outcome;
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
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
            state.status = String::from("the match was abandoned");
            effects.push(SideEffect::Send(ClientMessage::ListMatches));
            true
        }
        _ => false,
    }
}

/// Applies an authentication failure: shows the error on the auth form when
/// the client is on that screen, otherwise on the status line.
fn apply_auth_failure(
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

fn show_auth(state: &mut AppState, mode: AuthMode, pending: Option<PendingAction>) {
    state.screen = Screen::Auth(Box::new(AuthForm::new(mode, pending)));
}

fn retry_pending(state: &mut AppState, effects: &mut Vec<SideEffect>) {
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
    use common::domain::{Age, Board, Username};
    use common::protocol::{ClientId, MatchId, MatchSummary};

    use super::*;

    fn apply(state: &mut AppState, event: AppEvent) -> Vec<SideEffect> {
        let mut effects = Vec::new();
        apply_event(state, event, &mut effects);
        effects
    }

    fn profile() -> common::domain::UserProfile {
        common::domain::UserProfile {
            name: String::from("Alice Example"),
            username: Username::new("alice_99").unwrap(),
            age: Age::new(30).unwrap(),
        }
    }

    fn lobby_screen() -> Screen {
        Screen::Lobby {
            matches: Vec::new(),
            spectator_mode: false,
        }
    }

    fn lobby_with_match(id: u64) -> Screen {
        Screen::Lobby {
            matches: vec![MatchSummary {
                id: MatchId::new(id),
                host: String::from("bob"),
                spectator_count: 0,
            }],
            spectator_mode: false,
        }
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
        state.screen = lobby_screen();
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::MatchList {
                matches: vec![MatchSummary {
                    id: MatchId::new(0),
                    host: String::from("bob"),
                    spectator_count: 0,
                }],
            }),
        );
        match &state.screen {
            Screen::Lobby { matches, .. } => assert_eq!(matches.len(), 1),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn join_at_selects_the_right_match() {
        let mut state = AppState::new("alice");
        state.authenticated_as = Some(Username::new("alice_99").unwrap());
        state.screen = lobby_with_match(7);
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
        state.screen = lobby_screen();
        let effects = apply(&mut state, AppEvent::JoinMatchAt(3));
        assert!(effects.is_empty());
    }

    #[test]
    fn create_match_without_auth_shows_auth_screen() {
        let mut state = AppState::new("alice");
        state.screen = lobby_screen();
        let effects = apply(&mut state, AppEvent::CreateMatch);
        assert!(effects.is_empty());
        match &state.screen {
            Screen::Auth(form) => {
                assert_eq!(form.mode, AuthMode::Login);
                assert_eq!(form.pending_action, Some(PendingAction::CreateMatch));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn join_match_without_auth_shows_auth_screen_with_pending() {
        let mut state = AppState::new("alice");
        state.screen = lobby_with_match(5);
        let effects = apply(&mut state, AppEvent::JoinMatchAt(0));
        assert!(effects.is_empty());
        match &state.screen {
            Screen::Auth(form) => {
                assert_eq!(
                    form.pending_action,
                    Some(PendingAction::JoinMatch(MatchId::new(5)))
                );
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn auth_input_appends_to_current_field() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        let _ = apply(&mut state, AppEvent::AuthInput('a'));
        let _ = apply(&mut state, AppEvent::AuthInput('b'));
        if let Screen::Auth(form) = &state.screen {
            assert_eq!(form.username, "ab");
        } else {
            panic!("expected auth screen");
        }
    }

    #[test]
    fn auth_submit_sends_login_message() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        if let Screen::Auth(form) = &mut state.screen {
            form.username = String::from("alice_99");
            form.password = String::from("hunter2hunter2");
        }
        let effects = apply(&mut state, AppEvent::AuthSubmit);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            SideEffect::Send(ClientMessage::Login { username, .. }) => {
                assert_eq!(username, "alice_99");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn auth_submit_with_missing_field_sets_error() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        let effects = apply(&mut state, AppEvent::AuthSubmit);
        assert!(effects.is_empty());
        if let Screen::Auth(form) = &state.screen {
            assert!(form.error.is_some());
        } else {
            panic!("expected auth screen");
        }
    }

    #[test]
    fn auth_tab_moves_focus() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        let _ = apply(&mut state, AppEvent::AuthNextField);
        if let Screen::Auth(form) = &state.screen {
            assert_eq!(form.focused, crate::domain::AuthField::Password);
        } else {
            panic!("expected auth screen");
        }
    }

    #[test]
    fn auth_cancel_returns_to_lobby() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        let effects = apply(&mut state, AppEvent::AuthCancel);
        assert!(matches!(state.screen, Screen::Lobby { .. }));
        assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListMatches)]);
    }

    #[test]
    fn auth_toggle_mode_switches_to_register() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        let _ = apply(&mut state, AppEvent::AuthToggleMode);
        if let Screen::Auth(form) = &state.screen {
            assert_eq!(form.mode, AuthMode::Register);
        } else {
            panic!("expected auth screen");
        }
    }

    #[test]
    fn registered_sets_authenticated_and_returns_to_lobby() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Register, None)));
        let effects = apply(
            &mut state,
            AppEvent::Server(ServerMessage::Registered { profile: profile() }),
        );
        assert!(state.is_authenticated());
        assert_eq!(state.display_name, "Alice Example");
        assert!(matches!(state.screen, Screen::Lobby { .. }));
        assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListMatches)]);
    }

    #[test]
    fn registered_with_pending_create_sends_create() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(
            AuthMode::Login,
            Some(PendingAction::CreateMatch),
        )));
        let effects = apply(
            &mut state,
            AppEvent::Server(ServerMessage::LoginSucceeded { profile: profile() }),
        );
        assert_eq!(
            effects,
            vec![
                SideEffect::Send(ClientMessage::ListMatches),
                SideEffect::Send(ClientMessage::CreateMatch),
            ]
        );
    }

    #[test]
    fn authentication_failed_sets_error_on_form() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::AuthenticationFailed {
                reason: common::protocol::AuthFailureReason::InvalidCredentials,
                message: String::from("bad"),
            }),
        );
        if let Screen::Auth(form) = &state.screen {
            assert!(form.error.is_some());
        } else {
            panic!("expected auth screen");
        }
    }

    #[test]
    fn already_logged_in_sets_error_on_form() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Auth(Box::new(AuthForm::new(AuthMode::Login, None)));
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::AuthenticationFailed {
                reason: common::protocol::AuthFailureReason::AlreadyLoggedIn,
                message: String::from("this account is already signed in elsewhere"),
            }),
        );
        if let Screen::Auth(form) = &state.screen {
            let error = form.error.as_deref().unwrap_or_default();
            assert!(error.contains("AlreadyLoggedIn"));
        } else {
            panic!("expected auth screen");
        }
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
    fn authentication_required_error_shows_auth_screen() {
        let mut state = AppState::new("alice");
        state.screen = lobby_screen();
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::Error {
                code: ErrorCode::AuthenticationRequired,
                message: String::from("register first"),
            }),
        );
        assert!(matches!(state.screen, Screen::Auth(_)));
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

    #[test]
    fn show_ranking_moves_to_ranking_and_requests_the_list() {
        let mut state = AppState::new("alice");
        state.screen = lobby_screen();
        let effects = apply(&mut state, AppEvent::ShowRanking);
        assert!(matches!(state.screen, Screen::Ranking { .. }));
        assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListRanking)]);
    }

    #[test]
    fn ranking_message_updates_the_ranking_screen() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Ranking {
            entries: Vec::new(),
        };
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::Ranking {
                entries: vec![common::domain::RankingEntry {
                    username: Username::new("alice_99").unwrap(),
                    name: String::from("Alice"),
                    wins: 3,
                }],
            }),
        );
        match &state.screen {
            Screen::Ranking { entries } => assert_eq!(entries.len(), 1),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn ranking_message_is_ignored_on_other_screens() {
        let mut state = AppState::new("alice");
        state.screen = lobby_screen();
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::Ranking { entries: vec![] }),
        );
        assert!(matches!(state.screen, Screen::Lobby { .. }));
    }

    #[test]
    fn back_to_lobby_returns_to_the_lobby_and_refreshes() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Ranking {
            entries: Vec::new(),
        };
        let effects = apply(&mut state, AppEvent::BackToLobby);
        assert!(matches!(state.screen, Screen::Lobby { .. }));
        assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListMatches)]);
    }

    #[test]
    fn match_over_sets_finished_with_winner_name() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::MatchOver {
                board: Board::new(),
                status: GameStatus::Won(common::domain::Player::X),
                winner_name: Some(String::from("Alice")),
            }),
        );
        match &state.screen {
            Screen::Finished {
                winner_name,
                status,
                ..
            } => {
                assert_eq!(*status, GameStatus::Won(common::domain::Player::X));
                assert_eq!(winner_name.as_deref(), Some("Alice"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn toggle_spectator_mode_switches_the_flag() {
        let mut state = AppState::new("alice");
        state.screen = lobby_screen();
        let _ = apply(&mut state, AppEvent::ToggleSpectatorMode);
        match &state.screen {
            Screen::Lobby { spectator_mode, .. } => assert!(*spectator_mode),
            other => panic!("unexpected: {other:?}"),
        }
        let _ = apply(&mut state, AppEvent::ToggleSpectatorMode);
        match &state.screen {
            Screen::Lobby { spectator_mode, .. } => assert!(!*spectator_mode),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn spectate_at_emits_a_spectate_message() {
        let mut state = AppState::new("alice");
        state.screen = lobby_with_match(3);
        let effects = apply(&mut state, AppEvent::SpectateAt(0));
        assert_eq!(
            effects,
            vec![SideEffect::Send(ClientMessage::Spectate {
                match_id: MatchId::new(3)
            })]
        );
    }

    #[test]
    fn leave_spectate_emits_the_message() {
        let mut state = AppState::new("alice");
        let effects = apply(&mut state, AppEvent::LeaveSpectate);
        assert_eq!(
            effects,
            vec![SideEffect::Send(ClientMessage::LeaveSpectate)]
        );
    }

    #[test]
    fn spectate_started_transitions_to_spectating() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectateStarted {
                match_id: MatchId::new(1),
                host_name: String::from("Alice"),
                guest_name: String::from("Bob"),
                board: Board::new(),
                current_turn: common::domain::Player::X,
                status: GameStatus::InProgress,
                spectator_count: 2,
            }),
        );
        match &state.screen {
            Screen::Spectating(active) => {
                assert_eq!(active.id, MatchId::new(1));
                assert_eq!(active.host_name, "Alice");
                assert_eq!(active.guest_name, "Bob");
                assert_eq!(active.spectator_count, 2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn spectator_joined_updates_the_count() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectateStarted {
                match_id: MatchId::new(1),
                host_name: String::from("Alice"),
                guest_name: String::from("Bob"),
                board: Board::new(),
                current_turn: common::domain::Player::X,
                status: GameStatus::InProgress,
                spectator_count: 1,
            }),
        );
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectatorJoined {
                username: String::from("Carol"),
                spectator_count: 2,
            }),
        );
        match &state.screen {
            Screen::Spectating(active) => assert_eq!(active.spectator_count, 2),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn spectator_left_updates_the_count() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectateStarted {
                match_id: MatchId::new(1),
                host_name: String::from("Alice"),
                guest_name: String::from("Bob"),
                board: Board::new(),
                current_turn: common::domain::Player::X,
                status: GameStatus::InProgress,
                spectator_count: 2,
            }),
        );
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectatorLeft {
                username: String::from("Carol"),
                spectator_count: 1,
            }),
        );
        match &state.screen {
            Screen::Spectating(active) => assert_eq!(active.spectator_count, 1),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn board_update_reaches_the_spectating_screen() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectateStarted {
                match_id: MatchId::new(1),
                host_name: String::from("Alice"),
                guest_name: String::from("Bob"),
                board: Board::new(),
                current_turn: common::domain::Player::X,
                status: GameStatus::InProgress,
                spectator_count: 1,
            }),
        );
        let mut board = Board::new();
        board
            .place(
                common::domain::Position::new(4).unwrap(),
                common::domain::Player::X,
            )
            .unwrap();
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::BoardUpdate {
                board,
                current_turn: common::domain::Player::O,
                status: GameStatus::InProgress,
            }),
        );
        match &state.screen {
            Screen::Spectating(active) => {
                assert_eq!(active.board, board);
                assert_eq!(active.current_turn, common::domain::Player::O);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn match_over_while_spectating_returns_to_the_lobby() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectateStarted {
                match_id: MatchId::new(1),
                host_name: String::from("Alice"),
                guest_name: String::from("Bob"),
                board: Board::new(),
                current_turn: common::domain::Player::X,
                status: GameStatus::InProgress,
                spectator_count: 1,
            }),
        );
        let effects = apply(
            &mut state,
            AppEvent::Server(ServerMessage::MatchOver {
                board: Board::new(),
                status: GameStatus::Won(common::domain::Player::X),
                winner_name: Some(String::from("Alice")),
            }),
        );
        assert!(matches!(state.screen, Screen::Lobby { .. }));
        assert!(state.status.contains("Alice"));
        assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListMatches)]);
    }

    #[test]
    fn match_abandoned_while_spectating_returns_to_the_lobby() {
        let mut state = AppState::new("alice");
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::SpectateStarted {
                match_id: MatchId::new(1),
                host_name: String::from("Alice"),
                guest_name: String::from("Bob"),
                board: Board::new(),
                current_turn: common::domain::Player::X,
                status: GameStatus::InProgress,
                spectator_count: 1,
            }),
        );
        let effects = apply(
            &mut state,
            AppEvent::Server(ServerMessage::MatchAbandoned {
                match_id: MatchId::new(1),
            }),
        );
        assert!(matches!(state.screen, Screen::Lobby { .. }));
        assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListMatches)]);
    }

    #[test]
    fn match_list_preserves_the_spectator_mode() {
        let mut state = AppState::new("alice");
        state.screen = Screen::Lobby {
            matches: Vec::new(),
            spectator_mode: true,
        };
        let _ = apply(
            &mut state,
            AppEvent::Server(ServerMessage::MatchList { matches: vec![] }),
        );
        match &state.screen {
            Screen::Lobby { spectator_mode, .. } => assert!(*spectator_mode),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
