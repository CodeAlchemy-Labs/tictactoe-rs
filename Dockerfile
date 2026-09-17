# syntax=docker/dockerfile:1.7

# ---------------------------------------------------------------------------
# Build stage: compile the workspace in release mode.
# ---------------------------------------------------------------------------
FROM rust:1.97-slim AS builder

WORKDIR /build

# Minimal system dependencies. The workspace has no native-linking
# requirement beyond what the Rust toolchain already provides, so we keep
# this layer as small as possible.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy the workspace manifest, the lockfile, and every source file. The
# whole tree is copied in one step so that Cargo sees consistent mtimes and
# rebuilds every crate that has changed. The dependency-graph caching trick
# (stub sources + later copy) is deliberately avoided: it silently reuses
# stale rlibs when Docker's COPY preserves the host's older mtimes.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release --workspace --bins

# ---------------------------------------------------------------------------
# Runtime stage: minimal Debian image with the three binaries.
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash --uid 1000 app

COPY --from=builder /build/target/release/server /usr/local/bin/server
COPY --from=builder /build/target/release/client /usr/local/bin/client
COPY --from=builder /build/target/release/hacker /usr/local/bin/hacker

USER app
WORKDIR /home/app

# The compose file selects which binary runs in each service by overriding
# `command`. Without an override the server runs by default.
CMD ["server"]