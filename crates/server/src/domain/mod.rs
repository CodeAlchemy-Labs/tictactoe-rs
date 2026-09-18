//! Server-side domain types.
//!
//! These types describe the in-memory state the server keeps for each
//! connected client and each active match. They carry no I/O and no async
//! concern. All mutation flows through the
//! [`LobbyService`](crate::application::lobby::LobbyService), which owns the
//! only lock protecting them.

pub mod match_state;
pub mod session;
pub mod user_record;

pub use match_state::Match;
pub use session::Session;
pub use user_record::UserRecord;