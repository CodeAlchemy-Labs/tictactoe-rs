//! Probes whether a spectator can interfere with a match.
//!
//! The scenario opens three independent WebSocket connections:
//!
//! 1. A host that registers and creates a match.
//! 2. A guest that registers and joins the match.
//! 3. A spectator that registers and watches the match.
//!
//! The spectator then tries to act on the match: make a move as if it were
//! a player, and join the same match while spectating it. Both attempts
//! must be rejected with the expected error code, and the underlying game
//! must remain untouched.
//!
//! If either probe succeeds, the server is reported as compromised.

use std::time::Duration;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use common::domain::Position;
use common::protocol::{ClientMessage, ErrorCode, MatchId, ServerMessage};

use crate::outcome::Outcome;

const IO_TIMEOUT: Duration = Duration::from_secs(3);
const HACKER_PASSWORD: &str = "hacker-can-haz-password";

/// Runs the `spectator_isolation` scenario.
///
/// # Errors
///
/// Returns an error if the network transport fails in a way that prevents
/// the scenario from completing.
pub async fn run(target: &str) -> anyhow::Result<Outcome> {
    // Connection 1: host creates a match.
    let (mut host, _) = connect_async(target)
        .await
        .with_context(|| format!("failed to connect host to {target}"))?;
    let host_name = format!("spect_host_{}", rand_suffix());
    register(&mut host, &host_name).await?;
    send(&mut host, &ClientMessage::CreateMatch).await?;
    let match_id = match recv(&mut host).await? {
        ServerMessage::MatchCreated { match_id } => match_id,
        other => {
            return Ok(Outcome::compromised(format!(
                "host CreateMatch returned {other:?}"
            )));
        }
    };

    // Connection 2: guest joins the match.
    let (mut guest, _) = connect_async(target)
        .await
        .with_context(|| format!("failed to connect guest to {target}"))?;
    let guest_name = format!("spect_guest_{}", rand_suffix());
    register(&mut guest, &guest_name).await?;
    send(&mut guest, &ClientMessage::JoinMatch { match_id }).await?;
    match recv(&mut guest).await? {
        ServerMessage::MatchReady { .. } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "guest JoinMatch returned {other:?}"
            )));
        }
    }
    // Host receives MatchReady too.
    let _ = recv(&mut host).await?;

    // Connection 3: spectator watches the match.
    let (mut spectator, _) = connect_async(target)
        .await
        .with_context(|| format!("failed to connect spectator to {target}"))?;
    let spec_name = format!("spect_watch_{}", rand_suffix());
    register(&mut spectator, &spec_name).await?;
    send(&mut spectator, &ClientMessage::Spectate { match_id }).await?;
    match recv(&mut spectator).await? {
        ServerMessage::SpectateStarted { .. } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "spectator Spectate returned {other:?}"
            )));
        }
    }
    // Players receive SpectatorJoined.
    let _ = recv(&mut host).await?;
    let _ = recv(&mut guest).await?;

    // Probe 1: the spectator tries to make a move.
    if let Some(issue) = probe_spectator_move(&mut spectator).await? {
        return Ok(issue);
    }

    // Probe 2: the spectator tries to join the same match as a player.
    if let Some(issue) = probe_spectator_join(&mut spectator, match_id).await? {
        return Ok(issue);
    }

    Ok(Outcome::defended(
        "spectator cannot make moves or join the match as a player",
    ))
}

/// Probes that `MakeMove` from a spectator is rejected with `NotInMatch`.
async fn probe_spectator_move<S>(spectator: &mut S) -> anyhow::Result<Option<Outcome>>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(
        spectator,
        &ClientMessage::MakeMove {
            position: Position::new(4).context("hard-coded position")?,
        },
    )
    .await?;
    match recv(spectator).await? {
        ServerMessage::Error {
            code: ErrorCode::NotInMatch,
            ..
        } => Ok(None),
        other => Ok(Some(Outcome::compromised(format!(
            "spectator MakeMove was not rejected with NotInMatch: {other:?}"
        )))),
    }
}

/// Probes that `JoinMatch` from a spectator is rejected with
/// `AlreadySpectating`.
async fn probe_spectator_join<S>(
    spectator: &mut S,
    match_id: MatchId,
) -> anyhow::Result<Option<Outcome>>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(spectator, &ClientMessage::JoinMatch { match_id }).await?;
    match recv(spectator).await? {
        ServerMessage::Error {
            code: ErrorCode::AlreadySpectating,
            ..
        } => Ok(None),
        other => Ok(Some(Outcome::compromised(format!(
            "spectator JoinMatch was not rejected with AlreadySpectating: {other:?}"
        )))),
    }
}

/// Registers the connection with a fresh hacker username.
async fn register<S>(ws: &mut S, username: &str) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(
        ws,
        &ClientMessage::Register {
            name: String::from("Adversarial Observer"),
            username: username.to_string(),
            age: 30,
            password: HACKER_PASSWORD.to_string(),
        },
    )
    .await?;
    match recv(ws).await? {
        ServerMessage::Registered { .. } => Ok(()),
        other => anyhow::bail!("expected Registered, got {other:?}"),
    }
}

/// Generates a small random-ish suffix for usernames.
fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| u64::from(duration.subsec_nanos()));
    nanos % 1_000_000
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
