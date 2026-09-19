#!/usr/bin/env fish
# Fish stops on errors by default; explicit error-exit is via: exit

# Resolve repo root from the script location so this works regardless of cwd.
set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT

set DIST_DIR $REPO_ROOT/dist/linux
set VERSION "0.2.0"

echo "Cleaning old .deb artefacts..."
mkdir -p $DIST_DIR
rm -f $DIST_DIR/tictacli*.deb

# Source cargo env if it exists (container may have installed Rust via rustup)
if test -f "$HOME/.cargo/env"
    bash -c "source $HOME/.cargo/env && echo \$PATH" | read -l CARGO_PATH
    set -x PATH $CARGO_PATH
end

echo "Building release binaries..."
cargo build --release --locked --bin tictacli --bin server

echo "Building client .deb with cargo-deb..."
cargo deb -p client --no-build --output $DIST_DIR/

echo "Building server .deb with cargo-deb..."
cargo deb -p server --no-build --output $DIST_DIR/

echo "Building metapackage tictacli-full..."
set META (mktemp -d)
trap "rm -rf $META" EXIT
mkdir -p $META/DEBIAN
printf 'Package: tictacli-full\nVersion: %s\nArchitecture: all\nMaintainer: CodeAlchemy-Labs <maintainers@codealchemy-labs.example>\nDepends: tictacli, tictacli-server\nSection: metapackages\nPriority: optional\nDescription: TicTacToe full metapackage\n Installs both the client and server.\n' $VERSION > $META/DEBIAN/control
dpkg-deb --build $META $DIST_DIR/tictacli-full_"$VERSION"_all.deb

echo "Verifying .deb files..."
for file in $DIST_DIR/tictacli*.deb
    echo "--- $file ---"
    dpkg-deb --info $file
    sha256sum $file
end

echo "Done building Debian packages."
for file in $DIST_DIR/tictacli*.deb
    echo (realpath $file)
end
