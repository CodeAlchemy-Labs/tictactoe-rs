param(
    [Parameter(Mandatory=$true)][string]$InputFile,
    [Parameter(Mandatory=$true)][string]$OutputFile
)

$ErrorActionPreference = "Stop"

if (-not $env:WINDOWS_CERTIFICATE_PFX_BASE64) {
    Write-Error "WINDOWS_CERTIFICATE_PFX_BASE64 environment variable is not set."
    exit 1
}

if (-not $env:WINDOWS_CERTIFICATE_PASSWORD) {
    Write-Error "WINDOWS_CERTIFICATE_PASSWORD environment variable is not set."
    exit 1
}

$tempPfx = Join-Path $env:TEMP "$([guid]::NewGuid()).pfx"
try {
    $bytes = [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_PFX_BASE64)
    [IO.File]::WriteAllBytes($tempPfx, $bytes)
    
    if ($IsWindows) {
        $acl = Get-Acl $tempPfx
        $acl.SetAccessRuleProtection($true, $false)
        $rule = New-Object System.Security.AccessControl.FileSystemAccessRule([System.Security.Principal.WindowsIdentity]::GetCurrent().Name, "FullControl", "Allow")
        $acl.AddAccessRule($rule)
        Set-Acl -Path $tempPfx -AclObject $acl
    }

    $signer = $null
    if (Get-Command "osslsigncode" -ErrorAction SilentlyContinue) {
        $signer = "osslsigncode"
    } elseif (Get-Command "signtool" -ErrorAction SilentlyContinue) {
        $signer = "signtool"
    } else {
        Write-Error "Neither osslsigncode nor signtool was found on PATH."
        exit 1
    }

    if ($signer -eq "osslsigncode") {
        & osslsigncode sign -pkcs12 $tempPfx -pass $env:WINDOWS_CERTIFICATE_PASSWORD -n "TicTacToe Client" -i "https://github.com/CodeAlchemy-Labs/tictactoe-rs" -h sha256 -in $InputFile -out $OutputFile
        if ($LASTEXITCODE -ne 0) {
            Write-Error "osslsigncode failed to sign."
            exit 1
        }
        
        $crtPath = Join-Path $PSScriptRoot "..\certs\codealchemy-labs.crt"
        & osslsigncode verify -in $OutputFile -CAfile $crtPath
        if ($LASTEXITCODE -ne 0) {
            Write-Error "osslsigncode failed to verify the signature."
            exit 1
        }
    } else {
        Copy-Item -Path $InputFile -Destination $OutputFile -Force
        & signtool sign /f $tempPfx /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd sha256 /d "TicTacToe Client" /du "https://github.com/CodeAlchemy-Labs/tictactoe-rs" $OutputFile
        if ($LASTEXITCODE -ne 0) {
            Write-Error "signtool failed to sign."
            exit 1
        }
    }

    Write-Host "Signed: $OutputFile"
    exit 0
} catch {
    Write-Error $_
    exit 1
} finally {
    if (Test-Path $tempPfx) {
        Remove-Item -Path $tempPfx -Force
    }
}
