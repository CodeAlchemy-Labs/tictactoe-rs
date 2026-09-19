//! WebSocket Tic-Tac-Toe server.
//!
//! The crate is organized in three layers:
//!
//! - [`domain`]: in-memory session and match types with no I/O.
//! - [`application`]: the [`LobbyService`](application::LobbyService) that
//!   coordinates sessions and matches under a single mutex.
//! - [`infrastructure`]: Axum HTTP and WebSocket adapters, plus the
//!   [`SessionGuard`](infrastructure::session_guard::SessionGuard) RAII
//!   guard that performs deterministic cleanup when a client disconnects.

pub mod application;
pub mod config;
pub mod domain;
pub mod infrastructure;
