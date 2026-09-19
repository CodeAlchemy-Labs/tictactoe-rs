use crate::config::ClientConfig;
use url::Url;

/// Which field of the connection form currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionField {
    Server,
    GuestName,
    Tls,
}

/// Pure model of the connection form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionForm {
    pub host: String,
    pub guest_name: String,
    pub use_tls: bool,
    pub focus: ConnectionField,
    pub error: Option<String>,
}

impl ConnectionForm {
    /// Builds a new form with sensible defaults.
    ///
    /// `host` is pre-filled from the loaded config if present, else left
    /// empty. `guest_name` is pre-filled from the loaded config, else from
    /// `whoami::username()`, else `"guest"`.
    #[must_use]
    pub fn new(config: &ClientConfig) -> Self {
        let host = config
            .server_url
            .as_deref()
            .and_then(|url| Url::parse(url).ok())
            .map(|u| {
                let host_str = u.host_str().unwrap_or_default();
                let port_str = u.port().map(|p| format!(":{p}")).unwrap_or_default();
                let path_str = u.path();
                let path_str = if path_str == "/" { "" } else { path_str };
                format!("{host_str}{port_str}{path_str}")
            })
            .or_else(|| config.server_url.clone())
            .unwrap_or_default();

        let guest_name = config.guest_name.clone().unwrap_or_else(|| {
            let name = whoami::username();
            if name.is_empty() {
                "guest".to_string()
            } else {
                name
            }
        });

        let use_tls = config.use_tls.unwrap_or(false);

        Self {
            host,
            guest_name,
            use_tls,
            focus: ConnectionField::Server,
            error: None,
        }
    }

    /// Appends a character to the focused text field. The TLS field ignores
    /// characters.
    pub fn push_char(&mut self, c: char) {
        self.error = None;
        match self.focus {
            ConnectionField::Server => self.host.push(c),
            ConnectionField::GuestName => self.guest_name.push(c),
            ConnectionField::Tls => {}
        }
    }

    /// Removes the last character of the focused text field.
    pub fn pop_char(&mut self) {
        self.error = None;
        match self.focus {
            ConnectionField::Server => {
                self.host.pop();
            }
            ConnectionField::GuestName => {
                self.guest_name.pop();
            }
            ConnectionField::Tls => {}
        }
    }

    /// Cycles focus forwards (Tab).
    pub fn tab(&mut self) {
        self.error = None;
        self.focus = match self.focus {
            ConnectionField::Server => ConnectionField::GuestName,
            ConnectionField::GuestName => ConnectionField::Tls,
            ConnectionField::Tls => ConnectionField::Server,
        };
    }

    /// Cycles focus backwards (Shift+Tab).
    pub fn shift_tab(&mut self) {
        self.error = None;
        self.focus = match self.focus {
            ConnectionField::Server => ConnectionField::Tls,
            ConnectionField::GuestName => ConnectionField::Server,
            ConnectionField::Tls => ConnectionField::GuestName,
        };
    }

    /// Toggles the TLS field.
    pub fn toggle_tls(&mut self) {
        self.error = None;
        self.use_tls = !self.use_tls;
    }

    /// Validates the form and produces a fully-qualified WebSocket URL.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when the host is empty, the guest
    /// name is empty, or the resulting URL cannot be parsed.
    pub fn build_url(&self) -> Result<String, String> {
        let trimmed_host = self.host.trim();
        let trimmed_name = self.guest_name.trim();

        if trimmed_host.is_empty() {
            return Err("Host cannot be empty".to_string());
        }
        if trimmed_host.contains(' ') {
            return Err("Host cannot contain spaces".to_string());
        }
        if trimmed_name.is_empty() {
            return Err("Guest name cannot be empty".to_string());
        }

        let scheme = if self.use_tls { "wss" } else { "ws" };
        let url_str = format!("{scheme}://{trimmed_host}");

        match Url::parse(&url_str) {
            Ok(url) => match url.scheme() {
                "ws" | "wss" => Ok(url_str),
                other => Err(format!(
                    "unsupported URL scheme `{other}`; expected `ws` or `wss`"
                )),
            },
            Err(e) => Err(format!("Invalid URL: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_form_starts_on_host_field() {
        let form = ConnectionForm::new(&ClientConfig::default());
        assert_eq!(form.focus, ConnectionField::Server);
    }

    #[test]
    fn push_char_updates_focused_field() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.push_char('a');
        assert_eq!(form.host, "a");
        form.tab();
        form.guest_name.clear(); // clear default guest name
        form.push_char('b');
        assert_eq!(form.guest_name, "b");
    }

    #[test]
    fn pop_char_updates_focused_field() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.host = "abc".to_string();
        form.pop_char();
        assert_eq!(form.host, "ab");
    }

    #[test]
    fn connection_form_tab_cycles_fields() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.tab();
        assert_eq!(form.focus, ConnectionField::GuestName);
        form.tab();
        assert_eq!(form.focus, ConnectionField::Tls);
        form.tab();
        assert_eq!(form.focus, ConnectionField::Server);
    }

    #[test]
    fn connection_form_shift_tab_cycles_backwards() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.shift_tab();
        assert_eq!(form.focus, ConnectionField::Tls);
        form.shift_tab();
        assert_eq!(form.focus, ConnectionField::GuestName);
        form.shift_tab();
        assert_eq!(form.focus, ConnectionField::Server);
    }

    #[test]
    fn connection_form_toggle_tls() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.use_tls = false;
        form.toggle_tls();
        assert!(form.use_tls);
        form.toggle_tls();
        assert!(!form.use_tls);
    }

    #[test]
    fn connection_form_enter_with_empty_host_sets_error() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.host.clear();
        assert_eq!(form.build_url().unwrap_err(), "Host cannot be empty");
    }

    #[test]
    fn connection_form_rejects_spaces_in_host() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.host = "localhost 8080".to_string();
        assert_eq!(form.build_url().unwrap_err(), "Host cannot contain spaces");
    }

    #[test]
    fn connection_form_enter_with_empty_guest_name_sets_error() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.host = "localhost".to_string();
        form.guest_name.clear();
        assert_eq!(form.build_url().unwrap_err(), "Guest name cannot be empty");
    }

    #[test]
    fn connection_form_builds_url_correctly() {
        let mut form = ConnectionForm::new(&ClientConfig::default());
        form.host = "localhost:8080/ws".to_string();
        form.guest_name = "alice".to_string();
        form.use_tls = false;
        assert_eq!(form.build_url().unwrap(), "ws://localhost:8080/ws");

        form.use_tls = true;
        assert_eq!(form.build_url().unwrap(), "wss://localhost:8080/ws");
    }
}
