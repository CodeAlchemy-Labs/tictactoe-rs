//! Attempts to act on a match without being a legitimate participant.
//!
//! The scenario performs four probes against a live server. Each probe
//! targets a specific invariant:
//!
//! 1. `MakeMove` outside of a match must be rejected with `NotInMatch`.
//! 2. `JoinMatch` on a non-existent identifier must be rejected with
//!    `MatchNotFound`.
//! 3. A second `Hello` on an already-registered client must be rejected
//!    with `InvalidState`.
//! 4. A malformed JSON payload must be ignored, and the connection must
//!    remain usable.
//!
//! If any probe succeeds, the server is reported as compromised.

use std::time::Duration;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use common::domain::Position;
use common::protocol::{ClientMessage, ErrorCode, MatchId, ServerMessage};

use crate::outcome::Outcome;

const IO_TIMEOUT: Duration = Duration::from_secs(3);

/// Runs the `session_hijack` scenario.
///
/// # Errors
///
/// Returns an error if the network transport fails in a way that prevents
/// the scenario from completing.
pub async fn run(target: &str) -> anyhow::Result<Outcome> {
    let (mut ws, _response) = connect_async(target)
        .await
        .with_context(|| format!("failed to connect to {target}"))?;

    // Probe 1: register as an ordinary client.
    send(
        &mut ws,
        &ClientMessage::Hello {
            display_name: String::from("hacker"),
        },
    )
    .await?;
    match recv(&mut ws).await? {
        ServerMessage::Welcome { .. } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "expected Welcome, got {other:?}"
            )));
        }
    }

    // Probe 2: move without being in a match.
    send(
        &mut ws,
        &ClientMessage::MakeMove {
            position: Position::new(4).context("hard-coded position")?,
        },
    )
    .await?;
    match recv(&mut ws).await? {
        ServerMessage::Error {
            code: ErrorCode::NotInMatch,
            ..
        } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "MakeMove outside a match was not rejected: {other:?}"
            )));
        }
    }

    // Probe 3: join a non-existent match.
    send(
        &mut ws,
        &ClientMessage::JoinMatch {
            match_id: MatchId::new(u64::MAX),
        },
    )
    .await?;
    match recv(&mut ws).await? {
        ServerMessage::Error {
            code: ErrorCode::MatchNotFound,
            ..
        } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "JoinMatch on a bogus id was not rejected: {other:?}"
            )));
        }
    }

    // Probe 4: send Hello twice.
    send(
        &mut ws,
        &ClientMessage::Hello {
            display_name: String::from("hacker-again"),
        },
    )
    .await?;
    match recv(&mut ws).await? {
        ServerMessage::Error {
            code: ErrorCode::InvalidState,
            ..
        } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "double Hello was not rejected: {other:?}"
            )));
        }
    }

    // Probe 5: send malformed JSON, then a Ping. If the connection survives,
    // the server ignored the malformed payload without crashing or closing.
    ws.send(Message::Text(String::from("not json at all").into()))
        .await
        .context("failed to send malformed payload")?;
    send(&mut ws, &ClientMessage::Ping).await?;
    match recv(&mut ws).await? {
        ServerMessage::Pong => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "malformed payload disrupted the connection: {other:?}"
            )));
        }
    }

    Ok(Outcome::defended(
        "all probes rejected; malformed payload ignored without closing the connection",
    ))
}

async fn send<S>(ws: &mut S, message: &ClientMessage) -> anyhow::Result<()>
where
    S: SinkExt<Message> + Unpin,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    let payload = serde_json::to_string(message).context("serialize")?;
    ws.send(Message::Text(payload.into()))
        .await
        .context("websocket send failed")?;
    Ok(())
}

async fn recv<S>(ws: &mut S) -> anyhow::Result<ServerMessage>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let deadline = tokio::time::Instant::now() + IO_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let frame = tokio::time::timeout(remaining, ws.next())
            .await
            .context("timed out waiting for a server message")?
            .context("connection closed unexpectedly")?
            .context("websocket transport error")?;
        if let Message::Text(text) = frame {
            let message: ServerMessage =
                serde_json::from_str(&text).context("malformed server message")?;
            return Ok(message);
        }
    }
}
