#!/usr/bin/env fish
set -e

echo "Cleaning old .deb artefacts..."
mkdir -p dist/linux
rm -f dist/linux/tictacli*.deb

echo "Building Debian packages via Docker..."
docker run --rm \
    -v (pwd):/work \
    -w /work \
    -e CARGO_HOME=/work/.cargo-cache \
    debian:bullseye-slim \
    bash -c "set -euo pipefail && \
        apt-get update && \
        apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates curl dpkg-dev && \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.80.0 && \
        . ~/.cargo/env && \
        cargo install cargo-deb --locked && \
        cargo build --release --locked --bin tictacli --bin tictacli-server && \
        cargo deb -p client --no-build --output dist/linux/ && \
        cargo deb -p server --no-build --output dist/linux/ && \
        mkdir -p dist/linux/tictacli-full-meta/DEBIAN && \
        echo 'Package: tictacli-full' > dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo 'Version: 0.2.0' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo 'Architecture: all' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo 'Maintainer: CodeAlchemy-Labs <maintainers@codealchemy-labs.example>' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo 'Depends: tictacli, tictacli-server' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo 'Section: metapackages' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo 'Priority: optional' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo 'Description: TicTacToe full metapackage' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        echo ' Installs both the client and server.' >> dist/linux/tictacli-full-meta/DEBIAN/control && \
        dpkg-deb --build dist/linux/tictacli-full-meta dist/linux/tictacli-full_0.2.0_all.deb && \
        rm -rf dist/linux/tictacli-full-meta"

echo "Verifying generated .deb files..."
for file in dist/linux/tictacli*.deb
    echo "Checking $file"
    docker run --rm -v (pwd)/dist/linux:/pkg debian:bullseye-slim dpkg-deb --info /pkg/(basename $file)
    sha256sum $file
end

echo "Done building Debian packages."
