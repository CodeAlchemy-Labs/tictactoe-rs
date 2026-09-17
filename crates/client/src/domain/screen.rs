//! The screen the client is currently showing.

use common::domain::GameStatus;
use common::protocol::MatchSummary;

/// A match the local user has already engaged with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMatch {
    /// The match identifier.
    pub id: common::protocol::MatchId,
    /// The opponent's display name.
    pub opponent: String,
    /// The mark assigned to the local player.
    pub your_mark: common::domain::Player,
    /// The current board.
    pub board: common::domain::Board,
    /// Whose turn it is.
    pub current_turn: common::domain::Player,
    /// The current status.
    pub status: GameStatus,
}

/// The screen currently shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// Waiting for the connection to be established and the welcome message.
    Connecting,
    /// Browsing the list of open matches.
    Lobby {
        /// The most recent snapshot of open matches.
        matches: Vec<MatchSummary>,
    },
    /// Playing a match.
    InGame(Box<ActiveMatch>),
    /// The match has ended.
    Finished {
        /// The final board.
        board: common::domain::Board,
        /// The final status.
        status: GameStatus,
    },
    /// A fatal error occurred; the UI will display it and the process exits.
    Fatal(String),
}

impl Screen {
    /// Returns a short label suitable for the header.
    pub const fn title(&self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::Lobby { .. } => "Lobby",
            Self::InGame(_) => "Match",
            Self::Finished { .. } => "Result",
            Self::Fatal(_) => "Error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_matches_variant() {
        assert_eq!(Screen::Connecting.title(), "Connecting");
        assert_eq!(Screen::Lobby { matches: vec![] }.title(), "Lobby");
        assert_eq!(Screen::Fatal(String::from("boom")).title(), "Error");
    }
}