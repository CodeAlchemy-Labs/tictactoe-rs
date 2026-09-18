//! Public user profiles.

use serde::{Deserialize, Serialize};

use super::{Age, Username};

/// The public profile of a registered user.
///
/// The profile never contains the password or its hash. Those live only on
/// the server and are never sent over the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfile {
    /// The user's display name.
    pub name: String,
    /// The user's unique username.
    pub username: Username,
    /// The user's age.
    pub age: Age,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let profile = UserProfile {
            name: String::from("Alice Example"),
            username: Username::new("alice_99").unwrap(),
            age: Age::new(30).unwrap(),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let back: UserProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, back);
    }
}