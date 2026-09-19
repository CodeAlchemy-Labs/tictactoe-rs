//! Client identifiers.

use serde::{Deserialize, Serialize};

/// An opaque, server-assigned identifier for a connected client.
///
/// The inner value is private and only the server can create new instances,
/// which prevents clients from forging another player's identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClientId(u64);

impl ClientId {
    /// Creates a new identifier.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the underlying value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "client-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_prefixes_with_client() {
        assert_eq!(ClientId::new(7).to_string(), "client-7");
    }

    #[test]
    fn serde_is_transparent() {
        let json = serde_json::to_string(&ClientId::new(42)).unwrap();
        assert_eq!(json, "42");
        let back: ClientId = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value(), 42);
    }
}
