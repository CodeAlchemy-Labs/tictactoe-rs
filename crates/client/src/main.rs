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

use client::app::{AppEvent, AppState, apply_event};
use client::config::ArgsConfig;
use client::infrastructure::{Transport, WsTransport};
use client::tui::{InputEvent, KeyAction, read_input_event, render, translate_key};

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = ArgsConfig::from_args_and_env()?;
    let client_config = client::config::load();

    let mut terminal = TerminalSession::enter()?;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    // Global outgoing sender that can be replaced on reconnect
    let (dummy_tx, _) = mpsc::unbounded_channel();
    let global_outgoing = Arc::new(RwLock::new(dummy_tx));

    let mut state = AppState::with_config(
        config.server_url.as_deref(),
        config.display_name.as_deref(),
        &client_config,
    );
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
            if let Some(event) = app_event {
                if keyboard_events.send(event).is_err() {
                    return;
                }
            }
        }
    });

    // Fast path connection
    if !state.uses_connection_form {
        let url = config.server_url.clone().unwrap();
        let insecure = config.insecure;
        let name = config.display_name.clone().unwrap();

        let transport = WsTransport::new(url, insecure);
        let handle = transport.start();
        *global_outgoing.write().unwrap() = handle.outgoing.clone();
        let mut incoming = handle.incoming;

        let server_events = event_tx.clone();
        tokio::spawn(async move {
            while let Some(message) = incoming.recv().await {
                if server_events.send(AppEvent::Server(message)).is_err() {
                    return;
                }
            }
            let _ = server_events.send(AppEvent::Disconnected);
        });

        let _ = handle
            .outgoing
            .send(common::protocol::ClientMessage::Hello { display_name: name });
    }

    while !state.should_quit {
        terminal
            .terminal
            .draw(|frame| render(frame, &state))
            .context("failed to draw")?;

        let Some(event) = event_rx.recv().await else {
            break;
        };

        if let AppEvent::Server(common::protocol::ServerMessage::Welcome { .. }) = &event {
            if state.uses_connection_form {
                let url = state
                    .connection_form
                    .build_url()
                    .unwrap_or_else(|_| "ws://127.0.0.1:8080/ws".to_string());
                let c = client::config::ClientConfig {
                    server_url: Some(url),
                    guest_name: Some(state.connection_form.guest_name.clone()),
                    use_tls: Some(state.connection_form.use_tls),
                };
                if let Err(e) = client::config::save(&c) {
                    tracing::warn!("Failed to save client config: {}", e);
                }
            }
        }

        let mut effects = Vec::new();
        apply_event(&mut state, event, &mut effects);

        for effect in effects {
            match effect {
                client::app::update::SideEffect::Send(message) => {
                    let sender = global_outgoing.read().unwrap().clone();
                    let _ = sender.send(message);
                }
                client::app::update::SideEffect::Connect {
                    url,
                    name,
                    use_tls: _,
                } => {
                    let insecure = config.insecure;
                    let server_events = event_tx.clone();
                    let outgoing_ref = Arc::clone(&global_outgoing);

                    tokio::spawn(async move {
                        let transport = WsTransport::new(url, insecure);
                        let handle = transport.start();
                        let mut incoming = handle.incoming;
                        let outgoing = handle.outgoing;

                        if outgoing
                            .send(common::protocol::ClientMessage::Hello { display_name: name })
                            .is_err()
                        {
                            let _ = server_events.send(AppEvent::ConnectionFailed {
                                reason: "Failed to send Hello".to_string(),
                            });
                            return;
                        }

                        match tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            incoming.recv(),
                        )
                        .await
                        {
                            Ok(Some(msg)) => {
                                *outgoing_ref.write().unwrap() = outgoing;
                                if server_events.send(AppEvent::Server(msg)).is_err() {
                                    return;
                                }

                                tokio::spawn(async move {
                                    while let Some(m) = incoming.recv().await {
                                        if server_events.send(AppEvent::Server(m)).is_err() {
                                            return;
                                        }
                                    }
                                    let _ = server_events.send(AppEvent::Disconnected);
                                });
                            }
                            Ok(None) => {
                                let _ = server_events.send(AppEvent::ConnectionFailed {
                                    reason: "Connection refused or unreachable".to_string(),
                                });
                            }
                            Err(_) => {
                                let _ = server_events.send(AppEvent::ConnectionFailed {
                                    reason: "Connection timed out".to_string(),
                                });
                            }
                        }
                    });
                }
                client::app::update::SideEffect::Quit => state.should_quit = true,
            }
        }

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
