#!/usr/bin/env fish

# Resolve repo root from the script location so this works regardless of cwd.
set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT || exit 1

set DIST_DIR $REPO_ROOT/dist/linux

set VERSION (string match -r '^version\s*=\s*"(.+)"' < Cargo.toml | tail -n1)
if not string match -r -q '^\d+\.\d+\.\d+' "$VERSION"
    echo "Invalid or missing version in Cargo.toml: '$VERSION'" >&2
    exit 1
end

echo "Cleaning old .deb artefacts..."
mkdir -p $DIST_DIR || exit 1
find $DIST_DIR -name "tictacli*.deb" -delete || exit 1

# Source cargo env if it exists
if test -f "$HOME/.cargo/env"
    bash -c "source $HOME/.cargo/env && echo \$PATH" | read -l CARGO_PATH
    set -x PATH $CARGO_PATH
end

echo "Building release binaries..."
if not cargo build --release --locked --bin tictacli --bin tictacli-server
    echo "cargo build failed" >&2
    exit 1
end

echo "Building client .deb with cargo-deb..."
if not cargo deb -p client --no-build --output $DIST_DIR/
    echo "cargo deb failed for client" >&2
    exit 1
end

echo "Building server .deb with cargo-deb..."
if not cargo deb -p server --no-build --output $DIST_DIR/
    echo "cargo deb failed for server" >&2
    exit 1
end

echo "Building metapackage tictacli-full..."
set META (mktemp -d)
# We shouldn't use trap EXIT here because it only cleans up META, and we have multiple traps if we use it inside loop, but here it's fine. 
# However, to avoid issues, I'll clean it explicitly.
mkdir -p $META/DEBIAN || exit 1
printf 'Package: tictacli-full\nVersion: %s\nArchitecture: all\nMaintainer: CodeAlchemy-Labs <maintainers@codealchemy-labs.example>\nDepends: tictacli, tictacli-server\nSection: metapackages\nPriority: optional\nDescription: TicTacToe full metapackage\n Installs both the client and server.\n' $VERSION > $META/DEBIAN/control || exit 1
if not dpkg-deb --build $META $DIST_DIR/tictacli-full_"$VERSION"_all.deb
    echo "dpkg-deb failed for metapackage" >&2
    rm -rf $META
    exit 1
end
rm -rf $META

echo "Verifying .deb files..."
for file in $DIST_DIR/tictacli*.deb
    echo "--- $file ---"
    dpkg-deb --info $file || exit 1
    sha256sum $file || exit 1
end

echo "Done building Debian packages."
for file in $DIST_DIR/tictacli*.deb
    echo (realpath $file)
end
