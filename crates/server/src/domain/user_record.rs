//! Internal representation of a registered user.

use common::domain::UserProfile;

/// A registered user as stored by the server.
///
/// The password hash is kept here and never leaves the server. The public
/// [`UserProfile`] is the only part that is sent over the wire.
#[derive(Debug, Clone)]
pub struct UserRecord {
    /// The user's public profile.
    pub profile: UserProfile,
    /// The Argon2 password hash, including the salt and parameters.
    pub password_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::domain::{Age, Username};

    fn sample_profile() -> UserProfile {
        UserProfile {
            name: String::from("Alice Example"),
            username: Username::new("alice_99").unwrap(),
            age: Age::new(30).unwrap(),
        }
    }

    #[test]
    fn record_holds_both_fields() {
        let record = UserRecord {
            profile: sample_profile(),
            password_hash: String::from("$argon2id$v=19$m=19456,t=2,p=1$abc$def"),
        };
        assert_eq!(record.profile.username.as_str(), "alice_99");
        assert!(record.password_hash.starts_with("$argon2id$"));
    }
}
