//! Wire protocol messages exchanged over the WebSocket connection.
//!
//! Every message is JSON-serializable. The types in this module are the only
//! contract between the server, the client, and the hacker actor. Anything
//! that is not part of this contract lives in [`crate::domain`] or in the
//! individual crates.
//!
//! Messages are tagged with a `type` field in `snake_case`, which makes the
//! wire format self-describing and easy to inspect in logs.

mod auth_failure;
mod client_id;
mod client_message;
mod match_id;
mod match_summary;
mod server_message;

pub use auth_failure::AuthFailureReason;
pub use client_id::ClientId;
pub use client_message::ClientMessage;
pub use match_id::MatchId;
pub use match_summary::MatchSummary;
pub use server_message::{ErrorCode, ServerMessage};

/// Maximum number of spectators that may observe a single match.
///
/// The limit applies to spectators only. A match always seats exactly two
/// players and up to [`MAX_SPECTATORS`] observers.
pub const MAX_SPECTATORS: u32 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_spectators_is_five() {
        assert_eq!(MAX_SPECTATORS, 5);
    }
}
