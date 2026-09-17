//! Binary entry point for the Tic-Tac-Toe WebSocket server.

use std::io::IsTerminal;
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
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(ansi_supported())
        .init();
}

/// Returns `true` when the current stderr can render ANSI escape codes.
///
/// The check respects the two conventions that the wider CLI ecosystem
/// follows:
///
/// - `NO_COLOR` disables color unconditionally when it is present.
/// - `CLICOLOR_FORCE` (when not `"0"`) forces color even if the output is
///   redirected.
///
/// When neither is set, color is enabled only when stderr is a terminal and
/// the platform is known to interpret escapes. On Windows, the legacy
/// `cmd.exe` console does not process ANSI unless Virtual Terminal
/// Processing is explicitly enabled, so we require the marker environment
/// variable of a modern terminal (Windows Terminal, ConEmu, or VS Code's
/// integrated terminal). Emitting literal escape sequences is worse than
/// emitting no color at all.
fn ansi_supported() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var("CLICOLOR_FORCE").is_ok_and(|value| value != "0") {
        return true;
    }
    if !std::io::stderr().is_terminal() {
        return false;
    }

    #[cfg(windows)]
    {
        std::env::var_os("WT_SESSION").is_some()
            || std::env::var_os("ConEmuANSI").is_some()
            || std::env::var_os("TERM_PROGRAM").is_some()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
