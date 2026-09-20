//! Runtime configuration.

use anyhow::{bail, Context};
use clap::Parser;
use url::Url;

/// Command-line arguments.
///
/// Environment variables are used as fallbacks so that containerized
/// deployments can override values without rewriting the command line.
#[derive(Debug, Parser)]
#[command(name = "tictacli", about = "Terminal client for tictactoe-rs", version)]
pub struct Cli {
    /// WebSocket URL of the server. Accepts `ws://` and `wss://`.
    #[arg(long, env = "TICTACTOE_SERVER")]
    pub server: Option<String>,

    /// Display name announced to the server.
    #[arg(long, env = "TICTACTOE_NAME")]
    pub name: Option<String>,

    /// Disable TLS certificate verification.
    ///
    /// Intended for development against servers that present self-signed
    /// certificates. Do not use against production endpoints.
    #[arg(long, default_value_t = false)]
    pub insecure: bool,
}

/// Validated configuration for the terminal client.
#[derive(Debug, Clone)]
pub struct ArgsConfig {
    /// The WebSocket URL of the server.
    pub server_url: Option<String>,
    /// The display name to announce via `Hello`.
    pub display_name: Option<String>,
    /// Whether TLS certificate verification should be disabled.
    pub insecure: bool,
}

impl ArgsConfig {
    /// Builds the configuration from process arguments and environment
    /// variables.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses a scheme other than
    /// `ws` or `wss`.
    pub fn from_args_and_env() -> anyhow::Result<Self> {
        let cli = Cli::parse();
        if let Some(ref s) = cli.server {
            let url = Url::parse(s).with_context(|| format!("invalid server URL `{s}`"))?;
            match url.scheme() {
                "ws" | "wss" => {}
                other => bail!("unsupported URL scheme `{other}`; expected `ws` or `wss`"),
            }
        }
        Ok(Self {
            server_url: cli.server,
            display_name: cli.name,
            insecure: cli.insecure,
        })
    }
}

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Persisted user configuration for the client.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientConfig {
    /// Server WebSocket URL, e.g. `ws://127.0.0.1:8080/ws`.
    pub server_url: Option<String>,
    /// Preferred guest display name. Falls back to the OS username when None.
    pub guest_name: Option<String>,
    /// Whether TLS was requested the last time the user connected.
    pub use_tls: Option<bool>,
}

/// Returns the absolute path of the config file, or `None` if the platform
/// has no writable config directory.
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "tictacli")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// Loads the configuration from the platform config directory.
///
/// Returns `ClientConfig::default()` if the file is missing, unreadable, or
/// malformed. Never panics. Logs a warning on parse failure.
#[must_use]
pub fn load() -> ClientConfig {
    config_path().map_or_else(ClientConfig::default, |path| load_from(&path))
}

#[cfg(test)]
pub(crate) fn load_from(path: &Path) -> ClientConfig {
    let Ok(content) = std::fs::read_to_string(path) else {
        return ClientConfig::default();
    };
    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!("Failed to parse config at {}: {}", path.display(), e);
            ClientConfig::default()
        }
    }
}
#[cfg(not(test))]
fn load_from(path: &Path) -> ClientConfig {
    let Ok(content) = std::fs::read_to_string(path) else {
        return ClientConfig::default();
    };
    match toml::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!("Failed to parse config at {}: {}", path.display(), e);
            ClientConfig::default()
        }
    }
}

/// Persists the configuration to the platform config directory.
///
/// Creates parent directories as needed. Returns an error only if the write
/// itself fails; missing parents are not an error because the function
/// creates them.
///
/// # Errors
///
/// Returns an error if the serialization fails, if there is an I/O failure
/// writing the file or its directories, or if no writable config directory is found.
pub fn save(config: &ClientConfig) -> std::io::Result<()> {
    config_path().map_or_else(
        || {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No writable config directory found",
            ))
        },
        |path| save_to(&path, config),
    )
}

fn save_to(path: &Path, config: &ClientConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = match toml::to_string(config) {
        Ok(c) => c,
        Err(e) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to serialize config: {e}"),
            ));
        }
    };
    std::fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        let config = ClientConfig {
            server_url: Some("ws://127.0.0.1:8080/ws".to_string()),
            guest_name: Some("Alice".to_string()),
            use_tls: Some(true),
        };
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: ClientConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.toml");
        let config = load_from(&path);
        assert_eq!(config, ClientConfig::default());
    }

    #[test]
    fn test_load_malformed_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("malformed.toml");
        std::fs::write(&path, "invalid toml content = [] ]").unwrap();
        let config = load_from(&path);
        assert_eq!(config, ClientConfig::default());
    }

    #[test]
    fn test_save_creates_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dirs").join("config.toml");
        let config = ClientConfig {
            server_url: Some("ws://test".to_string()),
            ..Default::default()
        };
        assert!(save_to(&path, &config).is_ok());
        assert!(path.exists());

        let loaded = load_from(&path);
        assert_eq!(loaded, config);
    }

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
