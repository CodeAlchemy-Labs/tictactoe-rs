TicTacToe Portable Client
=========================

This is a self-contained `tictacli.exe` that runs the TicTacToe client.
It requires no installer, no external DLLs, and writes nothing to the registry.

How to run
----------
You can simply double-click `tictacli.exe` to start the client.
Alternatively, you can run it from `cmd` or PowerShell.

The client connects to a TicTacToe server over WebSocket. It lets you register,
log in, create or join matches, spectate games, and view the global ranking.

On first launch, if you do not provide `--server` and `--name` on the command
line, the client shows an interactive connection screen. You can also pass
those options explicitly:

    tictacli.exe --server ws://<host>:<port>/ws --name <your-name>

Use `wss://` for a server behind TLS.

Configuration
-------------
After a successful connection, the client stores its configuration at:

    %APPDATA%\tictacli\config.toml

See the included `config.example.toml` for the format.

Firewall
--------
Windows Defender Firewall may prompt you on the first launch. Allow the client
on private networks only.

Signature and SmartScreen
-------------------------
The `.exe` is signed with a self-signed certificate. To silence the Windows
SmartScreen warning, install the certificate from `packaging/certs/` as
described in `docs/INSTALLATION.md` in the repository.

Where to get the server
-----------------------
The release page ships a portable server `.zip` and Linux packages. See the
repository README for details.
