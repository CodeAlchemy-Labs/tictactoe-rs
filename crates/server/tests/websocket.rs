//! End-to-end WebSocket tests.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use common::protocol::{AuthFailureReason, ClientMessage, ErrorCode, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use server::application::lobby::LobbyService;
use server::infrastructure::http::build_router;
use tokio::net::TcpListener;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn spawn_server() -> (SocketAddr, Arc<LobbyService>) {
    let lobby = Arc::new(LobbyService::new());
    let app = build_router(Arc::clone(&lobby));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
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
