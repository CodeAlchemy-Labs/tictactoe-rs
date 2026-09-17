//! WebSocket upgrade handler and per-connection loop.

use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use common::protocol::{ClientMessage, ErrorCode, ServerMessage};

use crate::application::lobby::LobbyService;
use crate::infrastructure::session_guard::SessionGuard;

/// Axum handler that upgrades an HTTP request to a WebSocket connection.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(lobby): State<Arc<LobbyService>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, lobby))
}

async fn handle_socket(socket: WebSocket, lobby: Arc<LobbyService>) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    let client_id = lobby.register(tx);

    // The guard is created right after registration and dropped when this
    // function returns. That guarantees the session is removed even if the
    // reader loop exits early, if the writer task panics, or if the
    // connection is closed by the peer.
    let _guard = SessionGuard::new(client_id, Arc::clone(&lobby));
    tracing::info!(client_id = %client_id, "client connected");

    // Writer task: forwards `ServerMessage`s to the socket.
    let writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let payload = match serde_json::to_string(&message) {
                Ok(json) => json,
                Err(error) => {
                    tracing::error!(%error, "failed to serialize outgoing message");
                    continue;
                }
            };
            if sink.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    // Reader loop.
    while let Some(frame) = stream.next().await {
        match frame {
            Ok(Message::Text(text)) => {
                dispatch_text(&lobby, client_id, &text);
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_) | Message::Pong(_) | Message::Binary(_)) => {}
            Err(error) => {
                tracing::warn!(%error, "websocket receive error");
                break;
            }
        }
    }

    tracing::info!(client_id = %client_id, "client disconnected");
    // The guard drops here, invoking `lobby.disconnect(client_id)`, which
    // removes the session and drops the sender. The writer task observes the
    // closed channel and exits, closing the socket. There is no window
    // during which the session is still observable by other clients.
    let _ = writer.await;
}

fn dispatch_text(lobby: &LobbyService, client_id: common::protocol::ClientId, text: &str) {
    let message = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => message,
        Err(error) => {
            tracing::warn!(%error, "malformed client message");
            return;
        }
    };
    match message {
        ClientMessage::Hello { display_name } => lobby.hello(client_id, display_name),
        ClientMessage::ListMatches => lobby.list_matches(client_id),
        ClientMessage::CreateMatch => lobby.create_match(client_id),
        ClientMessage::JoinMatch { match_id } => lobby.join_match(client_id, match_id),
        ClientMessage::MakeMove { position } => lobby.make_move(client_id, position),
        ClientMessage::LeaveMatch => lobby.leave_match(client_id),
        ClientMessage::Ping => lobby.pong(client_id),
    }
}