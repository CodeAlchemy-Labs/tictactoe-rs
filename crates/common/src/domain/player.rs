//! Player marks.

use serde::{Deserialize, Serialize};

/// A player's mark on the board.
///
/// There are exactly two marks in classic Tic-Tac-Toe. The type is modeled as
/// a closed enum so the compiler can prove exhaustiveness at every match site
/// and no invalid value can ever be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Player {
    /// The first player.
    X,
    /// The second player.
    O,
}

impl Player {
    /// Returns the opposite mark.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::X => Self::O,
            Self::O => Self::X,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_is_involutive() {
        assert_eq!(Player::X.other().other(), Player::X);
        assert_eq!(Player::O.other().other(), Player::O);
    }

    #[test]
    fn other_swaps_mark() {
        assert_eq!(Player::X.other(), Player::O);
        assert_eq!(Player::O.other(), Player::X);
    }

    #[test]
    fn serde_round_trip() {
        for mark in [Player::X, Player::O] {
            let json = serde_json::to_string(&mark).unwrap();
            let back: Player = serde_json::from_str(&json).unwrap();
            assert_eq!(mark, back);
        }
    }
}
