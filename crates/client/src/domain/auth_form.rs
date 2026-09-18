//! Authentication form state for the terminal UI.

use common::protocol::{ClientMessage, MatchId};

/// The mode the authentication form is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Enter username and password to authenticate an existing account.
    Login,
    /// Enter the full profile to create a new account.
    Register,
}

impl AuthMode {
    /// Returns the human-readable title for the mode.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Login => "Login",
            Self::Register => "Register",
        }
    }

    /// Toggles between login and register.
    pub const fn toggled(self) -> Self {
        match self {
            Self::Login => Self::Register,
            Self::Register => Self::Login,
        }
    }
}

/// The fields of the authentication form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthField {
    /// Full name. Visible only in register mode.
    Name,
    /// Username. Always visible.
    Username,
    /// Age. Visible only in register mode.
    Age,
    /// Password. Always visible.
    Password,
}

/// An action the user was trying to perform when authentication was
/// required. Retried after a successful login or registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAction {
    /// The user pressed `c` in the lobby.
    CreateMatch,
    /// The user tried to join a specific match.
    JoinMatch(MatchId),
}

/// The state of the authentication form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthForm {
    /// Login or register.
    pub mode: AuthMode,
    /// Full name (register only).
    pub name: String,
    /// Username (both modes).
    pub username: String,
    /// Age as typed (register only). Parsed to `u8` on submit.
    pub age: String,
    /// Password (both modes).
    pub password: String,
    /// The field that currently has focus.
    pub focused: AuthField,
    /// Whether the password is rendered in plain text.
    pub reveal_password: bool,
    /// The last error reported by the server, if any.
    pub error: Option<String>,
    /// The action to retry after a successful authentication.
    pub pending_action: Option<PendingAction>,
}

impl AuthForm {
    /// Creates a new empty form.
    pub fn new(mode: AuthMode, pending_action: Option<PendingAction>) -> Self {
        Self {
            mode,
            name: String::new(),
            username: String::new(),
            age: String::new(),
            password: String::new(),
            focused: Self::visible_fields(mode)[0],
            reveal_password: false,
            error: None,
            pending_action,
        }
    }

