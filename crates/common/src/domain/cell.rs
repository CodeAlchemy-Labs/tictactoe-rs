//! Board cells.

use serde::{Deserialize, Serialize};

use super::Player;

/// The state of a single cell on the board.
///
/// A cell is either empty or claimed by exactly one player. Modeling it as a
/// closed enum instead of `Option<Player>` keeps the intent explicit and lets
/// the compiler enforce that all cases are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Cell {
    /// The cell has not been played yet.
    #[default]
    Empty,
    /// The cell has been claimed by the given player.
    Occupied(Player),
}

impl Cell {
    /// Returns `true` if the cell is empty.
    pub const fn is_empty(self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns the owning player, or `None` if the cell is empty.
    pub const fn owner(self) -> Option<Player> {
        match self {
            Self::Empty => None,
            Self::Occupied(player) => Some(player),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cell_has_no_owner() {
        assert!(Cell::Empty.is_empty());
        assert_eq!(Cell::Empty.owner(), None);
    }

    #[test]
    fn occupied_cell_reports_owner() {
        let cell = Cell::Occupied(Player::X);
        assert!(!cell.is_empty());
        assert_eq!(cell.owner(), Some(Player::X));
    }

    #[test]
    fn serde_round_trip() {
        for cell in [
            Cell::Empty,
            Cell::Occupied(Player::X),
            Cell::Occupied(Player::O),
        ] {
            let json = serde_json::to_string(&cell).unwrap();
            let back: Cell = serde_json::from_str(&json).unwrap();
            assert_eq!(cell, back);
        }
    }
}
