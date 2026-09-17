# tictactoe-rs

A WebSocket-based Tic-Tac-Toe game written in Rust, built as a teaching
reference for secure concurrent network programming. The project demonstrates
how Rust's ownership model and deterministic destruction eliminate whole
classes of resource leaks that are common in garbage-collected runtimes such
as Java or C#.

## Motivation

When a client disconnects from a server written in a garbage-collected
language, the socket, the session, and any associated buffers may survive
until the next GC cycle. During that window the port can remain bound, the
session token may stay valid, and an attacker has a chance to reuse
resources that should already be gone.

Rust closes that window by construction. Every `TcpListener`, every
WebSocket stream, and every `Session` value owns its underlying resource.
When the value goes out of scope, `Drop` runs immediately and
deterministically releases the resource. No GC, no finalizer queue, no race.

This repository makes that behavior observable through a scripted demo: a
"hacker" actor attempts to hijack a session and to bind a port that was just
released, and the server rejects both attempts because Rust has already
cleaned up.

## Architecture

The project is organized as a Cargo workspace with four crates:

- `crates/common` — shared protocol types, DTOs, and error contracts.
- `crates/server` — Axum + Tokio WebSocket server with an in-memory lobby.
- `crates/client` — Terminal UI built with `ratatui` and `crossterm`.
- `crates/hacker` — Adversarial actor used by the security demonstration.

Each crate follows a layered structure (`domain`, `application`,
`infrastructure`) and honors the SOLID principles and conventional Rust
idioms. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full
design.

## Repository layout

```
tictactoe-rs/
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── docker-compose.yml
├── Makefile
├── LICENSE
├── README.md
├── .dockerignore
├── .gitignore
├── .github/
│   └── workflows/
│       └── ci.yml
├── docs/
│   ├── ARCHITECTURE.md
│   └── SECURITY.md
└── crates/
    ├── common/
    ├── server/
    ├── client/
    └── hacker/
```

## Prerequisites

- Rust 1.97.1 or newer
- Cargo 1.97.1 or newer
- Docker 29.8.0 or newer
- Docker Compose 5.5.1 or newer
- Fish shell 3.x (or any POSIX-compatible shell)

## Quick start

### Local development

Run the server in one terminal:

```fish
make run-server
```

Run a client in a second terminal:

```fish
cargo run --release --bin client -- --server ws://127.0.0.1:8080/ws --name alice
```

Run a second client in a third terminal:

```fish
cargo run --release --bin client -- --server ws://127.0.0.1:8080/ws --name bob
```

Run the hacker against the running server:

```fish
make run-hacker
```

### Docker demo

Build the image and start the stack:

```fish
make demo
```

This brings up three containers: `server`, `client-1`, and `client-2`.
Attach to either client's TUI from two separate terminals:

```fish
make attach-client-1
```

```fish
make attach-client-2
```

Inside the client:

- `c` creates a match.
- `r` refreshes the list of open matches.
- `1`–`9` map to cells in row-major order.
- `q` quits.

Run the hacker scenarios against the running server:

```fish
docker compose --profile demo run --rm hacker
```

The hacker prints one line per scenario with `DEFENDED` or `COMPROMISED`,
and exits with code `0` if every scenario is defended. The full output is
shown in the next section.

Tear down the stack:

```fish
make demo-down
```

## What the demo shows

When the hacker runs against the live server, the output looks like this:

```
DEFENDED session_hijack: all probes rejected; malformed payload ignored without closing the connection
DEFENDED port_reuse: server holds 172.18.0.2:8080 (EADDRINUSE); ephemeral 127.0.0.1:43201 released synchronously on drop
DEFENDED flood: 32 concurrent sessions opened and closed; server still accepts new sessions (local failures: 0)
```

Three things are demonstrated:

1. **Illegal state transitions are rejected.** The hacker cannot act on a
   match it is not part of, cannot join a match that does not exist, cannot
   register twice on the same connection, and cannot disrupt the connection
   with a malformed payload.

2. **The server's port is owned for as long as the server runs.** The kernel
   refuses the hacker's bind attempt with `EADDRINUSE`. When the server
   process exits, the port is released synchronously.

3. **Ephemeral ports are released the instant the owning listener is
   dropped.** The hacker binds an ephemeral port, drops it, and immediately
   rebinds the same port. The rebind succeeds because Rust runs `Drop`
   synchronously and there is no GC window.

A fourth property is exercised by the integration test suite: when a client
disconnects, the server's session is removed from the lobby, its outbound
channel is closed, and the writer task terminates — all before the handler
returns. This is enforced by the `SessionGuard` RAII type and verified by
`disconnect_removes_the_session_from_the_lobby` in
`crates/server/tests/websocket.rs`.

## Development

### Running tests

```fish
make test
```

Or, for the full suite with coverage:

```fish
make coverage
```

The coverage report is generated at `target/llvm-cov/html/index.html`.

### Linting and formatting

```fish
make lint
make fmt
```

The workspace enforces `clippy::all` and `clippy::pedantic` at the crate
level, plus `missing_docs` and `unsafe_code`. Any warning is treated as an
error in CI.

### Documentation

```fish
make doc
```

The generated documentation is at `target/doc/tictactoe_rs/index.html`.

## Continuous integration

The GitHub Actions workflow at `.github/workflows/ci.yml` runs five jobs on
every push and pull request:

- `fmt` — checks formatting with `cargo fmt --check`.
- `clippy` — runs `cargo clippy --workspace --all-targets -- -D warnings`.
- `test` — runs `cargo test --workspace --all-targets`.
- `doc` — builds the documentation with `RUSTDOCFLAGS=-D warnings`.
- `coverage` — runs `cargo llvm-cov` and uploads the LCOV report.

## Design notes

For a detailed walk-through of the architecture, the concurrency model, and
the design patterns used, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

For the security rationale and the specific behaviors the hacker scenarios
verify, see [`docs/SECURITY.md`](docs/SECURITY.md).

## License

Released under the MIT License. See [LICENSE](LICENSE) for details.

## Authors

Maintained by [CodeAlchemy-Labs](https://github.com/CodeAlchemy-Labs).