//! Match identifiers.

use serde::{Deserialize, Serialize};

/// An opaque, server-assigned identifier for a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MatchId(u64);

impl MatchId {
    /// Creates a new identifier.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the underlying value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for MatchId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "match-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_prefixes_with_match() {
        assert_eq!(MatchId::new(3).to_string(), "match-3");
    }

    #[test]
    fn serde_is_transparent() {
        let json = serde_json::to_string(&MatchId::new(9)).unwrap();
        assert_eq!(json, "9");
        let back: MatchId = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value(), 9);
    }
}
