# Architecture

This document describes the design of `tictactoe-rs`, the reasoning behind
each crate boundary, and the patterns that keep the code testable and
auditable.

## Workspace layout

The project is a Cargo workspace with four member crates:

```
crates/
  common/   Shared protocol contracts and pure domain rules.
  server/   Axum + Tokio WebSocket server with in-memory lobby.
  client/   Terminal client with a state machine and a TUI.
  hacker/   Adversarial actor used to demonstrate server defenses.
```

Splitting into four crates is not ceremony. Each boundary enforces a rule:

- `common` has no I/O, no runtime, and no dependency on any other crate.
  Because of that, the protocol is a single source of truth: a change to the
  wire format causes a compile error in every consumer.
- `server` depends on `common` but never on `client` or `hacker`. The server
  cannot accidentally couple to a UI concern.
- `client` depends on `common` but never on `server`. The client cannot
  accidentally peek at server internals; it can only speak the protocol.
- `hacker` depends on `common` in production code and on `server` only as a
  dev-dependency for integration tests. The production binary speaks the same
  protocol as any other client.

## Layering inside each crate

Every crate follows the same three-layer structure:

- **Domain** — value objects, aggregates, and rules. No I/O, no async. The
  `Board` aggregate on both server and client is a value type; the `Session`
  and `Match` types on the server are stateful but lock-free by design.
- **Application** — orchestration. On the server this is `LobbyService`, `RankingService`, etc. On
  the client this is `AppState` plus the pure `apply_event` function.
- **Infrastructure** — adapters to the outside world. Axum routes and the
  WebSocket handler on the server; the `Transport` trait and its two
  implementations on the client.

This is a classic ports-and-adapters arrangement. The domain layer never
mentions `tokio`, `axum`, or `crossterm`; the infrastructure layer never
contains game logic.

## Concurrency model

### Server

One process-wide `LobbyService` holds a `Mutex<LobbyState>`:

```rust
struct LobbyState {
    next_client_id: u64,
    next_match_id: u64,
    sessions: HashMap<ClientId, Session>,
    matches: HashMap<MatchId, Match>,
}
```

The mutex is `std::sync::Mutex`, not `tokio::sync::Mutex`. The reason is
subtle and important: `SessionGuard::drop` is synchronous and needs to call
`LobbyService::disconnect` without awaiting. A `tokio::sync::Mutex` cannot be
locked from a synchronous `Drop`. Since every critical section performs only
`HashMap` mutations (nanoseconds) and never crosses an `.await`, using the
standard mutex is both correct and faster.

Each WebSocket connection runs two tasks plus the handler:

```
handler ─┬─> reader loop (stream.next().await)
         ├─> writer task (rx.recv().await)
         └─> SessionGuard (Drop on handler return)
```

The invariant is: when the handler returns, the guard drops, the session is
removed from the lobby, the sender side of the outbound channel drops, the
writer task observes `None` on `recv`, and the socket closes. Every step is
deterministic and enforced by the type system.

### Client

The client runs three tasks plus the main loop:

```
main loop ──> AppState + apply_event
   ▲
   ├── server task (incoming messages)
   └── keyboard task (spawn_blocking)
```

The main loop consumes `AppEvent` values, applies them to the state, and
draws the frame. The UI thread never blocks on I/O.

## TLS termination

The server does not implement TLS. In every supported deployment — local
development, Docker Compose, Render, and any other container platform —
TLS is terminated by the edge: the client speaks `wss://` to the edge, the
edge speaks plain `ws://` to the container, and the container only ever
binds a plain TCP socket.

This is a deliberate choice, not an omission:

- **Simplicity.** Certificate issuance, rotation, and renewal are the
  platform's responsibility. There is no TLS configuration to keep in
  sync with the deployment.
- **No dependency on OpenSSL or a bundled CA at runtime.** The server image
  stays small and free of any TLS library.
- **The client is where TLS matters.** The `client` crate enables
  `rustls-tls-native-roots` in `tokio-tungstenite`, so it can connect to
  any `wss://` endpoint using the system trust store, including the flag
  `--insecure` for development servers with self-signed certificates.

If the deployment model changes and the server must terminate TLS itself,
the change is localized to `server/src/infrastructure/http.rs` and the
`Dockerfile`. The domain and application layers would be unaffected.

## Protocol

The client and server communicate via WebSocket text frames containing JSON-serialized messages.
- `common::protocol::ClientMessage`: Messages sent from client to server (e.g., `Hello`, `JoinMatch`, `MakeMove`, `Register`, `Login`).
- `common::protocol::ServerMessage`: Messages sent from server to client (e.g., `Welcome`, `MatchReady`, `BoardUpdate`, `Error`).

