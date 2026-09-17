//! Client-side application layer: state, events, and update rules.

pub mod state;
pub mod update;

pub use state::{AppEvent, AppState};
pub use update::apply_event;
