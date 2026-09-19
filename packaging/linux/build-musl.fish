#!/usr/bin/env fish
# Fish stops on errors by default; explicit error-exit is via: exit

# Resolve repo root from the script location so this works regardless of cwd.
set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT

set DIST_DIR $REPO_ROOT/dist/linux
set VERSION "0.2.0"
set TARGET "x86_64-unknown-linux-musl"

echo "Cleaning old musl artefacts..."
mkdir -p $DIST_DIR
rm -f $DIST_DIR/tictacli*-linux-musl-*.tar.gz

# The musl target may already be installed by the workflow action; add it
# idempotently so the script also works when run locally.
rustup target add $TARGET 2>/dev/null; or true

echo "Building musl statically linked binaries..."
cargo build --release --locked --target $TARGET --bin tictacli --bin server

# --- Package client ---
echo "Packaging client..."
set CLIENT_TMP (mktemp -d)
trap "rm -rf $CLIENT_TMP" EXIT
set CLIENT_PKG $CLIENT_TMP/tictacli-$VERSION
mkdir -p $CLIENT_PKG
cp target/$TARGET/release/tictacli            $CLIENT_PKG/
cp packaging/linux/common/tictacli.desktop    $CLIENT_PKG/
cp packaging/linux/common/tictacli.1          $CLIENT_PKG/
cp LICENSE                                     $CLIENT_PKG/
tar -czf $DIST_DIR/tictacli-$VERSION-linux-musl-x86_64.tar.gz \
    -C $CLIENT_TMP tictacli-$VERSION

# --- Package server ---
echo "Packaging server..."
set SERVER_TMP (mktemp -d)
trap "rm -rf $SERVER_TMP" EXIT
set SERVER_PKG $SERVER_TMP/tictacli-server-$VERSION
mkdir -p $SERVER_PKG
cp target/$TARGET/release/server                       $SERVER_PKG/tictacli-server
cp packaging/linux/common/tictacli-server.1            $SERVER_PKG/
cp packaging/linux/common/tictacli-server.service      $SERVER_PKG/
cp LICENSE                                              $SERVER_PKG/
tar -czf $DIST_DIR/tictacli-server-$VERSION-linux-musl-x86_64.tar.gz \
    -C $SERVER_TMP tictacli-server-$VERSION

echo "Verifying musl archives..."
sha256sum $DIST_DIR/tictacli*-linux-musl-*.tar.gz

echo "Done building musl packages."
for file in $DIST_DIR/tictacli*-linux-musl-*.tar.gz
    echo (realpath $file)
end
