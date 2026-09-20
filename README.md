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


## Client

The `tictacli` client presents a full-screen terminal interface. On startup, it attempts to connect to a server. If the server is provided via CLI flags (`--server` / `--name`) or environment variables, it fast-tracks the connection. Otherwise, it presents an interactive Connection screen to input the host, guest name, and TLS toggle.

If a connection attempt fails or times out (10-second limit), it returns gracefully to the Connection form to allow retrying without restarting the application. Successfully connected credentials are saved locally across restarts.

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
├── rustfmt.toml
├── Dockerfile                  ← demo image (server + client + hacker)
├── Dockerfile.server           ← production image (server only)
├── docker-compose.yml          ← demo stack
├── docker-compose.prod.yml     ← local production testing
├── Makefile
├── LICENSE
├── README.md
├── CHANGELOG.md
├── render.yaml
├── .dockerignore
├── .gitignore
├── .github/
│   └── workflows/
│       ├── ci.yml
│       ├── package-linux.yml
│       ├── package-portable.yml
│       ├── package-windows.yml
│       └── package-windows-legacy.yml
├── docs/
│   ├── ARCHITECTURE.md
│   ├── SECURITY.md
│   ├── USER_GUIDE.md
│   ├── DEPLOYMENT.md
│   └── INSTALLATION.md
├── packaging/
│   ├── linux/
│   │   └── appimage/
│   └── windows/
│       ├── legacy/
│       ├── portable/
│       └── build-installers.ps1
└── crates/
    ├── common/
    ├── server/
    ├── client/
    └── hacker/
```

## Prerequisites

- Rust 1.77.2 or newer
- Cargo 1.77.2 or newer
- Docker 29.8.0 or newer
- Docker Compose 5.5.1 or newer
- Fish shell 3.x (or any POSIX-compatible shell)

## Installation

We provide pre-built native packages for modern Windows 10/11, legacy Windows 7/8/8.1 compatibility builds, and various Linux distributions. See the [Installation Guide](docs/INSTALLATION.md) for download links, setup instructions, and compatibility details.

## Packaging

The CI pipeline automatically produces native packages on every release:

- **Windows:** `.exe` (Inno Setup) and `.msi` (WiX Toolset)
- **Linux:** `.deb` (Debian/Ubuntu), `.rpm` (Fedora/RHEL), `.pkg.tar.zst` (Arch), and a statically linked `.tar.gz` (musl)

If you only want to try the demo and do not need to install the application system-wide, the release page also provides portable archives containing the executables:

- `tictacli-<version>-x86_64.AppImage`
- `tictacli-server-<version>-x86_64.AppImage`
- `tictacli-server-<version>-x86_64-pc-windows-gnu.zip`
- `tictacli-<version>-x86_64-legacy.exe`
- `tictacli-<version>-i686-legacy.exe`
- `tictacli-<version>-legacy-setup.exe`
- `tictacli-<version>-legacy.msi`

The legacy Windows artefacts target Windows 7/8/8.1 and are built with the Rust 1.77 MSRV and `legacy-console` compatibility mode.

You can build the portable distributions locally by running:
```fish
make package-portable
```

Extract the archive or run the binaries directly. For example, to run the server:


```fish
./tictacli-server
```

In two other terminals, run the two clients:

```fish
./tictacli --server ws://127.0.0.1:8080/ws --name alice
```

```fish
./tictacli --server ws://127.0.0.1:8080/ws --name bob
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

The legacy Windows clients are intentionally separate from the modern installers
and are meant for the older Windows 7/8/8.1 console stack. They are end-of-life
compatibility builds and are not the default installation path.

On Windows, use `.\tictacli-server.exe`, `.\tictacli.exe`, and `.\hacker.exe` from
PowerShell or `cmd`.
## Quick start

### Local development

Run the server in one terminal:

```fish
make run-server
```

Run a client in a second terminal:

```fish
cargo run --release --bin tictacli -- --server ws://127.0.0.1:8080/ws --name alice
```

Run a second client in a third terminal:

