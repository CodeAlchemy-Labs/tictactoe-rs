# Architecture

The workspace is organized into four main crates:

- **`common`**: Shared protocol contracts (`ClientMessage`, `ServerMessage`) and domain types (`Board`, `Player`, `Position`, `GameStatus`). Contains no I/O or networking code.
- **`server`**: The WebSocket server. Manages the lobby state, authentication, and game logic coordination.
- **`client`**: A Terminal UI (TUI) client built with `ratatui` for interacting with the server.
- **`hacker`**: An adversarial actor designed to test the server's defenses against malicious behavior, fuzzy inputs, and denial-of-service attempts.

## Protocol

The client and server communicate via WebSocket text frames containing JSON-serialized messages.
- `common::protocol::ClientMessage`: Messages sent from client to server (e.g., `Hello`, `JoinMatch`, `MakeMove`, `Register`, `Login`).
- `common::protocol::ServerMessage`: Messages sent from server to client (e.g., `Welcome`, `MatchReady`, `BoardUpdate`, `Error`).

## Lobby State Machine

The central `LobbyService` manages all connected sessions and active matches. It owns a `Mutex<LobbyState>` and mutates the state by locking it, performing necessary updates, and releasing it before any `.await` points to avoid deadlocks.

## Session Lifecycle & Grace Period

- **Guest**: Sessions start unauthenticated.
- **Authenticated**: Sessions can `Register` or `Login`. Enforces a single-session-per-username invariant.
- **Grace Period**: If an active player disconnects during a match, they enter a grace period (handled by `LobbyService::disconnect`). The match pauses, and the opponent is notified. If the player reconnects in time, the match resumes; otherwise, it is abandoned and the remaining player is awarded the win.

## Spectator Model

Clients can spectate ongoing matches. Spectators receive a full `SpectateStarted` snapshot, followed by real-time `BoardUpdate`, `SpectatorJoined`, and `SpectatorLeft` broadcasts.

## Ranking Service

The `RankingService` tracks wins for registered users. It is isolated from the `LobbyService` to prevent circular dependencies, receiving updates only when a match concludes with a legitimate victory (not a draw or abandonment).

## Security Boundaries

The server strictly validates all client inputs, enforces rate limiting per IP, limits maximum concurrent sessions, and uses Argon2id for secure password hashing on dedicated blocking threads to avoid stalling the async runtime.
