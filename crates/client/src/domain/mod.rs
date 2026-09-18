//! Client-side domain types.
//!
//! Nothing in this module performs I/O. The types describe what the UI needs
//! to display and what the state machine needs to track.

pub mod auth_form;
pub mod screen;

pub use auth_form::{AuthField, AuthForm, AuthMode, PendingAction};
pub use screen::{ActiveMatch, Screen, SpectatedMatch};