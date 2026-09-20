//! End-to-end WebSocket tests.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use common::protocol::{AuthFailureReason, ClientMessage, ErrorCode, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use server::application::lobby::LobbyService;
use server::infrastructure::http::build_router;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;

type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn spawn_server() -> (SocketAddr, Arc<LobbyService>) {
    let lobby = Arc::new(LobbyService::default());
    let app = build_router(Arc::clone(&lobby));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    (addr, lobby)
}

async fn connect(addr: SocketAddr) -> Client {
    let url = format!("ws://{addr}/ws");
    let (socket, _response) = connect_async(url).await.unwrap();
    socket
}

async fn send(client: &mut Client, message: &ClientMessage) {
    let payload = serde_json::to_string(message).unwrap();
    client.send(Message::Text(payload.into())).await.unwrap();
}

async fn recv(client: &mut Client) -> ServerMessage {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), client.next())
            .await
            .expect("timeout waiting for a frame")
            .expect("stream ended")
            .expect("transport error");
        if let Message::Text(text) = frame {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn register(client: &mut Client, username: &str) {
    send(
        client,
        &ClientMessage::Register {
            name: format!("{username} name"),
            username: username.to_string(),
            age: 30,
            password: String::from("hunter2hunter2"),
        },
    )
    .await;
    let _ = recv(client).await;
}

#[tokio::test]
async fn hello_returns_welcome_with_display_name() {
    let (addr, _) = spawn_server().await;
    let mut client = connect(addr).await;
    send(
        &mut client,
        &ClientMessage::Hello {
            display_name: String::from("alice"),
        },
    )
    .await;
    match recv(&mut client).await {
        ServerMessage::Welcome { display_name, .. } => assert_eq!(display_name, "alice"),
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn joining_a_nonexistent_match_fails() {
    let (addr, _) = spawn_server().await;
    let mut client = connect(addr).await;
    register(&mut client, "alice_99").await;
    send(
        &mut client,
        &ClientMessage::JoinMatch {
            match_id: common::protocol::MatchId::new(999),
        },
    )
    .await;
    match recv(&mut client).await {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::MatchNotFound),
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn guest_cannot_create_a_match() {
    let (addr, _) = spawn_server().await;
    let mut client = connect(addr).await;
    send(
        &mut client,
        &ClientMessage::Hello {
            display_name: String::from("guest"),
        },
    )
    .await;
    let _ = recv(&mut client).await;
    send(&mut client, &ClientMessage::CreateMatch).await;
    match recv(&mut client).await {
        ServerMessage::Error { code, .. } => {
            assert_eq!(code, ErrorCode::AuthenticationRequired);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn disconnect_removes_the_session_from_the_lobby() {
    let (addr, lobby) = spawn_server().await;
    let mut client = connect(addr).await;
    send(
        &mut client,
        &ClientMessage::Hello {
            display_name: String::from("alice"),
        },
    )
    .await;
    let _ = recv(&mut client).await;
    assert_eq!(lobby.session_count(), 1);

    client.close(None).await.unwrap();
    drop(client);

    for _ in 0..50 {
        if lobby.session_count() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(lobby.session_count(), 0);
}

#[tokio::test]
async fn registration_succeeds_and_authenticates_the_connection() {
    let (addr, _) = spawn_server().await;
    let mut client = connect(addr).await;
    send(
        &mut client,
        &ClientMessage::Register {
            name: String::from("Alice Example"),
            username: String::from("alice_99"),
            age: 30,
            password: String::from("hunter2hunter2"),
        },
    )
    .await;
    match recv(&mut client).await {
        ServerMessage::Registered { profile } => {
            assert_eq!(profile.name, "Alice Example");
            assert_eq!(profile.username.as_str(), "alice_99");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn registration_with_invalid_input_is_rejected() {
    let (addr, _) = spawn_server().await;
    let mut client = connect(addr).await;
    send(
        &mut client,
        &ClientMessage::Register {
            name: String::from("Alice Example"),
            username: String::from("a!"),
            age: 30,
            password: String::from("hunter2hunter2"),
        },
    )
    .await;
    match recv(&mut client).await {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::UsernameInvalid);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn login_after_registration_succeeds_on_a_new_connection() {
    let (addr, _) = spawn_server().await;

    let mut register_client = connect(addr).await;
    send(
        &mut register_client,
        &ClientMessage::Register {
            name: String::from("Alice Example"),
            username: String::from("alice_99"),
            age: 30,
            password: String::from("hunter2hunter2"),
        },
    )
    .await;
    let _ = recv(&mut register_client).await;
    register_client.close(None).await.unwrap();
    drop(register_client);

    let mut login_client = connect(addr).await;
    send(
        &mut login_client,
        &ClientMessage::Login {
            username: String::from("alice_99"),
            password: String::from("hunter2hunter2"),
        },
    )
    .await;
    match recv(&mut login_client).await {
        ServerMessage::LoginSucceeded { profile } => {
            assert_eq!(profile.username.as_str(), "alice_99");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn second_login_with_the_same_account_is_rejected() {
    let (addr, _) = spawn_server().await;

    // First connection: register and keep it open.
    let mut first = connect(addr).await;
    register(&mut first, "alice_99").await;

    // Second connection: try to log in with the same credentials.
    let mut second = connect(addr).await;
    send(
        &mut second,
        &ClientMessage::Login {
            username: String::from("alice_99"),
            password: String::from("hunter2hunter2"),
        },
    )
    .await;
    match recv(&mut second).await {
        ServerMessage::AuthenticationFailed { reason, .. } => {
            assert_eq!(reason, AuthFailureReason::AlreadyLoggedIn);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn login_after_the_first_session_disconnects_succeeds() {
    let (addr, _) = spawn_server().await;

    // First connection: register, then close.
    let mut first = connect(addr).await;
    register(&mut first, "alice_99").await;
    first.close(None).await.unwrap();
    drop(first);

    // Give the server a moment to run the SessionGuard.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second connection: log in. Must succeed.
    let mut second = connect(addr).await;
    send(
        &mut second,
        &ClientMessage::Login {
            username: String::from("alice_99"),
            password: String::from("hunter2hunter2"),
        },
    )
    .await;
    match recv(&mut second).await {
        ServerMessage::LoginSucceeded { profile } => {
            assert_eq!(profile.username.as_str(), "alice_99");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn spectator_receives_the_snapshot_and_board_updates() {
    let (addr, _) = spawn_server().await;
    let mut host = connect(addr).await;
    let mut guest = connect(addr).await;
    let mut spec = connect(addr).await;

    register(&mut host, "host_99").await;
    register(&mut guest, "guest_99").await;
    register(&mut spec, "watch_99").await;

    send(&mut host, &ClientMessage::CreateMatch).await;
    let match_id = match recv(&mut host).await {
        ServerMessage::MatchCreated { match_id } => match_id,
        other => panic!("unexpected: {other:?}"),
    };
    send(&mut guest, &ClientMessage::JoinMatch { match_id }).await;
    let _ = recv(&mut host).await; // MatchReady
    let _ = recv(&mut guest).await; // MatchReady

    send(&mut spec, &ClientMessage::Spectate { match_id }).await;
    match recv(&mut spec).await {
        ServerMessage::SpectateStarted {
            match_id: got_id,
            host_name,
            guest_name,
            spectator_count,
            ..
        } => {
            assert_eq!(got_id, match_id);
            assert_eq!(host_name, "host_99 name");
            assert_eq!(guest_name, "guest_99 name");
            assert_eq!(spectator_count, 1);
        }
        other => panic!("unexpected: {other:?}"),
    }
    // The players were notified about the new spectator.
    let _ = recv(&mut host).await; // SpectatorJoined
    let _ = recv(&mut guest).await; // SpectatorJoined

    // The host plays a move. All three should receive a BoardUpdate.
    send(
        &mut host,
        &ClientMessage::MakeMove {
            position: common::domain::Position::new(0).unwrap(),
        },
    )
    .await;

    assert!(matches!(
        recv(&mut host).await,
        ServerMessage::BoardUpdate { .. }
    ));
    assert!(matches!(
        recv(&mut guest).await,
        ServerMessage::BoardUpdate { .. }
    ));
    assert!(matches!(
        recv(&mut spec).await,
        ServerMessage::BoardUpdate { .. }
    ));
}

#[tokio::test]
async fn spectator_cannot_act_on_the_match() {
    let (addr, _) = spawn_server().await;
    let mut host = connect(addr).await;
    let mut guest = connect(addr).await;
    let mut spec = connect(addr).await;

    register(&mut host, "host_99").await;
    register(&mut guest, "guest_99").await;
    register(&mut spec, "watch_99").await;

    send(&mut host, &ClientMessage::CreateMatch).await;
    let match_id = match recv(&mut host).await {
        ServerMessage::MatchCreated { match_id } => match_id,
        other => panic!("unexpected: {other:?}"),
    };
    send(&mut guest, &ClientMessage::JoinMatch { match_id }).await;
    let _ = recv(&mut host).await;
    let _ = recv(&mut guest).await;

    send(&mut spec, &ClientMessage::Spectate { match_id }).await;
    let _ = recv(&mut spec).await;
    let _ = recv(&mut host).await;
    let _ = recv(&mut guest).await;

    // Probe 1: try to make a move from the spectator. The server must
    // reject it because the spectator is not a participant of the match.
    send(
        &mut spec,
        &ClientMessage::MakeMove {
            position: common::domain::Position::new(4).unwrap(),
        },
    )
    .await;
    match recv(&mut spec).await {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::NotInMatch),
        other => panic!("unexpected: {other:?}"),
    }

    // Probe 2: try to join the match as a player while spectating it.
    send(&mut spec, &ClientMessage::JoinMatch { match_id }).await;
    match recv(&mut spec).await {
        ServerMessage::Error { code, .. } => assert_eq!(code, ErrorCode::AlreadySpectating),
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn spectator_can_spectate_another_match_after_the_first_one_finishes() {
    let (addr, _) = spawn_server().await;
    let mut host = connect(addr).await;
    let mut guest = connect(addr).await;
    let mut spec = connect(addr).await;

    register(&mut host, "host_99").await;
    register(&mut guest, "guest_99").await;
    register(&mut spec, "watch_99").await;

    // First match: host wins with 0,1,2.
    send(&mut host, &ClientMessage::CreateMatch).await;
    let match_id_1 = match recv(&mut host).await {
        ServerMessage::MatchCreated { match_id } => match_id,
        other => panic!("unexpected: {other:?}"),
    };
    send(
        &mut guest,
        &ClientMessage::JoinMatch {
            match_id: match_id_1,
        },
    )
    .await;
    let _ = recv(&mut host).await;
    let _ = recv(&mut guest).await;

    send(
        &mut spec,
        &ClientMessage::Spectate {
            match_id: match_id_1,
        },
    )
    .await;
    let _ = recv(&mut spec).await; // SpectateStarted
    let _ = recv(&mut host).await; // SpectatorJoined
    let _ = recv(&mut guest).await; // SpectatorJoined

    // First 4 moves: wait for BoardUpdate to synchronize.
    for (is_host, pos) in [(true, 0u8), (false, 3), (true, 1), (false, 4)] {
        let client = if is_host { &mut host } else { &mut guest };

        send(
            client,
            &ClientMessage::MakeMove {
                position: common::domain::Position::new(pos).unwrap(),
            },
        )
        .await;

        assert!(matches!(
            recv(&mut host).await,
            ServerMessage::BoardUpdate { .. }
        ));
        assert!(matches!(
            recv(&mut guest).await,
            ServerMessage::BoardUpdate { .. }
        ));
        assert!(matches!(
            recv(&mut spec).await,
            ServerMessage::BoardUpdate { .. }
        ));
    }

    // Final move that wins the game.
    send(
        &mut host,
        &ClientMessage::MakeMove {
            position: common::domain::Position::new(2).unwrap(),
        },
    )
    .await;

    // Drain every message until MatchOver on all three connections.
    for client in [&mut host, &mut guest, &mut spec] {
        loop {
            if matches!(recv(client).await, ServerMessage::MatchOver { .. }) {
                break;
            }
        }
    }

    // The spectator dismisses the result screen.
    send(&mut spec, &ClientMessage::LeaveSpectate).await;

    // The same host creates a second match.
    send(&mut host, &ClientMessage::CreateMatch).await;
    let match_id_2 = match recv(&mut host).await {
        ServerMessage::MatchCreated { match_id } => match_id,
        other => panic!("unexpected: {other:?}"),
    };

    // The spectator spectates the new match.
    send(
        &mut spec,
        &ClientMessage::Spectate {
            match_id: match_id_2,
        },
    )
    .await;

    match recv(&mut spec).await {
        ServerMessage::SpectateStarted { match_id, .. } => assert_eq!(match_id, match_id_2),
        other => panic!("unexpected: {other:?}"),
    }
}
