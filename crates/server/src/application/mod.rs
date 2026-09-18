//! Application services that orchestrate the domain types.

pub mod auth;
pub mod lobby;
pub mod ranking;

pub use auth::{AuthError, AuthService};
pub use lobby::LobbyService;
pub use ranking::{RankingService, TOP_N};
