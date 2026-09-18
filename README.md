# tictactoe-rs

[![ci](https://github.com/CodeAlchemy-Labs/tictactoe-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/CodeAlchemy-Labs/tictactoe-rs/actions/workflows/ci.yml)
[![release](https://github.com/CodeAlchemy-Labs/tictactoe-rs/actions/workflows/release.yml/badge.svg)](https://github.com/CodeAlchemy-Labs/tictactoe-rs/actions/workflows/release.yml)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

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

## Pre-built binaries

If you only want to try the demo and do not need to build from source, the
release page provides pre-compiled binaries for Linux and Windows:

- `tictactoe-rs-<version>-x86_64-unknown-linux-gnu.tar.gz`
- `tictactoe-rs-<version>-x86_64-pc-windows-msvc.zip`

Each archive contains three executables (`server`, `client`, `hacker` and
their `.exe` counterparts on Windows), plus `README.md`, `CHANGELOG.md`, and
`LICENSE`.

Download the archive for your platform from
[the latest release](https://github.com/CodeAlchemy-Labs/tictactoe-rs/releases/latest),
extract it, and run the server:

```fish
./server
```

In two other terminals, run the two clients:

```fish
./client --server ws://127.0.0.1:8080/ws --name alice
```

```fish
./client --server ws://127.0.0.1:8080/ws --name bob
```

And in a fourth terminal, run the hacker scenarios:

```fish
./hacker --target ws://127.0.0.1:8080/ws --scenario all
```

The binaries accept both `ws://` and `wss://` URLs. TLS uses the system's
trusted certificate store; no additional configuration is needed to connect
to a server behind a valid certificate authority. If you need to connect to
a development server with a self-signed certificate, pass `--insecure` to
disable certificate verification. Do not use `--insecure` against a
production endpoint.

On Windows, use `.\server.exe`, `.\client.exe`, and `.\hacker.exe` from
PowerShell or `cmd`.

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

### Connecting to a remote instance

The client speaks both `ws://` and `wss://`. TLS is terminated by the
platform that hosts the server (for example, Render's edge proxy), so the
container itself continues to listen for plain HTTP/WS on its internal
port. The client verifies the certificate against the system's trusted
store, and the only thing that changes between a local and a remote session
is the URL:

```fish
# Local
./client --server ws://127.0.0.1:8080/ws --name alice

# Remote over TLS (replace with the URL of your deployment)
./client --server wss://<your-service>.onrender.com/ws --name alice
```

The server reads its bind address from `TICTACTOE_BIND` first and falls back
to `PORT`, the variable that container platforms such as Render inject.
This means the same image runs locally on `0.0.0.0:8080` and on a platform
without any configuration change:

```fish
# Local default
./server

# Explicit port
PORT=9090 ./server

# Explicit full bind address
TICTACTOE_BIND=127.0.0.1:9999 ./server
```

#### Deploying to Render

A `render.yaml` blueprint is included at the root of the repository. To
create the service:

1. Push the repository to GitHub.
2. In the Render dashboard, choose **New > Blueprint** and select the
   repository.
3. Render reads `render.yaml`, builds the image, and deploys it. The service is reachable at `https://<service-name>.onrender.com`, and
   WebSocket clients connect to `wss://<service-name>.onrender.com/ws`. Use the
   service name you chose in the dashboard; it is not fixed by the blueprint.

The free tier spins down after 15 minutes of inactivity. The first request
after a spin-down takes roughly 30 seconds while the container restarts.

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

Or, for the full suite with coverage, install the required tools first:

```fish
make tools
```

This installs `cargo-llvm-cov` and the `llvm-tools-preview` component. Then:

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