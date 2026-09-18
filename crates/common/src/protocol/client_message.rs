//! Messages sent by a client to the server.

use serde::{Deserialize, Serialize};

use super::MatchId;
use crate::domain::Position;

/// A message sent by a client to the server.
///
/// The enum is internally tagged with a `type` field in `snake_case`. For
/// example, `Hello` serializes as:
///
/// ```json
/// { "type": "hello", "display_name": "alice" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Announces the client's display name and asks to enter the lobby as a
    /// guest.
    ///
    /// Guest clients can list matches, view the ranking, and spectate, but
    /// they cannot create or join matches. Registration or login is
    /// required to play.
    Hello {
        /// The display name chosen by the player.
        display_name: String,
    },
    /// Creates a new account and authenticates the client in the same step.
    ///
    /// The fields are raw input; the server validates them and responds
    /// with `Registered` or `AuthenticationFailed`.
    Register {
        /// The user's display name.
        name: String,
        /// The desired username.
        username: String,
        /// The user's age.
        age: u8,
        /// The password, in plain text. TLS protects it in transit.
        password: String,
    },
    /// Authenticates an existing account.
    Login {
        /// The account's username.
        username: String,
        /// The account's password, in plain text.
        password: String,
    },
    /// Requests the current list of open matches.
    ListMatches,
    /// Requests the top-players ranking.
    ///
    /// The ranking is public: guests and authenticated users can request it.
    ListRanking,
    /// Creates a new match hosted by the caller.
    CreateMatch,
    /// Joins an existing match.
    JoinMatch {
        /// The identifier of the match to join.
        match_id: MatchId,
    },
    /// Plays a move in the caller's current match.
    MakeMove {
        /// The position the player wants to claim.
        position: Position,
    },
    /// Abandons the caller's current match.
    LeaveMatch,
    /// Liveness probe.
    Ping,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_serializes_with_type_tag() {
        let message = ClientMessage::Hello {
            display_name: String::from("alice"),
        };
        let value: serde_json::Value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "hello");
        assert_eq!(value["display_name"], "alice");
    }

    #[test]
    fn unit_variant_serializes_as_bare_tag() {
        let value = serde_json::to_value(ClientMessage::Ping).unwrap();
        assert_eq!(value["type"], "ping");
        assert!(value.get("display_name").is_none());
    }

    #[test]
    fn list_ranking_serializes_as_bare_tag() {
        let value = serde_json::to_value(ClientMessage::ListRanking).unwrap();
        assert_eq!(value["type"], "list_ranking");
    }

    #[test]
    fn make_move_round_trips_through_position_validation() {
        let message = ClientMessage::MakeMove {
            position: Position::new(4).unwrap(),
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn make_move_rejects_out_of_range_position() {
        let json = r#"{"type":"make_move","position":9}"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn join_match_round_trip() {
        let message = ClientMessage::JoinMatch {
            match_id: MatchId::new(5),
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn register_round_trip() {
        let message = ClientMessage::Register {
            name: String::from("Alice Example"),
            username: String::from("alice_99"),
            age: 30,
            password: String::from("hunter2hunter2"),
        };
        let json = serde_json::to_string(&message).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(message, back);
    }

    #[test]
    fn login_serializes_with_type_tag() {
        let message = ClientMessage::Login {
            username: String::from("alice_99"),
            password: String::from("hunter2hunter2"),
        };
        let value: serde_json::Value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["type"], "login");
        assert_eq!(value["username"], "alice_99");
    }
}