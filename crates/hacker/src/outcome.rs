//! Outcome type for a scenario run.

/// The result of running one scenario against the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The server defended against the probe.
    Defended {
        /// A human-readable explanation of what was observed.
        detail: String,
    },
    /// The server was successfully attacked.
    Compromised {
        /// A human-readable explanation of the failure.
        detail: String,
    },
}

impl Outcome {
    /// Constructs a `Defended` outcome.
    pub fn defended(detail: impl Into<String>) -> Self {
        Self::Defended {
            detail: detail.into(),
        }
    }

    /// Constructs a `Compromised` outcome.
    pub fn compromised(detail: impl Into<String>) -> Self {
        Self::Compromised {
            detail: detail.into(),
        }
    }

    /// Returns `true` when the server defended.
    pub const fn is_defended(&self) -> bool {
        matches!(self, Self::Defended { .. })
    }

    /// Returns the uppercase label used in the report line.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Defended { .. } => "DEFENDED",
            Self::Compromised { .. } => "COMPROMISED",
        }
    }

    /// Returns the human-readable detail string.
    pub fn detail(&self) -> &str {
        match self {
            Self::Defended { detail } | Self::Compromised { detail } => detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defended_is_reported_correctly() {
        let outcome = Outcome::defended("ok");
        assert!(outcome.is_defended());
        assert_eq!(outcome.label(), "DEFENDED");
        assert_eq!(outcome.detail(), "ok");
    }

    #[test]
    fn compromised_is_reported_correctly() {
        let outcome = Outcome::compromised("bad");
        assert!(!outcome.is_defended());
        assert_eq!(outcome.label(), "COMPROMISED");
        assert_eq!(outcome.detail(), "bad");
    }
}