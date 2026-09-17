//! Probes the server's port ownership and Rust's deterministic release.
//!
//! Two checks are performed:
//!
//! 1. The hacker tries to bind the server's port. The kernel must refuse
//!    with `EADDRINUSE`, which proves the server still owns the port.
//! 2. The hacker binds an ephemeral port, drops the listener, and
//!    immediately rebinds the same port. In Rust, `drop` releases the port
//!    synchronously through the `Drop` implementation of `TcpListener`, so
//!    the rebind succeeds. There is no window during which a garbage
//!    collector decides whether the port is free.

use std::io::ErrorKind;

use anyhow::Context;
use tokio::net::TcpListener;

use crate::config::parse_target;
use crate::outcome::Outcome;

/// Runs the `port_reuse` scenario.
///
/// # Errors
///
/// Returns an error if the target URL cannot be parsed or if the bind
/// attempts fail for reasons unrelated to the attack.
pub async fn run(target: &str) -> anyhow::Result<Outcome> {
    let url = parse_target(target)?;
    let host = url.host_str().context("target URL has no host")?;
    let port = url
        .port_or_known_default()
        .context("target URL has no port")?;
    let address = format!("{host}:{port}");

    // Check 1: bind the server's port.
    match TcpListener::bind(&address).await {
        Ok(listener) => {
            drop(listener);
            Ok(Outcome::compromised(format!(
                "hacker bound the server port {address} — the server is not holding its listener"
            )))
        }
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            // Check 2: deterministic ephemeral-port release.
            let ephemeral = TcpListener::bind("127.0.0.1:0")
                .await
                .context("failed to bind an ephemeral port")?;
            let ephemeral_address = ephemeral
                .local_addr()
                .context("failed to read the ephemeral address")?;
            drop(ephemeral);

            match TcpListener::bind(ephemeral_address).await {
                Ok(rebound) => {
                    drop(rebound);
                    Ok(Outcome::defended(format!(
                        "server holds {address} (EADDRINUSE); ephemeral {ephemeral_address} released synchronously on drop"
                    )))
                }
                Err(error) => Ok(Outcome::compromised(format!(
                    "failed to rebind dropped ephemeral port {ephemeral_address}: {error}"
                ))),
            }
        }
        Err(error) => {
            Err(anyhow::Error::from(error)
                .context(format!("unexpected bind failure for {address}")))
        }
    }
}
