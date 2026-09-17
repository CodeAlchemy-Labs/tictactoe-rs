//! Runtime configuration.

use std::net::SocketAddr;

use anyhow::Context;

/// Configuration for the HTTP and WebSocket server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// The address the server binds to.
    pub bind_address: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: SocketAddr::from(([0, 0, 0, 0], 8080)),
        }
    }
}

impl ServerConfig {
    /// Builds the configuration from the process environment.
    ///
    /// Reads `TICTACTOE_BIND` when present; otherwise falls back to the
    /// default of `0.0.0.0:8080`.
    ///
    /// # Errors
    ///
    /// Returns an error when `TICTACTOE_BIND` is present but not a valid
    /// socket address.
    pub fn from_env() -> anyhow::Result<Self> {
        match std::env::var("TICTACTOE_BIND") {
            Ok(value) => {
                let bind_address = value
                    .parse::<SocketAddr>()
                    .with_context(|| format!("invalid TICTACTOE_BIND `{value}`"))?;
                Ok(Self { bind_address })
            }
            Err(_) => Ok(Self::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_binds_to_all_interfaces_on_port_8080() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_address.port(), 8080);
        assert!(config.bind_address.ip().is_unspecified());
    }
}