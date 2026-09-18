//! Reasons an authentication attempt can fail.

use serde::{Deserialize, Serialize};

/// A stable, machine-readable reason an authentication attempt failed.
///
/// The enum is part of the wire contract: clients can match on it to give
/// specific guidance to the user without parsing the human-readable
/// message that accompanies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthFailureReason {
    /// The provided display name was empty.
    NameEmpty,
    /// The requested username is already taken.
    UsernameTaken,
    /// The provided username is not a valid username.
    UsernameInvalid,
    /// The provided age is outside the accepted range.
    AgeOutOfRange,
    /// The provided password is shorter than the minimum.
    PasswordTooShort,
    /// The username and password combination is not valid.
    InvalidCredentials,
    /// The client is already authenticated.
    AlreadyAuthenticated,
    /// The server failed to complete the operation for an infrastructure
    /// reason. Retrying may work; the details are in the server logs.
    InternalError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_in_snake_case() {
        let value = serde_json::to_string(&AuthFailureReason::UsernameTaken).unwrap();
        assert_eq!(value, "\"username_taken\"");
        let value = serde_json::to_string(&AuthFailureReason::InvalidCredentials).unwrap();
        assert_eq!(value, "\"invalid_credentials\"");
        let value = serde_json::to_string(&AuthFailureReason::InternalError).unwrap();
        assert_eq!(value, "\"internal_error\"");
    }

    #[test]
    fn round_trip() {
        for reason in [
            AuthFailureReason::NameEmpty,
            AuthFailureReason::UsernameTaken,
            AuthFailureReason::UsernameInvalid,
            AuthFailureReason::AgeOutOfRange,
            AuthFailureReason::PasswordTooShort,
            AuthFailureReason::InvalidCredentials,
            AuthFailureReason::AlreadyAuthenticated,
            AuthFailureReason::InternalError,
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            let back: AuthFailureReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, back);
        }
    }
}