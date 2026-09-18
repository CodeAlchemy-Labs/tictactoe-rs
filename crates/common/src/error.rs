//! Errors raised by the domain and protocol layers.
//!
//! The crate exposes two error enums:
//!
//! - [`DomainError`]: violations of the domain rules (invalid positions,
//!   occupied cells, invalid usernames, ages out of range).
//! - [`ProtocolError`]: failures while reading or writing wire messages.
//!
//! Both implement [`std::error::Error`] through `thiserror`, so they compose
//! cleanly with `anyhow` in the binary crates and with `?` in library code.

use thiserror::Error;

use crate::domain::{Position, UsernameError};

/// Errors raised by the domain layer.
#[derive(Debug, Error)]
pub enum DomainError {
    /// The requested position is outside the valid range.
    #[error("position {value} is out of range; valid values are 0..=8")]
    InvalidPosition {
        /// The offending value.
        value: u8,
    },

    /// The requested cell is already occupied.
    #[error("cell at position {position:?} is already occupied")]
    CellOccupied {
        /// The position that was already taken.
        position: Position,
    },

    /// The provided username fails validation.
    #[error("invalid username `{value}`: {reason}")]
    InvalidUsername {
        /// The offending value.
        value: String,
        /// Why the username was rejected.
        reason: UsernameError,
    },

    /// The provided age is outside the accepted range.
    #[error("age {value} is out of range; valid values are 8..=90")]
    InvalidAge {
        /// The offending value.
        value: u8,
    },

    /// A display name was required but was empty.
    #[error("display name must not be empty")]
    EmptyName,
}

/// Errors raised by the protocol layer.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// A message could not be serialized or deserialized.
    #[error("failed to (de)serialize a protocol message: {0}")]
    Serde(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_position_message_includes_value() {
        let error = DomainError::InvalidPosition { value: 9 };
        let rendered = error.to_string();
        assert!(rendered.contains('9'));
    }

    #[test]
    fn invalid_age_message_includes_value() {
        let error = DomainError::InvalidAge { value: 91 };
        let rendered = error.to_string();
        assert!(rendered.contains("91"));
    }
}
