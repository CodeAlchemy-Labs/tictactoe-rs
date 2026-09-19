param(
    [string]$Version = "",
    [switch]$SkipSign,
    [switch]$SkipMsi
)

$ErrorActionPreference = "Stop"

try {
    if ([string]::IsNullOrWhiteSpace($Version)) {
        $cargoToml = Join-Path $PSScriptRoot "..\..\Cargo.toml"
        $match = Select-String -Path $cargoToml -Pattern '^version = "(.+)"'
        if ($match) {
            $Version = $match.Matches[0].Groups[1].Value
        } else {
            Write-Error "Could not determine version from Cargo.toml"
            exit 1
        }
    }

    Write-Host "Building for version: $Version"

    Set-Location (Join-Path $PSScriptRoot "..\..")

    Write-Host "Running cargo build..."
    & cargo build --release --locked --bin tictacli --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { Write-Error "Cargo build failed"; exit 1 }

    $targetExe = "target\x86_64-pc-windows-msvc\release\tictacli.exe"
    if (-not (Test-Path $targetExe)) {
        Write-Error "Could not find built executable at $targetExe"
        exit 1
    }

    $distDir = "dist\windows"
    if (-not (Test-Path $distDir)) {
        New-Item -ItemType Directory -Path $distDir | Out-Null
    }

    $clientExe = Join-Path $distDir "tictacli.exe"

    if (-not $SkipSign) {
        Write-Host "Signing binary..."
        & pwsh -NoProfile -File "packaging\windows\sign.ps1" -InputFile $targetExe -OutputFile $clientExe
        if ($LASTEXITCODE -ne 0) { Write-Error "Failed to sign binary"; exit 1 }
    } else {
        Copy-Item -Path $targetExe -Destination $clientExe -Force
    }

    Set-Location $PSScriptRoot

    Write-Host "Building Inno Setup installer..."
    if (-not (Get-Command "ISCC.exe" -ErrorAction SilentlyContinue)) {
        Write-Error "ISCC.exe not found on PATH"
        exit 1
    }
    & ISCC.exe "/DAppVersion=$Version" installer.iss
    if ($LASTEXITCODE -ne 0) { Write-Error "Inno Setup build failed"; exit 1 }

    $setupExe = "..\..\dist\windows\tictacli-$Version-setup.exe"
    $msiFile = "..\..\dist\windows\tictacli-$Version.msi"

    if (-not $SkipMsi) {
        Write-Host "Building WiX MSI..."
        if (-not (Get-Command "wix" -ErrorAction SilentlyContinue)) {
            Write-Error "wix not found on PATH"
            exit 1
        }
        & wix build installer.wxs -d AppVersion=$Version -o $msiFile
        if ($LASTEXITCODE -ne 0) { Write-Error "WiX build failed"; exit 1 }
    }

    if (-not $SkipSign) {
        Write-Host "Signing Inno Setup installer..."
        $tempExe = Join-Path $env:TEMP "setup-temp.exe"
        & pwsh -NoProfile -File "sign.ps1" -InputFile $setupExe -OutputFile $tempExe
        if ($LASTEXITCODE -ne 0) { Write-Error "Failed to sign Inno Setup installer"; exit 1 }
        Move-Item -Path $tempExe -Destination $setupExe -Force

        if (-not $SkipMsi) {
            Write-Host "Signing WiX MSI..."
            $tempMsi = Join-Path $env:TEMP "msi-temp.msi"
            & pwsh -NoProfile -File "sign.ps1" -InputFile $msiFile -OutputFile $tempMsi
            if ($LASTEXITCODE -ne 0) { Write-Error "Failed to sign WiX MSI"; exit 1 }
            Move-Item -Path $tempMsi -Destination $msiFile -Force
        }
    }

    Write-Host "Artefact Hashes:"
    Get-FileHash $setupExe -Algorithm SHA256 | Format-List
    if (-not $SkipMsi) {
        Get-FileHash $msiFile -Algorithm SHA256 | Format-List
    }

    Write-Host "Build complete!"
    exit 0
} catch {
    Write-Error $_
    exit 1
}
