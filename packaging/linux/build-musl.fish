#!/usr/bin/env fish

# Resolve repo root from the script location so this works regardless of cwd.
set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT || exit 1

set DIST_DIR $REPO_ROOT/dist/linux
set TARGET "x86_64-unknown-linux-musl"

set VERSION (string match -r '^version\s*=\s*"(.+)"' < Cargo.toml | tail -n1)
if not string match -r -q '^\d+\.\d+\.\d+' "$VERSION"
    echo "Invalid or missing version in Cargo.toml: '$VERSION'" >&2
    exit 1
end

echo "Cleaning old musl artefacts..."
mkdir -p $DIST_DIR || exit 1
find $DIST_DIR -name "tictacli*-linux-musl-*.tar.gz" -delete || exit 1

if not rustup target list --installed | string match -q "*$TARGET*"
    rustup target add $TARGET
    or begin
        echo "Failed to add target $TARGET" >&2
        exit 1
    end
end

echo "Building musl statically linked binaries..."
if not cargo build --release --locked --target $TARGET --bin tictacli --bin tictacli-server
    echo "cargo build failed" >&2
    exit 1
end

set CLIENT_TMP ""
set SERVER_TMP ""

function cleanup
    if test -n "$CLIENT_TMP"; and test -d "$CLIENT_TMP"
        rm -rf $CLIENT_TMP
    end
    if test -n "$SERVER_TMP"; and test -d "$SERVER_TMP"
        rm -rf $SERVER_TMP
    end
end
trap cleanup EXIT

# --- Package client ---
echo "Packaging client..."
set CLIENT_TMP (mktemp -d)
set CLIENT_PKG $CLIENT_TMP/tictacli-$VERSION
mkdir -p $CLIENT_PKG || exit 1
cp target/$TARGET/release/tictacli            $CLIENT_PKG/ || exit 1
cp packaging/linux/common/tictacli.desktop    $CLIENT_PKG/ || exit 1
cp packaging/linux/common/tictacli.1          $CLIENT_PKG/ || exit 1
cp LICENSE                                     $CLIENT_PKG/ || exit 1
tar -czf $DIST_DIR/tictacli-$VERSION-linux-musl-x86_64.tar.gz \
    -C $CLIENT_TMP tictacli-$VERSION || exit 1

# --- Package server ---
echo "Packaging server..."
set SERVER_TMP (mktemp -d)
set SERVER_PKG $SERVER_TMP/tictacli-server-$VERSION
mkdir -p $SERVER_PKG || exit 1
cp target/$TARGET/release/tictacli-server              $SERVER_PKG/ || exit 1
cp packaging/linux/common/tictacli-server.1            $SERVER_PKG/ || exit 1
cp packaging/linux/common/tictacli-server.service      $SERVER_PKG/ || exit 1
cp LICENSE                                              $SERVER_PKG/ || exit 1
tar -czf $DIST_DIR/tictacli-server-$VERSION-linux-musl-x86_64.tar.gz \
    -C $SERVER_TMP tictacli-server-$VERSION || exit 1

echo "Verifying musl archives..."
sha256sum $DIST_DIR/tictacli*-linux-musl-*.tar.gz || exit 1

echo "Done building musl packages."
for file in $DIST_DIR/tictacli*-linux-musl-*.tar.gz
    echo (realpath $file)
end
