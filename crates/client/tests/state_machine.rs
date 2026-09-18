//! Integration tests for the client state machine driven by a mock transport.

use client::app::AppState;
use client::app::state::AppEvent;
use client::app::update::{SideEffect, apply_event};
use client::domain::{AuthMode, Screen};
use client::infrastructure::{MockTransport, Transport};
use common::domain::{Age, Board, Player, UserProfile, Username};
use common::protocol::{ClientId, ClientMessage, MatchId, ServerMessage};

fn sample_profile() -> UserProfile {
    UserProfile {
        name: String::from("Alice Example"),
        username: Username::new("alice_99").unwrap(),
        age: Age::new(30).unwrap(),
    }
}

#[tokio::test]
async fn full_happy_path_over_the_mock_transport() {
    let (transport, mut server_rx, server_tx) = MockTransport::new();
    let handle = transport.start();
    let outgoing = handle.outgoing;
    let mut incoming = handle.incoming;

    let mut state = AppState::new("alice");

    // The main loop always starts by sending Hello.
    outgoing
        .send(ClientMessage::Hello {
            display_name: String::from("alice"),
        })
        .unwrap();

    let sent = server_rx.recv().await.unwrap();
    assert!(matches!(sent, ClientMessage::Hello { .. }));

    // Server responds with Welcome; the client should ask for the list.
    server_tx
        .send(ServerMessage::Welcome {
            client_id: ClientId::new(1),
            display_name: String::from("alice"),
        })
        .unwrap();
    let message = incoming.recv().await.unwrap();
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::Server(message), &mut effects);
    assert_eq!(effects, vec![SideEffect::Send(ClientMessage::ListMatches)]);
    assert!(matches!(state.screen, Screen::Lobby { .. }));
    assert!(!state.is_authenticated());

    // The user registers so they can create a match later.
    let mut effects = Vec::new();
    apply_event(
        &mut state,
        AppEvent::ShowAuth {
            mode: AuthMode::Register,
            pending: None,
        },
        &mut effects,
    );
    if let Screen::Auth(form) = &mut state.screen {
        form.name = String::from("Alice Example");
        form.username = String::from("alice_99");
        form.age = String::from("30");
        form.password = String::from("hunter2hunter2");
    }
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::AuthSubmit, &mut effects);
    assert_eq!(effects.len(), 1);

    let sent = server_rx.recv().await.unwrap();
    assert!(matches!(sent, ClientMessage::Register { .. }));

    server_tx
        .send(ServerMessage::Registered {
            profile: sample_profile(),
        })
        .unwrap();
    let message = incoming.recv().await.unwrap();
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::Server(message), &mut effects);
    assert!(state.is_authenticated());
    assert_eq!(state.display_name, "Alice Example");
    assert!(matches!(state.screen, Screen::Lobby { .. }));

    // Drain the ListMatches that the client sent after registration.
    let sent = server_rx.recv().await.unwrap();
    assert!(matches!(sent, ClientMessage::ListMatches));

    // Server sends the match list.
    server_tx
        .send(ServerMessage::MatchList { matches: vec![] })
        .unwrap();
    let message = incoming.recv().await.unwrap();
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::Server(message), &mut effects);

    // Now the user can create a match.
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::CreateMatch, &mut effects);
    assert_eq!(effects, vec![SideEffect::Send(ClientMessage::CreateMatch)]);

    // Server confirms, then pairs the players.
    server_tx
        .send(ServerMessage::MatchCreated {
            match_id: MatchId::new(0),
        })
        .unwrap();
    let message = incoming.recv().await.unwrap();
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::Server(message), &mut effects);

    server_tx
        .send(ServerMessage::MatchReady {
            match_id: MatchId::new(0),
            opponent: String::from("bob"),
            your_mark: Player::X,
            board: Board::new(),
            current_turn: Player::X,
        })
        .unwrap();
    let message = incoming.recv().await.unwrap();
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::Server(message), &mut effects);
    assert!(matches!(state.screen, Screen::InGame(_)));

    // The user plays a move; verify the outgoing message is well formed.
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::PlayMove(5), &mut effects);
    assert_eq!(effects.len(), 1);

    // Server reports the final board; the state transitions to Finished.
    server_tx
        .send(ServerMessage::MatchOver {
            board: Board::new(),
            status: common::domain::GameStatus::Draw,
        })
        .unwrap();
    let message = incoming.recv().await.unwrap();
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::Server(message), &mut effects);
    assert!(matches!(state.screen, Screen::Finished { .. }));
}

#[tokio::test]
async fn server_disconnect_marks_state_as_fatal() {
    let (transport, _server_rx, server_tx) = MockTransport::new();
    let handle = transport.start();
    let mut incoming = handle.incoming;

    let mut state = AppState::new("alice");
    drop(server_tx);
    assert!(incoming.recv().await.is_none());
    let mut effects = Vec::new();
    apply_event(&mut state, AppEvent::Disconnected, &mut effects);
    assert!(matches!(state.screen, Screen::Fatal(_)));
    assert!(state.should_quit);
}
