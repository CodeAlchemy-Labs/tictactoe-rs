//! Command-line configuration for the hacker binary.

use anyhow::Context;
use clap::{Parser, ValueEnum};
use url::Url;

/// The scenarios the hacker can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ScenarioChoice {
    /// Tries to act on a match without being a legitimate participant.
    SessionHijack,
    /// Tries to bind the server's port and probes ephemeral-port release.
    PortReuse,
    /// Opens many concurrent connections and verifies recovery.
    Flood,
    /// Probes that a spectator cannot interfere with a match.
    SpectatorIsolation,
    /// Runs every scenario in sequence.
    All,
}

/// Command-line arguments.
#[derive(Debug, Parser)]
#[command(name = "hacker", about = "Adversarial actor for tictactoe-rs", version)]
pub struct Cli {
    /// Which scenario (or set of scenarios) to run.
    #[arg(long, value_enum, default_value_t = ScenarioChoice::All)]
    pub scenario: ScenarioChoice,

    /// WebSocket URL of the target server.
    #[arg(long, default_value = "ws://127.0.0.1:8080/ws")]
    pub target: String,
}

/// Parses and validates a WebSocket URL.
///
/// # Errors
///
/// Returns an error if `target` is not a valid URL.
pub fn parse_target(target: &str) -> anyhow::Result<Url> {
    Url::parse(target).with_context(|| format!("invalid target URL `{target}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_accepts_ws_urls() {
        let url = parse_target("ws://127.0.0.1:8080/ws").unwrap();
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(8080));
    }

    #[test]
    fn parse_target_rejects_garbage() {
        assert!(parse_target("not a url").is_err());
    }
}