Errors are propagated natively as `ErrorCode` enumerations inside `ServerMessage::Error`.

## Lobby State Machine

The central `LobbyService` manages all connected sessions and active matches. It owns a `Mutex<LobbyState>` and mutates the state by locking it, performing necessary updates, and releasing it before any `.await` points to avoid deadlocks. This includes transitions for adding players, attaching spectators, and managing disconnection events (grace period tracking).

## Session Lifecycle

The lifecycle of a session flows chronologically based on client interaction:

1. **Guest**: Sessions start unauthenticated.
2. **Authenticated**: Sessions can `Register` or `Login`. Enforces a single-session-per-username invariant.
3. **Playing / Spectating**: Clients can start a match or spectate an existing one.
4. **Disconnected**: Handled by the `SessionGuard` chain.

## Grace Period

If an active player disconnects during a match, they enter a grace period (handled by `LobbyService::disconnect`). The match pauses, and the opponent is notified via `OpponentDisconnected`. If the player reconnects within the 2-second timeout window, the match resumes (`OpponentReconnected`); otherwise, it is forcefully abandoned and the remaining player is awarded the win (`MatchAbandoned`).

## Spectator Model

Clients can spectate ongoing matches. Spectators receive a full `SpectateStarted` snapshot, followed by real-time `BoardUpdate`, `SpectatorJoined`, and `SpectatorLeft` broadcasts. To prevent resource exhaustion, the server enforces a strict 5-spectator cap per match.

## Ranking Service

The `RankingService` tracks wins for registered users. It is explicitly isolated from the `LobbyService` to prevent circular dependencies. The lobby sends a message to the ranking service only when a match concludes with a legitimate victory (not a draw or abandonment).

## Security Boundaries

The server strictly validates all client inputs. As part of its 0.2.0 production hardening:
- **Argon2id on Blocking Pool**: Password hashing uses Argon2id and is offloaded to a dedicated `spawn_blocking` pool to prevent stalling the async runtime.
- **Caps & Limits**: The server respects environment variables that define global session limits, per-IP connection limits, and per-IP authentication rate limits (using a token bucket algorithm). 

## Patterns

### Aggregate root

`Board` (in `common`) and `Match` (in `server`) are aggregates. Their state
is private or only mutated through methods that enforce invariants. The
`Board::place` method is the only way to set a cell, and it rejects occupied
positions. The compiler and the API surface make it impossible to construct
an invalid board.

### Strategy

The client's `Transport` trait is the strategy. `WsTransport` speaks
WebSocket; `MockTransport` exchanges messages over in-memory channels. The
rest of the client is generic over the trait and therefore testable without
binding a socket.

### State machine as a pure function

`apply_event(state, event, effects)` is a pure function. It does not touch
the network, the terminal, or the clock. A test drives state transitions and
inspects both the resulting state and the list of `SideEffect` values. This
is the reason the client has ~15 unit tests and no flaky integration tests.

### RAII guard

Both `SessionGuard` on the server and `TerminalSession` on the client are
RAII guards. They acquire a resource in `new` and release it in `Drop`. There
is no `try`/`finally` in Rust; the guard is the idiom, and the compiler
guarantees the release runs on every exit path, including panics.

## Error handling

- Library crates use `thiserror` to define typed errors (`DomainError`,
  `ProtocolError`, `TransportError`).
- Binaries use `anyhow` to attach context to errors as they bubble up.
- Protocol errors are values, not exceptions: `ServerMessage::Error` carries
  a stable `ErrorCode` plus a human message. Clients can match on the code
  without parsing text.

The rule is: errors are either part of the domain (typed, matched on) or
they are bugs/environmental failures (propagated with context, logged, and
either recovered or aborted).

## Testing strategy

- Unit tests live next to the code they exercise, in `#[cfg(test)] mod tests`.
- Integration tests live in `tests/` and exercise the public API across
  process boundaries (server WebSocket, hacker scenarios, client state
  machine).
- The `common` crate has no I/O and is fully testable without a runtime.
- The server has 3 integration tests that spin up a real Axum server on an
  ephemeral port.
- The client has 2 integration tests that drive the state machine over a
  `MockTransport`, with no socket.
- The hacker has 3 integration tests that run each scenario against a real
  server and assert `Outcome::Defended`.

## What the architecture deliberately does not do

- No database. All state is in memory and disappears when the server stops.
- No reconnection logic in the client. A dropped WebSocket is fatal; the user
  restarts the client.
- No graceful drain of in-flight matches on server shutdown. Axum's
  `with_graceful_shutdown` waits for in-flight requests, but the lobby does
  not persist anything.

Each omission is documented here so that no one mistakes the demo for
production software.
