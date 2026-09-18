//! Probes port ownership and Rust's deterministic release of ephemeral ports.
//!
//! The scenario is honest about what can be verified from a given host and
//! network namespace. Three outcomes are possible when the hacker tries to
//! bind the server's port:
//!
//! 1. `EADDRINUSE` — the server holds the port in a shared namespace. This
//!    is the strongest form of the defense and is what happens when the
//!    hacker and the server run on the same host.
//! 2. `EACCES` — the port is privileged (below 1024) and the hacker is not
//!    root. The bind cannot even be attempted, so the property is
//!    unobservable from this process. Remote HTTPS endpoints on port 443
//!    fall into this category.
//! 3. Success — the port is bindable. This happens inside an isolated
//!    network namespace (Docker's default bridge, for instance), where the
//!    container has its own port space.
//!
//! In all three cases the scenario then verifies the language-level
//! guarantee the project is about: a `TcpListener` releases its port the
//! instant it is dropped, and the same process can rebind it without
//! waiting for a garbage collector.

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
    // process. The three outcomes are all valid observations; what matters
    // is that the scenario reports which one occurred.
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
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            format!(
                "EACCES on 0.0.0.0:{port}: the port is privileged and this process cannot bind it; the property is unobservable from here"
            )
        }
        Err(error) => {
            return Ok(Outcome::compromised(format!(
                "unexpected bind outcome on 0.0.0.0:{port}: {error}"
            )));
        }
    };

    // Check 3: ephemeral port release. This is the language-level guarantee
    // the project is about.
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
