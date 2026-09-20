param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargoToml = Join-Path $PSScriptRoot "..\..\Cargo.toml"
    $match = Select-String -Path $cargoToml -Pattern '^version = "(.+)"'
    if (-not $match) {
        throw "Could not determine version from Cargo.toml"
    }
    $Version = $match.Matches[0].Groups[1].Value
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$distDir = Join-Path $repoRoot "dist\windows"
$outputPath = Join-Path $distDir ("tictacli-{0}-legacy-win7.zip" -f $Version)
$binaryPath = Join-Path $repoRoot "target\x86_64-pc-windows-gnu\release\tictacli.exe"

New-Item -ItemType Directory -Force -Path $distDir | Out-Null

Write-Host "Building legacy Windows binary for version $Version"
cargo +1.77.2 build --release --target x86_64-pc-windows-gnu --no-default-features --features legacy-console --bin tictacli
if ($LASTEXITCODE -ne 0) {
    throw "Legacy Windows build failed"
}

if (-not (Test-Path $binaryPath)) {
    throw "Legacy Windows binary was not produced at $binaryPath"
}

Compress-Archive -Path $binaryPath -DestinationPath $outputPath -Force
Write-Host "Legacy Windows archive created: $outputPath"
