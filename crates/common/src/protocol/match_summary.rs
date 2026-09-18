//! Summary of an open match shown in the lobby.

use serde::{Deserialize, Serialize};

use super::MatchId;

/// A short description of a match, sent to clients browsing the lobby.
///
/// The server sends every match, full or not, and lets the client decide
/// what to display. A client that wants to join filters out the full ones;
/// a client that wants to spectate shows them all. Keeping the decision on
/// the client side means the server does not need to know which mode the
/// user is in.
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
    /// Whether both player slots are occupied.
    ///
    /// Optional on the wire for the same reason as `spectator_count`.
    #[serde(default)]
    pub is_full: bool,
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
            is_full: true,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let back: MatchSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary, back);
    }

    #[test]
    fn optional_fields_default_when_absent() {
        let json = r#"{"id":1,"host":"alice"}"#;
        let summary: MatchSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.spectator_count, 0);
        assert!(!summary.is_full);
    }
}
