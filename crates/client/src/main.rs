//! Binary entry point for the terminal client.

use anyhow::Context;
use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::{mpsc, watch};
use tracing_subscriber::EnvFilter;

use client::app::update::dispatch;
use client::app::{AppEvent, AppState, apply_event};
use client::config::ClientConfig;
use client::domain::screen::Screen;
use client::infrastructure::{Transport, WsTransport};
use client::tui::{KeyAction, read_key_action, render};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = ClientConfig::from_args_and_env()?;

    let mut terminal = TerminalSession::enter()?;

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

    // A `watch` channel broadcasts the current screen from the main loop to
    // the keyboard thread. The keyboard thread is the only reader, and it
    // must know which screen is showing to translate digits correctly.
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

    // Send the initial Hello.
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

        // Publish the (possibly) updated screen to the keyboard thread. If
        // no receiver is alive, the send is ignored silently.
        let _ = screen_tx.send(state.screen.clone());
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
