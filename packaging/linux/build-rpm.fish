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

echo "Cleaning old .rpm artefacts..."
mkdir -p $DIST_DIR || exit 1
find $DIST_DIR -name "tictacli*.rpm" -delete || exit 1

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

echo "Building client .rpm with cargo-generate-rpm..."
cd crates/client || exit 1
if not cargo generate-rpm -o $DIST_DIR/
    echo "cargo generate-rpm failed for client" >&2
    exit 1
end
cd ../.. || exit 1

echo "Building server .rpm with cargo-generate-rpm..."
cd crates/server || exit 1
if not cargo generate-rpm -o $DIST_DIR/
    echo "cargo generate-rpm failed for server" >&2
    exit 1
end
cd ../.. || exit 1

echo "Building metapackage tictacli-full..."
set SPEC (mktemp /tmp/tictacli-full-XXXXXX.spec)
printf 'Name: tictacli-full\nVersion: %s\nRelease: 1\nSummary: TicTacToe full metapackage\nLicense: MIT\nRequires: tictacli, tictacli-server\nBuildArch: noarch\n%%description\nInstalls both the TicTacToe client and server.\n%%files\n' $VERSION > $SPEC || exit 1
if not rpmbuild -bb \
    --define "_rpmdir $DIST_DIR" \
    --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
    $SPEC
    echo "rpmbuild failed for metapackage" >&2
    rm -f $SPEC
    exit 1
end
rm -f $SPEC

# rpmbuild puts noarch packages in a noarch subdir; flatten it.
if test -d $DIST_DIR/noarch
    mv $DIST_DIR/noarch/*.rpm $DIST_DIR/ || exit 1
    rmdir $DIST_DIR/noarch || exit 1
end

echo "Verifying .rpm files..."
for file in $DIST_DIR/tictacli*.rpm
    echo "--- $file ---"
    rpm -qip $file || exit 1
    sha256sum $file || exit 1
end

echo "Done building RPM packages."
for file in $DIST_DIR/tictacli*.rpm
    echo (realpath $file)
end
