//! Binary entry point for the terminal client.

use anyhow::Context;
use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use client::app::update::dispatch;
use client::app::{AppEvent, AppState, apply_event};
use client::config::ClientConfig;
use client::infrastructure::{Transport, WsTransport};
use client::tui::{KeyAction, read_key_action, render};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = ClientConfig::from_args_and_env()?;

    // Terminal setup. The `TerminalSession` guard restores the previous
    // state on drop, which includes the panic path.
    let mut terminal = TerminalSession::enter()?;

    // Split the transport and wire it to the event queue.
    let transport = WsTransport::new(config.server_url.clone());
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

    // Task: read the keyboard in a blocking thread.
    let keyboard_events = event_tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            match read_key_action() {
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

    // Send the initial Hello.
    let mut state = AppState::new(config.display_name.clone());
    outgoing
        .send(common::protocol::ClientMessage::Hello {
            display_name: config.display_name,
        })
        .map_err(|_| anyhow::anyhow!("transport closed before sending hello"))?;

    // Main loop.
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
    }

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
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
        let terminal = Terminal::new(backend).context("failed to create terminal")?;
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
