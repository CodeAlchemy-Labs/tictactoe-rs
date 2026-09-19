#Requires -RunAsAdministrator
[CmdletBinding(SupportsShouldProcess=$true, ConfirmImpact='High')]
param()

$ErrorActionPreference = "Stop"

if (-not $PSCmdlet.ShouldProcess("Local Computer", "Trust CodeAlchemy-Labs self-signed certificate")) {
    Write-Error "This script modifies the system certificate store and must be run interactively to confirm the action. Do not run it with -Confirm:`$false."
    exit 1
}

try {
    $certPath = Join-Path $PSScriptRoot "..\certs\codealchemy-labs.cer"
    if (-not (Test-Path $certPath)) {
        Write-Error "Certificate not found at $certPath"
        exit 1
    }

    $tempCer = Join-Path $env:TEMP "codealchemy-labs-temp.cer"
    Copy-Item -Path $certPath -Destination $tempCer -Force

    Write-Host "Adding to Trusted Root Certification Authorities..."
    & certutil -addstore -f "Root" $tempCer
    if ($LASTEXITCODE -ne 0) { Write-Error "Failed to add to Root store"; exit 1 }

    Write-Host "Adding to Trusted Publishers..."
    & certutil -addstore -f "TrustedPublisher" $tempCer
    if ($LASTEXITCODE -ne 0) { Write-Error "Failed to add to TrustedPublisher store"; exit 1 }

    Write-Host "`nCertificate installed successfully. Current record in Root store:"
    Get-ChildItem Cert:\LocalMachine\Root | Where-Object { $_.Subject -like "*CodeAlchemy-Labs*" } | Format-List

    Write-Warning "To remove this trust later, you must run:`ncertutil -delstore `"Root`" <thumbprint>`ncertutil -delstore `"TrustedPublisher`" <thumbprint>"
    Write-Warning "Only trust this certificate if the fingerprint matches the one documented in packaging/certs/README.md."

    exit 0
} catch {
    Write-Error $_
    exit 1
} finally {
    if (Test-Path $tempCer) {
        Remove-Item -Path $tempCer -Force
    }
}
