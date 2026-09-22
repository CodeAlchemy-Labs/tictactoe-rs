$ErrorActionPreference = "Stop"

# Resolve repo root
$RepoRoot = Resolve-Path "$PSScriptRoot\..\.."

# 1. Read version from Cargo.toml
$CargoToml = Join-Path $RepoRoot "Cargo.toml"
$VersionMatch = Select-String -Path $CargoToml -Pattern '^version\s*=\s*"(.+)"' | Select-Object -Last 1
if (-not $VersionMatch) {
    Write-Error "Invalid or missing version in Cargo.toml"
    exit 1
}
$Version = $VersionMatch.Matches.Groups[1].Value
if (-not ($Version -match '^\d+\.\d+\.\d+')) {
    Write-Error "Invalid version format: $Version"
    exit 1
}
Write-Output "Building portable client version $Version"

# 2. Build
Write-Output "Building release binary for x86_64-pc-windows-gnu..."
Set-Location $RepoRoot
cargo build --release --locked --target x86_64-pc-windows-gnu --bin tictacli
if ($LASTEXITCODE -ne 0) {
    Write-Error "cargo build failed"
    exit 1
}

# 3. Copy to dist
$DistWindows = Join-Path $RepoRoot "dist\windows"
$DistPortable = Join-Path $DistWindows "portable"
if (-not (Test-Path $DistPortable)) {
    New-Item -ItemType Directory -Path $DistPortable | Out-Null
}

$ExeSrc = Join-Path $RepoRoot "target\x86_64-pc-windows-gnu\release\tictacli.exe"
if (-not (Test-Path $ExeSrc)) {
    Write-Error "Built client binary was not found: $ExeSrc"
    exit 1
}
$ExeDst = Join-Path $DistPortable "tictacli.exe"
Copy-Item -Path $ExeSrc -Destination $ExeDst -Force

# 4. Sign
if ($args -contains "-SkipSign") {
    Write-Output "Skipping signing as requested..."
} else {
    Write-Output "Signing the binary..."
    $SignScript = Join-Path $RepoRoot "packaging\windows\sign.ps1"
    $SignedExe = Join-Path $DistPortable "tictacli-signed.exe"
    & pwsh -NoProfile -File $SignScript -InputFile $ExeDst -OutputFile $SignedExe
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Signing failed"
        exit 1
    }
    if (-not (Test-Path $SignedExe)) {
        Write-Error "Signing completed without producing an output file"
        exit 1
    }
    Move-Item -Path $SignedExe -Destination $ExeDst -Force
}

# 5. Assemble staging
Write-Output "Assembling staging directory..."
$Staging = Join-Path $DistPortable "staging-client"
if (Test-Path $Staging) {
    Remove-Item -Path $Staging -Recurse -Force
}
New-Item -ItemType Directory -Path $Staging | Out-Null

Copy-Item -Path $ExeDst -Destination $Staging
Copy-Item -Path (Join-Path $RepoRoot "packaging\windows\portable\README-client.txt") -Destination (Join-Path $Staging "README.txt")
Copy-Item -Path (Join-Path $RepoRoot "LICENSE") -Destination $Staging
Copy-Item -Path (Join-Path $RepoRoot "packaging\windows\portable\config.example.toml") -Destination $Staging

# 6. Zip
$ZipPath = Join-Path $DistWindows "tictacli-$Version-x86_64-portable.zip"
Write-Output "Creating zip archive: $ZipPath"
Compress-Archive -Path "$Staging\*" -DestinationPath $ZipPath -Force
if (-not (Test-Path $ZipPath)) {
    Write-Error "Zip archive was not created: $ZipPath"
    exit 1
}

# 7. Checksums
Write-Output "Checksums:"
Get-FileHash -Algorithm SHA256 $ExeDst | Format-List
Get-FileHash -Algorithm SHA256 $ZipPath | Format-List

# 8. Clean staging
Remove-Item -Path $Staging -Recurse -Force

Write-Output "Done building portable client."
