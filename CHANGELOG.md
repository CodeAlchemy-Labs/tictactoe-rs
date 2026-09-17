# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- Extensive documentation: `docs/ARCHITECTURE.md` (layering, concurrency
  model, patterns, testing strategy) and `docs/SECURITY.md` (threat model,
  RAII rationale, adversarial scenario description).
- 91 tests across the workspace: 40 in `common`, 18 + 3 in `server`,
  21 + 2 in `client`, and 4 + 3 in `hacker`.

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

[Unreleased]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/releases/tag/v0.1.0