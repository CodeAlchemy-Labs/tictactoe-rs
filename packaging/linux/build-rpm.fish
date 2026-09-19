#!/usr/bin/env fish
set -e

echo "Cleaning old .rpm artefacts..."
mkdir -p dist/linux
rm -f dist/linux/tictacli*.rpm

echo "Building RPM packages via Docker..."
docker run --rm \
    -v (pwd):/work \
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
for file in dist/linux/tictacli*.rpm
    echo "Checking $file"
    docker run --rm -v (pwd)/dist/linux:/pkg rockylinux:8 rpm -qip /pkg/(basename $file)
    sha256sum $file
end

echo "Done building RPM packages."
