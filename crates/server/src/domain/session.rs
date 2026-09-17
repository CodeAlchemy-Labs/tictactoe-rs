//! Connected-client session.

use common::domain::Player;
use common::protocol::{ClientId, MatchId, ServerMessage};
use tokio::sync::mpsc;

/// The server-side state associated with one connected WebSocket client.
///
/// A session owns the outbound channel used to push messages to that client.
/// When the session is removed from the lobby, the channel sender is dropped
/// and the writer task on the connection observes `None`, which terminates
/// the socket. This is what makes disconnection cleanup deterministic.
pub struct Session {
    /// The identifier assigned to this client.
    pub client_id: ClientId,
    /// The display name the client sent via `Hello`, if any.
    pub display_name: Option<String>,
    /// The outbound channel used to push messages to this client.
    pub sender: mpsc::UnboundedSender<ServerMessage>,
    /// The match the client is currently playing, if any.
    pub current_match: Option<MatchId>,
    /// The mark assigned to the client in its current match, if any.
    pub mark: Option<Player>,
}

impl Session {
    /// Creates a new session for the given client and outbound channel.
    pub fn new(client_id: ClientId, sender: mpsc::UnboundedSender<ServerMessage>) -> Self {
        Self {
            client_id,
            display_name: None,
            sender,
            current_match: None,
            mark: None,
        }
    }

    /// Sends a message to the client.
    ///
    /// The send is non-blocking. A failure means the client has already been
    /// disconnected and the receiving side of the channel was dropped; in
    /// that case the message is silently discarded, and the reader loop on
    /// the connection is expected to observe the closure shortly after.
    pub fn try_send(&self, message: ServerMessage) {
        let _ = self.sender.send(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_is_empty() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let session = Session::new(ClientId::new(1), tx);
        assert!(session.display_name.is_none());
        assert!(session.current_match.is_none());
        assert!(session.mark.is_none());
    }

    #[test]
    fn try_send_delivers_to_the_receiver() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let session = Session::new(ClientId::new(1), tx);
        session.try_send(ServerMessage::Pong);
        assert_eq!(rx.try_recv().unwrap(), ServerMessage::Pong);
    }

    #[test]
    fn try_send_is_silent_after_receiver_drops() {
        let (tx, rx) = mpsc::unbounded_channel();
        let session = Session::new(ClientId::new(1), tx);
        drop(rx);
        session.try_send(ServerMessage::Pong);
        // No panic: this is the contract that makes cleanup safe.
    }
}
