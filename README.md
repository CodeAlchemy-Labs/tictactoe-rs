# Tic-Tac-Toe RS

A WebSocket-based Tic-Tac-Toe game demonstrating Rust ownership-driven safety guarantees.

## Features

- **Authentication**: Secure Argon2id password hashing and user registry.
- **Ranking**: Global ranking of top players by wins.
- **Spectators**: Real-time spectating of ongoing matches.
- **Grace-Period Abandon**: Reconnection grace period for unexpected disconnects.
- **Production Hardening**: Rate limiting, session caps, and strict mode controls.

## Prerequisites

- Rust toolchain (MSRV: 1.97.0)

## How to Run

### Server
```sh
cargo run -p server
```

### Client
```sh
cargo run -p client -- ws://127.0.0.1:8080
```

### Hacker (Adversarial Actor)
```sh
cargo run -p hacker -- ws://127.0.0.1:8080
```
Note: The hacker requires the `--allow-production` flag if run against a remote environment.

## Environment Variables (Server)

| Variable | Description |
|---|---|
| `TICTACTOE_ENV` | `development` or `production`. Defaults to `development`. |
| `TICTACTOE_BIND` | Bind address (e.g. `0.0.0.0:8080`). |
| `PORT` | Bind port, used by platforms like Render. |
| `TICTACTOE_MAX_SESSIONS` | Global concurrent session limit. |
| `TICTACTOE_MAX_SESSIONS_PER_IP` | Per-IP concurrent session limit. |
| `TICTACTOE_AUTH_RATE_LIMIT_PER_MINUTE` | Max authentication attempts per minute per IP. |

## Client Keyboard Shortcuts

| Key | Action |
|---|---|
| `Up`, `Down`, `Left`, `Right`, `h`, `j`, `k`, `l` | Move cursor |
| `Enter`, `Space` | Place mark / Select |
| `q`, `Esc` | Quit / Back / Leave Match |
| `Tab` | Switch focus (e.g., in forms) |
| `Backspace` | Delete character in input fields |

## Testing

Run the test suite with:
```sh
cargo test --workspace
```

## License

This project is licensed under the MIT License.
