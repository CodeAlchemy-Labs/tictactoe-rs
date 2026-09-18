//! Attempts to act on a match without being a legitimate participant.
//!
//! The scenario authenticates a fresh account and then performs five probes,
//! each targeting a specific invariant:
//!
//! 1. `MakeMove` outside of a match must be rejected with `NotInMatch`.
//! 2. `JoinMatch` on a non-existent identifier must be rejected with
//!    `MatchNotFound`.
//! 3. A second `Register` on an already-authenticated session must be
//!    rejected with `AlreadyAuthenticated`.
//! 4. A malformed JSON payload must be ignored, and the connection must
//!    remain usable.
//! 5. A guest (pre-authentication) connection must not be able to create a
//!    match; the server rejects it with `AuthenticationRequired`.
//!
//! If any probe does not produce the expected rejection, the server is
//! reported as compromised.

use std::time::Duration;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use common::domain::Position;
use common::protocol::{AuthFailureReason, ClientMessage, ErrorCode, MatchId, ServerMessage};

use crate::outcome::Outcome;

const IO_TIMEOUT: Duration = Duration::from_secs(3);
const HACKER_PASSWORD: &str = "hacker-can-haz-password";

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

    // Phase 0: try to create a match as a guest. The server must reject it
    // with `AuthenticationRequired`.
    send(&mut ws, &ClientMessage::CreateMatch).await?;
    match recv(&mut ws).await? {
        ServerMessage::Error {
            code: ErrorCode::AuthenticationRequired,
            ..
        } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "guest CreateMatch was not rejected with AuthenticationRequired: {other:?}"
            )));
        }
    }

    // Phase 1: register a fresh account so the rest of the probes run as an
    // authenticated session.
    let username = format!("hacker_{}", rand_suffix());
    send(
        &mut ws,
        &ClientMessage::Register {
            name: String::from("Adversarial Actor"),
            username: username.clone(),
            age: 30,
            password: HACKER_PASSWORD.to_string(),
        },
    )
    .await?;
    match recv(&mut ws).await? {
        ServerMessage::Registered { profile } => {
            if profile.username.as_str() != username {
                return Ok(Outcome::compromised(format!(
                    "Registered echoed a different username: {}",
                    profile.username
                )));
            }
        }
        other => {
            return Ok(Outcome::compromised(format!(
                "expected Registered, got {other:?}"
            )));
        }
    }

    // Probe 1: move without being in a match.
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
                "MakeMove outside a match was not rejected with NotInMatch: {other:?}"
            )));
        }
    }

    // Probe 2: join a non-existent match.
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
                "JoinMatch on a bogus id was not rejected with MatchNotFound: {other:?}"
            )));
        }
    }

    // Probe 3: try to register again on an already-authenticated session.
    send(
        &mut ws,
        &ClientMessage::Register {
            name: String::from("Hacker Again"),
            username: format!("hacker_{}", rand_suffix()),
            age: 30,
            password: HACKER_PASSWORD.to_string(),
        },
    )
    .await?;
    match recv(&mut ws).await? {
        ServerMessage::AuthenticationFailed {
            reason: AuthFailureReason::AlreadyAuthenticated,
            ..
        } => {}
        other => {
            return Ok(Outcome::compromised(format!(
                "double Register was not rejected with AlreadyAuthenticated: {other:?}"
            )));
        }
    }

    // Probe 4: send malformed JSON, then a Ping. If the connection survives,
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
        "guest cannot create a match; authenticated hacker rejected on move-without-match, join-bogus-id, and double register; malformed payload ignored without closing the connection",
    ))
}

/// Generates a small random-ish suffix for the hacker's username.
///
/// The goal is only to avoid collisions between test runs on a fresh
/// server, not to be cryptographically unpredictable.
fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as u64)
        .unwrap_or(0);
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
