//! Terminal UI: rendering and input.

pub mod input;
pub mod render;

pub use input::{KeyAction, read_key_action};
pub use render::render;