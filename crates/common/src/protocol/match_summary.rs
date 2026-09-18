//! Summary of an open match shown in the lobby.

use serde::{Deserialize, Serialize};

use super::MatchId;

/// A short description of an open match, sent to clients browsing the lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSummary {
    /// The match identifier.
    pub id: MatchId,
    /// The display name of the player who created the match.
    pub host: String,
    /// The number of spectators currently watching the match.
    ///
    /// Optional on the wire so older servers that predate the spectator
    /// feature remain compatible with newer clients.
    #[serde(default)]
    pub spectator_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let summary = MatchSummary {
            id: MatchId::new(1),
            host: String::from("alice"),
            spectator_count: 3,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let back: MatchSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary, back);
    }

    #[test]
    fn spectator_count_defaults_to_zero_when_absent() {
        let json = r#"{"id":1,"host":"alice"}"#;
        let summary: MatchSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.spectator_count, 0);
    }
}
