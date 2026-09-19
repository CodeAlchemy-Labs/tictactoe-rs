# Windows 10/11 Packaging

This directory contains the scripts and assets required to build the Windows 10 and 11 installers for the TicTacToe Client.

* Legacy Windows (7/8/8.1) is handled in Stage 6.
* Automated CI integration will be generalized in Stage 7.
* This folder is specifically for the modern `.exe` and `.msi` installers.

## Prerequisites (Windows)

To build the installers locally on a Windows machine, you need:

1. **Rust MSVC Toolchain**: `rustup default stable-x86_64-pc-windows-msvc`
2. **Visual Studio Build Tools**: Ensure the C++ build tools are installed.
3. **Inno Setup 6**: Available from [jrsoftware.org](https://jrsoftware.org/isdl.php) or via Chocolatey (`choco install innosetup`). Ensures `ISCC.exe` is on your PATH.
4. **WiX Toolset v4**: Install via .NET tools: `dotnet tool install --global wix`
5. **osslsigncode** or **signtool**: `osslsigncode` is preferred and can be installed via Chocolatey (`choco install osslsigncode`).

## Providing the Code-Signing Certificate

The scripts expect the certificate and its password to be provided via environment variables, to ensure secrets are never written to disk or accidentally committed.

In PowerShell, set them like this:

```powershell
$env:WINDOWS_CERTIFICATE_PFX_BASE64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes("path\to\codesign.pfx"))
$env:WINDOWS_CERTIFICATE_PASSWORD = "your_actual_password"
```

## Building

Run the orchestrator script to build the Rust binary, sign it, package both installers, and sign the installers:

```powershell
.\build-installers.ps1
```

The resulting artefacts will be placed in `../../dist/windows/`:
* `tictacli-<version>-setup.exe`
* `tictacli-<version>.msi`
* `tictacli.exe`

## Testing the Installers

For the most accurate test, use a clean Windows 10 or 11 Virtual Machine.

1. Copy the `.exe` installer to the VM.
2. Run it and complete the installation.
3. Launch the client from the Start Menu shortcut.
4. Verify the client opens.
5. Uninstall the application via "Add or remove programs".
6. Verify that `%LOCALAPPDATA%\Programs\TicTacToe Client` and `%PROGRAMFILES%\TicTacToe Client` are removed.
7. Repeat the process for the `.msi` installer.

## Verifying the Signatures

You can verify the signature on Windows using `Get-AuthenticodeSignature`:

```powershell
Get-AuthenticodeSignature -FilePath ..\..\dist\windows\tictacli-0.2.0-setup.exe | Format-List
```

Or using `osslsigncode` (which requires passing the self-signed certificate as the CA):

```powershell
osslsigncode verify -in ..\..\dist\windows\tictacli-0.2.0-setup.exe -CAfile ..\certs\codealchemy-labs.crt
```
