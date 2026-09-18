//! Messages sent by the server to a client.

use serde::{Deserialize, Serialize};

use super::{AuthFailureReason, ClientId, MatchId, MatchSummary};
use crate::domain::{Board, GameStatus, Player, RankingEntry, UserProfile};

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
    /// Sent after a successful
    /// [`ClientMessage::Register`](crate::protocol::ClientMessage::Register).
    Registered {
        /// The newly created profile.
        profile: UserProfile,
    },
    /// Sent after a successful
    /// [`ClientMessage::Login`](crate::protocol::ClientMessage::Login).
    LoginSucceeded {
        /// The authenticated user's profile.
        profile: UserProfile,
    },
    /// Sent when an authentication attempt fails.
    AuthenticationFailed {
        /// A stable, machine-readable reason.
        reason: AuthFailureReason,
        /// A human-readable description.
        message: String,
    },
    /// The current list of open matches.
    MatchList {
        /// The available matches.
        matches: Vec<MatchSummary>,
    },
    /// The current top-players ranking, ordered by wins descending.
    Ranking {
        /// The top entries, at most ten.
        entries: Vec<RankingEntry>,
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
    /// Sent when the match has ended normally.
    MatchOver {
        /// The final board.
        board: Board,
        /// The final status.
        status: GameStatus,
        /// The display name of the winner, if any.
        ///
        /// `Some(name)` when `status` is `Won`, `None` otherwise. The client
        /// uses it to render "The player {name} won" instead of the raw
        /// mark. The field is optional on the wire so that clients can talk
        /// to servers that predate it.
        #[serde(default)]
        winner_name: Option<String>,
    },
    /// Sent when a match is dissolved before it ends: a player left, or
    /// disconnected. Spectators and any remaining participant receive this
    /// instead of `MatchOver`.
    MatchAbandoned {
        /// The match identifier.
        match_id: MatchId,
    },
    /// Sent when the opponent has left the match.
    OpponentLeft {
        /// The match identifier.
        match_id: MatchId,
    },
    /// Sent to the remaining participants of a match when one of the two
    /// players has disconnected or left.
    ///
    /// The match does not end immediately: the server waits for
    /// `grace_seconds` before considering it abandoned. The recipient
    /// should keep the game state on screen and show a "waiting for
    /// reconnection" notice.
    OpponentDisconnected {
        /// The match identifier.
        match_id: MatchId,
        /// How many seconds the server will wait before abandoning the
        /// match.
        grace_seconds: u32,
    },
    /// Sent to the remaining participants of a match when the disconnected
    /// player reconnects.
    OpponentReconnected {
        /// The match identifier.
        match_id: MatchId,
    },
    /// Sent to a client that has just started spectating a match.
    ///
    /// Contains a snapshot of the board at the moment the spectator joined,
    /// plus the identity of the two players and the current count of
    /// spectators, including the recipient.
    SpectateStarted {
        /// The match identifier.
        match_id: MatchId,
        /// The host's display name.
        host_name: String,
        /// The guest's display name, or an empty string when no guest has
        /// joined yet.
        guest_name: String,
        /// The board at the moment the spectator joined.
        board: Board,
        /// The player whose turn it is.
        current_turn: Player,
        /// The current status.
        status: GameStatus,
        /// The number of spectators, including the recipient.
        spectator_count: u32,
    },
    /// Sent to every participant and spectator of a match when a new
    /// spectator joins.
    SpectatorJoined {
        /// The display name of the spectator that joined.
        username: String,
        /// The total number of spectators after the join.
        spectator_count: u32,
    },
    /// Sent to every participant and spectator of a match when a spectator
    /// leaves.
    SpectatorLeft {
        /// The display name of the spectator that left.
        username: String,
        /// The total number of spectators after the departure.
        spectator_count: u32,
    },
    /// A protocol-level error.
    Error {
        /// A stable, machine-readable error code.
        ///
        /// The field is optional on the wire: servers that predate the
        /// introduction of error codes omit it, and the client falls back
        /// to [`ErrorCode::Unknown`]. This keeps older deployments
        /// compatible with newer clients.
        #[serde(default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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
    /// The action requires an authenticated account.
    AuthenticationRequired,
    /// The client tried to join a match it already hosts.
    CannotJoinOwnMatch,
    /// The match already has the maximum number of spectators.
    SpectatorLimitReached,
    /// The client is already spectating a match.
    AlreadySpectating,
    /// The server did not provide a code, or provided one the client does
    /// not recognize. Used as the fallback for older servers.
    #[default]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Age, Position, Username};

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
        board.place(Position::new(4).unwrap(), Player::X).unwrap();
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
            winner_name: Some(String::from("Alice Example")),
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn match_over_without_winner_name_deserializes() {
        let json = r#"{"type":"match_over","board":{"cells":["Empty","Empty","Empty","Empty","Empty","Empty","Empty","Empty","Empty"]},"status":"draw"}"#;
        let message: ServerMessage = serde_json::from_str(json).unwrap();
        match message {
            ServerMessage::MatchOver {
                status,
                winner_name,
                ..
            } => {
                assert_eq!(status, GameStatus::Draw);
                assert_eq!(winner_name, None);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn match_abandoned_serializes_as_bare_tag() {
        let message = ServerMessage::MatchAbandoned {
            match_id: MatchId::new(7),
        };
        let value: serde_json::Value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "match_abandoned");
        assert_eq!(value["match_id"], 7);
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

    #[test]
    fn spectator_error_codes_serialize_in_snake_case() {
        let value = serde_json::to_string(&ErrorCode::SpectatorLimitReached).unwrap();
        assert_eq!(value, "\"spectator_limit_reached\"");
        let value = serde_json::to_string(&ErrorCode::AlreadySpectating).unwrap();
        assert_eq!(value, "\"already_spectating\"");
    }

    #[test]
    fn cannot_join_own_match_serializes_in_snake_case() {
        let value = serde_json::to_string(&ErrorCode::CannotJoinOwnMatch).unwrap();
        assert_eq!(value, "\"cannot_join_own_match\"");
    }

    #[test]
    fn error_without_code_deserializes_as_unknown() {
        let json = r#"{"type":"error","message":"legacy server"}"#;
        let message: ServerMessage = serde_json::from_str(json).unwrap();
        match message {
            ServerMessage::Error { code, message } => {
                assert_eq!(code, ErrorCode::Unknown);
                assert_eq!(message, "legacy server");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn error_with_unknown_code_deserializes_as_unknown() {
        let json = r#"{"type":"error","code":"some_future_code","message":"future"}"#;
        let result: Result<ServerMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn registered_round_trip() {
        let message = ServerMessage::Registered {
            profile: UserProfile {
                name: String::from("Alice Example"),
                username: Username::new("alice_99").unwrap(),
                age: Age::new(30).unwrap(),
            },
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn authentication_failed_serializes_with_reason() {
        let message = ServerMessage::AuthenticationFailed {
            reason: AuthFailureReason::UsernameTaken,
            message: String::from("username taken"),
        };
        let value: serde_json::Value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "authentication_failed");
        assert_eq!(value["reason"], "username_taken");
    }

    #[test]
    fn ranking_round_trip() {
        let message = ServerMessage::Ranking {
            entries: vec![
                RankingEntry {
                    username: Username::new("alice_99").unwrap(),
                    name: String::from("Alice Example"),
                    wins: 7,
                },
                RankingEntry {
                    username: Username::new("bob_77").unwrap(),
                    name: String::from("Bob Example"),
                    wins: 3,
                },
            ],
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn ranking_serializes_with_type_tag() {
        let message = ServerMessage::Ranking { entries: vec![] };
        let value: serde_json::Value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "ranking");
        assert_eq!(value["entries"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn spectate_started_round_trip() {
        let message = ServerMessage::SpectateStarted {
            match_id: MatchId::new(2),
            host_name: String::from("Alice"),
            guest_name: String::from("Bob"),
            board: Board::new(),
            current_turn: Player::X,
            status: GameStatus::InProgress,
            spectator_count: 3,
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn spectator_joined_round_trip() {
        let message = ServerMessage::SpectatorJoined {
            username: String::from("Carol"),
            spectator_count: 2,
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn spectator_left_round_trip() {
        let message = ServerMessage::SpectatorLeft {
            username: String::from("Carol"),
            spectator_count: 1,
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn opponent_disconnected_round_trip() {
        let message = ServerMessage::OpponentDisconnected {
            match_id: MatchId::new(1),
            grace_seconds: 2,
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn opponent_reconnected_round_trip() {
        let message = ServerMessage::OpponentReconnected {
            match_id: MatchId::new(1),
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }
}
