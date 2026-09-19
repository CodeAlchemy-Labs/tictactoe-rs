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
This directory focuses on Stage 4 of the deployment strategy (native distros).
AppImage support (Stage 5) and fully automated CI publication (Stage 7) will be added later. Currently, builds are executed on demand through a manually triggered GitHub Actions workflow (`package-linux.yml`) or locally via `make package-linux`.
