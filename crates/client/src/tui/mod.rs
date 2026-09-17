//! Terminal UI: rendering and input.

pub mod input;
pub mod render;

pub use input::{KeyAction, read_key_action, translate_key};
pub use render::render;
