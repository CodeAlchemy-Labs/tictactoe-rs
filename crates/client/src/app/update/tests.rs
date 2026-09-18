//! Unit tests for the update function.
//!
//! The state machine is driven entirely through `apply_event`. Each test
//! inspects the resulting state and, when relevant, the list of side
//! effects produced.

use common::domain::{Age, Board, GameStatus, Username};
use common::protocol::{ClientId, ClientMessage, ErrorCode, MatchId, MatchSummary, ServerMessage};

use super::{SideEffect, apply_event};
use crate::app::state::{AppEvent, AppState};
use crate::domain::screen::ActiveMatch;
use crate::domain::{AuthForm, AuthMode, PendingAction, Screen};

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
            is_full: false,
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
                is_full: false,
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

#[test]
fn join_at_ignores_full_matches() {
    let mut state = AppState::new("alice");
    state.authenticated_as = Some(Username::new("alice_99").unwrap());
    state.screen = Screen::Lobby {
        matches: vec![MatchSummary {
            id: MatchId::new(7),
            host: String::from("bob"),
            spectator_count: 0,
            is_full: true,
        }],
        spectator_mode: false,
    };
    let effects = apply(&mut state, AppEvent::JoinMatchAt(0));
    assert!(effects.is_empty());
}

#[test]
fn spectate_at_reaches_full_matches() {
    let mut state = AppState::new("alice");
    state.screen = Screen::Lobby {
        matches: vec![MatchSummary {
            id: MatchId::new(3),
            host: String::from("bob"),
            spectator_count: 0,
            is_full: true,
        }],
        spectator_mode: true,
    };
    let effects = apply(&mut state, AppEvent::SpectateAt(0));
    assert_eq!(
        effects,
        vec![SideEffect::Send(ClientMessage::Spectate {
            match_id: MatchId::new(3)
        })]
    );
}

#[test]
fn board_update_reaches_the_in_game_screen() {
    use common::domain::Player;
    use common::domain::Position;

    let mut state = AppState::new("alice");
    state.screen = Screen::InGame(Box::new(ActiveMatch {
        id: MatchId::new(1),
        opponent: String::from("Bob"),
        your_mark: Player::X,
        board: Board::new(),
        current_turn: Player::X,
        status: GameStatus::InProgress,
    }));
    let mut board = Board::new();
    board.place(Position::new(4).unwrap(), Player::X).unwrap();
    let _ = apply(
        &mut state,
        AppEvent::Server(ServerMessage::BoardUpdate {
            board,
            current_turn: Player::O,
            status: GameStatus::InProgress,
        }),
    );
    match &state.screen {
        Screen::InGame(active) => {
            assert_eq!(active.board, board);
            assert_eq!(active.current_turn, Player::O);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn redraw_event_is_a_noop() {
    let mut state = AppState::new("alice");
    let effects = apply(&mut state, AppEvent::Redraw);
    assert!(effects.is_empty());
}

#[test]
fn opponent_disconnected_updates_the_status_during_a_game() {
    let mut state = AppState::new("alice");
    state.screen = Screen::InGame(Box::new(ActiveMatch {
        id: MatchId::new(1),
        opponent: String::from("Bob"),
        your_mark: common::domain::Player::X,
        board: Board::new(),
        current_turn: common::domain::Player::X,
        status: GameStatus::InProgress,
    }));
    let _ = apply(
        &mut state,
        AppEvent::Server(ServerMessage::OpponentDisconnected {
            match_id: MatchId::new(1),
            grace_seconds: 2,
        }),
    );
    assert!(state.status.contains("disconnected"));
    assert!(matches!(state.screen, Screen::InGame(_)));
}

#[test]
fn opponent_reconnected_updates_the_status_during_a_game() {
    let mut state = AppState::new("alice");
    state.screen = Screen::InGame(Box::new(ActiveMatch {
        id: MatchId::new(1),
        opponent: String::from("Bob"),
        your_mark: common::domain::Player::X,
        board: Board::new(),
        current_turn: common::domain::Player::X,
        status: GameStatus::InProgress,
    }));
    let _ = apply(
        &mut state,
        AppEvent::Server(ServerMessage::OpponentReconnected {
            match_id: MatchId::new(1),
        }),
    );
    assert!(state.status.contains("reconnected"));
    assert!(matches!(state.screen, Screen::InGame(_)));
}

#[test]
fn match_over_while_spectating_shows_the_finished_screen() {
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
    assert!(effects.is_empty());
    match &state.screen {
        Screen::Finished { winner_name, .. } => {
            assert_eq!(winner_name.as_deref(), Some("Alice"));
        }
        other => panic!("expected Finished, got {other:?}"),
    }
}

#[test]
fn opponent_disconnected_updates_the_status_for_spectators() {
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
        AppEvent::Server(ServerMessage::OpponentDisconnected {
            match_id: MatchId::new(1),
            grace_seconds: 2,
        }),
    );
    assert!(state.status.contains("disconnected"));
    assert!(matches!(state.screen, Screen::Spectating(_)));
}
