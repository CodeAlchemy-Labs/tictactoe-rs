//! The screen the client is currently showing.

use common::domain::{Board, GameStatus, Player, RankingEntry};
use common::protocol::{MatchId, MatchSummary};

use super::auth_form::AuthForm;

/// A match the local user has already engaged with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMatch {
    /// The match identifier.
    pub id: MatchId,
    /// The opponent's display name.
    pub opponent: String,
    /// The mark assigned to the local player.
    pub your_mark: Player,
    /// The current board.
    pub board: Board,
    /// Whose turn it is.
    pub current_turn: Player,
    /// The current status.
    pub status: GameStatus,
}

/// The state of the match the local user is spectating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectatedMatch {
    /// The match identifier.
    pub id: MatchId,
    /// The host's display name.
    pub host_name: String,
    /// The guest's display name, or an empty string when no guest has joined.
    pub guest_name: String,
    /// The current board.
    pub board: Board,
    /// Whose turn it is.
    pub current_turn: Player,
    /// The current status.
    pub status: GameStatus,
    /// The number of spectators, including the local user.
    pub spectator_count: u32,
}

/// The screen currently shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// Waiting for the connection to be established and the welcome message.
    Connecting,
    /// Authenticating as a registered user.
    Auth(Box<AuthForm>),
    /// Browsing the list of open matches.
    Lobby {
        /// The most recent snapshot of open matches.
        matches: Vec<MatchSummary>,
        /// When `true`, digits select a match to spectate instead of a match
        /// to join. The mode is toggled with `s` and cleared with `Esc`.
        spectator_mode: bool,
    },
    /// Viewing the top-players ranking.
    Ranking {
        /// The most recent snapshot of the ranking.
        entries: Vec<RankingEntry>,
    },
    /// Playing a match.
    InGame(Box<ActiveMatch>),
    /// Watching a match without playing it.
    Spectating(Box<SpectatedMatch>),
    /// The match has ended.
    Finished {
        /// The final board.
        board: Board,
        /// The final status.
        status: GameStatus,
        /// The winner's display name, when the match ended with a victory.
        winner_name: Option<String>,
    },
    /// A fatal error occurred; the UI will display it and the process exits.
    Fatal(String),
}

impl Screen {
    /// Returns a short label suitable for the header.
    pub const fn title(&self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::Auth(_) => "Auth",
            Self::Lobby { .. } => "Lobby",
            Self::Ranking { .. } => "Ranking",
            Self::InGame(_) => "In game",
            Self::Spectating(_) => "Spectating",
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
        assert_eq!(
            Screen::Lobby {
                matches: vec![],
                spectator_mode: false
            }
            .title(),
            "Lobby"
        );
        assert_eq!(Screen::Ranking { entries: vec![] }.title(), "Ranking");
        assert_eq!(Screen::Fatal(String::from("boom")).title(), "Error");
    }
}
