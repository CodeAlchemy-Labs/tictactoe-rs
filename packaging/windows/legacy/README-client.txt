TicTacToe Client (Legacy, Windows 7 / 8 / 8.1)

This is the legacy compatibility build of the TicTacToe client. It targets
Windows 7, 8, and 8.1, which Microsoft no longer supports. Prefer the modern
installer on Windows 10 and 11.

HOW TO RUN

  Double-click tictacli.exe. A terminal window opens and the client asks
  for the server URL and your guest name.

  If you already know the server, run from the command line:

    tictacli.exe --server ws://<host>:<port>/ws --name <your-name>

  For a server using TLS, use wss:// instead of ws://.

WHAT IT DOES

  Connects to a TicTacToe server over WebSocket. You can create or join a
  match, spectate games, and view the global ranking.

KNOWN LIMITATIONS

  - Underline colours are disabled to work around a Windows 7 console
    limitation.
  - Requires the Visual C++ runtime. If the client fails to start, install
    the Microsoft Visual C++ Redistributable for Visual Studio 2015-2022.
  - The client is signed with a self-signed certificate. To silence the
    Windows SmartScreen warning, install the certificate from
    packaging/certs/ as described in docs/INSTALLATION.md.

For full documentation, see:
https://github.com/CodeAlchemy-Labs/tictactoe-rs