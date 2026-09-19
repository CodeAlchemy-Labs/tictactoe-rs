#!/usr/bin/env fish
set -e

set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT

set DIST_DIR $REPO_ROOT/dist/linux

echo "Cleaning old Arch artefacts..."
mkdir -p $DIST_DIR
rm -f $DIST_DIR/tictacli*.pkg.tar.zst

echo "Building Arch packages..."
# Copy to scratch dir to avoid modifying repo state
set SCRATCH (mktemp -d)
trap "rm -rf $SCRATCH" EXIT

cp -r . $SCRATCH/repo
cd $SCRATCH/repo/packaging/linux/arch

echo "Building tictacli..."
cp PKGBUILD-tictacli PKGBUILD
makepkg --cleanbuild --force
mv *.pkg.tar.zst $DIST_DIR/

echo "Building tictacli-server..."
cp PKGBUILD-tictacli-server PKGBUILD
makepkg --cleanbuild --force
mv *.pkg.tar.zst $DIST_DIR/

cd $REPO_ROOT
echo "Running namcap..."
if command -v namcap >/dev/null
    for file in $DIST_DIR/tictacli*.pkg.tar.zst
        namcap $file
    end
else
    echo "Warning: namcap not installed. Skipping checks."
end

echo "Done building Arch packages."
for file in $DIST_DIR/tictacli*.pkg.tar.zst
    echo (realpath $file)
end
