# syntax=docker/dockerfile:1.7

# ---------------------------------------------------------------------------
# Build stage: compile the workspace in release mode.
#
# Both stages pin Debian to `bookworm`. Building on a newer base (trixie)
# produces binaries that link against GLIBC symbols the older runtime base
# does not provide, which manifests at container start as
# "version `GLIBC_2.39' not found". Pinning both to bookworm keeps the two
# stages ABI-compatible.
# ---------------------------------------------------------------------------
FROM rust:1.97-slim-bookworm AS builder

WORKDIR /build

# Minimal system dependencies. The workspace has no native-linking
# requirement beyond what the Rust toolchain already provides.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy the workspace manifest, the lockfile, and every source file. The
# whole tree is copied in one step so that Cargo sees consistent mtimes and
# rebuilds every crate that has changed.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release --locked --workspace --bins

# ---------------------------------------------------------------------------
# Runtime stage: minimal Debian image with the three binaries.
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash --uid 1000 app

COPY --from=builder /build/target/release/tictacli-server /usr/local/bin/tictacli-server
COPY --from=builder /build/target/release/tictacli /usr/local/bin/tictacli
COPY --from=builder /build/target/release/hacker /usr/local/bin/hacker

USER app
WORKDIR /home/app

# The compose file selects which binary runs in each service by overriding
# `command`. Without an override the server runs by default.
CMD ["tictacli-server"]