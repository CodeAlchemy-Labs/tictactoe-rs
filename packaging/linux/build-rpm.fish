#!/usr/bin/env fish
set -e

set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT

set DIST_DIR $REPO_ROOT/dist/linux

echo "Cleaning old .rpm artefacts..."
mkdir -p $DIST_DIR
rm -f $DIST_DIR/tictacli*.rpm

echo "Building RPM packages via Docker..."
docker run --rm \
    -v $REPO_ROOT:/work \
    -w /work \
    -e CARGO_HOME=/work/.cargo-cache \
    rockylinux:8 \
    bash -c "set -euo pipefail && \
        dnf install -y gcc rpm-build curl && \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.80.0 && \
        . ~/.cargo/env && \
        cargo install cargo-generate-rpm --locked && \
        cargo build --release --locked --bin tictacli --bin tictacli-server && \
        cargo generate-rpm -p client -o dist/linux/ && \
        cargo generate-rpm -p server -o dist/linux/ && \
        cat << 'SPEC' > dist/linux/tictacli-full.spec
Name: tictacli-full
Version: 0.2.0
Release: 1
Summary: TicTacToe full metapackage
License: MIT
Requires: tictacli, tictacli-server
BuildArch: noarch
%description
Installs both the client and server.
%files
SPEC
        rpmbuild -bb --define \"_rpmdir /work/dist/linux/\" /work/dist/linux/tictacli-full.spec && \
        mv /work/dist/linux/noarch/*.rpm /work/dist/linux/tictacli-full-0.2.0-1.noarch.rpm && \
        rm -rf /work/dist/linux/noarch /work/dist/linux/tictacli-full.spec"

echo "Verifying generated .rpm files..."
for file in $DIST_DIR/tictacli*.rpm
    echo "Checking $file"
    docker run --rm -v $DIST_DIR:/pkg rockylinux:8 rpm -qip /pkg/(basename $file)
    sha256sum $file
end

echo "Done building RPM packages."
for file in $DIST_DIR/tictacli*.rpm
    echo (realpath $file)
end
