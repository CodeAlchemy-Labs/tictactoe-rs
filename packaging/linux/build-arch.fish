#!/usr/bin/env fish
set -e

echo "Cleaning old Arch artefacts..."
mkdir -p dist/linux
rm -f dist/linux/tictacli*.pkg.tar.zst

echo "Building Arch packages..."
# Copy to scratch dir to avoid modifying repo state
set SCRATCH (mktemp -d)
trap "rm -rf $SCRATCH" EXIT

cp -r . $SCRATCH/repo
cd $SCRATCH/repo/packaging/linux/arch

echo "Building tictacli..."
cp PKGBUILD-tictacli PKGBUILD
makepkg --cleanbuild --force
mv *.pkg.tar.zst ../../../../dist/linux/

echo "Building tictacli-server..."
cp PKGBUILD-tictacli-server PKGBUILD
makepkg --cleanbuild --force
mv *.pkg.tar.zst ../../../../dist/linux/

cd ../../../../
echo "Running namcap..."
if command -v namcap >/dev/null
    for file in dist/linux/tictacli*.pkg.tar.zst
        namcap $file
    end
else
    echo "Warning: namcap not installed. Skipping checks."
end

echo "Done building Arch packages."
