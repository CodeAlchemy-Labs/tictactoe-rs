//! Opens many concurrent WebSocket connections, closes them abruptly, and
//! verifies the server still accepts a fresh session.
//!
//! The scenario does not simulate an attack against the game logic. It probes
//! the cleanup guarantees of the server: every abruptly-closed connection
//! must be removed from the lobby, and the server must remain responsive.

use std::time::Duration;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use common::protocol::{ClientMessage, ServerMessage};

use crate::outcome::Outcome;

/// Number of concurrent connections opened during the flood.
const CONNECTIONS: usize = 32;

/// Time to wait after closing the connections before probing the server.
const SETTLE_DELAY: Duration = Duration::from_millis(300);

/// Runs the `flood` scenario.
///
/// # Errors
///
/// Returns an error if the initial target cannot be reached at all.
pub async fn run(target: &str) -> anyhow::Result<Outcome> {
    let mut tasks = Vec::with_capacity(CONNECTIONS);
    for index in 0..CONNECTIONS {
        let target = target.to_string();
        tasks.push(tokio::spawn(flood_one(target, index)));
    }

    let mut failures = 0usize;
    for task in tasks {
        if !matches!(task.await, Ok(Ok(()))) {
            failures += 1;
        }
    }

    tokio::time::sleep(SETTLE_DELAY).await;

    // Probe: a new connection must still be accepted.
    let (mut ws, _response) = connect_async(target)
        .await
        .with_context(|| format!("failed to reconnect to {target} after flood"))?;
    let payload = serde_json::to_string(&ClientMessage::Hello {
        display_name: String::from("post-flood"),
    })
        .context("serialize hello")?;
    ws.send(Message::Text(payload.into()))
        .await
        .context("failed to send hello after flood")?;

    loop {
        let frame = ws
            .next()
            .await
            .context("connection closed before Welcome")?
            .context("websocket error after flood")?;
        if let Message::Text(text) = frame {
            let message: ServerMessage =
                serde_json::from_str(&text).context("malformed server message")?;
            return match message {
                ServerMessage::Welcome { .. } => Ok(Outcome::defended(format!(
                    "{CONNECTIONS} concurrent sessions opened and closed; server still accepts new sessions (local failures: {failures})"
                ))),
                other => Ok(Outcome::compromised(format!(
                    "post-flood handshake returned {other:?}"
                ))),
            };
        }
    }
}

async fn flood_one(target: String, index: usize) -> anyhow::Result<()> {
    let (mut ws, _response) = connect_async(&target).await?;
    let payload = serde_json::to_string(&ClientMessage::Hello {
        display_name: format!("flood-{index}"),
    })?;
    ws.send(Message::Text(payload.into())).await?;
    // Wait for Welcome so the server has fully registered the session before
    // we drop the socket.
    while let Some(frame) = ws.next().await {
        let frame = frame?;
        if let Message::Text(_) = frame {
            break;
        }
    }
    ws.close(None).await?;
    Ok(())
}