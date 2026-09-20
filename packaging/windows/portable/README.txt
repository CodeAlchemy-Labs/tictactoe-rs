TicTacToe Portable Server
=========================

This is a self-contained `.exe` that runs the TicTacToe WebSocket server.
It requires no installation, no external DLLs, and writes nothing to the registry.

How to run
----------
You can simply double-click `tictacli-server.exe` to start the server.
Alternatively, you can run it from `cmd` or PowerShell.

Configuration
-------------
By default, the server binds to `0.0.0.0:8080` and runs in development mode.
To change the bind address, set the `TICTACTOE_BIND` environment variable before launching.

Production Mode
---------------
For public deployments, you must enable production mode and configure appropriate limits.
Set `TICTACTOE_ENV=production` and configure the following three limits before launching:
- `TICTACTOE_MAX_SESSIONS`
- `TICTACTOE_MAX_SESSIONS_PER_IP`
- `TICTACTOE_AUTH_RATE_LIMIT_PER_MINUTE`

Please reference the included `sample.env` file for the recommended values.

Stopping the server
-------------------
To stop the server, press `Ctrl+C` in the console window.

Firewall
--------
Windows Defender Firewall will prompt you on the first launch. It is recommended to allow the app on private networks. Do not allow it on public networks unless you fully understand the exposure.

Signature and SmartScreen
-------------------------
The `.exe` is signed with a self-signed certificate. Trusting the certificate is optional. Instructions for trusting it can be found in `packaging/certs/README.md` on the repository.
Without trusting the certificate, Windows SmartScreen will display a warning on the first run.

Where to get the client
-----------------------
You can download the `tictacli` client installer from the GitHub releases page.
