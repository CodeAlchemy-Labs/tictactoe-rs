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
session token may stay valid, and an attacker has a chance to reuse resources
that should already be gone.

Rust closes that window by construction. Every `TcpListener`, every
`WebSocket` stream, and every `Session` value owns its underlying resource.
When the value goes out of scope, `Drop` runs immediately and deterministically
releases the resource. No GC, no finalizer queue, no race.

This repository makes that behavior observable through a scripted demo: a
"hacker" actor attempts to hijack a session and to bind a port that was just
released, and the server rejects both attempts because Rust has already
cleaned up.

## Architecture

The project is organized as a Cargo workspace with three crates plus one
auxiliary binary:

- `crates/common` — shared protocol types, DTOs, and error contracts.
- `crates/server` — Axum + Tokio WebSocket server, in-memory lobby and match state.
- `crates/client` — Terminal UI built with `ratatui` and `crossterm`.
- `crates/hacker` — Adversarial actor used by the security demonstration.

Each crate follows a layered structure (`domain`, `application`,
`infrastructure`) and honors the SOLID principles and conventional Rust
idioms.

## Repository layout

```
tictactoe-rs/
├── Cargo.toml
├── LICENSE
├── README.md
├── crates/
│   ├── common/
│   ├── server/
│   ├── client/
│   └── hacker/
├── docs/
├── scripts/
├── Dockerfile
├── docker-compose.yml
└── Makefile
```

The directories under `crates/`, `docs/`, `scripts/`, and the Docker and
Makefile artifacts are added in subsequent steps.

## Prerequisites

- Rust 1.97.1 or newer
- Cargo 1.97.1 or newer
- Docker 29.8.0 or newer
- Docker Compose 5.5.1 or newer
- Fish shell 3.x (or any POSIX-compatible shell)

## Status

Work in progress. The repository currently contains only the workspace
bootstrap. Implementation of the crates, the Docker image, the automated
demo, and the test suite is delivered step by step.

## License

Released under the MIT License. See [LICENSE](LICENSE) for details.

## Authors

Maintained by [CodeAlchemy-Labs](https://github.com/CodeAlchemy-Labs).