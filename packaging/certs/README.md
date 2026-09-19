# CodeAlchemy-Labs Code-Signing Certificate

This directory contains the public certificate used to sign Windows installers for the TicTacToe client.

## What is this?

This is a **self-signed** code-signing certificate (CN=CodeAlchemy-Labs) valid for 10 years. Because it is self-signed, Windows does not trust it by default, and SmartScreen will block the installers.

Trusting a self-signed certificate is a deliberate decision with security implications. By trusting this certificate, you are explicitly telling your computer that you trust binaries signed by CodeAlchemy-Labs. A commercial EV certificate would remove this manual step entirely, but this is a community project.

## Fingerprint

Before trusting the certificate, ensure its SHA-256 fingerprint matches exactly:

```text
A6:F4:13:A8:D5:34:24:E3:5A:EA:57:DB:51:F3:C2:41:21:E0:17:DE:EC:44:DC:B1:27:81:0A:04:B5:99:7C:41
```

## How to trust the certificate on Windows

There are two ways to trust the certificate so that Windows will allow the installers to run smoothly.

### Option A: Command Line (Elevated)

Open an **Administrator** PowerShell window in this directory and run:

```powershell
certutil -addstore -f "Root" codealchemy-labs.cer
certutil -addstore -f "TrustedPublisher" codealchemy-labs.cer
```

### Option B: GUI (Certificate Manager)

1. Press `Win + R`, type `certmgr.msc`, and hit Enter.
2. Expand **Trusted Root Certification Authorities** -> **Certificates**.
3. Right-click **Certificates** -> **All Tasks** -> **Import...**
4. Browse to `codealchemy-labs.cer` and complete the wizard.
5. Repeat the process for **Trusted Publishers** -> **Certificates**.

## Verifying an installer

Once the certificate is trusted, you can verify that an installer was properly signed and hasn't been tampered with.

Open PowerShell and run:

```powershell
Get-AuthenticodeSignature -FilePath .\tictacli-0.2.0-setup.exe | Format-List
```

If you have trusted the certificate, the output will show `Status : Valid`.
If you haven't trusted it yet, it will show `Status : UnknownError`.
Both outcomes mean the signature is intact. If you see `HashMismatch` or `NotSigned`, the file has been tampered with or corrupted.
