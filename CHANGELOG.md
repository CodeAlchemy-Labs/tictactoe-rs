# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-09-18

### Added
- **Authentication**: Secure Argon2id password hashing and user registry. Enforces single-session-per-username invariants.
- **Ranking**: Global ranking system that tracks and displays the top 10 players by wins.
- **Spectators**: Real-time spectating of ongoing matches with full state synchronization.
- **Grace-Period Abandon**: Robust reconnection grace period for sudden disconnects, ensuring matches are not immediately abandoned on a transient network drop.
- **Production Hardening**: Rate limiting, global session limits, and per-IP connection limits configured via environment variables.
- **Adversarial Testing**: The `hacker` crate now supports an `--allow-production` guard for safely running scenarios against remote targets.

### Changed
- Elevated workspace `rustc` and `clippy` lint configurations. Denied standard warnings and promoted a curated subset of `clippy::pedantic` lints to `warn` level.
- Formatted the codebase utilizing `rustfmt` with 2024 edition styling and shorthand initializers.

## [0.1.1]

### Added
- Initial release featuring the core Tic-Tac-Toe engine, WebSocket protocol, and terminal UI client.
