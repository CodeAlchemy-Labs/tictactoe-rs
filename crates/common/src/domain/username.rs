//! Validated usernames.

use std::fmt;
use std::hash::{Hash, Hasher};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::DomainError;

/// The reasons a username can be rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameError {
    /// The username is shorter than the minimum length.
    TooShort {
        /// The minimum accepted length, in characters.
        min: usize,
        /// The actual length.
        actual: usize,
    },
    /// The username is longer than the maximum length.
    TooLong {
        /// The maximum accepted length, in characters.
        max: usize,
        /// The actual length.
        actual: usize,
    },
    /// The username contains a character outside `[a-zA-Z0-9_]`.
    InvalidCharacter {
        /// The offending character.
        character: char,
    },
}

impl fmt::Display for UsernameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { min, actual } => write!(
                formatter,
                "too short ({actual} characters, minimum is {min})"
            ),
            Self::TooLong { max, actual } => write!(
                formatter,
                "too long ({actual} characters, maximum is {max})"
            ),
            Self::InvalidCharacter { character } => {
                write!(formatter, "invalid character `{character}`")
            }
        }
    }
}

/// A validated, case-insensitive username.
///
/// The original casing is preserved for display. Equality and hashing are
/// based on the lowercased form, so `Alice` and `alice` refer to the same
/// user. This is what users expect from a username field, and it also keeps
/// lookups stable regardless of how a client formats the value.
#[derive(Debug, Clone)]
pub struct Username {
    display: String,
    key: String,
}

impl Username {
    /// The minimum accepted length, in characters.
    pub const MIN_LEN: usize = 5;
    /// The maximum accepted length, in characters.
    pub const MAX_LEN: usize = 20;

    /// Creates a new username, validating it against the accepted rules.
    ///
    /// Leading and trailing whitespace is trimmed before validation, so a
    /// client that accidentally sends `" alice "` is accepted and stored
    /// as `"alice"`.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidUsername`] when the value is too
    /// short, too long, or contains a character outside `[a-zA-Z0-9_]`.
    pub fn new(value: &str) -> Result<Self, DomainError> {
        let trimmed = value.trim();
        let length = trimmed.chars().count();

        if length < Self::MIN_LEN {
            return Err(DomainError::InvalidUsername {
                value: value.to_string(),
                reason: UsernameError::TooShort {
                    min: Self::MIN_LEN,
                    actual: length,
                },
            });
        }
        if length > Self::MAX_LEN {
            return Err(DomainError::InvalidUsername {
                value: value.to_string(),
                reason: UsernameError::TooLong {
                    max: Self::MAX_LEN,
                    actual: length,
                },
            });
        }
        for character in trimmed.chars() {
            if !character.is_ascii_alphanumeric() && character != '_' {
                return Err(DomainError::InvalidUsername {
                    value: value.to_string(),
                    reason: UsernameError::InvalidCharacter { character },
                });
            }
        }

        let display = trimmed.to_string();
        let key = display.to_lowercase();
        Ok(Self { display, key })
    }

    /// Returns the username with its original casing.
    pub fn as_str(&self) -> &str {
        &self.display
    }

    /// Returns the lowercase lookup key used for equality and hashing.
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl PartialEq for Username {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Username {}

impl Hash for Username {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl fmt::Display for Username {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display)
    }
}

impl Serialize for Username {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.display)
    }
}

impl<'de> Deserialize<'de> for Username {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(&raw).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_usernames() {
        assert!(Username::new("alice").is_ok());
        assert!(Username::new("alice_bob_123").is_ok());
        assert!(Username::new("A_1_b_2_c").is_ok());
    }

    #[test]
    fn accepts_the_boundary_lengths() {
        let min = "a".repeat(Username::MIN_LEN);
        let max = "a".repeat(Username::MAX_LEN);
        assert!(Username::new(&min).is_ok());
        assert!(Username::new(&max).is_ok());
    }

    #[test]
    fn preserves_original_casing() {
        let username = Username::new("Alice_99").unwrap();
        assert_eq!(username.as_str(), "Alice_99");
        assert_eq!(username.key(), "alice_99");
    }

    #[test]
    fn case_insensitive_equality() {
        let upper = Username::new("Alice").unwrap();
        let lower = Username::new("alice").unwrap();
        assert_eq!(upper, lower);
    }

    #[test]
    fn rejects_short_username() {
        let result = Username::new("abc");
        assert!(matches!(
            result,
            Err(DomainError::InvalidUsername {
                reason: UsernameError::TooShort { .. },
                ..
            })
        ));
    }

    #[test]
    fn rejects_long_username() {
        let long = "a".repeat(Username::MAX_LEN + 1);
        let result = Username::new(&long);
        assert!(matches!(
            result,
            Err(DomainError::InvalidUsername {
                reason: UsernameError::TooLong { .. },
                ..
            })
        ));
    }

    #[test]
    fn rejects_invalid_characters() {
        for bad in ["alice!", "alice bob", "alice-bob", "alic\u{e9}"] {
            let result = Username::new(bad);
            assert!(
                matches!(
                    result,
                    Err(DomainError::InvalidUsername {
                        reason: UsernameError::InvalidCharacter { .. },
                        ..
                    })
                ),
                "expected invalid character for `{bad}`"
            );
        }
    }

    #[test]
    fn trims_whitespace_before_validating() {
        let username = Username::new("  alice  ").unwrap();
        assert_eq!(username.as_str(), "alice");
    }

    #[test]
    fn serde_round_trip() {
        let username = Username::new("Alice_99").unwrap();
        let json = serde_json::to_string(&username).unwrap();
        assert_eq!(json, "\"Alice_99\"");
        let back: Username = serde_json::from_str(&json).unwrap();
        assert_eq!(back.as_str(), "Alice_99");
    }

    #[test]
    fn serde_rejects_invalid_username() {
        let result: Result<Username, _> = serde_json::from_str("\"a!\"");
        assert!(result.is_err());
    }
}