```fish
cargo run --release --bin tictacli -- --server ws://127.0.0.1:8080/ws --name bob
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

## Production image

The demo image (`Dockerfile`) ships all three binaries — `server`, `client`,
and `hacker` — so that `make demo`, `make client-1`, `make client-2`, and
`make hacker` all work from the same image. That is deliberate for a demo,
but wrong for production:

- `hacker` is an adversarial tool. Shipping it in a production container
  hands an attacker a ready-made weapon if they ever break in.
- `client` is a TUI that requires a terminal. It has no function in a
  headless server container.

**`Dockerfile.server`** is the production image. It builds and ships only
the `server` binary, defaults to `TICTACTOE_ENV=production`, runs as
uid 1000, and includes a `/health` liveness check.

Build the production image:

```fish
make prod-build
```

Start the server on localhost:

```fish
make prod-up
```

Smoke-test it with the bundled client:

```fish
./tictacli --server ws://127.0.0.1:8080/ws --name alice
```

Or, if you don't have a local binary:

```fish
cargo run --release --bin tictacli -- --server ws://127.0.0.1:8080/ws --name alice
```

Stop the stack:

```fish
make prod-down
```

`docker-compose.prod.yml` binds to `127.0.0.1:8080`, not `0.0.0.0:8080`,
on purpose. If you want to expose the service on your LAN or the internet,
put a TLS-terminating reverse proxy (Caddy, Traefik, nginx) in front of it.
See [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md) for the full deployment
walkthrough including Render, Fly.io, and self-hosted paths.

### Connecting to a remote instance

For a step-by-step guide to deploying your own server, including production hardening and protection from the adversarial `hacker` crate, see [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md).


The client speaks both `ws://` and `wss://`. TLS is terminated by the
platform that hosts the server (for example, Render's edge proxy), so the
container itself continues to listen for plain HTTP/WS on its internal
port. The client verifies the certificate against the system's trusted
store, and the only thing that changes between a local and a remote session
is the URL:

```fish
# Local
./tictacli --server ws://127.0.0.1:8080/ws --name alice

# Remote over TLS (replace with the URL of your deployment)
./tictacli --server wss://<your-service>.onrender.com/ws --name alice
```

The server reads its bind address from `TICTACTOE_BIND` first and falls back
to `PORT`, the variable that container platforms such as Render inject.
This means the same image runs locally on `0.0.0.0:8080` and on a platform
without any configuration change:

```fish
# Local default
./tictacli-server

# Explicit port
PORT=9090 ./tictacli-server

# Explicit full bind address
TICTACTOE_BIND=127.0.0.1:9999 ./tictacli-server
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

## Environment variables

The server is configured entirely via environment variables.

| Variable | Default (Development) | Default (Production) | Description |
|---|---|---|---|
| `TICTACTOE_ENV` | `development` | N/A | The deployment environment mode. Accepted values: `development` or `production`. Controls the default values of limits. |
| `TICTACTOE_BIND` | `0.0.0.0:8080` | `0.0.0.0:8080` | The address and port the server binds to. Takes precedence over `PORT`. |
| `PORT` | `8080` | `8080` | The port the server binds to. Used by container platforms like Render. |
| `TICTACTOE_MAX_SESSIONS` | `10000` | `10000` | The global maximum number of concurrent WebSocket sessions allowed. |
| `TICTACTOE_MAX_SESSIONS_PER_IP` | `10000` | `10` | The maximum number of concurrent sessions allowed from a single IP address. |
| `TICTACTOE_AUTH_RATE_LIMIT_PER_MINUTE` | `1000` | `10` | The maximum number of authentication attempts (register/login) per minute per IP address. |

## What the demo shows

The hacker runs four scenarios. Output from a Docker run, with the server
reachable over the compose bridge network:

```
DEFENDED session_hijack: all probes rejected; malformed payload ignored without closing the connection
DEFENDED port_reuse: server reachable at server:8080; isolated network namespace: 0.0.0.0:8080 is bindable from this process because each namespace owns its own port space; ephemeral 127.0.0.1:43095 released synchronously on drop and immediately rebindable
DEFENDED flood: 32 concurrent sessions opened and closed; server still accepts new sessions (local failures: 0)
DEFENDED spectator_isolation: non-players cannot interact with the board; already spectating a match is properly tracked
```

Output from a local run, with the server and the hacker sharing the host's
network namespace:

```
DEFENDED session_hijack: all probes rejected; malformed payload ignored without closing the connection
DEFENDED port_reuse: server reachable at 127.0.0.1:8080; EADDRINUSE on 0.0.0.0:8080: the server holds the port in the shared namespace; ephemeral 127.0.0.1:42379 released synchronously on drop and immediately rebindable
DEFENDED flood: 32 concurrent sessions opened and closed; server still accepts new sessions (local failures: 0)
DEFENDED spectator_isolation: non-players cannot interact with the board; already spectating a match is properly tracked
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

## Security highlights

The server includes extensive production hardening logic added in v0.2.0:
- Argon2id password hashing is offloaded to a blocking thread pool to avoid starving the async runtime.
- Global session caps prevent memory exhaustion from runaway connections.
- Per-IP connection caps mitigate simplistic botnets and malicious tenants on shared networks.
- A token-bucket rate limiter governs authentication requests per IP to thwart brute-force password guessing.
- An adversarial actor (`hacker`) ensures these state machine invariants and resource defenses hold. By default, the hacker refuses to target non-local servers. Targeting a remote production instance requires the explicit `--allow-production` flag.

For the full security rationale and the specific behaviors the hacker scenarios
verify, see [`docs/SECURITY.md`](docs/SECURITY.md).

## User guide

For the full walkthrough of screens, keys, and flows, see [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md).

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

The workspace curates a custom `clippy` profile centrally in `Cargo.toml`. Standard rust compiler warnings are denied, and a subset of `clippy::pedantic` lints are promoted to `warn` level. Any warning is treated as an error in CI.

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
