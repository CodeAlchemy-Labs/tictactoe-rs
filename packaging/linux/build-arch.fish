#!/usr/bin/env fish
set -e

# Resolve repo root from the script location so this works regardless of cwd.
set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT

set DIST_DIR $REPO_ROOT/dist/linux
set ARCH_DIR $REPO_ROOT/packaging/linux/arch
set VERSION "0.2.0"

echo "Cleaning old Arch artefacts..."
mkdir -p $DIST_DIR
rm -f $DIST_DIR/tictacli*.pkg.tar.zst

# Create a local source tarball from the repo so makepkg does not need
# network access. The PKGBUILD expects tictacli-0.2.0.tar.gz containing
# a top-level directory tictactoe-rs-0.2.0/.
set TARBALL (mktemp -d)
trap "rm -rf $TARBALL" EXIT
git -C $REPO_ROOT archive --prefix=tictactoe-rs-$VERSION/ HEAD \
    | gzip -c > $TARBALL/tictacli-$VERSION.tar.gz

# Build each package in an isolated scratch directory.
for PKG in tictacli tictacli-server
    echo ""
    echo "Building $PKG..."
    set BUILD_DIR (mktemp -d)
    trap "rm -rf $BUILD_DIR" EXIT

    # Lay out everything makepkg needs inside the build dir.
    cp $ARCH_DIR/PKGBUILD-$PKG $BUILD_DIR/PKGBUILD
    cp $ARCH_DIR/$PKG.install $BUILD_DIR/ 2>/dev/null; or true
    cp $TARBALL/tictacli-$VERSION.tar.gz $BUILD_DIR/

    cd $BUILD_DIR
    # --skipinteg: the tarball was just created from HEAD, no checksum needed.
    # --nodeps: deps are already installed in the container by the workflow.
    makepkg --cleanbuild --force --skipinteg --nodeps
    mv $BUILD_DIR/*.pkg.tar.zst $DIST_DIR/
    cd $REPO_ROOT
end

echo ""
echo "Running namcap..."
if command -v namcap >/dev/null
    for file in $DIST_DIR/tictacli*.pkg.tar.zst
        namcap $file
    end
else
    echo "Warning: namcap not installed. Skipping checks."
end

echo ""
echo "Done building Arch packages."
for file in $DIST_DIR/tictacli*.pkg.tar.zst
    echo (realpath $file)
end
