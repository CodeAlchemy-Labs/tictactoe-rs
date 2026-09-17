//! Runtime configuration.

use anyhow::{Context, bail};
use clap::Parser;
use url::Url;

/// Command-line arguments.
///
/// Environment variables are used as fallbacks so that containerized
/// deployments can override values without rewriting the command line.
#[derive(Debug, Parser)]
#[command(name = "client", about = "Terminal client for tictactoe-rs", version)]
pub struct Cli {
    /// WebSocket URL of the server. Accepts `ws://` and `wss://`.
    #[arg(
        long,
        env = "TICTACTOE_SERVER",
        default_value = "ws://127.0.0.1:8080/ws"
    )]
    pub server: String,

    /// Display name announced to the server.
    #[arg(long, env = "TICTACTOE_NAME")]
    pub name: String,

    /// Disable TLS certificate verification.
    ///
    /// Intended for development against servers that present self-signed
    /// certificates. Do not use against production endpoints.
    #[arg(long, default_value_t = false)]
    pub insecure: bool,
}

/// Validated configuration for the terminal client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The WebSocket URL of the server.
    pub server_url: String,
    /// The display name to announce via `Hello`.
    pub display_name: String,
    /// Whether TLS certificate verification should be disabled.
    pub insecure: bool,
}

impl ClientConfig {
    /// Builds the configuration from process arguments and environment
    /// variables.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses a scheme other than
    /// `ws` or `wss`.
    pub fn from_args_and_env() -> anyhow::Result<Self> {
        let cli = Cli::parse();
        let url = Url::parse(&cli.server)
            .with_context(|| format!("invalid server URL `{}`", cli.server))?;
        match url.scheme() {
            "ws" | "wss" => {}
            other => bail!("unsupported URL scheme `{other}`; expected `ws` or `wss`"),
        }
        Ok(Self {
            server_url: cli.server,
            display_name: cli.name,
            insecure: cli.insecure,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(url: &str) -> anyhow::Result<()> {
        let parsed = Url::parse(url).with_context(|| format!("invalid `{url}`"))?;
        match parsed.scheme() {
            "ws" | "wss" => Ok(()),
            other => bail!("unsupported URL scheme `{other}`"),
        }
    }

    #[test]
    fn accepts_ws_and_wss() {
        validate("ws://127.0.0.1:8080/ws").unwrap();
        validate("wss://tictactoe.onrender.com/ws").unwrap();
    }

    #[test]
    fn rejects_http_and_https() {
        assert!(validate("http://example.com").is_err());
        assert!(validate("https://example.com").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(validate("not a url").is_err());
    }
}
