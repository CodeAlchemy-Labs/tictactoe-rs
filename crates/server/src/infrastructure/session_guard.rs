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
//!
//! In addition to freeing the socket, the guard is the point where the
//! grace-period timer for a pending reconnection is scheduled. The lobby
//! performs the bookkeeping, but it cannot know whether it is being called
//! from a context that has an active Tokio runtime. The guard is always
//! invoked from inside the WebSocket handler, which runs on the runtime,
//! so it is the natural place to spawn the timer.

use std::sync::Arc;
use std::time::Duration;

use common::protocol::ClientId;

use crate::application::lobby::LobbyService;

/// RAII guard that removes a client from the lobby when it is dropped.
///
/// Create one immediately after calling
/// [`LobbyService::register_client`](crate::application::lobby::LobbyService::register_client)
/// and keep it alive for the duration of the connection handler. When the
/// handler returns, the guard drops and the session is removed. If the
/// client was in a live match, a reconnection timer is also scheduled.
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
        let disconnection = self.lobby.disconnect(self.client_id);
        if let Some(disconnection) = disconnection {
            let lobby = Arc::clone(&self.lobby);
            let username = disconnection.username;
            let grace = Duration::from_secs(u64::from(common::protocol::GRACE_PERIOD_SECS));
            tokio::spawn(async move {
                tokio::time::sleep(grace).await;
                lobby.expire_disconnection(&username);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn dropping_the_guard_removes_the_session() {
        let lobby = Arc::new(LobbyService::default());
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = lobby
            .register_client(tx, std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
            .unwrap();
        assert_eq!(lobby.session_count(), 1);
        {
            let _guard = SessionGuard::new(client, Arc::clone(&lobby));
            assert_eq!(lobby.session_count(), 1);
        }
        assert_eq!(lobby.session_count(), 0);
    }

    #[test]
    fn client_id_is_accessible() {
        let lobby = Arc::new(LobbyService::default());
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = lobby
            .register_client(tx, std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
            .unwrap();
        let guard = SessionGuard::new(client, lobby);
        assert_eq!(guard.client_id(), client);
    }
}
