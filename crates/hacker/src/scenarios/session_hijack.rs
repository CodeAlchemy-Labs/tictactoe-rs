//! Attempts to act on a match without being a legitimate participant.
//!
//! The scenario performs six probes, each targeting a specific invariant:
//!
//! 1. A guest that has not even sent `Hello` cannot create a match; the
//!    server rejects it with `InvalidState`.
//! 2. After `Hello` but before authentication, `CreateMatch` is rejected
//!    with `AuthenticationRequired`.
//! 3. An authenticated session that is not in a match cannot make a move;
//!    the server rejects it with `NotInMatch`.
//! 4. `JoinMatch` on a non-existent identifier is rejected with
//!    `MatchNotFound`.
//! 5. A second `Register` on an already-authenticated session is rejected
//!    with `AlreadyAuthenticated`.
//! 6. A malformed JSON payload is ignored and the connection remains
//!    usable, as demonstrated by a subsequent `Ping`/`Pong` exchange.
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

    probe_unidentified_guest(&mut ws).await?;
    probe_unauthenticated_guest(&mut ws).await?;
    register_hacker(&mut ws).await?;
    probe_move_without_match(&mut ws).await?;
    probe_join_bogus_match(&mut ws).await?;
    probe_double_register(&mut ws).await?;
    probe_malformed_payload(&mut ws).await?;

    Ok(Outcome::defended(
        "every probe rejected; malformed payload ignored without closing the connection",
    ))
}

/// Probe 1: a fresh connection has not sent `Hello` yet.
async fn probe_unidentified_guest<S>(ws: &mut S) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(ws, &ClientMessage::CreateMatch).await?;
    match recv(ws).await? {
        ServerMessage::Error {
            code: ErrorCode::InvalidState,
            ..
        } => Ok(()),
        other => Err(anyhow::anyhow!(
            "CreateMatch before Hello was not rejected with InvalidState: {other:?}"
        )),
    }
}

/// Probe 2: a guest that has sent `Hello` but has not authenticated.
async fn probe_unauthenticated_guest<S>(ws: &mut S) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(
        ws,
        &ClientMessage::Hello {
            display_name: String::from("hacker"),
        },
    )
    .await?;
    match recv(ws).await? {
        ServerMessage::Welcome { .. } => {}
        other => {
            return Err(anyhow::anyhow!(
                "expected Welcome after Hello, got {other:?}"
            ));
        }
    }

    send(ws, &ClientMessage::CreateMatch).await?;
    match recv(ws).await? {
        ServerMessage::Error {
            code: ErrorCode::AuthenticationRequired,
            ..
        } => Ok(()),
        other => Err(anyhow::anyhow!(
            "guest CreateMatch was not rejected with AuthenticationRequired: {other:?}"
        )),
    }
}

/// Registers a fresh account so the rest of the probes run authenticated.
async fn register_hacker<S>(ws: &mut S) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    let username = format!("hacker_{}", rand_suffix());
    send(
        ws,
        &ClientMessage::Register {
            name: String::from("Adversarial Actor"),
            username: username.clone(),
            age: 30,
            password: HACKER_PASSWORD.to_string(),
        },
    )
    .await?;
    match recv(ws).await? {
        ServerMessage::Registered { profile } => {
            if profile.username.as_str() != username {
                return Err(anyhow::anyhow!(
                    "Registered echoed a different username: {}",
                    profile.username
                ));
            }
            Ok(())
        }
        other => Err(anyhow::anyhow!("expected Registered, got {other:?}")),
    }
}

/// Probe 3: move without being in a match.
async fn probe_move_without_match<S>(ws: &mut S) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(
        ws,
        &ClientMessage::MakeMove {
            position: Position::new(4).context("hard-coded position")?,
        },
    )
    .await?;
    match recv(ws).await? {
        ServerMessage::Error {
            code: ErrorCode::NotInMatch,
            ..
        } => Ok(()),
        other => Err(anyhow::anyhow!(
            "MakeMove outside a match was not rejected with NotInMatch: {other:?}"
        )),
    }
}

/// Probe 4: join a non-existent match.
async fn probe_join_bogus_match<S>(ws: &mut S) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(
        ws,
        &ClientMessage::JoinMatch {
            match_id: MatchId::new(u64::MAX),
        },
    )
    .await?;
    match recv(ws).await? {
        ServerMessage::Error {
            code: ErrorCode::MatchNotFound,
            ..
        } => Ok(()),
        other => Err(anyhow::anyhow!(
            "JoinMatch on a bogus id was not rejected with MatchNotFound: {other:?}"
        )),
    }
}

/// Probe 5: try to register again on an already-authenticated session.
async fn probe_double_register<S>(ws: &mut S) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    send(
        ws,
        &ClientMessage::Register {
            name: String::from("Hacker Again"),
            username: format!("hacker_{}", rand_suffix()),
            age: 30,
            password: HACKER_PASSWORD.to_string(),
        },
    )
    .await?;
    match recv(ws).await? {
        ServerMessage::AuthenticationFailed {
            reason: AuthFailureReason::AlreadyAuthenticated,
            ..
        } => Ok(()),
        other => Err(anyhow::anyhow!(
            "double Register was not rejected with AlreadyAuthenticated: {other:?}"
        )),
    }
}

/// Probe 6: malformed payload followed by a Ping.
async fn probe_malformed_payload<S>(ws: &mut S) -> anyhow::Result<()>
where
    S: SinkExt<Message>
        + Unpin
        + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    <S as futures_util::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    ws.send(Message::Text(String::from("not json at all").into()))
        .await
        .context("failed to send malformed payload")?;
    send(ws, &ClientMessage::Ping).await?;
    match recv(ws).await? {
        ServerMessage::Pong => Ok(()),
        other => Err(anyhow::anyhow!(
            "malformed payload disrupted the connection: {other:?}"
        )),
    }
}

/// Generates a small random-ish suffix for the hacker's username.
///
/// The goal is only to avoid collisions between test runs on a fresh
/// server, not to be cryptographically unpredictable.
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
