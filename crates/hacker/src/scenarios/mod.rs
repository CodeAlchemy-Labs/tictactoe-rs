//! The scenarios the hacker can execute.
//!
//! Each scenario is a self-contained asynchronous function that takes a
//! WebSocket target URL and returns an [`Outcome`].
//! The [`run`] dispatcher maps a scenario name to the corresponding function,
//! which keeps the binary free of `match`-on-string noise.

pub mod flood;
pub mod port_reuse;
pub mod session_hijack;
pub mod spectator_isolation;

use crate::outcome::Outcome;

/// Runs the scenario named `name` against `target`.
///
/// # Errors
///
/// Returns an error if `name` is not a known scenario, or if the scenario
/// itself fails for a reason other than a successful attack.
pub async fn run(name: &str, target: &str) -> anyhow::Result<Outcome> {
    match name {
        "session_hijack" => session_hijack::run(target).await,
        "port_reuse" => port_reuse::run(target).await,
        "flood" => flood::run(target).await,
        "spectator_isolation" => spectator_isolation::run(target).await,
        other => anyhow::bail!("unknown scenario `{other}`"),
    }
}
