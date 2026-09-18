//! In-memory match state.

use std::collections::HashMap;

use common::domain::{Board, GameStatus, Player, Position};
use common::protocol::{ClientId, MatchId, MAX_SPECTATORS};

/// A Tic-Tac-Toe match between a host and (once joined) a guest.
///
/// The host always plays `X` and moves first. The guest always plays `O`.
/// The board and status are mutated exclusively through
/// [`LobbyService::make_move`](crate::application::lobby::LobbyService::make_move).
///
/// A match also holds a list of spectators. Spectators never interact with
/// the board; they only receive a snapshot when they join and every
/// subsequent broadcast. The order of `spectators` reflects the order of
/// arrival, which the UI may use to display who is watching.
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
    /// The spectators currently watching the match, in arrival order.
    pub spectators: Vec<ClientId>,
    /// The display name of each spectator, keyed by client identifier.
    pub spectator_names: HashMap<ClientId, String>,
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
            spectators: Vec::new(),
            spectator_names: HashMap::new(),
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

    /// Returns `true` if `client` is a player in this match.
    pub fn is_player(&self, client: ClientId) -> bool {
        client == self.host || Some(client) == self.guest
    }

    /// Returns `true` if `client` is currently spectating this match.
    pub fn is_spectator(&self, client: ClientId) -> bool {
        self.spectator_names.contains_key(&client)
    }

    /// Returns the number of spectators currently watching.
    pub fn spectator_count(&self) -> u32 {
        u32::try_from(self.spectators.len()).unwrap_or(u32::MAX)
    }

    /// Returns `true` when the match can accept another spectator.
    pub fn has_room_for_spectator(&self) -> bool {
        self.spectator_count() < MAX_SPECTATORS
    }

    /// Adds a spectator to the match.
    ///
    /// Returns `true` if the spectator was added. The caller is expected to
    /// have checked [`Match::has_room_for_spectator`] and to have verified
    /// that the client is not a player. Adding a client that is already a
    /// spectator is a no-op and returns `false`.
    pub fn add_spectator(&mut self, client: ClientId, name: String) -> bool {
        if self.is_spectator(client) {
            return false;
        }
        self.spectators.push(client);
        self.spectator_names.insert(client, name);
        true
    }

    /// Removes a spectator from the match.
    ///
    /// Returns the removed spectator's display name, or `None` if the
    /// client was not spectating.
    pub fn remove_spectator(&mut self, client: ClientId) -> Option<String> {
        let index = self.spectators.iter().position(|id| *id == client)?;
        self.spectators.remove(index);
        self.spectator_names.remove(&client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_match_starts_empty_and_x_to_move() {
        let m = Match::new(MatchId::new(0), ClientId::new(0));
        assert!(!m.is_full());
        assert_eq!(m.current_turn, Player::X);
        assert_eq!(m.status, GameStatus::InProgress);
        assert_eq!(m.spectator_count(), 0);
        assert!(m.spectators.is_empty());
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
    fn is_player_distinguishes_players_from_outsiders() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        assert!(m.is_player(ClientId::new(0)));
        assert!(!m.is_player(ClientId::new(1)));
        m.guest = Some(ClientId::new(1));
        assert!(m.is_player(ClientId::new(1)));
        assert!(!m.is_player(ClientId::new(2)));
    }

    #[test]
    fn placing_is_delegated_to_board() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        m.board.place(Position::new(0).unwrap(), Player::X).unwrap();
        assert_eq!(m.board.status(), GameStatus::InProgress);
    }

    #[test]
    fn add_spectator_records_the_name_and_increments_the_count() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        assert!(m.add_spectator(ClientId::new(10), String::from("Carol")));
        assert_eq!(m.spectator_count(), 1);
        assert!(m.is_spectator(ClientId::new(10)));
        assert_eq!(m.spectator_names.get(&ClientId::new(10)).unwrap(), "Carol");
    }

    #[test]
    fn add_spectator_ignores_duplicates() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        assert!(m.add_spectator(ClientId::new(10), String::from("Carol")));
        assert!(!m.add_spectator(ClientId::new(10), String::from("Carol")));
        assert_eq!(m.spectator_count(), 1);
    }

    #[test]
    fn remove_spectator_returns_the_name() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        m.add_spectator(ClientId::new(10), String::from("Carol"));
        let removed = m.remove_spectator(ClientId::new(10));
        assert_eq!(removed.as_deref(), Some("Carol"));
        assert_eq!(m.spectator_count(), 0);
        assert!(!m.is_spectator(ClientId::new(10)));
    }

    #[test]
    fn remove_unknown_spectator_returns_none() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        assert_eq!(m.remove_spectator(ClientId::new(10)), None);
    }

    #[test]
    fn has_room_for_spectator_respects_the_limit() {
        let mut m = Match::new(MatchId::new(0), ClientId::new(0));
        for index in 0..MAX_SPECTATORS {
            assert!(m.has_room_for_spectator());
            m.add_spectator(ClientId::new(u64::from(index + 1)), format!("s{index}"));
        }
        assert!(!m.has_room_for_spectator());
        assert_eq!(m.spectator_count(), MAX_SPECTATORS);
    }
}