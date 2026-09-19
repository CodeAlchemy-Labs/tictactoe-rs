#!/usr/bin/env fish
set -e

# Resolve repo root from the script location so this works regardless of cwd.
set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT

set DIST_DIR $REPO_ROOT/dist/linux
set VERSION "0.2.0"

echo "Cleaning old .rpm artefacts..."
mkdir -p $DIST_DIR
rm -f $DIST_DIR/tictacli*.rpm

# Source cargo env if it exists (container may have installed Rust via rustup)
if test -f "$HOME/.cargo/env"
    bash -c "source $HOME/.cargo/env && echo \$PATH" | read -l CARGO_PATH
    set -x PATH $CARGO_PATH
end

echo "Building release binaries..."
cargo build --release --locked --bin tictacli --bin server

echo "Building client .rpm with cargo-generate-rpm..."
cargo generate-rpm -p client -o $DIST_DIR/

echo "Building server .rpm with cargo-generate-rpm..."
cargo generate-rpm -p server -o $DIST_DIR/

echo "Building metapackage tictacli-full..."
set SPEC (mktemp /tmp/tictacli-full-XXXXXX.spec)
trap "rm -f $SPEC" EXIT
printf 'Name: tictacli-full\nVersion: %s\nRelease: 1\nSummary: TicTacToe full metapackage\nLicense: MIT\nRequires: tictacli, tictacli-server\nBuildArch: noarch\n%%description\nInstalls both the TicTacToe client and server.\n%%files\n' $VERSION > $SPEC
rpmbuild -bb \
    --define "_rpmdir $DIST_DIR" \
    --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
    $SPEC
# rpmbuild puts noarch packages in a noarch subdir; flatten it.
if test -d $DIST_DIR/noarch
    mv $DIST_DIR/noarch/*.rpm $DIST_DIR/
    rmdir $DIST_DIR/noarch
end

echo "Verifying .rpm files..."
for file in $DIST_DIR/tictacli*.rpm
    echo "--- $file ---"
    rpm -qip $file
    sha256sum $file
end

echo "Done building RPM packages."
for file in $DIST_DIR/tictacli*.rpm
    echo (realpath $file)
end
