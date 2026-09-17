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

When the server and the hacker run on the same host, the hacker shares the
host's network namespace. The `port_reuse` scenario then reports
`EADDRINUSE` for the server's port, which is the strongest form of the
defense. The full local output is shown in the next section.

### Docker demo

Build the image and start the server:

```fish
make demo
```

This builds the image and starts a single container (`server`). The two
clients are interactive terminal programs; running them in the background
would leave them without a TTY and they would exit immediately. Instead,
each client runs in the foreground in its own terminal, using
`docker compose run -it`:

```fish
make client-1
```

```fish
make client-2
```

Inside the client:

- `c` creates a match.
- `r` refreshes the list of open matches.
- `1`–`9` select the numbered match in the lobby, or map to a cell in
  row-major order while playing.
- `q` quits.

Run the hacker scenarios against the running server from a third terminal:

```fish
make hacker
```

The hacker prints one line per scenario with `DEFENDED` or `COMPROMISED`,
and exits with code `0` if every scenario is defended. The Docker output is
shown in the next section.

Tear down the stack:

```fish
make demo-down
```

## What the demo shows

The hacker runs three scenarios. Output from a Docker run, with the server
reachable over the compose bridge network:

```
DEFENDED session_hijack: all probes rejected; malformed payload ignored without closing the connection
DEFENDED port_reuse: server reachable at server:8080; isolated network namespace: 0.0.0.0:8080 is bindable from this process because each namespace owns its own port space; ephemeral 127.0.0.1:43095 released synchronously on drop and immediately rebindable
DEFENDED flood: 32 concurrent sessions opened and closed; server still accepts new sessions (local failures: 0)
```

Output from a local run, with the server and the hacker sharing the host's
network namespace:

```
DEFENDED session_hijack: all probes rejected; malformed payload ignored without closing the connection
DEFENDED port_reuse: server reachable at 127.0.0.1:8080; EADDRINUSE on 0.0.0.0:8080: the server holds the port in the shared namespace; ephemeral 127.0.0.1:42379 released synchronously on drop and immediately rebindable
DEFENDED flood: 32 concurrent sessions opened and closed; server still accepts new sessions (local failures: 0)
```

The `port_reuse` scenario is deliberately honest about the two environments.
Docker's default bridge network gives every container its own port space, so
a bind attempt from one container does not contend with another. What the
scenario proves in both environments is the property that matters for this
project: a `TcpListener` releases its port the instant it is dropped, and
the same process can rebind it immediately. There is no window during which
a garbage collector decides whether the port is free.

Three behaviors are demonstrated:

1. **Illegal state transitions are rejected.** The hacker cannot act on a
   match it is not part of (`NotInMatch`), cannot join a match that does not
   exist (`MatchNotFound`), cannot register twice on the same connection
   (`InvalidState`), and cannot disrupt the connection with a malformed
   payload (the connection survives and answers a subsequent `Ping` with
   `Pong`).

2. **The server owns its port for as long as it runs.** When the hacker
   shares the server's network namespace, the kernel refuses the bind with
   `EADDRINUSE`. When the server process exits, the port is released
   synchronously through the `Drop` of `TcpListener`.

3. **Ephemeral ports are released the instant the owning listener is
   dropped.** The hacker binds an ephemeral port, drops the listener, and
   immediately rebinds the same port. The rebind succeeds because Rust runs
   `Drop` synchronously and there is no GC window.

A fourth property is exercised by the integration test suite: when a client
disconnects, the server's session is removed from the lobby, its outbound
channel is closed, and the writer task terminates — all before the handler
returns. This is enforced by the `SessionGuard` RAII type and verified by
`disconnect_removes_the_session_from_the_lobby` in
`crates/server/tests/websocket.rs`. The server log during a flood shows the
session guard dropping for every client, in order, without delay.

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