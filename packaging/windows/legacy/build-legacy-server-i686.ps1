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
$serverDir = Join-Path $distDir "server"
New-Item -ItemType Directory -Force -Path $serverDir | Out-Null

if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargoToml = Join-Path $repoRoot "Cargo.toml"
    $match = Select-String -Path $cargoToml -Pattern '^version = "(.+)"'
    if (-not $match) { throw "Could not determine version from Cargo.toml" }
    $Version = $match.Matches[0].Groups[1].Value
}

$target = "i686-pc-windows-gnu"
Write-Host "Building legacy Windows server for $target"
& cargo +1.77.2 build --release --locked --target $target --bin tictacli-server
if ($LASTEXITCODE -ne 0) { throw "Legacy Windows server build failed for $target" }

$binaryPath = Join-Path $repoRoot "target\$target\release\tictacli-server.exe"
if (-not (Test-Path $binaryPath)) { throw "Binary not found at $binaryPath" }

$unsignedBinary = Join-Path $serverDir "tictacli-server-$target-legacy-unsigned.exe"
$signedBinary = Join-Path $serverDir "tictacli-server-i686-legacy.exe"
Copy-Item -Path $binaryPath -Destination $unsignedBinary -Force
& pwsh -NoProfile -File (Join-Path $repoRoot "packaging\windows\sign.ps1") -InputFile $unsignedBinary -OutputFile $signedBinary
if ($LASTEXITCODE -ne 0) { throw "Failed to sign i686 legacy server binary" }
Remove-Item -Path $unsignedBinary -Force

$portableRoot = Join-Path $distDir "staging-server-i686"
$portableDir = Join-Path $portableRoot "tictacli-server-$Version-i686-legacy"
if (Test-Path $portableRoot) { Remove-Item -Path $portableRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $portableDir | Out-Null
Copy-Item -Path $signedBinary -Destination (Join-Path $portableDir "tictacli-server.exe")
Copy-Item -Path (Join-Path $repoRoot "packaging\windows\portable\README.txt") -Destination (Join-Path $portableDir "README.txt")
Copy-Item -Path (Join-Path $repoRoot "packaging\windows\portable\sample.env") -Destination (Join-Path $portableDir "sample.env")
Copy-Item -Path (Join-Path $repoRoot "LICENSE") -Destination (Join-Path $portableDir "LICENSE")

$portableZip = Join-Path $distDir "tictacli-server-$Version-i686-legacy-portable.zip"
Compress-Archive -Path "$portableDir\*" -DestinationPath $portableZip -Force
Write-Host "Signed server binary hash:"
Get-FileHash -Path $signedBinary -Algorithm SHA256 | Format-List
Write-Host "Portable server zip hash:"
Get-FileHash -Path $portableZip -Algorithm SHA256 | Format-List
Remove-Item -Path $portableRoot -Recurse -Force

Write-Host "i686 legacy server packaging complete."