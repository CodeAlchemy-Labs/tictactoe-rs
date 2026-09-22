//! Embeds `tictacli.ico` into the Windows executable so Explorer, the taskbar
//! and the Add/Remove Programs entry all show the application icon.
//!
//! The script is a no-op on non-Windows targets. `winresource` shells out to
//! `windres` (cross-compilation) or the MSVC resource compiler (native). The
//! build fails loudly if the icon cannot be embedded, because a silent failure
//! would ship a binary with the default Windows icon.

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/windows/tictacli.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../packaging/windows/tictacli.ico");
    res.set("ProductName", "TicTacToe Client");
    res.set("FileDescription", "TicTacToe Client");
    res.set("LegalCopyright", "Copyright (c) CodeAlchemy-Labs");

    res.compile()
        .expect("failed to embed Windows resources into tictacli.exe");
}
