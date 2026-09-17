//! Wire protocol messages exchanged over the WebSocket connection.
//!
//! Every message is JSON-serializable. The types in this module are the only
//! contract between the server, the client, and the hacker actor. Anything
//! that is not part of this contract lives in [`crate::domain`] or in the
//! individual crates.
//!
//! Messages are tagged with a `type` field in `snake_case`, which makes the
//! wire format self-describing and easy to inspect in logs.

mod client_id;
mod client_message;
mod match_id;
mod match_summary;
mod server_message;

pub use client_id::ClientId;
pub use client_message::ClientMessage;
pub use match_id::MatchId;
pub use match_summary::MatchSummary;
pub use server_message::{ErrorCode, ServerMessage};