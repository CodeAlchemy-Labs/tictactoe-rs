//! Game status.

use serde::{Deserialize, Serialize};

use super::Player;

/// The current status of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameStatus {
    /// The game is still being played.
    InProgress,
    /// The game ended with a winner.
    Won(Player),
    /// The game ended in a draw.
    Draw,
}

impl GameStatus {
    /// Returns `true` if the game has ended.
    #[must_use]
    pub const fn is_finished(self) -> bool {
        !matches!(self, Self::InProgress)
    }

    /// Returns the winner, if any.
    #[must_use]
    pub const fn winner(self) -> Option<Player> {
        match self {
            Self::Won(player) => Some(player),
            Self::InProgress | Self::Draw => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_progress_is_not_finished_and_has_no_winner() {
        let status = GameStatus::InProgress;
        assert!(!status.is_finished());
        assert_eq!(status.winner(), None);
    }

    #[test]
    fn won_is_finished_and_reports_winner() {
        let status = GameStatus::Won(Player::X);
        assert!(status.is_finished());
        assert_eq!(status.winner(), Some(Player::X));
    }

    #[test]
    fn draw_is_finished_without_winner() {
        let status = GameStatus::Draw;
        assert!(status.is_finished());
        assert_eq!(status.winner(), None);
    }

    #[test]
    fn serde_round_trip() {
        for status in [
            GameStatus::InProgress,
            GameStatus::Won(Player::X),
            GameStatus::Won(Player::O),
            GameStatus::Draw,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: GameStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }
}
