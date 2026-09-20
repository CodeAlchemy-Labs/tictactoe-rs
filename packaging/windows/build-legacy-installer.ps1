param(
    [string]$Version = "0.2.0"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$distDir = Join-Path $repoRoot "dist\windows"
$outputArtifact = Join-Path $distDir ("tictacli-{0}-legacy-win7.zip" -f $Version)
$binaryPath = Join-Path $repoRoot "target\x86_64-pc-windows-gnu\release\tictacli.exe"

New-Item -ItemType Directory -Force -Path $distDir | Out-Null

cargo +1.77.2 build --release --target x86_64-pc-windows-gnu --no-default-features --features legacy-console --bin tictacli

if (-not (Test-Path $binaryPath)) {
    throw "Legacy Windows binary was not produced at $binaryPath"
}

Compress-Archive -Path $binaryPath -DestinationPath $outputArtifact -Force

Write-Host "Legacy Windows artifact created: $outputArtifact"
