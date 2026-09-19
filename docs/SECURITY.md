# Security

## Threat Model

The `tictactoe-rs` server is designed to withstand a hostile network environment. We assume clients may be malicious, sending invalid JSON, out-of-order messages, unauthorized actions, or attempting denial-of-service (DoS).

## Production Hardening

In `production` mode (set via `TICTACTOE_ENV=production`), the server enforces strict limits:
- **Session Caps**: Maximum concurrent sessions are aggressively bounded to prevent memory exhaustion.
- **Per-IP Limits**: Caps concurrent connections from a single IP to mitigate simple botnets or malicious actors on shared networks.
- **Rate Limiting**: Authentication endpoints (login, register) are rate-limited via a token bucket algorithm to prevent brute-force attacks and CPU starvation from Argon2id hashing.

## The Hacker Crate

The workspace includes a `hacker` crate. This is an automated, adversarial actor built to run fuzzing-style and exploit scenarios against the server. It tests for:
- Invalid authentication attempts.
- State-machine violations (e.g., trying to place marks in a match you are not part of, or out of turn).
- Protocol abuse and malformed payloads.

### `--allow-production` Guard

By default, the `hacker` refuses to connect to a remote server. To run the hacker against a remote environment (e.g., a staging server), you must explicitly pass the `--allow-production` flag, acknowledging the risk of running adversarial scenarios against deployed infrastructure.

## Reporting a Vulnerability

If you discover a security vulnerability, please refrain from disclosing it publicly until a patch is released. Report it directly to the maintainers by opening a confidential security advisory on GitHub.
