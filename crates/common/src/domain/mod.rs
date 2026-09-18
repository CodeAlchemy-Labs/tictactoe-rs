//! Pure domain types and rules for the Tic-Tac-Toe game.
//!
//! This module is free of any I/O, async, or transport concern. It models
//! the board, the players, the win/draw detection rules, and the user model
//! as a small set of value objects and one aggregate ([`Board`]) that owns
//! its invariants.
//!
//! The types are also `serde`-serializable because they double as the wire
//! format for the protocol layer. Serialization is a trait contract, not an
//! infrastructure concern, so keeping the derives here does not violate the
//! separation between domain and transport.

mod age;
mod board;
mod cell;
mod game_status;
mod player;
mod position;
mod user_profile;
mod username;

pub use age::Age;
pub use board::Board;
pub use cell::Cell;
pub use game_status::GameStatus;
pub use player::Player;
pub use position::Position;
pub use user_profile::UserProfile;
pub use username::{Username, UsernameError};