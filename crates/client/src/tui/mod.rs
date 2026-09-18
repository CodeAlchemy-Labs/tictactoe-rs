//! Terminal UI: rendering and input.

pub mod input;
pub mod render;

pub use input::{KeyAction, read_key_action, read_key_code, translate_key};
pub use render::render;
