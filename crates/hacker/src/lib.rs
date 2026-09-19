//! Adversarial actor for `tictactoe-rs`.
//!
//! The crate exposes three scenarios, each of which probes a different
//! property of the server:
//!
//! - [`scenarios::session_hijack`]: tries to act on a match without being a
//!   legitimate participant.
//! - [`scenarios::port_reuse`]: tries to bind the server's port and then
//!   demonstrates that Rust releases ephemeral ports the instant the owning
//!   listener is dropped.
//! - [`scenarios::flood`]: opens many concurrent connections, closes them
//!   abruptly, and verifies the server still accepts a fresh session.
//!
//! Every scenario returns an [`Outcome`](outcome::Outcome) describing whether
//! the server defended or was compromised. The binary exits with code `0`
//! when every scenario reports `Defended`.


pub mod config;
pub mod outcome;
pub mod scenarios;
