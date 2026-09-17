//! Messages sent by the server to a client.

use serde::{Deserialize, Serialize};

use super::{ClientId, MatchId, MatchSummary};
use crate::domain::{Board, GameStatus, Player};

/// A message sent by the server to a client.
///
/// Like [`ClientMessage`](crate::protocol::ClientMessage), this enum is
/// internally tagged with a `type` field in `snake_case`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Sent once after a successful
    /// [`ClientMessage::Hello`](crate::protocol::ClientMessage::Hello).
    Welcome {
        /// The identifier assigned to the client.
        client_id: ClientId,
        /// The display name echoed back.
        display_name: String,
    },
    /// The current list of open matches.
    MatchList {
        /// The available matches.
        matches: Vec<MatchSummary>,
    },
    /// Sent when a match has been created and is waiting for an opponent.
    MatchCreated {
        /// The identifier of the freshly created match.
        match_id: MatchId,
    },
    /// Sent when two players have been paired and the game is ready to start.
    MatchReady {
        /// The match identifier.
        match_id: MatchId,
        /// The opponent's display name.
        opponent: String,
        /// The mark assigned to the recipient.
        your_mark: Player,
        /// The initial board state.
        board: Board,
        /// The player who moves first.
        current_turn: Player,
    },
    /// Sent after every accepted move.
    BoardUpdate {
        /// The updated board.
        board: Board,
        /// The player who must move next.
        current_turn: Player,
        /// The current status.
        status: GameStatus,
    },
    /// Sent when the match has ended.
    MatchOver {
        /// The final board.
        board: Board,
        /// The final status.
        status: GameStatus,
    },
    /// Sent when the opponent has left the match.
    OpponentLeft {
        /// The match identifier.
        match_id: MatchId,
    },
    /// A protocol-level error.
    Error {
        /// A stable, machine-readable error code.
        code: ErrorCode,
        /// A human-readable description.
        message: String,
    },
    /// Response to [`ClientMessage::Ping`](crate::protocol::ClientMessage::Ping).
    Pong,
}

/// A stable, machine-readable error code.
///
/// Codes are part of the wire contract: clients can match on them without
/// parsing the human-readable message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The client sent a message that is not valid in its current state.
    InvalidState,
    /// The client referenced a match that does not exist.
    MatchNotFound,
    /// The client referenced a match that is already full.
    MatchFull,
    /// The client attempted an illegal move.
    IllegalMove,
    /// The message could not be parsed.
    MalformedMessage,
    /// The client is not part of any match.
    NotInMatch,
    /// The client sent a display name that was rejected.
    InvalidDisplayName,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Position;

    #[test]
    fn welcome_serializes_with_type_tag() {
        let message = ServerMessage::Welcome {
            client_id: ClientId::new(1),
            display_name: String::from("alice"),
        };
        let value: serde_json::Value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "welcome");
        assert_eq!(value["client_id"], 1);
        assert_eq!(value["display_name"], "alice");
    }

    #[test]
    fn board_update_round_trip() {
        let mut board = Board::new();
        board
            .place(Position::new(4).unwrap(), Player::X)
            .unwrap();
        let message = ServerMessage::BoardUpdate {
            board,
            current_turn: Player::O,
            status: GameStatus::InProgress,
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn match_over_round_trip_with_winner() {
        let message = ServerMessage::MatchOver {
            board: Board::new(),
            status: GameStatus::Won(Player::X),
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn error_code_serializes_in_snake_case() {
        let message = ServerMessage::Error {
            code: ErrorCode::MatchNotFound,
            message: String::from("no such match"),
        };
        let value: serde_json::Value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "error");
        assert_eq!(value["code"], "match_not_found");
    }
}