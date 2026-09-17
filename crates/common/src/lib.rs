//! Shared protocol contracts and domain types for the `tictactoe-rs` workspace.
//!
//! This crate is intentionally free of I/O, async runtimes, and any transport
//! concern. It provides three layers:
//!
//! - [`domain`]: the pure Tic-Tac-Toe rules (board, cells, players,
//!   positions, and status).
//! - [`protocol`]: the JSON-serializable message types exchanged over the
//!   WebSocket connection.
//! - [`error`]: typed errors raised by the two layers above.
//!
//! By keeping the contract in a dedicated crate, the server, the client, and
//! the hacker actor all agree on the exact same wire format and on the exact
//! same game rules. Changing the protocol forces a compile error in every
//! consumer, which is the entire point.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
// The following allows keep `clippy::pedantic` actionable for this crate's size.
// They do not weaken safety or correctness guarantees.
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]

pub mod domain;
pub mod error;
pub mod protocol;
