//! Terminal client for `tictactoe-rs`.
//!
//! The crate is organized as follows:
//!
//! - [`app`]: the local state machine and event loop that drives the UI.
//! - [`config`]: runtime configuration for the client process.
//! - [`domain`]: pure client-side types (the current screen, the local view
//!   of a match).
//! - [`infrastructure`]: WebSocket transport, hidden behind the [`Transport`]
//!   trait so it can be replaced by a mock in tests.
//! - [`tui`]: rendering and input handling for the terminal UI.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]

pub mod app;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod tui;
