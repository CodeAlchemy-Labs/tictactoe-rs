# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-09-18

### Added

- Authentication system using Argon2id password hashing and user registry.
- Global ranking service that tracks and exposes the top 10 players by wins.
- Real-time spectating of ongoing matches, including a 5-spectator cap and full state synchronization.
- Reconnection grace period for sudden disconnects, ensuring matches are not immediately abandoned on a transient network drop (2-second timer).
- Environment variable `TICTACTOE_ENV` to configure deployment mode (`development` vs `production`).
- Production hardening configurations including global session caps, per-IP connection limits, and per-IP auth rate limiting (token bucket algorithm).
- `--allow-production` flag in the hacker crate to bypass the local-target requirement.
- `spectator_isolation` adversarial scenario to ensure non-players cannot interfere with active matches.
- `docs/USER_GUIDE.md` comprehensive guide covering the client TUI, server configuration, screen flows, and troubleshooting.

### Changed

- Elevated workspace `rustc` and `clippy` lint configurations. Denied standard warnings and promoted a curated subset of `clippy::pedantic` lints to `warn` level.
- Refactored formatting to align with the Rust 2024 edition style guidelines, configuring `rustfmt.toml` with shorthand initializers and Unix newlines.
- The `hacker` binary refuses to target remote addresses (non-local) by default, forcing explicit opt-in for testing deployed infrastructure.
- Expanded the terminal client keyboard handler to accommodate new views (Ranking, Spectating, Auth).

### Fixed

- Resolved the remaining `clippy::pedantic` and `clippy::nursery` warnings (`tuple_array_conversions`, `significant_drop_tightening`, `suboptimal_flops`, `use_self`, `doc_markdown`, `must_use_candidate`, and `missing_const_for_fn`).
- Corrected a broken intra-doc link (`ErrorCode::TooManySessions`) in the server documentation.

### Security

- Enforced strict session bounds (global and per-IP limits) and authentication rate limiting to mitigate denial-of-service (DoS) vectors in production deployments.
- Default-denial behavior in the `hacker` adversarial actor to prevent accidental disruption of production clusters.

## [0.1.1] - 2026-09-17

### Added

- TLS support in the client. `tokio-tungstenite` is now built with the
  `rustls-tls-native-roots` feature, so the client can connect to `wss://`
  endpoints using the system trust store, with no dependency on OpenSSL.
- `--insecure` flag in the client, which disables TLS certificate
  verification. Intended for development servers with self-signed
  certificates and documented as unsafe for production endpoints.
- `PORT` environment variable support in the server. The bind address is
  resolved in the order `TICTACTOE_BIND`, `PORT`, then the default
  `0.0.0.0:8080`. The `PORT` fallback makes the server deployable on
  container platforms such as Render without any configuration change.
- `render.yaml` blueprint at the repository root. It creates the Render
  service with a single click, using the existing `Dockerfile` and the
  `/health` endpoint.
- Section in `README.md` describing how to connect a client to a remote
  instance and how to deploy the server to Render.
- Section in `docs/ARCHITECTURE.md` explaining why TLS is terminated at
  the edge and not in the container.
- Section in `docs/SECURITY.md` describing the trust boundaries when TLS
  terminates at a proxy, including the implications of `--insecure`.

### Changed

- The client now uses `clap` for argument parsing, matching the hacker
  crate. Environment variables (`TICTACTOE_SERVER`, `TICTACTOE_NAME`) are
  read as fallbacks for the corresponding flags.
- The client validates the URL scheme on startup and rejects anything that
  is not `ws://` or `wss://`.
- The `port_reuse` hacker scenario reports three possible outcomes when
  probing the server's port: `EADDRINUSE` in a shared namespace, `EACCES`
  when the port is privileged and unobservable, and a successful bind in an
  isolated namespace. Previous versions treated `EACCES` as an unexpected
  error.
- `tracing` output now disables ANSI escape codes when stderr is not a
  terminal, on platforms that do not render them, or when `NO_COLOR` is
  set. `CLICOLOR_FORCE` is honored. This fixes the literal escape sequences
  (`←[2m...`) that appeared when running the Windows binaries in the legacy
  `cmd.exe` console.

### Fixed

- ANSI escape sequences are no longer emitted on Windows consoles that do
  not process Virtual Terminal codes. The server, client, and hacker
  binaries all detect the console capability and fall back to plain text.
