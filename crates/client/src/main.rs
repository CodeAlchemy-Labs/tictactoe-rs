//! Binary entry point for the terminal client.

use anyhow::Context;
use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use tokio::sync::{mpsc, watch};
use tracing_subscriber::EnvFilter;

use client::app::update::dispatch;
use client::app::{AppEvent, AppState, apply_event};
use client::config::ClientConfig;
use client::infrastructure::{Transport, WsTransport};
use client::tui::{KeyAction, read_key_action, render};
use std::io::IsTerminal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = ClientConfig::from_args_and_env()?;

    let mut terminal = TerminalSession::enter()?;

    let transport = WsTransport::new(config.server_url.clone(), config.insecure);
    let handle = transport.start();
    let outgoing = handle.outgoing;
    let mut incoming = handle.incoming;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    // Task: forward incoming server messages as app events.
    let server_events = event_tx.clone();
    tokio::spawn(async move {
        while let Some(message) = incoming.recv().await {
            if server_events.send(AppEvent::Server(message)).is_err() {
                return;
            }
        }
        let _ = server_events.send(AppEvent::Disconnected);
    });

    // Broadcast the current screen from the main loop to the keyboard
    // thread. The keyboard thread is the only reader and needs the screen
    // to translate digits correctly (join in the lobby, move in a game).
    let mut state = AppState::new(config.display_name.clone());
    let (screen_tx, screen_rx) = watch::channel(state.screen.clone());

    // Task: read the keyboard in a blocking thread.
    let keyboard_events = event_tx.clone();
    let keyboard_screen = screen_rx;
    tokio::task::spawn_blocking(move || {
        loop {
            let screen = keyboard_screen.borrow().clone();
            match read_key_action(&screen) {
                Ok(KeyAction::Event(event)) => {
                    if keyboard_events.send(event).is_err() {
                        return;
                    }
                }
                Ok(KeyAction::Ignored) => {}
                Err(error) => {
                    tracing::warn!(%error, "keyboard input failed");
                    return;
                }
            }
        }
    });

    outgoing
        .send(common::protocol::ClientMessage::Hello {
            display_name: config.display_name,
        })
        .map_err(|_| anyhow::anyhow!("transport closed before sending hello"))?;

    while !state.should_quit {
        terminal
            .terminal
            .draw(|frame| render(frame, &state))
            .context("failed to draw")?;

        let Some(event) = event_rx.recv().await else {
            break;
        };
        let mut effects = Vec::new();
        apply_event(&mut state, event, &mut effects);
        let mut quit = state.should_quit;
        dispatch(effects, &outgoing, &mut quit);
        state.should_quit = quit;

        let _ = screen_tx.send(state.screen.clone());
    }

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(ansi_supported())
        .init();
}

/// Returns `true` when the current stderr can render ANSI escape codes.
///
/// See the identical helper in `server/src/main.rs` for the rationale. The
/// logic is duplicated in each binary because the helper depends only on
/// `std` and the tracing crates, and duplicating it keeps each binary's
/// dependency graph minimal.
fn ansi_supported() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var("CLICOLOR_FORCE").is_ok_and(|value| value != "0") {
        return true;
    }
    if !std::io::stderr().is_terminal() {
        return false;
    }

    #[cfg(windows)]
    {
        std::env::var_os("WT_SESSION").is_some()
            || std::env::var_os("ConEmuANSI").is_some()
            || std::env::var_os("TERM_PROGRAM").is_some()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// RAII guard that configures the terminal and restores it on drop.
struct TerminalSession {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl TerminalSession {
    fn enter() -> anyhow::Result<Self> {
        enable_raw_mode().context("failed to enable raw mode")?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide).context("failed to enter alternate screen")?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("failed to create terminal")?;
        force_terminal_resize(&mut terminal).context("failed to determine terminal size")?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen, Show);
        let _ = self.terminal.show_cursor();
    }
}

/// Polls the terminal size until it is non-zero and forces ratatui to adopt
/// it before the first frame is drawn.
///
/// Under Docker, the `ioctl` used to query the terminal size can report
/// `0x0` until the PTY forwarding is fully established. Ratatui caches the
/// size at construction time, so without this step the first frames are
/// drawn into a zero-sized area and appear blank until a key press triggers
/// a refresh. Polling for a non-zero size and calling `resize` makes the
/// first frame visible immediately.
fn force_terminal_resize(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<()> {
    for _ in 0..40 {
        let (columns, rows) = crossterm::terminal::size()?;
        if columns > 0 && rows > 0 {
            terminal.resize(Rect::new(0, 0, columns, rows))?;
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    anyhow::bail!("terminal reported 0x0 dimensions after one second")
}
