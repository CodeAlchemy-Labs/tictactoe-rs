//! Runtime configuration.

use anyhow::{Context, bail};

/// Configuration for the terminal client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The WebSocket URL of the server, e.g. `ws://server:8080/ws`.
    pub server_url: String,
    /// The display name to announce via `Hello`.
    pub display_name: String,
}

impl ClientConfig {
    /// Builds the configuration from command-line arguments, falling back to
    /// environment variables and then to defaults.
    ///
    /// Accepted arguments:
    ///
    /// - `--server <url>`: sets the WebSocket URL.
    /// - `--name <name>`: sets the display name.
    ///
    /// Recognized environment variables:
    ///
    /// - `TICTACTOE_SERVER`
    /// - `TICTACTOE_NAME`
    ///
    /// # Errors
    ///
    /// Returns an error if a required value is missing after all fallbacks
    /// have been tried.
    pub fn from_args_and_env() -> anyhow::Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut server_url: Option<String> = None;
        let mut display_name: Option<String> = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--server" => {
                    server_url = Some(args.next().context("--server requires a value")?);
                }
                "--name" => {
                    display_name = Some(args.next().context("--name requires a value")?);
                }
                other => bail!("unrecognized argument `{other}`"),
            }
        }

        let server_url = server_url
            .or_else(|| std::env::var("TICTACTOE_SERVER").ok())
            .unwrap_or_else(|| String::from("ws://127.0.0.1:8080/ws"));

        let display_name = display_name
            .or_else(|| std::env::var("TICTACTOE_NAME").ok())
            .context("display name is required: pass --name or set TICTACTOE_NAME")?;

        Ok(Self {
            server_url,
            display_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_holds_both_fields() {
        let config = ClientConfig {
            server_url: String::from("ws://x/ws"),
            display_name: String::from("alice"),
        };
        assert_eq!(config.display_name, "alice");
        assert_eq!(config.server_url, "ws://x/ws");
    }
}
