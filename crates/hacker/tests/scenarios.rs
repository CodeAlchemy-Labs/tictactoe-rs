//! Integration tests for the hacker scenarios.
//!
//! Each test spins up an in-process server on an ephemeral port and runs the
//! corresponding scenario against it. The server is the same code path used
//! in production, so the tests validate the exact defense the demo relies on.

use std::net::SocketAddr;
use std::sync::Arc;

use hacker::outcome::Outcome;
use hacker::scenarios::{flood, port_reuse, session_hijack, spectator_isolation};
use server::application::lobby::LobbyService;
use server::infrastructure::http::build_router;
use tokio::net::TcpListener;

async fn spawn_server() -> SocketAddr {
    let lobby = Arc::new(LobbyService::default());
    let app = build_router(Arc::clone(&lobby));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    addr
}

fn ws_url(addr: SocketAddr) -> String {
    format!("ws://{addr}/ws")
}

#[tokio::test]
async fn session_hijack_is_defended() {
    let addr = spawn_server().await;
    let outcome = session_hijack::run(&ws_url(addr)).await.unwrap();
    assert!(
        matches!(outcome, Outcome::Defended { .. }),
        "expected defended, got {outcome:?}"
    );
}

#[tokio::test]
async fn port_reuse_is_defended() {
    let addr = spawn_server().await;
    let outcome = port_reuse::run(&ws_url(addr)).await.unwrap();
    assert!(
        matches!(outcome, Outcome::Defended { .. }),
        "expected defended, got {outcome:?}"
    );
}

#[tokio::test]
async fn flood_is_defended() {
    let addr = spawn_server().await;
    let outcome = flood::run(&ws_url(addr)).await.unwrap();
    assert!(
        matches!(outcome, Outcome::Defended { .. }),
        "expected defended, got {outcome:?}"
    );
}

#[tokio::test]
async fn spectator_isolation_is_defended() {
    let addr = spawn_server().await;
    let outcome = spectator_isolation::run(&ws_url(addr)).await.unwrap();
    assert!(
        matches!(outcome, Outcome::Defended { .. }),
        "expected defended, got {outcome:?}"
    );
}
