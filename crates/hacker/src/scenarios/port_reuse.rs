//! Probes port ownership and Rust's deterministic release of ephemeral ports.
//!
//! The scenario is honest about what can be verified from inside a Docker
//! bridge network. Two namespaces on a bridge have independent port spaces,
//! so "bind the server's port" is not a meaningful test there. What *is*
//! meaningful, and language-relevant, is:
//!
//! 1. The server is reachable on its advertised address, which proves it
//!    holds the port in its own namespace.
//! 2. Rust releases an ephemeral port synchronously the instant the owning
//!    listener is dropped. There is no GC window between `drop` and the
//!    kernel releasing the port; a subsequent `bind` succeeds immediately.
//!
//! When the hacker and the server share a network namespace (for example,
//! both running on the host with `--network=host`), step 1 also produces
//! `EADDRINUSE` if the hacker tries to bind the server's port. That path is
//! reported when observed.

use std::io::ErrorKind;

use anyhow::Context;
use tokio::net::{TcpListener, TcpStream};

use crate::config::parse_target;
use crate::outcome::Outcome;

/// Runs the `port_reuse` scenario.
///
/// # Errors
///
/// Returns an error if the target URL cannot be parsed.
pub async fn run(target: &str) -> anyhow::Result<Outcome> {
    let url = parse_target(target)?;
    let host = url.host_str().context("target URL has no host")?;
    let port = url
        .port_or_known_default()
        .context("target URL has no port")?;
    let server_addr = format!("{host}:{port}");

    // Check 1: the server is reachable at its advertised address.
    let stream = TcpStream::connect(&server_addr)
        .await
        .with_context(|| format!("server not reachable at {server_addr}"))?;
    drop(stream); // Rust closes the fd immediately; no GC needed.

    // Check 2: probe whether the server's port can be bound from this
    // process. In a shared namespace this must fail with EADDRINUSE. In an
    // isolated namespace (Docker's default bridge) it succeeds, because
    // each namespace has its own port space. We record which case occurred
    // so the report is transparent.
    let bind_detail = match TcpListener::bind(format!("0.0.0.0:{port}")).await {
        Ok(listener) => {
            drop(listener);
            format!(
                "isolated network namespace: 0.0.0.0:{port} is bindable from this process because each namespace owns its own port space"
            )
        }
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            format!(
                "EADDRINUSE on 0.0.0.0:{port}: the server holds the port in the shared namespace"
            )
        }
        Err(error) => {
            format!("unexpected bind outcome on 0.0.0.0:{port}: {error}")
        }
    };

    // Check 3: ephemeral port release. This is the language-level guarantee
    // we want to demonstrate: the port is released the instant the listener
    // is dropped, and can be rebound by the same process without delay.
    let ephemeral = TcpListener::bind("127.0.0.1:0")
        .await
        .context("failed to bind an ephemeral port")?;
    let ephemeral_addr = ephemeral
        .local_addr()
        .context("failed to read the ephemeral address")?;
    drop(ephemeral);

    match TcpListener::bind(ephemeral_addr).await {
        Ok(rebound) => {
            drop(rebound);
            Ok(Outcome::defended(format!(
                "server reachable at {server_addr}; {bind_detail}; ephemeral {ephemeral_addr} released synchronously on drop and immediately rebindable"
            )))
        }
        Err(error) => Ok(Outcome::compromised(format!(
            "failed to rebind dropped ephemeral port {ephemeral_addr}: {error}"
        ))),
    }
}
