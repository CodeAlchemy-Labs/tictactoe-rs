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

# Copy manifests first so that the dependency graph is cached across source
# changes. The workspace uses a virtual manifest with members under
# `crates/*`, so we copy the root manifest and each member manifest before
# the source.
COPY Cargo.toml Cargo.lock ./
COPY crates/common/Cargo.toml ./crates/common/Cargo.toml
COPY crates/server/Cargo.toml ./crates/server/Cargo.toml
COPY crates/client/Cargo.toml ./crates/client/Cargo.toml
COPY crates/hacker/Cargo.toml ./crates/hacker/Cargo.toml

# Compile the dependency graph once. This layer is reused when only source
# files change.
RUN mkdir -p crates/common/src crates/server/src crates/client/src crates/hacker/src \
    && echo 'fn main() {}' > crates/common/src/lib.rs \
    && echo 'fn main() {}' > crates/server/src/main.rs \
    && echo 'fn main() {}' > crates/client/src/main.rs \
    && echo 'fn main() {}' > crates/hacker/src/main.rs \
    && cargo build --release --workspace --bins \
    && rm -rf crates

# Now copy the real sources and build.
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