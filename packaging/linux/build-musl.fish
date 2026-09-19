#!/usr/bin/env fish
set -e

echo "Cleaning old musl artefacts..."
mkdir -p dist/linux
rm -f dist/linux/tictacli*-linux-musl-*.tar.gz

echo "Building musl statically linked packages..."
rustup target add x86_64-unknown-linux-musl
cargo build --release --locked --target x86_64-unknown-linux-musl --bin tictacli --bin tictacli-server

set VERSION "0.2.0"

# Package client
set CLIENT_TMP (mktemp -d)
trap "rm -rf $CLIENT_TMP" EXIT
mkdir -p $CLIENT_TMP/tictacli-$VERSION
cp target/x86_64-unknown-linux-musl/release/tictacli $CLIENT_TMP/tictacli-$VERSION/
cp packaging/linux/common/tictacli.desktop $CLIENT_TMP/tictacli-$VERSION/
cp packaging/linux/common/tictacli.1 $CLIENT_TMP/tictacli-$VERSION/
cp LICENSE $CLIENT_TMP/tictacli-$VERSION/
tar -czf dist/linux/tictacli-$VERSION-linux-musl-x86_64.tar.gz -C $CLIENT_TMP tictacli-$VERSION

# Package server
set SERVER_TMP (mktemp -d)
trap "rm -rf $SERVER_TMP" EXIT
mkdir -p $SERVER_TMP/tictacli-server-$VERSION
cp target/x86_64-unknown-linux-musl/release/tictacli-server $SERVER_TMP/tictacli-server-$VERSION/
cp packaging/linux/common/tictacli-server.1 $SERVER_TMP/tictacli-server-$VERSION/
cp packaging/linux/common/tictacli-server.service $SERVER_TMP/tictacli-server-$VERSION/
cp LICENSE $SERVER_TMP/tictacli-server-$VERSION/
tar -czf dist/linux/tictacli-server-$VERSION-linux-musl-x86_64.tar.gz -C $SERVER_TMP tictacli-server-$VERSION

echo "Verifying generated musl files..."
sha256sum dist/linux/tictacli*-linux-musl-*.tar.gz

echo "Done building musl packages."
