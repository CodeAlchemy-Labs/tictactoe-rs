#!/usr/bin/env fish

# Resolve repo root from the script location so this works regardless of cwd.
set REPO_ROOT (realpath (dirname (status filename))/../..)
cd $REPO_ROOT || exit 1

set DIST_DIR $REPO_ROOT/dist/linux
set ARCH_DIR $REPO_ROOT/packaging/linux/arch

set VERSION (string match -r '^version\s*=\s*"(.+)"' < Cargo.toml | tail -n1)
if not string match -r -q '^\d+\.\d+\.\d+' "$VERSION"
    echo "Invalid or missing version in Cargo.toml: '$VERSION'" >&2
    exit 1
end

echo "Cleaning old Arch artefacts..."
mkdir -p $DIST_DIR || exit 1
find $DIST_DIR -name "tictacli*.pkg.tar.zst" -delete

# Create a local source tarball from the repo so makepkg does not need
# network access. The PKGBUILD expects tictacli-$VERSION.tar.gz containing
# a top-level directory tictactoe-rs-$VERSION/.
set TARBALL (mktemp -d)
trap "rm -rf $TARBALL" EXIT

if command -q git; and test -d .git
    git archive --format=tar.gz --prefix=tictactoe-rs-$VERSION/ HEAD > $TARBALL/tictacli-$VERSION.tar.gz || exit 1
else
    mkdir -p $TARBALL/tictactoe-rs-$VERSION || exit 1
    for item in crates Cargo.toml Cargo.lock LICENSE README.md CHANGELOG.md packaging/linux/common
        if test -e $item
            cp -a --parents $item $TARBALL/tictactoe-rs-$VERSION/ || exit 1
        else
            echo "Missing required file/directory: $item" >&2
            exit 1
        end
    end
    tar -czf $TARBALL/tictacli-$VERSION.tar.gz -C $TARBALL tictactoe-rs-$VERSION || exit 1
end

if not test -f $TARBALL/tictacli-$VERSION.tar.gz
    echo "Expected tarball not found: $TARBALL/tictacli-$VERSION.tar.gz" >&2
    exit 1
end

# Build each package in an isolated scratch directory.
for PKG in tictacli tictacli-server
    echo ""
    echo "Building $PKG..."
    set BUILD_DIR (mktemp -d)

    # Inject the current version into the PKGBUILD before makepkg reads it.
    # `sed` rewrites only the `pkgver=` line; everything else is preserved.
    set PKGBUILD_SRC $ARCH_DIR/PKGBUILD-$PKG
    set PKGBUILD_DST $BUILD_DIR/PKGBUILD
    sed -e "s/^pkgver=.*/pkgver=$VERSION/" \
        -e "s/^pkgrel=.*/pkgrel=1/" \
        $PKGBUILD_SRC > $PKGBUILD_DST
    if not test -s $PKGBUILD_DST
        echo "Failed to generate PKGBUILD for $PKG" >&2
        exit 1
    end
    if not grep -q "^pkgver=$VERSION\$" $PKGBUILD_DST
        echo "PKGBUILD for $PKG does not contain pkgver=$VERSION" >&2
        echo "--- $PKGBUILD_DST ---" >&2
        cat $PKGBUILD_DST >&2
        exit 1
    end

    # Lay out everything makepkg needs inside the build dir.
    if test -f $ARCH_DIR/$PKG.install
        cp $ARCH_DIR/$PKG.install $BUILD_DIR/ || exit 1
    end
    cp $TARBALL/tictacli-$VERSION.tar.gz $BUILD_DIR/ || exit 1

    cd $BUILD_DIR || exit 1
    # --skipinteg: git archive changes with every commit, so its checksum
    # cannot be pinned in the PKGBUILD.
    if not makepkg --cleanbuild --force --skipinteg
        echo "makepkg failed for $PKG!" >&2
        exit 1
    end
    mv $BUILD_DIR/*.pkg.tar.zst $DIST_DIR/ || exit 1
    cd $REPO_ROOT || exit 1
    rm -rf $BUILD_DIR || exit 1
end

echo ""
echo "Running namcap..."
if command -v namcap >/dev/null
    for file in $DIST_DIR/tictacli*.pkg.tar.zst
        namcap $file || exit 1
    end
else
    echo "Warning: namcap not installed. Skipping checks."
end

echo ""
echo "Done building Arch packages."
for file in $DIST_DIR/tictacli*.pkg.tar.zst
    echo (realpath $file)
end
