//! Board positions.

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// A validated position on the 3x3 board.
///
/// Positions are indexed `0` through `8` in row-major order:
///
/// ```text
///   0 | 1 | 2
///  ---+---+---
///   3 | 4 | 5
///  ---+---+---
///   6 | 7 | 8
/// ```
///
/// The inner value is private and can only be built through [`Position::new`],
/// which validates the range. Once constructed, the value is guaranteed to be
/// in range forever, which removes an entire class of bounds-checking bugs at
/// every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Position(u8);

impl Position {
    /// The total number of positions on a classic board.
    pub const COUNT: u8 = 9;

    /// Creates a new position, validating the range `0..=8`.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidPosition`] when `value` is out of range.
    pub const fn new(value: u8) -> Result<Self, DomainError> {
        if value < Self::COUNT {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidPosition { value })
        }
    }

    /// Returns the zero-based index into the flat cell array.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Returns the row (`0`, `1`, or `2`) of the position.
    #[must_use]
    pub const fn row(self) -> u8 {
        self.0 / 3
    }

    /// Returns the column (`0`, `1`, or `2`) of the position.
    #[must_use]
    pub const fn column(self) -> u8 {
        self.0 % 3
    }

    /// Iterates over every valid position.
    pub fn iter() -> impl Iterator<Item = Self> {
        (0..Self::COUNT).map(Self)
    }
}

impl TryFrom<u8> for Position {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Position> for u8 {
    fn from(value: Position) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_positions_construct() {
        for value in 0..9 {
            assert!(Position::new(value).is_ok());
        }
    }

    #[test]
    fn invalid_position_is_rejected() {
        assert!(Position::new(9).is_err());
        assert!(Position::new(u8::MAX).is_err());
    }

    #[test]
    fn row_and_column_are_computed_correctly() {
        let cases = [
            (0, 0, 0),
            (1, 0, 1),
            (2, 0, 2),
            (3, 1, 0),
            (4, 1, 1),
            (5, 1, 2),
            (6, 2, 0),
            (7, 2, 1),
            (8, 2, 2),
        ];
        for (value, expected_row, expected_column) in cases {
            let position = Position::new(value).unwrap();
            assert_eq!(position.row(), expected_row);
            assert_eq!(position.column(), expected_column);
        }
    }

    #[test]
    fn iter_yields_every_valid_position() {
        let positions: Vec<_> = Position::iter().collect();
        assert_eq!(positions.len(), 9);
        for (index, position) in positions.iter().enumerate() {
            assert_eq!(position.index(), index);
        }
    }

    #[test]
    fn serde_round_trip_preserves_index() {
        for value in 0..9 {
            let position = Position::new(value).unwrap();
            let json = serde_json::to_string(&position).unwrap();
            let back: Position = serde_json::from_str(&json).unwrap();
            assert_eq!(position, back);
        }
    }

    #[test]
    fn serde_rejects_out_of_range() {
        let result: Result<Position, _> = serde_json::from_str("9");
        assert!(result.is_err());
    }
}
