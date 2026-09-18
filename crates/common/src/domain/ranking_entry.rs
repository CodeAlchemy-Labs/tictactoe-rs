//! Ranking entries.

use serde::{Deserialize, Serialize};

use super::Username;

/// A single entry in the top-players ranking.
///
/// The `username` is the stable identity of the player; the `name` is the
/// display name shown in the UI. The `wins` counter only counts games won
/// by normal play (not by abandonment).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankingEntry {
    /// The player's unique username.
    pub username: Username,
    /// The player's display name at the time of the last recorded win.
    pub name: String,
    /// The number of games the player has won.
    pub wins: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let entry = RankingEntry {
            username: Username::new("alice_99").unwrap(),
            name: String::from("Alice Example"),
            wins: 7,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: RankingEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }
}
