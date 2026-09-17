//! End-to-end WebSocket tests.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use common::protocol::{ClientMessage, ErrorCode, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use server::application::lobby::LobbyService;
use server::infrastructure::http::build_router;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

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
    send(
        &mut client,
        &ClientMessage::Hello {
            display_name: String::from("alice"),
        },
    )
        .await;
    let _ = recv(&mut client).await;
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

    // Wait until the guard has run. The cleanup is synchronous on the server
    // side, so we only need to give the task a chance to be scheduled.
    for _ in 0..50 {
        if lobby.session_count() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(lobby.session_count(), 0);
}