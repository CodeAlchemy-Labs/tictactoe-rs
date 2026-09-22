#!/usr/bin/env bash
# Builds the macOS aarch64 portable client.
#
# The script must run on an Apple Silicon macOS host. It does not cross-compile.
# The binary is ad-hoc signed, which is mandatory on Apple Silicon. Notarization
# requires an Apple Developer account and is not performed here.

set -euo pipefail

# --- Resolve repository root (two levels up from this script) ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

# --- Read version from Cargo.toml ---
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "Invalid version in Cargo.toml: '$VERSION'" >&2
  exit 1
fi

TARGET="aarch64-apple-darwin"
DIST_DIR="$REPO_ROOT/dist/macos"
STAGING_ROOT="$DIST_DIR/staging"
PKG_DIR="$STAGING_ROOT/tictacli-$VERSION-$TARGET"
TARBALL="$DIST_DIR/tictacli-$VERSION-$TARGET.tar.gz"

rm -rf "$STAGING_ROOT"
mkdir -p "$PKG_DIR"

# --- Build ---
echo "Building $TARGET"
rustup target add "$TARGET"
cargo build --release --locked --target "$TARGET" --bin tictacli

BINARY="$REPO_ROOT/target/$TARGET/release/tictacli"
if [[ ! -f "$BINARY" ]]; then
  echo "Binary not found at $BINARY" >&2
  exit 1
fi

# --- Ad-hoc sign (mandatory on Apple Silicon) ---
echo "Signing the binary (ad-hoc)"
codesign --force --sign - "$BINARY"
codesign --verify --verbose "$BINARY"

# --- Assemble staging directory ---
cp "$BINARY" "$PKG_DIR/tictacli"
cp "$REPO_ROOT/packaging/macos/portable/README.txt" "$PKG_DIR/README.txt"
cp "$REPO_ROOT/packaging/common/config.example.toml" "$PKG_DIR/config.example.toml"
cp "$REPO_ROOT/LICENSE" "$PKG_DIR/LICENSE"
chmod +x "$PKG_DIR/tictacli"

# --- Create the tarball (folder-wrapped) ---
tar -czf "$TARBALL" -C "$STAGING_ROOT" "tictacli-$VERSION-$TARGET"

# --- Clean staging ---
rm -rf "$STAGING_ROOT"

# --- Report ---
echo "Artefact:"
ls -la "$TARBALL"
shasum -a 256 "$TARBALL"
