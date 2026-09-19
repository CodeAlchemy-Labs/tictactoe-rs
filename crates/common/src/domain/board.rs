//! The board aggregate.

use serde::{Deserialize, Serialize};

use super::{Cell, GameStatus, Player, Position};
use crate::error::DomainError;

/// Every winning line, expressed as indices into the flat cell array.
const WINNING_LINES: [[u8; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

/// The 3x3 Tic-Tac-Toe board.
///
/// The board owns its invariants:
///
/// - cells can only be set through [`Board::place`], which rejects occupied cells
/// - the winning condition is evaluated on demand and always reflects the
///   current cells
///
/// This is the classic aggregate-root pattern: state changes flow through a
/// single entry point, and the outside world cannot observe an inconsistent
/// board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    cells: [Cell; 9],
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    /// Creates an empty board.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cells: [Cell::Empty; 9],
        }
    }

    /// Returns the cell at the given position.
    #[must_use]
    pub const fn get(&self, position: Position) -> Cell {
        self.cells[position.index()]
    }

    /// Returns an iterator over every cell in row-major order.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        self.cells.iter().copied()
    }

    /// Returns `true` if every cell is occupied.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.cells.iter().all(|cell| !cell.is_empty())
    }

    /// Places the given player's mark at the given position.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::CellOccupied`] if the position is already taken.
    pub const fn place(&mut self, position: Position, player: Player) -> Result<(), DomainError> {
        let index = position.index();
        if !self.cells[index].is_empty() {
            return Err(DomainError::CellOccupied { position });
        }
        self.cells[index] = Cell::Occupied(player);
        Ok(())
    }

    /// Evaluates the current game status.
    #[must_use]
    pub fn status(&self) -> GameStatus {
        for line in WINNING_LINES {
            let [a, b, c] = line.map(|index| self.cells[index as usize]);
            if let [Cell::Occupied(p), Cell::Occupied(q), Cell::Occupied(r)] = [a, b, c]
                && p == q
                && q == r
            {
                return GameStatus::Won(p);
            }
        }

        if self.is_full() {
            GameStatus::Draw
        } else {
            GameStatus::InProgress
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(value: u8) -> Position {
        Position::new(value).unwrap()
    }

    #[test]
    fn new_board_is_empty_and_in_progress() {
        let board = Board::new();
        assert!(!board.is_full());
        assert_eq!(board.status(), GameStatus::InProgress);
        for cell in board.cells() {
            assert!(cell.is_empty());
        }
    }

    #[test]
    fn placing_on_empty_cell_succeeds() {
        let mut board = Board::new();
        board.place(position(0), Player::X).unwrap();
        assert_eq!(board.get(position(0)), Cell::Occupied(Player::X));
    }

    #[test]
    fn placing_on_occupied_cell_fails() {
        let mut board = Board::new();
        board.place(position(0), Player::X).unwrap();
        assert!(board.place(position(0), Player::O).is_err());
    }

    #[test]
    fn row_win_is_detected() {
        let mut board = Board::new();
        board.place(position(0), Player::X).unwrap();
        board.place(position(1), Player::X).unwrap();
        board.place(position(2), Player::X).unwrap();
        assert_eq!(board.status(), GameStatus::Won(Player::X));
    }

    #[test]
    fn column_win_is_detected() {
        let mut board = Board::new();
        board.place(position(0), Player::O).unwrap();
        board.place(position(3), Player::O).unwrap();
        board.place(position(6), Player::O).unwrap();
        assert_eq!(board.status(), GameStatus::Won(Player::O));
    }

    #[test]
    fn main_diagonal_win_is_detected() {
        let mut board = Board::new();
        board.place(position(0), Player::X).unwrap();
        board.place(position(4), Player::X).unwrap();
        board.place(position(8), Player::X).unwrap();
        assert_eq!(board.status(), GameStatus::Won(Player::X));
    }

    #[test]
    fn anti_diagonal_win_is_detected() {
        let mut board = Board::new();
        board.place(position(2), Player::O).unwrap();
        board.place(position(4), Player::O).unwrap();
        board.place(position(6), Player::O).unwrap();
        assert_eq!(board.status(), GameStatus::Won(Player::O));
    }

    #[test]
    fn full_board_without_winner_is_draw() {
        // X O X
        // X O O
        // O X X
        let moves = [
            (0, Player::X),
            (1, Player::O),
            (2, Player::X),
            (3, Player::X),
            (4, Player::O),
            (5, Player::O),
            (6, Player::O),
            (7, Player::X),
            (8, Player::X),
        ];
        let mut board = Board::new();
        for (index, player) in moves {
            board.place(position(index), player).unwrap();
        }
        assert!(board.is_full());
        assert_eq!(board.status(), GameStatus::Draw);
    }

    #[test]
    fn serde_round_trip_preserves_state() {
        let mut board = Board::new();
        board.place(position(4), Player::X).unwrap();
        let json = serde_json::to_string(&board).unwrap();
        let back: Board = serde_json::from_str(&json).unwrap();
        assert_eq!(board, back);
    }
}
