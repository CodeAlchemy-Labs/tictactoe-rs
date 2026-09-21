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

  pwsh -File packaging/windows/legacy/build-legacy-x86_64.ps1
  pwsh -File packaging/windows/legacy/build-legacy-i686.ps1
  pwsh -File packaging/windows/legacy/build-legacy-server-x86_64.ps1
  pwsh -File packaging/windows/legacy/build-legacy-server-i686.ps1

Each script builds the corresponding binary, signs it, produces the
installer (client only) and the portable `.zip`, and writes everything
under `dist/windows/legacy/`.

## Portable artefacts

Each portable archive contains the executable under its versioned directory,
the relevant README, and the `LICENSE` file. Server archives also include
`sample.env`. Archives use the naming convention
`tictacli[-server]-<version>-<architecture>-legacy-portable.zip`.

## Installer parameterization

`installer-legacy.iss` and `installer-legacy.wxs` do not hard-code paths.
They receive `SourceBinary` and `OutputDir` (or the WiX equivalent) as
absolute paths from the PowerShell build script. This keeps the installer
scripts independent of the repository layout and ensures they package the
signed binary rather than the pre-signing artefact in `target/`.

If you invoke `ISCC.exe` or `wix build` manually, you must pass these
variables yourself:

```powershell
ISCC.exe /DAppVersion=0.2.0 `
         /DSourceBinary=C:\path\to\dist\windows\legacy\tictacli-x86_64-legacy.exe `
         /DOutputDir=C:\path\to\dist\windows\legacy `
         /DOutputBaseName=tictacli-x86_64-legacy-setup `
         /DTargetArch=x86_64-pc-windows-gnu `
         installer-legacy.iss
```

## Build host prerequisites

- Rust 1.77.2 with the appropriate target:
  - 64-bit: `rustup +1.77.2 target add x86_64-pc-windows-gnu`
  - 32-bit: `rustup +1.77.2 target add i686-pc-windows-gnu`
- MinGW-w64 toolchain for the corresponding architecture:
  - 64-bit: `x86_64-w64-mingw32-gcc`
  - 32-bit: `i686-w64-mingw32-gcc`
- On GitHub Actions, each job installs only the MinGW platform it needs via `egor-tensin/setup-mingw@v3`.
- On a local Windows machine, install MSYS2 and run:
  ```
  pacman -S mingw-w64-x86_64-gcc   # for the 64-bit build
  pacman -S mingw-w64-i686-gcc     # for the 32-bit build
  ```
  Add the corresponding `bin` directory to `PATH` before building. Do not add both at once; that is the conflict that produced the `invalid bfd target` error.

## Build commands

- 64-bit: `packaging\windows\legacy\build-legacy-x86_64.ps1`
- 32-bit: `packaging\windows\legacy\build-legacy-i686.ps1`

## Root resolution

The script resolves the repository root explicitly instead of relying on the caller's current working directory:

- In GitHub Actions, it prefers `$env:GITHUB_WORKSPACE` when that checked-out repository path is present.
- Locally, it walks up from `packaging/windows/legacy` to the repo root using the correct three-level path (`legacy -> windows -> packaging -> repo root`).
- If neither resolution succeeds, it exits with a clear error before any packaging work starts.

This keeps the script stable even when it is launched from a different working directory.

## Notes

The legacy build disables the modern underline-color rendering path so older `cmd.exe` consoles keep working without requiring modern terminal features.

The Stage 6 workflow keeps this artifact separate from the modern Windows 10/11 installer (`packaging/windows/`).

## Icon

The legacy installers use the same icon as the modern installers, from
`packaging/windows/tictacli.ico`. The path is passed to both Inno Setup
and WiX by the PowerShell build scripts as an absolute path.
