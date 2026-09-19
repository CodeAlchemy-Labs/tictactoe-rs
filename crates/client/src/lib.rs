//! Terminal client for `tictactoe-rs`.
//!
//! The crate is organized as follows:
//!
//! - [`app`]: the local state machine and event loop that drives the UI.
//! - [`config`]: runtime configuration for the client process.
//! - [`domain`]: pure client-side types (the current screen, the local view
//!   of a match).
//! - [`infrastructure`]: WebSocket transport, hidden behind the
//!   [`Transport`](infrastructure::Transport) trait so it can be replaced by
//!   a mock in tests.
//! - [`tui`]: rendering and input handling for the terminal UI.

pub mod app;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod tui;
