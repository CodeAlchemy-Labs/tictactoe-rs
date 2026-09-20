//! Terminal UI: rendering and input.

pub mod input;
pub mod render;

pub use input::{
    read_input_event, read_key_action, read_key_code, translate_key, InputEvent, KeyAction,
};
pub use render::render;
