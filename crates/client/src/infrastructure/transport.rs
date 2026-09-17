//! Transport abstraction and its in-memory mock.
//!
//! Every network concern in the client is hidden behind [`Transport`]. The
//! production implementation ([`WsTransport`](crate::infrastructure::ws_transport::WsTransport))
//! speaks WebSocket; the [`MockTransport`] used in tests exchanges messages
//! over in-memory channels. The rest of the client has no idea which one is
//! in use, which makes the state machine and the UI fully testable without
//! binding a socket.

use common::protocol::{ClientMessage, ServerMessage};
use thiserror::Error;
use tokio::sync::mpsc;

/// Errors that can surface from a transport.
#[derive(Debug, Error)]
pub enum TransportError {
    /// The underlying WebSocket reported an error.
    #[error("websocket error: {0}")]
    WebSocket(String),
    /// The transport is not connected.
    #[error("transport is not connected")]
    Disconnected,
}

/// The pair of channels every transport exposes once it has started.
pub struct TransportHandle {
    /// Sends messages to the server.
    pub outgoing: mpsc::UnboundedSender<ClientMessage>,
    /// Receives messages from the server.
    pub incoming: mpsc::UnboundedReceiver<ServerMessage>,
}

/// A transport between the client and the server.
pub trait Transport: Send + 'static {
    /// Consumes the transport and returns the handle the UI interacts with.
    fn start(self) -> TransportHandle;
}

/// An in-memory transport used in tests.
///
/// The mock exposes the server side of both channels, so a test can inspect
/// what the client sent and inject messages as if the server had sent them.
pub struct MockTransport {
    outgoing_tx: mpsc::UnboundedSender<ClientMessage>,
    incoming_rx: mpsc::UnboundedReceiver<ServerMessage>,
}

impl MockTransport {
    /// Creates a new mock transport plus the server-side endpoints.
    ///
    /// Returns `(transport, server_rx, server_tx)`. The test uses `server_rx`
    /// to inspect what the client sent, and `server_tx` to inject messages
    /// toward the client.
    pub fn new() -> (
        Self,
        mpsc::UnboundedReceiver<ClientMessage>,
        mpsc::UnboundedSender<ServerMessage>,
    ) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        let transport = Self {
            outgoing_tx: client_tx,
            incoming_rx: client_rx,
        };
        (transport, server_rx, server_tx)
    }
}

impl Transport for MockTransport {
    fn start(self) -> TransportHandle {
        TransportHandle {
            outgoing: self.outgoing_tx,
            incoming: self.incoming_rx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_transport_forwards_messages_both_ways() {
        let (transport, mut server_rx, server_tx) = MockTransport::new();
        let handle = transport.start();

        handle
            .outgoing
            .send(ClientMessage::Ping)
            .expect("send to mock should succeed");
        assert_eq!(server_rx.recv().await.unwrap(), ClientMessage::Ping);

        server_tx.send(ServerMessage::Pong).unwrap();
        assert_eq!(handle.incoming.recv().await.unwrap(), ServerMessage::Pong);
    }

    #[tokio::test]
    async fn dropping_server_sender_closes_incoming_channel() {
        let (transport, _server_rx, server_tx) = MockTransport::new();
        let mut handle = transport.start();
        drop(server_tx);
        assert!(handle.incoming.recv().await.is_none());
    }
}
