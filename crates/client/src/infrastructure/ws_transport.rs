//! WebSocket transport implementation.

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use common::protocol::{ClientMessage, ServerMessage};

use crate::infrastructure::transport::{Transport, TransportHandle};

/// A WebSocket transport that connects to a `ws://` URL.
pub struct WsTransport {
    url: String,
}

impl WsTransport {
    /// Creates a transport that will connect to `url` when started.
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

impl Transport for WsTransport {
    fn start(self) -> TransportHandle {
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<ClientMessage>();
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel::<ServerMessage>();

        tokio::spawn(async move {
            let url = self.url;
            let connect_result = connect_async(&url).await;
            let (socket, _response) = match connect_result {
                Ok(pair) => pair,
                Err(error) => {
                    tracing::error!(%error, url = %url, "websocket connect failed");
                    return;
                }
            };
            let (mut sink, mut stream) = socket.split();

            let writer = tokio::spawn(async move {
                while let Some(message) = outgoing_rx.recv().await {
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

            while let Some(frame) = stream.next().await {
                match frame {
                    Ok(Message::Text(text)) => match serde_json::from_str::<ServerMessage>(&text) {
                        Ok(message) => {
                            if incoming_tx.send(message).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::warn!(%error, "malformed server message");
                        }
                    },
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(_) | Message::Pong(_) | Message::Binary(_)) => {}
                    Err(error) => {
                        tracing::warn!(%error, "websocket receive error");
                        break;
                    }
                }
            }

            // The incoming sender drops here; the UI observes `None` on its
            // receiver and knows the connection is gone.
            drop(incoming_tx);
            let _ = writer.await;
        });

        TransportHandle {
            outgoing: outgoing_tx,
            incoming: incoming_rx,
        }
    }
}