- `ServerMessage::Error` now tolerates a missing `code` field. Servers
  deployed before the introduction of error codes omit it, and the client
  falls back to `ErrorCode::Unknown` instead of failing to deserialize.
  This restores compatibility between newer clients and older servers.

## [0.1.0] - 2026-09-17

### Added

- Cargo workspace with four crates: `common`, `server`, `client`, and `hacker`.
- `common` crate with the JSON wire protocol (`ClientMessage`, `ServerMessage`,
  `ErrorCode`, `ClientId`, `MatchId`, `MatchSummary`) and the pure Tic-Tac-Toe
  domain (`Board`, `Cell`, `Position`, `Player`, `GameStatus`) with typed
  errors via `thiserror`.
- `server` crate with Axum + Tokio WebSocket handling, an in-memory
  `LobbyService` coordinated under a single `Mutex<LobbyState>`, and the
  `SessionGuard` RAII type that performs deterministic cleanup on disconnect.
- `client` crate with a `ratatui` + `crossterm` terminal UI, a pure state
  machine driven by `apply_event(state, event, effects)`, and a `Transport`
  trait with two implementations: `WsTransport` for production and
  `MockTransport` for tests.
- `hacker` crate with three adversarial scenarios: `session_hijack`,
  `port_reuse`, and `flood`.
- Multi-stage `Dockerfile` with a non-root runtime, building all three
  binaries on Debian bookworm.
- `docker-compose.yml` with separate profiles for the server, the interactive
  clients, and the hacker.
- `Makefile` with targets for building, testing, linting, formatting,
  documenting, measuring coverage, and running the full demo.
- GitHub Actions CI workflow (`ci.yml`) with five jobs: format check, Clippy,
  tests, documentation, and coverage.
- GitHub Actions release workflow (`release.yml`) that builds Linux and
  Windows binaries when a version tag is pushed.
- Extensive documentation: `docs/ARCHITECTURE.md` (layering, concurrency
  model, patterns, testing strategy) and `docs/SECURITY.md` (threat model,
  RAII rationale, adversarial scenario description).
- 91 tests across the workspace at the time of release.

### Changed

- The `port_reuse` hacker scenario now reports the outcome honestly for both
  shared and isolated network namespaces, instead of assuming that the
  hacker and the server always share one.
- The `SessionGuard` is dropped explicitly before awaiting the writer task,
  making the cleanup ordering visible in the code and eliminating a
  potential deadlock.

### Fixed

- Writer task deadlock: the WebSocket handler awaited the writer task before
  dropping the `SessionGuard`, which prevented the outbound channel from
  closing. Detected by the integration test
  `disconnect_removes_the_session_from_the_lobby`. Fixed by making the drop
  order explicit.
- `u8` underflow in the client's `play_move` handler: `cell - 1` panicked in
  debug builds when the cell index was `0`. Fixed with `checked_sub`.
- Screen-aware key translation in the client: digits `1`–`9` selected a
  match in the lobby and played a move in a game; the original handler did
  not distinguish the two. Fixed by extracting `translate_key(code, screen)`
  and unit-testing it.
- `make demo` no longer tries to run the interactive TUI clients in the
  background, which killed them for lack of a TTY.
- Docker Compose top-level `name` field removed (unsupported in Compose
  5.5.1) and the unusable `server --version` healthcheck dropped.
- Multi-stage `Dockerfile` pinned to Debian bookworm on both stages. The
  builder was silently tracking Debian trixie, producing binaries that
  required GLIBC 2.39 and failed to start on the bookworm runtime.
- Fragile dependency-cache layer in the `Dockerfile` removed. The stub +
  copy trick reused stale rlibs because Docker's `COPY` preserved the
  host's older mtimes.
- Terminal UI did not render until the first key press under Docker. Fixed
  by polling the terminal size and calling `terminal.resize` before the
  first draw.
- Several `clippy::pedantic` lints resolved across the workspace, including
  `return_self_not_must_use`, `needless_pass_by_value`, `manual_let_else`,
  `collapsible_if`, `doc_markdown`, and `missing_panics_doc`.
- Redundant and broken intra-doc links resolved in `server` and `client`.

### Security

- The `SessionGuard` `Drop` implementation guarantees that a session is
  removed from the lobby the instant the WebSocket handler returns. There is
  no window during which a stale session is observable by other clients.
- The three hacker scenarios verify that illegal state transitions are
  rejected, that the server owns its port for as long as it runs, and that
  ephemeral ports are released synchronously on `drop`.

[Unreleased]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/releases/tag/v0.1.0
