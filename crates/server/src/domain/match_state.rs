//! In-memory match state.

use common::domain::{Board, GameStatus, Player};
use common::protocol::{ClientId, MatchId};

/// A Tic-Tac-Toe match between a host and (once joined) a guest.
///
/// The host always plays `X` and moves first. The guest always plays `O`. The
/// board and status are mutated exclusively through
/// [`LobbyService::make_move`](crate::application::lobby::LobbyService::make_move).
pub struct Match {
    /// The identifier of this match.
    pub id: MatchId,
    /// The client that created the match.
    pub host: ClientId,
    /// The client that joined the match, if any.
    pub guest: Option<ClientId>,
    /// The current board.
    pub board: Board,
    /// The player whose turn it is.
    pub current_turn: Player,
    /// The current status.
    pub status: GameStatus,
}

impl Match {
    /// Creates a new empty match hosted by `host`.
    pub fn new(id: MatchId, host: ClientId) -> Self {
        Self {
            id,
            host,
            guest: None,
            board: Board::new(),
            current_turn: Player::X,
            status: GameStatus::InProgress,
        }
    }

    /// Returns `true` when both players are present.
    pub fn is_full(&self) -> bool {
        self.guest.is_some()
    }

    /// Returns the mark assigned to `client` in this match, if any.
    pub fn mark_of(&self, client: ClientId) -> Option<Player> {
        if client == self.host {
            Some(Player::X)
        } else if Some(client) == self.guest {
            Some(Player::O)
        } else {
            None
        }
    }

    /// Returns the opponent of `client`, if the client is a participant.
    pub fn opponent_of(&self, client: ClientId) -> Option<ClientId> {
        if client == self.host {
            self.guest
        } else if Some(client) == self.guest {
            Some(self.host)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use common::domain::Position;

    use super::*;

    #[test]
    fn new_match_starts_empty_and_x_to_move() {
        let m = Match::new(MatchId::new(0), ClientId::new(0));
        assert!(!m.is_full());
        assert_eq!(m.current_turn, Player::X);
        assert_eq!(m.status, GameStatus::InProgress);
    }

    #[test]
    fn mark_of_returns_x_for_host_and_o_for_guest() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        assert_eq!(m.mark_of(ClientId::new(0)), Some(Player::X));
        m.guest = Some(ClientId::new(1));
        assert_eq!(m.mark_of(ClientId::new(1)), Some(Player::O));
        assert_eq!(m.mark_of(ClientId::new(2)), None);
    }

    #[test]
    fn opponent_of_returns_other_participant() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        m.guest = Some(ClientId::new(1));
        assert_eq!(m.opponent_of(ClientId::new(0)), Some(ClientId::new(1)));
        assert_eq!(m.opponent_of(ClientId::new(1)), Some(ClientId::new(0)));
        assert_eq!(m.opponent_of(ClientId::new(2)), None);
    }

    #[test]
    fn placing_is_delegated_to_board() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        m.board.place(Position::new(0).unwrap(), Player::X).unwrap();
        assert_eq!(m.board.status(), GameStatus::InProgress);
    }
}