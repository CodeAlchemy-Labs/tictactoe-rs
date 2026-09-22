TicTacToe Portable Client for macOS
====================================

This is a self-contained `tictacli` binary that runs the TicTacToe client on macOS. It has no installer, no `.app` bundle, and no Gatekeeper notarization.

Supported platform
------------------
macOS 11 (Big Sur) or newer on Apple Silicon (M1, M2, M3, M4). Intel Macs are not supported in this release.

How to run
-----------
After extracting the archive, run:

    chmod +x tictacli
    xattr -d com.apple.quarantine tictacli 2>/dev/null || true
    ./tictacli

The `xattr -d` command removes the quarantine attribute that macOS adds to downloaded files. Without it, Gatekeeper blocks the first launch. The `|| true` is a documentation courtesy: the command fails harmlessly if the attribute is not present.

First launch
------------
If no `--server` and `--name` are provided, the client shows an interactive connection screen. You can also pass them explicitly:

    ./tictacli --server ws://<host>:<port>/ws --name <your-name>

Use `wss://` for a server behind TLS.

Configuration
-------------
The configuration file is stored at `~/Library/Application Support/tictacli/config.toml`. It is created automatically after a successful connection. See `config.example.toml` in this archive for the format.

Signature status
----------------
The binary is ad-hoc signed, not notarized. macOS may show a security warning on first launch. Right-click the binary and choose **Open** to bypass it once, or run the `xattr -d` command above.

Where to get the server
-----------------------
The release page ships a portable server `.zip` for Windows and Linux packages. See the repository README.
