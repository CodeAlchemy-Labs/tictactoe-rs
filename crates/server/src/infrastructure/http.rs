//! Axum router construction.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;

use crate::application::lobby::LobbyService;
use crate::infrastructure::ws_handler::ws_handler;

/// Builds the HTTP router exposed by the server.
///
/// Two routes are provided:
///
/// - `GET /health` returns a plain `ok` for liveness checks.
/// - `GET /ws` upgrades to a WebSocket connection and drives the game.
pub fn build_router(lobby: Arc<LobbyService>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .with_state(lobby)
}

async fn health() -> &'static str {
    "ok"
}