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
    if (-not $resolved -or -not (Test-Path (Join-Path $resolved.Path "Cargo.toml"))) {
        throw "Could not locate the repository root. Expected Cargo.toml above $PSScriptRoot"
    }
    return $resolved.Path
}

$repoRoot = Get-RepoRoot
$distDir = Join-Path $repoRoot "dist\windows\legacy"
$iconFile = Join-Path $repoRoot "packaging\windows\tictacli.ico"
if (-not (Test-Path $iconFile)) { throw "Icon not found at $iconFile" }
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargoToml = Join-Path $repoRoot "Cargo.toml"
    $match = Select-String -Path $cargoToml -Pattern '^version = "(.+)"'
    if (-not $match) { throw "Could not determine version from Cargo.toml" }
    $Version = $match.Matches[0].Groups[1].Value
}

$target = "i686-pc-windows-gnu"
Write-Host "Building legacy Windows binary for $target"
& cargo +1.77.2 build --release --locked --target $target --no-default-features --features legacy-console --bin tictacli
if ($LASTEXITCODE -ne 0) { throw "Legacy Windows build failed for $target" }

$binaryPath = Join-Path $repoRoot "target\$target\release\tictacli.exe"
if (-not (Test-Path $binaryPath)) { throw "Binary not found at $binaryPath" }

$signedBinary = Join-Path $distDir "tictacli-i686-legacy.exe"
& pwsh -NoProfile -File (Join-Path $repoRoot "packaging\windows\sign.ps1") -InputFile $binaryPath -OutputFile $signedBinary
if ($LASTEXITCODE -ne 0) { throw "Failed to sign i686 legacy binary" }

Set-Location (Join-Path $repoRoot "packaging\windows\legacy")
$setupExe = Join-Path $distDir "tictacli-i686-legacy-setup.exe"
$setupBaseName = "tictacli-i686-legacy-setup"
& ISCC.exe `
    "/DAppVersion=$Version" `
    "/DSourceBinary=$signedBinary" `
    "/DOutputDir=$distDir" `
    "/DIconFile=$iconFile" `
    "/DOutputBaseName=$setupBaseName" `
    "/DTargetArch=$target" `
    "installer-legacy.iss"
if ($LASTEXITCODE -ne 0) { throw "Inno Setup build failed for i686" }
if (-not (Test-Path $setupExe)) { throw "Setup installer not found at $setupExe" }

$tempSetup = Join-Path $env:TEMP "legacy-setup-i686.exe"
& pwsh -NoProfile -File (Join-Path $repoRoot "packaging\windows\sign.ps1") -InputFile $setupExe -OutputFile $tempSetup
if ($LASTEXITCODE -ne 0) { throw "Failed to sign i686 setup installer" }
Move-Item -Path $tempSetup -Destination $setupExe -Force

$msiFile = Join-Path $distDir "tictacli-i686-legacy.msi"
& wix build installer-legacy.wxs -d SourceBinary=$signedBinary -d AppVersion=$Version -d TargetArch=$target -d IconFile=$iconFile -o $msiFile
if ($LASTEXITCODE -ne 0) { throw "WiX build failed for i686" }
if (-not (Test-Path $msiFile)) { throw "MSI not found at $msiFile" }

$tempMsi = Join-Path $env:TEMP "legacy-msi-i686.msi"
& pwsh -NoProfile -File (Join-Path $repoRoot "packaging\windows\sign.ps1") -InputFile $msiFile -OutputFile $tempMsi
if ($LASTEXITCODE -ne 0) { throw "Failed to sign i686 MSI" }
Move-Item -Path $tempMsi -Destination $msiFile -Force

$portableRoot = Join-Path $repoRoot "dist\windows\legacy\staging-client-i686"
$portableDir = Join-Path $portableRoot "tictacli-$Version-i686-legacy"
if (Test-Path $portableRoot) { Remove-Item -Path $portableRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $portableDir | Out-Null
Copy-Item -Path $signedBinary -Destination (Join-Path $portableDir "tictacli.exe")
Copy-Item -Path (Join-Path $repoRoot "packaging\windows\legacy\README-client.txt") -Destination (Join-Path $portableDir "README.txt")
Copy-Item -Path (Join-Path $repoRoot "LICENSE") -Destination (Join-Path $portableDir "LICENSE")
$portableZip = Join-Path $distDir "tictacli-$Version-i686-legacy-portable.zip"
Compress-Archive -Path "$portableDir\*" -DestinationPath $portableZip -Force
Remove-Item -Path $portableRoot -Recurse -Force

Write-Host "i686 legacy artefact hashes:"
Get-ChildItem -Path $distDir -File |
    Where-Object { $_.Name -match 'i686' -and ($_.Extension -in '.exe', '.msi') } |
    ForEach-Object { Get-FileHash $_.FullName -Algorithm SHA256 | Format-List }

Write-Host "i686 legacy packaging complete."