    /// Returns the fields visible in the given mode, in focus order.
    pub const fn visible_fields(mode: AuthMode) -> &'static [AuthField] {
        match mode {
            AuthMode::Login => &[AuthField::Username, AuthField::Password],
            AuthMode::Register => &[
                AuthField::Name,
                AuthField::Username,
                AuthField::Age,
                AuthField::Password,
            ],
        }
    }

    /// Appends a character to the focused field.
    ///
    /// The age field accepts only ASCII digits and is capped at three
    /// characters so that the value fits in a `u8` after parsing.
    pub fn push_char(&mut self, character: char) {
        match self.focused {
            AuthField::Name => self.name.push(character),
            AuthField::Username => self.username.push(character),
            AuthField::Age => {
                if character.is_ascii_digit() && self.age.len() < 3 {
                    self.age.push(character);
                }
            }
            AuthField::Password => self.password.push(character),
        }
    }

    /// Removes the last character from the focused field.
    pub fn pop_char(&mut self) {
        match self.focused {
            AuthField::Name => {
                self.name.pop();
            }
            AuthField::Username => {
                self.username.pop();
            }
            AuthField::Age => {
                self.age.pop();
            }
            AuthField::Password => {
                self.password.pop();
            }
        }
    }

    /// Moves focus to the next visible field, wrapping around.
    pub fn focus_next(&mut self) {
        let fields = Self::visible_fields(self.mode);
        let index = fields
            .iter()
            .position(|field| *field == self.focused)
            .unwrap_or(0);
        self.focused = fields[(index + 1) % fields.len()];
    }

    /// Moves focus to the previous visible field, wrapping around.
    pub fn focus_previous(&mut self) {
        let fields = Self::visible_fields(self.mode);
        let index = fields
            .iter()
            .position(|field| *field == self.focused)
            .unwrap_or(0);
        let next = if index == 0 { fields.len() - 1 } else { index - 1 };
        self.focused = fields[next];
    }

    /// Toggles between login and register.
    ///
    /// Clears the current error and resets focus to the first visible
    /// field if the previously focused field is not visible in the new
    /// mode.
    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggled();
        self.error = None;
        let fields = Self::visible_fields(self.mode);
        if !fields.contains(&self.focused) {
            self.focused = fields[0];
        }
    }

    /// Toggles password visibility.
    pub const fn toggle_reveal_password(&mut self) {
        self.reveal_password = !self.reveal_password;
    }

    /// Builds the protocol message for the current form contents.
    ///
    /// # Errors
    ///
    /// Returns a human-readable reason when a required field is missing or
    /// the age cannot be parsed. The server remains the authority on what
    /// is valid; this only prevents obviously incomplete submissions.
    pub fn build_message(&self) -> Result<ClientMessage, &'static str> {
        match self.mode {
            AuthMode::Login => {
                if self.username.trim().is_empty() {
                    return Err("username is required");
                }
                if self.password.is_empty() {
                    return Err("password is required");
                }
                Ok(ClientMessage::Login {
                    username: self.username.clone(),
                    password: self.password.clone(),
                })
            }
            AuthMode::Register => {
                if self.name.trim().is_empty() {
                    return Err("name is required");
                }
                if self.username.trim().is_empty() {
                    return Err("username is required");
                }
                if self.age.is_empty() {
                    return Err("age is required");
                }
                let age = self.age.parse::<u8>().map_err(|_| "age must be a number")?;
                if self.password.is_empty() {
                    return Err("password is required");
                }
                Ok(ClientMessage::Register {
                    name: self.name.clone(),
                    username: self.username.clone(),
                    age,
                    password: self.password.clone(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_form_starts_with_username_focused() {
        let form = AuthForm::new(AuthMode::Login, None);
        assert_eq!(form.focused, AuthField::Username);
    }

    #[test]
    fn register_form_starts_with_name_focused() {
        let form = AuthForm::new(AuthMode::Register, None);
        assert_eq!(form.focused, AuthField::Name);
    }

    #[test]
    fn push_char_routes_to_the_focused_field() {
        let mut form = AuthForm::new(AuthMode::Login, None);
        form.push_char('a');
        assert_eq!(form.username, "a");
        form.focus_next();
        form.push_char('p');
        assert_eq!(form.password, "p");
    }

    #[test]
    fn age_accepts_digits_only() {
        let mut form = AuthForm::new(AuthMode::Register, None);
        form.focused = AuthField::Age;
        for character in "1a2b3c".chars() {
            form.push_char(character);
        }
        assert_eq!(form.age, "123");
    }

    #[test]
    fn age_is_capped_at_three_digits() {
        let mut form = AuthForm::new(AuthMode::Register, None);
        form.focused = AuthField::Age;
        for _ in 0..10 {
            form.push_char('1');
        }
        assert_eq!(form.age, "111");
    }

    #[test]
    fn pop_char_removes_from_the_focused_field() {
        let mut form = AuthForm::new(AuthMode::Login, None);
        form.username = String::from("alice");
        form.pop_char();
        assert_eq!(form.username, "alic");
    }

    #[test]
    fn tab_cycles_through_login_fields() {
        let mut form = AuthForm::new(AuthMode::Login, None);
        assert_eq!(form.focused, AuthField::Username);
        form.focus_next();
        assert_eq!(form.focused, AuthField::Password);
        form.focus_next();
        assert_eq!(form.focused, AuthField::Username);
    }

    #[test]
    fn shift_tab_cycles_backwards() {
        let mut form = AuthForm::new(AuthMode::Login, None);
        assert_eq!(form.focused, AuthField::Username);
        form.focus_previous();
        assert_eq!(form.focused, AuthField::Password);
    }

    #[test]
    fn toggle_mode_switches_to_register_and_resets_focus() {
        let mut form = AuthForm::new(AuthMode::Login, None);
        form.focused = AuthField::Password;
        form.toggle_mode();
        assert_eq!(form.mode, AuthMode::Register);
        assert_eq!(form.focused, AuthField::Name);
    }

    #[test]
    fn toggle_mode_keeps_focus_when_still_visible() {
        let mut form = AuthForm::new(AuthMode::Login, None);
        form.focused = AuthField::Username;
        form.toggle_mode();
        assert_eq!(form.mode, AuthMode::Register);
        assert_eq!(form.focused, AuthField::Username);
    }

    #[test]
    fn build_login_message_requires_username() {
        let form = AuthForm::new(AuthMode::Login, None);
        assert!(form.build_message().is_err());
    }

    #[test]
    fn build_login_message_succeeds_when_filled() {
        let mut form = AuthForm::new(AuthMode::Login, None);
        form.username = String::from("alice_99");
        form.password = String::from("hunter2hunter2");
        assert!(matches!(
            form.build_message(),
            Ok(ClientMessage::Login { .. })
        ));
    }

    #[test]
    fn build_register_message_requires_age_to_parse() {
        let mut form = AuthForm::new(AuthMode::Register, None);
        form.name = String::from("Alice Example");
        form.username = String::from("alice_99");
        form.age = String::from("aaa");
        form.password = String::from("hunter2hunter2");
        assert!(form.build_message().is_err());
    }

    #[test]
    fn build_register_message_succeeds_when_filled() {
        let mut form = AuthForm::new(AuthMode::Register, None);
        form.name = String::from("Alice Example");
        form.username = String::from("alice_99");
        form.age = String::from("30");
        form.password = String::from("hunter2hunter2");
        assert!(matches!(
            form.build_message(),
            Ok(ClientMessage::Register { age: 30, .. })
        ));
    }
}