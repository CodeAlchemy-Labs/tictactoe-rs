//! Runtime configuration.

use std::net::SocketAddr;

use anyhow::Context;

/// Environment variable that overrides the bind address entirely.
pub const ENV_BIND: &str = "TICTACTOE_BIND";

/// Environment variable used by Render and similar platforms to specify the
/// port the process must listen on.
pub const ENV_PORT: &str = "PORT";

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
    /// Resolution order:
    ///
    /// 1. `TICTACTOE_BIND` if it is set and parses as a socket address.
    /// 2. `PORT` if it is set and parses as a port. The bind address is
    ///    `0.0.0.0:<port>`, which is what container platforms such as Render
    ///    expect.
    /// 3. The default of `0.0.0.0:8080`.
    ///
    /// # Errors
    ///
    /// Returns an error if a variable is present but does not parse.
    pub fn from_env() -> anyhow::Result<Self> {
        if let Ok(value) = std::env::var(ENV_BIND) {
            let bind_address = value
                .parse::<SocketAddr>()
                .with_context(|| format!("invalid {ENV_BIND} `{value}`"))?;
            return Ok(Self { bind_address });
        }

        if let Ok(value) = std::env::var(ENV_PORT) {
            let port = value
                .parse::<u16>()
                .with_context(|| format!("invalid {ENV_PORT} `{value}`"))?;
            return Ok(Self {
                bind_address: SocketAddr::from(([0, 0, 0, 0], port)),
            });
        }

        Ok(Self::default())
    }

    /// Builds the configuration from a set of environment variables provided
    /// explicitly. Intended for tests.
    ///
    /// # Errors
    ///
    /// Returns an error if a provided variable does not parse.
    pub fn from_iter<I, K, V>(vars: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut bind: Option<String> = None;
        let mut port: Option<String> = None;
        for (key, value) in vars {
            match key.as_ref() {
                ENV_BIND => bind = Some(value.as_ref().to_string()),
                ENV_PORT => port = Some(value.as_ref().to_string()),
                _ => {}
            }
        }

        if let Some(value) = bind {
            let bind_address = value
                .parse::<SocketAddr>()
                .with_context(|| format!("invalid {ENV_BIND} `{value}`"))?;
            return Ok(Self { bind_address });
        }

        if let Some(value) = port {
            let port = value
                .parse::<u16>()
                .with_context(|| format!("invalid {ENV_PORT} `{value}`"))?;
            return Ok(Self {
                bind_address: SocketAddr::from(([0, 0, 0, 0], port)),
            });
        }

        Ok(Self::default())
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

    #[test]
    fn port_env_variable_overrides_the_default() {
        let config = ServerConfig::from_iter([(ENV_PORT, "9090")]).unwrap();
        assert_eq!(config.bind_address.port(), 9090);
    }

    #[test]
    fn bind_env_variable_takes_precedence_over_port() {
        let config =
            ServerConfig::from_iter([(ENV_BIND, "127.0.0.1:1234"), (ENV_PORT, "9090")]).unwrap();
        assert_eq!(config.bind_address.port(), 1234);
        assert_eq!(config.bind_address.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn invalid_bind_is_rejected() {
        let result = ServerConfig::from_iter([(ENV_BIND, "not an address")]);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_port_is_rejected() {
        let result = ServerConfig::from_iter([(ENV_PORT, "not a port")]);
        assert!(result.is_err());
    }

    #[test]
    fn no_variables_falls_back_to_default() {
        let config = ServerConfig::from_iter([("UNRELATED", "x")]).unwrap();
        assert_eq!(config.bind_address.port(), 8080);
    }
}
