//! RAII guard that performs deterministic cleanup when a client disconnects.
//!
//! This type is the core of the safety demonstration. Its `Drop` runs
//! synchronously the moment the WebSocket handler returns, which is
//! guaranteed by Rust's ownership model: there is no garbage collector, no
//! finalizer queue, and no scheduler between the end of the handler and the
//! execution of `Drop`.
//!
//! In a GC'd runtime the equivalent cleanup is typically triggered by a
//! finalizer or a scheduled cleanup task, both of which run at an
//! unspecified later time. During that window the session can remain
//! reachable and the port associated with the connection can remain open,
//! which is exactly the class of bug this demo eliminates.

use std::sync::Arc;

use common::protocol::ClientId;

use crate::application::lobby::LobbyService;

/// RAII guard that removes a client from the lobby when it is dropped.
///
/// Create one immediately after calling
/// [`LobbyService::register`](crate::application::lobby::LobbyService::register)
/// and keep it alive for the duration of the connection handler. When the
/// handler returns, the guard drops and the session is removed.
pub struct SessionGuard {
    client_id: ClientId,
    lobby: Arc<LobbyService>,
}

impl SessionGuard {
    /// Creates a new guard for the given client.
    pub fn new(client_id: ClientId, lobby: Arc<LobbyService>) -> Self {
        Self { client_id, lobby }
    }

    /// Returns the guarded client identifier.
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        tracing::info!(client_id = %self.client_id, "session guard dropping");
        self.lobby.disconnect(self.client_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn dropping_the_guard_removes_the_session() {
        let lobby = Arc::new(LobbyService::new());
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = lobby.register(tx);
        assert_eq!(lobby.session_count(), 1);
        {
            let _guard = SessionGuard::new(client, Arc::clone(&lobby));
            assert_eq!(lobby.session_count(), 1);
        }
        assert_eq!(lobby.session_count(), 0);
    }

    #[test]
    fn client_id_is_accessible() {
        let lobby = Arc::new(LobbyService::new());
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = lobby.register(tx);
        let guard = SessionGuard::new(client, lobby);
        assert_eq!(guard.client_id(), client);
    }
}