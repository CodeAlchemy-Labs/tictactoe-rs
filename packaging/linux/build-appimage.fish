#!/usr/bin/env fish

set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT || exit 1

set VERSION (string match -r '^version\s*=\s*"(.+)"' < Cargo.toml | tail -n1)
if not string match -r -q '^\d+\.\d+\.\d+' "$VERSION"
    echo "Invalid or missing version in Cargo.toml: '$VERSION'" >&2
    exit 1
end

set DIST_DIR $REPO_ROOT/dist/linux
mkdir -p $DIST_DIR || exit 1
find $DIST_DIR -name "tictacli*.AppImage" -delete || exit 1

echo "Building AppImages via Docker..."

# Run inside Docker
docker run --pull always --rm -v "$REPO_ROOT:/work" -w /work debian:bullseye-slim bash -c "
set -e
echo 'deb http://archive.debian.org/debian/ bullseye main' > /etc/apt/sources.list
apt-get update -o Acquire::Check-Valid-Until=false
apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates curl file wget fuse squashfs-tools

echo 'Installing Rust...'
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97.0 --profile minimal
source \$HOME/.cargo/env

echo 'Downloading linuxdeploy...'
wget --quiet --show-progress https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

echo 'Building release binaries...'
cargo build --release --locked --bin tictacli --bin tictacli-server

echo 'Packaging client AppImage...'
APPDIR=AppDir-client
mkdir -p \$APPDIR/usr/bin \$APPDIR/usr/share/applications \$APPDIR/usr/share/icons/hicolor/256x256/apps
cp target/release/tictacli \$APPDIR/usr/bin/
cp packaging/linux/appimage/tictacli.desktop \$APPDIR/usr/share/applications/
cp packaging/linux/common/tictacli.png \$APPDIR/usr/share/icons/hicolor/256x256/apps/tictacli.png
VERSION=$VERSION ./linuxdeploy-x86_64.AppImage --appdir \$APPDIR --executable \$APPDIR/usr/bin/tictacli --desktop-file \$APPDIR/usr/share/applications/tictacli.desktop --icon-file \$APPDIR/usr/share/icons/hicolor/256x256/apps/tictacli.png --output appimage

echo 'Packaging server AppImage...'
APPDIR_SERVER=AppDir-server
mkdir -p \$APPDIR_SERVER/usr/bin \$APPDIR_SERVER/usr/share/applications \$APPDIR_SERVER/usr/share/icons/hicolor/256x256/apps
cp target/release/tictacli-server \$APPDIR_SERVER/usr/bin/
cp packaging/linux/appimage/tictacli-server.desktop \$APPDIR_SERVER/usr/share/applications/
cp packaging/linux/common/tictacli.png \$APPDIR_SERVER/usr/share/icons/hicolor/256x256/apps/tictacli-server.png
VERSION=$VERSION ./linuxdeploy-x86_64.AppImage --appdir \$APPDIR_SERVER --executable \$APPDIR_SERVER/usr/bin/tictacli-server --desktop-file \$APPDIR_SERVER/usr/share/applications/tictacli-server.desktop --icon-file \$APPDIR_SERVER/usr/share/icons/hicolor/256x256/apps/tictacli-server.png --output appimage

mv TicTacToe_Client-$VERSION-x86_64.AppImage dist/linux/tictacli-$VERSION-x86_64.AppImage
mv TicTacToe_Server-$VERSION-x86_64.AppImage dist/linux/tictacli-server-$VERSION-x86_64.AppImage
" || exit 1

echo "Verifying AppImages..."
for app in $DIST_DIR/tictacli*.AppImage
    if not test -e $app
        echo "File not found: $app" >&2
        exit 1
    end
    chmod +x $app
    $app --appimage-version || exit 1
    sha256sum $app || exit 1
end

echo "Done building AppImages."
for file in $DIST_DIR/tictacli*.AppImage
    echo (realpath $file)
end
