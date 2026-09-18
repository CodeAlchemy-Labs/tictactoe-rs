//! Validated ages.

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// A validated age, restricted to `8..=90`.
///
/// The upper bound reflects the fact that this is a game intended for
/// children as young as eight; the lower bound is a deliberate cap that
/// keeps the field sensible for a demo that does not need to store ages
/// beyond a realistic human lifespan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Age(u8);

impl Age {
    /// The minimum accepted age.
    pub const MIN: u8 = 8;
    /// The maximum accepted age.
    pub const MAX: u8 = 90;

    /// Creates a new age, validating it against the accepted range.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidAge`] when `value` is outside
    /// `8..=90`.
    pub const fn new(value: u8) -> Result<Self, DomainError> {
        if value >= Self::MIN && value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidAge { value })
        }
    }

    /// Returns the age as a `u8`.
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Age {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Age> for u8 {
    fn from(value: Age) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_boundary_values() {
        assert!(Age::new(Age::MIN).is_ok());
        assert!(Age::new(Age::MAX).is_ok());
    }

    #[test]
    fn accepts_every_value_in_the_range() {
        for value in Age::MIN..=Age::MAX {
            assert!(Age::new(value).is_ok(), "expected {value} to be valid");
        }
    }

    #[test]
    fn rejects_below_the_minimum() {
        for value in 0..Age::MIN {
            assert!(Age::new(value).is_err(), "expected {value} to be rejected");
        }
    }

    #[test]
    fn rejects_above_the_maximum() {
        for value in (Age::MAX + 1)..=u8::MAX {
            assert!(Age::new(value).is_err(), "expected {value} to be rejected");
        }
    }

    #[test]
    fn serde_round_trip() {
        let age = Age::new(25).unwrap();
        let json = serde_json::to_string(&age).unwrap();
        assert_eq!(json, "25");
        let back: Age = serde_json::from_str(&json).unwrap();
        assert_eq!(age, back);
    }

    #[test]
    fn serde_rejects_out_of_range() {
        let result: Result<Age, _> = serde_json::from_str("7");
        assert!(result.is_err());
    }
}