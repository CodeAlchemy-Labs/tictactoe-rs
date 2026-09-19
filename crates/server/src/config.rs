//! Runtime configuration.

use std::net::SocketAddr;

use anyhow::Context;

/// Environment variable that overrides the bind address entirely.
pub const ENV_BIND: &str = "TICTACTOE_BIND";

/// Environment variable used by Render and similar platforms to specify the
/// port the process must listen on.
pub const ENV_PORT: &str = "PORT";

/// Environment variable for the deployment environment.
pub const ENV_TICTACTOE_ENV: &str = "TICTACTOE_ENV";

/// Environment variable for global session limit.
pub const ENV_MAX_SESSIONS: &str = "TICTACTOE_MAX_SESSIONS";

/// Environment variable for per-IP session limit.
pub const ENV_MAX_SESSIONS_PER_IP: &str = "TICTACTOE_MAX_SESSIONS_PER_IP";

/// Environment variable for authentication rate limit.
pub const ENV_AUTH_RATE_LIMIT: &str = "TICTACTOE_AUTH_RATE_LIMIT_PER_MINUTE";

/// The deployment environment mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    /// Development environment with relaxed limits.
    #[default]
    Development,
    /// Production environment with strict limits.
    Production,
}

impl std::str::FromStr for Environment {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" => Ok(Self::Development),
            "production" => Ok(Self::Production),
            _ => anyhow::bail!("invalid environment `{s}`"),
        }
    }
}

/// Configuration for the HTTP and WebSocket server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// The deployment environment mode.
    pub env: Environment,
    /// The address the server binds to.
    pub bind_address: SocketAddr,
    /// The global maximum concurrent sessions limit.
    pub max_sessions: usize,
    /// The per-IP maximum concurrent sessions limit.
    pub max_sessions_per_ip: usize,
    /// The maximum number of auth attempts (register/login) per minute per IP.
    pub auth_rate_limit_per_minute: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            env: Environment::Development,
            bind_address: SocketAddr::from(([0, 0, 0, 0], 8080)),
            max_sessions: 10000,
            max_sessions_per_ip: 10000, // Very relaxed for dev by default
            auth_rate_limit_per_minute: 1000, // Very relaxed for dev by default
        }
    }
}

impl ServerConfig {
    fn default_production() -> Self {
        Self {
            env: Environment::Production,
            bind_address: SocketAddr::from(([0, 0, 0, 0], 8080)),
            max_sessions: 10000,
            max_sessions_per_ip: 10,
            auth_rate_limit_per_minute: 10,
        }
    }

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
        let mut vars = Vec::new();
        for (key, value) in std::env::vars() {
            vars.push((key, value));
        }
        Self::from_vars(vars)
    }

    /// Builds the configuration from a set of key/value pairs provided
    /// explicitly. Intended for tests.
    ///
    /// # Errors
    ///
    /// Returns an error if a provided variable does not parse.
    pub fn from_vars<I, K, V>(vars: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut bind: Option<String> = None;
        let mut port: Option<String> = None;
        let mut env: Option<String> = None;
        let mut max_sessions: Option<String> = None;
        let mut max_sessions_per_ip: Option<String> = None;
        let mut auth_rate_limit: Option<String> = None;

        for (key, value) in vars {
            match key.as_ref() {
                ENV_BIND => bind = Some(value.as_ref().to_string()),
                ENV_PORT => port = Some(value.as_ref().to_string()),
                ENV_TICTACTOE_ENV => env = Some(value.as_ref().to_string()),
                ENV_MAX_SESSIONS => max_sessions = Some(value.as_ref().to_string()),
                ENV_MAX_SESSIONS_PER_IP => max_sessions_per_ip = Some(value.as_ref().to_string()),
                ENV_AUTH_RATE_LIMIT => auth_rate_limit = Some(value.as_ref().to_string()),
                _ => {}
            }
        }

        let environment = match env {
            Some(e) => e.parse::<Environment>()?,
            None => Environment::Development,
        };

        let mut config = match environment {
            Environment::Development => Self::default(),
            Environment::Production => Self::default_production(),
        };

        if let Some(value) = bind {
            config.bind_address = value
                .parse::<SocketAddr>()
                .with_context(|| format!("invalid {ENV_BIND} `{value}`"))?;
        } else if let Some(value) = port {
            let port = value
                .parse::<u16>()
                .with_context(|| format!("invalid {ENV_PORT} `{value}`"))?;
            config.bind_address = SocketAddr::from(([0, 0, 0, 0], port));
        }

        if let Some(value) = max_sessions {
            config.max_sessions = value
                .parse::<usize>()
                .with_context(|| format!("invalid {ENV_MAX_SESSIONS} `{value}`"))?;
        }

        if let Some(value) = max_sessions_per_ip {
            config.max_sessions_per_ip = value
                .parse::<usize>()
                .with_context(|| format!("invalid {ENV_MAX_SESSIONS_PER_IP} `{value}`"))?;
        }

        if let Some(value) = auth_rate_limit {
            config.auth_rate_limit_per_minute = value
                .parse::<u32>()
                .with_context(|| format!("invalid {ENV_AUTH_RATE_LIMIT} `{value}`"))?;
        }

        Ok(config)
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
        let config = ServerConfig::from_vars([(ENV_PORT, "9090")]).unwrap();
        assert_eq!(config.bind_address.port(), 9090);
    }

    #[test]
    fn bind_env_variable_takes_precedence_over_port() {
        let config =
            ServerConfig::from_vars([(ENV_BIND, "127.0.0.1:1234"), (ENV_PORT, "9090")]).unwrap();
        assert_eq!(config.bind_address.port(), 1234);
        assert_eq!(config.bind_address.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn invalid_bind_is_rejected() {
        let result = ServerConfig::from_vars([(ENV_BIND, "not an address")]);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_port_is_rejected() {
        let result = ServerConfig::from_vars([(ENV_PORT, "not a port")]);
        assert!(result.is_err());
    }

    #[test]
    fn no_variables_falls_back_to_default() {
        let config = ServerConfig::from_vars([("UNRELATED", "x")]).unwrap();
        assert_eq!(config.bind_address.port(), 8080);
    }
    #[test]
    fn parses_development_env_correctly() {
        let config = ServerConfig::from_vars([(ENV_TICTACTOE_ENV, "development")]).unwrap();
        assert_eq!(config.env, Environment::Development);
    }

    #[test]
    fn parses_production_env_correctly() {
        let config = ServerConfig::from_vars([(ENV_TICTACTOE_ENV, "production")]).unwrap();
        assert_eq!(config.env, Environment::Production);
        assert_eq!(config.max_sessions_per_ip, 10);
    }

    #[test]
    fn invalid_env_fails() {
        let result = ServerConfig::from_vars([(ENV_TICTACTOE_ENV, "staging")]);
        assert!(result.is_err());
    }

    #[test]
    fn overrides_limits_from_vars() {
        let config = ServerConfig::from_vars([
            (ENV_MAX_SESSIONS, "50"),
            (ENV_MAX_SESSIONS_PER_IP, "2"),
            (ENV_AUTH_RATE_LIMIT, "5"),
        ])
        .unwrap();
        assert_eq!(config.max_sessions, 50);
        assert_eq!(config.max_sessions_per_ip, 2);
        assert_eq!(config.auth_rate_limit_per_minute, 5);
    }
}
