//! Connected-client session.

use common::domain::{Player, Username};
use common::protocol::{ClientId, MatchId, ServerMessage};
use tokio::sync::mpsc;

/// The server-side state associated with one connected WebSocket client.
///
/// A session owns the outbound channel used to push messages to that client.
/// When the session is removed from the lobby, the channel sender is dropped
/// and the writer task on the connection observes `None`, which terminates
/// the socket. This is what makes disconnection cleanup deterministic.
///
/// A session can be in exactly one of three states with respect to matches:
///
/// - **Idle**: `current_match` and `spectating` are both `None`.
/// - **Playing**: `current_match` is `Some(_)`. The session is a player.
/// - **Spectating**: `spectating` is `Some(_)`. The session observes a
///   match without playing it.
///
/// The two are mutually exclusive: a client cannot play and spectate at the
/// same time, and cannot spectate two matches at once. The lobby enforces
/// these invariants.
pub struct Session {
    /// The identifier assigned to this client.
    pub client_id: ClientId,
    /// The display name the client sent via `Hello` or the one derived from
    /// the authenticated user's profile.
    pub display_name: Option<String>,
    /// The username of the authenticated user, if any.
    ///
    /// Guest sessions leave this as `None`. Authenticated sessions set it
    /// after `Register` or `Login` succeeds.
    pub authenticated_as: Option<Username>,
    /// The outbound channel used to push messages to this client.
    pub sender: mpsc::UnboundedSender<ServerMessage>,
    /// The match the client is currently playing, if any.
    pub current_match: Option<MatchId>,
    /// The mark assigned to the client in its current match, if any.
    pub mark: Option<Player>,
    /// The match the client is currently spectating, if any.
    pub spectating: Option<MatchId>,
    /// The peer IP address of the connection.
    pub peer_ip: std::net::IpAddr,
}

impl Session {
    /// Creates a new session for the given client and outbound channel.
    #[must_use]
    pub const fn new(
        client_id: ClientId,
        sender: mpsc::UnboundedSender<ServerMessage>,
        peer_ip: std::net::IpAddr,
    ) -> Self {
        Self {
            client_id,
            display_name: None,
            authenticated_as: None,
            sender,
            current_match: None,
            mark: None,
            spectating: None,
            peer_ip,
        }
    }

    /// Returns `true` when the session belongs to an authenticated user.
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        self.authenticated_as.is_some()
    }

    /// Returns `true` when the client is currently playing a match.
    #[must_use]
    pub const fn is_playing(&self) -> bool {
        self.current_match.is_some()
    }

    /// Returns `true` when the client is currently spectating a match.
    #[must_use]
    pub const fn is_spectating(&self) -> bool {
        self.spectating.is_some()
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
    use std::net::{IpAddr, Ipv4Addr};

    fn dummy_ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    }

    #[test]
    fn new_session_is_empty() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let session = Session::new(ClientId::new(1), tx, dummy_ip());
        assert!(session.display_name.is_none());
        assert!(session.authenticated_as.is_none());
        assert!(session.current_match.is_none());
        assert!(session.mark.is_none());
        assert!(session.spectating.is_none());
        assert!(!session.is_authenticated());
        assert!(!session.is_playing());
        assert!(!session.is_spectating());
    }

    #[test]
    fn try_send_delivers_to_the_receiver() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let session = Session::new(ClientId::new(1), tx, dummy_ip());
        session.try_send(ServerMessage::Pong);
        assert_eq!(rx.try_recv().unwrap(), ServerMessage::Pong);
    }

    #[test]
    fn try_send_is_silent_after_receiver_drops() {
        let (tx, rx) = mpsc::unbounded_channel();
        let session = Session::new(ClientId::new(1), tx, dummy_ip());
        drop(rx);
        session.try_send(ServerMessage::Pong);
        // No panic: this is the contract that makes cleanup safe.
    }

    #[test]
    fn authenticated_session_reports_it() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut session = Session::new(ClientId::new(1), tx, dummy_ip());
        session.authenticated_as = Some(Username::new("alice_99").unwrap());
        assert!(session.is_authenticated());
    }

    #[test]
    fn playing_and_spectating_flags_reflect_the_match_fields() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut session = Session::new(ClientId::new(1), tx, dummy_ip());
        session.current_match = Some(MatchId::new(3));
        assert!(session.is_playing());
        assert!(!session.is_spectating());

        session.current_match = None;
        session.spectating = Some(MatchId::new(3));
        assert!(!session.is_playing());
        assert!(session.is_spectating());
    }
}
