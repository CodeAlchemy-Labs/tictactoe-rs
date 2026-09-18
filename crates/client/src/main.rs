//! Binary entry point for the terminal client.

use std::sync::{Arc, RwLock};

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
use client::domain::screen::Screen;
use client::infrastructure::{Transport, WsTransport};
use client::tui::{InputEvent, KeyAction, read_input_event, render, translate_key};

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

    let server_events = event_tx.clone();
    tokio::spawn(async move {
        while let Some(message) = incoming.recv().await {
            if server_events.send(AppEvent::Server(message)).is_err() {
                return;
            }
        }
        let _ = server_events.send(AppEvent::Disconnected);
    });

    // The keyboard thread reads a raw input event, then looks up the current
    // screen through an `RwLock` and translates the key against it. Reading
    // the screen after the key arrives keeps the translation fresh without
    // adding any lag. Resize events are forwarded as `AppEvent::Redraw`.
    let mut state = AppState::new(config.display_name.clone());
    let screen = Arc::new(RwLock::new(state.screen.clone()));

    let keyboard_events = event_tx.clone();
    let keyboard_screen = Arc::clone(&screen);
    tokio::task::spawn_blocking(move || {
        loop {
            let input = match read_input_event() {
                Ok(input) => input,
                Err(error) => {
                    tracing::warn!(%error, "keyboard input failed");
                    return;
                }
            };
            let app_event = match input {
                InputEvent::Key(code) => {
                    let current = keyboard_screen
                        .read()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    match translate_key(code, &current) {
                        KeyAction::Event(event) => Some(event),
                        KeyAction::Ignored => None,
                    }
                }
                InputEvent::Resize(_, _) => Some(AppEvent::Redraw),
            };
            if let Some(event) = app_event
                && keyboard_events.send(event).is_err()
            {
                return;
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

        *screen
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = state.screen.clone();
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
