//! Binary entry point for the Tic-Tac-Toe WebSocket server.

use std::sync::Arc;

use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use server::application::lobby::LobbyService;
use server::config::ServerConfig;
use server::infrastructure::http::build_router;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = ServerConfig::from_env()?;
    let lobby = Arc::new(LobbyService::new());
    let app = build_router(Arc::clone(&lobby));

    let listener = TcpListener::bind(config.bind_address)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_address))?;

    tracing::info!(address = %config.bind_address, "server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server terminated with an error")?;

    tracing::info!("server stopped");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}