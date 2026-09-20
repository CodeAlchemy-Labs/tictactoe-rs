param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot {
    if ($env:GITHUB_WORKSPACE -and (Test-Path (Join-Path $env:GITHUB_WORKSPACE "Cargo.toml"))) {
        return (Resolve-Path $env:GITHUB_WORKSPACE).Path
    }

    $candidate = Join-Path $PSScriptRoot "..\..\.."
    $resolved = Resolve-Path $candidate -ErrorAction SilentlyContinue
    if (-not $resolved) {
        Write-Error "Could not locate the repository root. Expected Cargo.toml above $PSScriptRoot"
        exit 1
    }

    if (-not (Test-Path (Join-Path $resolved.Path "Cargo.toml"))) {
        Write-Error "Could not locate the repository root. Expected Cargo.toml at $($resolved.Path)"
        exit 1
    }

    return $resolved.Path
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $repoRoot = Get-RepoRoot
    $cargoToml = Join-Path $repoRoot "Cargo.toml"
    $match = Select-String -Path $cargoToml -Pattern '^version = "(.+)"'
    if (-not $match) {
        throw "Could not determine version from Cargo.toml"
    }
    $Version = $match.Matches[0].Groups[1].Value
}

$repoRoot = Get-RepoRoot
$distDir = Join-Path $repoRoot "dist\windows\legacy"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

$targets = @(
    @{ Name = "x86_64"; Target = "x86_64-pc-windows-gnu"; Binary = "tictacli-x86_64-legacy.exe"; Setup = "tictacli-x86_64-legacy-setup.exe"; Msi = "tictacli-x86_64-legacy.msi" },
    @{ Name = "i686"; Target = "i686-pc-windows-gnu"; Binary = "tictacli-i686-legacy.exe"; Setup = "tictacli-i686-legacy-setup.exe"; Msi = "tictacli-i686-legacy.msi" }
)

foreach ($target in $targets) {
    $binaryPath = Join-Path $repoRoot ("target\{0}\release\tictacli.exe" -f $target.Target)
    Write-Host "Building legacy Windows binary for $($target.Name) ($($target.Target))"
    & cargo +1.77.2 build --release --locked --target $target.Target --no-default-features --features legacy-console --bin tictacli
    if ($LASTEXITCODE -ne 0) {
        throw "Legacy Windows build failed for $($target.Target)"
    }

    if (-not (Test-Path $binaryPath)) {
        throw "Legacy Windows binary was not produced at $binaryPath"
    }

    $signedBinary = Join-Path $distDir $target.Binary
    & pwsh -NoProfile -File (Join-Path $repoRoot "packaging\windows\sign.ps1") -InputFile $binaryPath -OutputFile $signedBinary
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to sign $($target.Binary)"
    }
}

Set-Location (Join-Path $repoRoot "packaging\windows\legacy")

foreach ($target in $targets) {
    $setupExe = Join-Path $distDir $target.Setup
    $tempSetup = Join-Path $env:TEMP ("legacy-setup-{0}.exe" -f $target.Name)
    $msiFile = Join-Path $distDir $target.Msi
    $tempMsi = Join-Path $env:TEMP ("legacy-msi-{0}.msi" -f $target.Name)

    Write-Host "Packaging legacy installer for $($target.Name)"
    & ISCC.exe "/DAppVersion=$Version" "/DOutputBaseName=$($target.Setup -replace '\.exe$', '')" "/DTargetArch=$($target.Target)" installer-legacy.iss
    if ($LASTEXITCODE -ne 0) {
        throw "Inno Setup build failed for $($target.Name)"
    }
    if (-not (Test-Path $setupExe)) {
        throw "Legacy installer was not produced at $setupExe"
    }

    & pwsh -NoProfile -File (Join-Path $repoRoot "packaging\windows\sign.ps1") -InputFile $setupExe -OutputFile $tempSetup
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to sign setup installer for $($target.Name)"
    }
    Move-Item -Path $tempSetup -Destination $setupExe -Force

    & wix build installer-legacy.wxs -d AppVersion=$Version -d TargetArch=$($target.Target) -o $msiFile
    if ($LASTEXITCODE -ne 0) {
        throw "WiX build failed for $($target.Name)"
    }
    if (-not (Test-Path $msiFile)) {
        throw "Legacy MSI was not produced at $msiFile"
    }

    & pwsh -NoProfile -File (Join-Path $repoRoot "packaging\windows\sign.ps1") -InputFile $msiFile -OutputFile $tempMsi
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to sign MSI installer for $($target.Name)"
    }
    Move-Item -Path $tempMsi -Destination $msiFile -Force
}

Write-Host "Legacy Windows artefact hashes:"
Get-ChildItem -Path $distDir -File | Where-Object { $_.Name -match 'legacy' -and ($_.Extension -in '.exe', '.msi') } |
    ForEach-Object { Get-FileHash $_.FullName -Algorithm SHA256 | Format-List }

Write-Host "Legacy Windows packaging complete."
