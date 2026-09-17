# Security

This document explains what `tictactoe-rs` demonstrates about deterministic
resource management in Rust, and what the adversarial scenarios actually
prove.

## The problem: resource lifetime in managed runtimes

In Java or C#, the lifecycle of a socket, a session, or a buffer is
ultimately governed by the garbage collector or by a finalizer queue. When a
client disconnects, the language runtime does not guarantee that:

1. The socket is closed before the next scheduler tick.
2. The session object becomes unreachable before the next GC cycle.
3. The port associated with the socket is released before a subsequent bind.

The window between "the client is gone" and "the OS has reclaimed the
resource" is observable by any concurrent actor with access to the same
machine or network. During that window, an attacker may be able to:

- Reconnect using a session token that the server still considers valid.
- Bind a port that the server has not yet closed.
- Observe state associated with a session that should already be gone.

These classes of bug are not hypothetical; they are a recurring source of
CVEs in server software written in managed languages.

## How Rust closes the window

Rust guarantees that a value's `Drop` implementation runs synchronously when
the value goes out of scope. The guarantee is provided by the ownership
model: there is no GC, no finalizer queue, and no scheduler between the end
of a scope and the execution of `Drop`. The `Drop` runs on the thread that
drops the value, before the next statement executes.

In `tictactoe-rs` this guarantee is used at three levels.

### Level 1: TcpListener and TcpStream

The server's `TcpListener` is created inside `main` and held by the
`axum::serve` future. When the future is dropped (either because the process
is shutting down or because an error propagated), the listener is dropped
and the port is released by the kernel synchronously. There is no lag.

The client's `TcpStream` is owned by the WebSocket stream, which is split
into a sink and a stream inside the transport task. When the task ends, the
stream is dropped, the file descriptor is closed, and the ephemeral port is
released. This is verified by the `port_reuse` scenario: the hacker binds
an ephemeral port, drops the listener, and immediately rebinds the same
port. It succeeds, because Rust has already released it.

### Level 2: Session and its outbound channel

Each connected client has a `Session` in the lobby. The `Session` owns the
`UnboundedSender<ServerMessage>` used to push messages to that client. When
the session is removed from the lobby, the sender is dropped, the receiver
side of the channel observes the closure, and the writer task terminates.
This chains through the following types:

```
SessionGuard (Drop)
   └─> LobbyService::disconnect
         └─> remove Session from HashMap
               └─> drop UnboundedSender
                     └─> writer task's rx.recv() returns None
                           └─> SplitSink drops
                                 └─> TcpStream drops
                                       └─> fd closes
```

Every step is enforced by the compiler. There is no path through
`handle_socket` that skips the guard, and there is no way to leak the
session into a global collection.

### Level 3: the SessionGuard RAII wrapper

`SessionGuard` is created immediately after `LobbyService::register` and
lives until `handle_socket` returns. The handler has exactly one explicit
`drop(guard)` before `writer.await`, which orders the cleanup correctly.
Without that ordering, the writer task would deadlock on `rx.recv()`,
waiting for a sender that only drops when the guard drops, which only
happens after the writer returns. This deadlock was discovered by an
integration test (`disconnect_removes_the_session_from_the_lobby`) and fixed
by making the ordering explicit.

The fact that the bug was caught deterministically — not as a race but as a
reproducible test failure — is the point. A GC language would have hidden
the ordering issue until production.

## TLS termination and trust boundaries

When the server is deployed behind a TLS-terminating proxy (such as
Render's edge), the wire between the client and the proxy is encrypted, and
the wire between the proxy and the container is plain HTTP/WS on a private
network. This is the standard model for container platforms and has three
security consequences worth spelling out.

1. **Confidentiality on the public segment.** A passive observer on the
   public internet cannot read the WebSocket frames. The proxy's
   certificate is validated by the client against the system trust store
   (`rustls-tls-native-roots`), so an attacker who hijacks the DNS or the
   routing path cannot forge a valid endpoint without also compromising a
   trusted certificate authority.

2. **Trust inside the platform.** The proxy-to-container hop is not
   encrypted. This is acceptable when the platform's internal network is
   trusted, which is the case for every major container platform. It is
   not acceptable if the container is reachable directly from the public
   internet on its plain port; in that case, the deployment must bind
   `0.0.0.0:8080` to a private network or firewall the port at the
   platform's edge.

3. **`--insecure` mode.** The client accepts `--insecure` to disable
   certificate verification. This is intended only for development
   servers with self-signed certificates. When `--insecure` is passed,
   the client is vulnerable to any on-path attacker: the connection is
   still encrypted, but the peer is not authenticated. Documented here so
   that the flag is never mistaken for a safe default.

The `port_reuse` and `session_hijack` scenarios run over the same plain
WebSocket connection they would run over in a local deployment. TLS is an
orthogonal transport concern; the server's state machine and cleanup
guarantees are independent of whether the connection is encrypted.


## The adversarial scenarios

The `hacker` crate runs three probes. Each one targets a specific defense.

### `session_hijack`

Four probes against a live server:

1. `MakeMove` without being in a match must be rejected with `NotInMatch`.
2. `JoinMatch` on a non-existent identifier must be rejected with
   `MatchNotFound`.
3. A second `Hello` on the same connection must be rejected with
   `InvalidState`.
4. A malformed JSON payload must be ignored without closing the connection.

Every probe is checked against the server's actual response. Any probe that
does not produce the expected error is reported as `COMPROMISED`.

### `port_reuse`

Two probes:

1. The hacker tries to bind the server's port. The kernel must refuse with
   `EADDRINUSE`, proving that the server's `TcpListener` still owns it.
2. The hacker binds an ephemeral port, drops the listener, and immediately
   rebinds the same port. The rebind must succeed, proving that Rust
   released the port synchronously on `drop`.

The first probe demonstrates that a running server is safe from port theft.
The second demonstrates that a dropped listener does not linger.

### `flood`

32 concurrent WebSocket connections are opened, each sends `Hello`, waits
for `Welcome`, and closes abruptly. The hacker then opens a fresh connection
and sends `Hello`. The server must respond with `Welcome`. If the server
still has any session leaks from the flood, the fresh `Hello` may fail or
the server may refuse to accept the new connection.

## What this project does not claim

The demo does not prove that Rust is immune to all security bugs. It proves
that a specific class of resource-lifetime bugs is eliminated by language
design, and that the elimination is observable and reproducible. Logic
errors, protocol mistakes, and business-rule bugs are still possible and
still need tests.

## Threat model

The threat model is deliberately narrow:

- The attacker is a network client with the same access to the server as a
  legitimate client.
- The attacker does not have OS-level access to the server host.
- The attacker cannot modify the server binary.

Under this model, the defenses the hacker scenario verifies are:

- Rejection of illegal state transitions.
- Rejection of references to resources the attacker does not own.
- Deterministic release of server-side resources when a client disconnects.

If the threat model expands (attacker has host access, attacker can race the
scheduler, etc.), additional defenses are required and are out of scope for
this project.