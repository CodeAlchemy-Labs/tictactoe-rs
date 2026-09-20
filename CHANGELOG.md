# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-09-20

### Added
- Legacy Windows 7/8/8.1 compatibility path for the client with the `legacy-console` feature and a Rust 1.77 MSRV-compatible build profile.
- A dedicated Windows legacy packaging script and documentation for the compatibility installer and MSI flow.
- Unified release pipeline. Pushing a `v*.*.*` tag now triggers every packaging workflow and attaches all artefacts to a single GitHub release.
- `docs/RELEASING.md`: maintainer-facing guide for cutting a release.

### Changed
- Downgraded the workspace to Rust 2021, resolver 2, and `rust-version = "1.77"` to match the supported legacy Windows toolchain.
- Pinned the dependency graph to Rust 1.77-safe releases (`ratatui = 0.24.0`, `axum = 0.7.0`, `toml = 0.8.23`, `unicode-segmentation = 1.12.0`, and related compatibility pins).
- Kept the modern Windows 10/11 installer flow intact while segregating the legacy Windows compatibility bundle.

### Fixed
- Server display-name validation now counts Unicode characters, not bytes.
- Replaced modern Rust 2024-only syntax and const-mutating methods with 1.77-compatible code paths.
- Restored a clean changelog structure without duplicate or stale release entries.
- The legacy Windows installers now show the CodeAlchemy-Labs icon in the Windows Explorer, the taskbar, and the Add/Remove Programs entry.
- `packaging/linux/arch/PKGBUILD-*`: the `pkgver` was hardcoded and fell out of sync with the workspace. `build-arch.fish` now injects the version from `Cargo.toml` at build time, and the source tarball is consumed locally instead of being downloaded from an old tag.

### Security
- The server's RAII cleanup guarantees remain the critical defense against stale sessions and leaked resources during disconnects.

## [Unreleased]

## [0.2.0] - 2026-09-18

### Added
- Native Linux packages (`.deb`, `.rpm`, Arch `PKGBUILD`), musl tarballs, and AppImage support.
- Signed Windows 10/11 installers with Inno Setup and WiX.
- Client connection screen, config persistence, authentication, ranking, spectating, and production server hardening.
- Docker and Render deployment support for the server.

### Changed
- Standardized the client binary name to `tictacli` across the project.
- Migrated the workspace and packaging metadata to the unified release structure and updated installation docs.

### Fixed
- Corrected broken packaging references, stale binary names, and documentation mismatches around release artifacts.
- Resolved several compile-time and compatibility issues in the client and server code paths before the release.

## [0.1.1] - 2026-09-17

### Added
- TLS support in the client using the system trust store.
- The client's `--insecure` flag for development servers with self-signed certificates.
- `PORT` environment variable support and the root `render.yaml` deployment blueprint.
- README, architecture, and security documentation for remote deployments and TLS termination.

### Changed
- The client now uses `clap` and environment-variable fallbacks for its connection flags.
- The client validates `ws://` and `wss://` URL schemes on startup.
- The `port_reuse` scenario reports shared-namespace, privileged-port, and isolated-namespace outcomes honestly.
- Terminal tracing output disables ANSI sequences when the output terminal cannot render them; `NO_COLOR` and `CLICOLOR_FORCE` are honored.

### Fixed
- ANSI escape sequences are no longer emitted on Windows consoles that do not process Virtual Terminal codes.
- `ServerMessage::Error` tolerates a missing `code` field and falls back to `ErrorCode::Unknown` for compatibility with older servers.

## [0.1.0] - 2026-09-17

### Added
- Initial workspace, core game protocol, and the `common`, `server`, `client`, and `hacker` crates.
- Docker-based local development flow and initial packaging scaffolding.

### Changed
- Established the project structure and initial release pipeline for local and packaged deployment.
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

[Unreleased]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/CodeAlchemy-Labs/tictactoe-rs/releases/tag/v0.1.0
