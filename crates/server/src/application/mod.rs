//! Application services that orchestrate the domain types.

pub mod auth;
pub mod lobby;

pub use auth::{AuthError, AuthService};
pub use lobby::LobbyService;
