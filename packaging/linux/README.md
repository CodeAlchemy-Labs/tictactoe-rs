# Linux Packaging

This directory contains the scripts and assets for producing Linux packaging artefacts for TicTacToe.

## Supported Distros and glibc Requirements
- `.deb`: Built on Debian 11 (bullseye). Targets glibc >= 2.31 (Debian 11+, Ubuntu 20.04+, Linux Mint 20+, Pop!_OS 20.04+).
- `.rpm`: Built on Rocky Linux 8. Targets glibc >= 2.28 (RHEL 8+, Fedora 30+, Rocky 8+, Alma 8+).
- Arch Linux (`.pkg.tar.zst`): Built natively on the host running the build script, ensuring linking with the rolling release glibc.
- Static `musl`: Built as an alternative with `x86_64-unknown-linux-musl`. Use this variant for maximum cross-distro compatibility, older environments, or chroot systems without glibc.

## Testing Packages in a Clean Environment

To test a generated `.deb` package using Docker:
```bash
docker run --rm -it -v $(pwd)/dist/linux:/pkg debian:stable-slim bash
apt install /pkg/tictacli_*.deb
```

To test a generated `.rpm` package:
```bash
docker run --rm -it -v $(pwd)/dist/linux:/pkg rockylinux:9 bash
dnf install /pkg/tictacli-*.rpm
```

To test a generated `.pkg.tar.zst` package:
```bash
docker run --rm -it -v $(pwd)/dist/linux:/pkg archlinux:latest bash
pacman -U /pkg/tictacli-*.pkg.tar.zst
```

## Context in the Delivery Pipeline
This directory focuses on Stage 4 of the deployment strategy (native distros) and Stage 5 (AppImage).
Fully automated CI publication (Stage 7) will be added later. Currently, builds are executed on demand through manually triggered GitHub Actions workflows (`package-linux.yml`, `package-portable.yml`) or locally via `make package-linux` / `make package-appimage`.

## AppImage (Portable)

The AppImage provides a self-contained, single-executable version of TicTacToe. It is ideal for users on immutable distributions (like Fedora Silverblue or NixOS), systems where root access is unavailable, or environments where the native `.deb` / `.rpm` packages cannot be used.

### Building Locally

You can build the AppImage locally using the provided script. It requires Docker and will build inside a `debian:bullseye-slim` container (glibc 2.31) to ensure broad compatibility.

```bash
fish packaging/linux/build-appimage.fish
```

### Testing in a Clean Environment

To verify that the AppImage is truly self-contained, you can test it inside a clean Docker container (e.g., `archlinux:latest` or `debian:stable-slim`) that lacks a Rust toolchain:

```bash
docker run --rm -it -v $(pwd)/dist/linux:/pkg archlinux:latest bash
# Inside the container:
chmod +x /pkg/tictacli-*.AppImage
/pkg/tictacli-*.AppImage --appimage-version
```

### Known Limitations

- **FUSE Requirement**: AppImages rely on FUSE (Filesystem in Userspace) to mount themselves. Older systems or certain container environments may lack `libfuse2`.
- **Fallback**: If FUSE is unavailable, you can extract and run the contents directly:
  ```bash
  ./tictacli-*.AppImage --appimage-extract-and-run
  ```
- **Terminal Execution**: The TicTacToe server AppImage is a console application; it must be run from a terminal to view its logs.
