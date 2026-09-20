# Legacy Windows Packaging

This folder contains the compatibility artifacts for the Stage 6 legacy Windows target.

## Scope

- Windows 7 / 8 / 8.1 compatibility build
- Rust MSRV: 1.77
- Target: `x86_64-pc-windows-gnu`
- Feature set: `--no-default-features --features legacy-console`
- Output: a portable `tictacli.exe` zip archive, not the modern installer flow

## Build locally

From the repository root:

```powershell
cargo +1.77.2 build --release --target x86_64-pc-windows-gnu --no-default-features --features legacy-console --bin tictacli
pwsh -File packaging/windows/legacy/build-legacy.ps1
```

The script writes the archive to `dist/windows/tictacli-<version>-legacy-win7.zip`.

## Notes

The legacy build disables the modern underline-color rendering path so older `cmd.exe` consoles keep working without requiring modern terminal features.

The Stage 6 workflow keeps this artifact separate from the modern Windows 10/11 installer (`packaging/windows/`).
