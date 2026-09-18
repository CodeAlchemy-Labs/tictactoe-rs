//! Binary entry point for the adversarial actor.

use clap::Parser;
use tracing_subscriber::EnvFilter;

use hacker::config::{Cli, ScenarioChoice};
use hacker::scenarios;
use std::io::IsTerminal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();

    let scenario_names: Vec<&'static str> = match cli.scenario {
        ScenarioChoice::All => vec![
            "session_hijack",
            "port_reuse",
            "flood",
            "spectator_isolation",
        ],
        ScenarioChoice::SessionHijack => vec!["session_hijack"],
        ScenarioChoice::PortReuse => vec!["port_reuse"],
        ScenarioChoice::Flood => vec!["flood"],
        ScenarioChoice::SpectatorIsolation => vec!["spectator_isolation"],
    };

    let mut all_defended = true;
    for name in scenario_names {
        match scenarios::run(name, &cli.target).await {
            Ok(outcome) => {
                if !outcome.is_defended() {
                    all_defended = false;
                }
                eprintln!("{} {name}: {}", outcome.label(), outcome.detail());
            }
            Err(error) => {
                all_defended = false;
                eprintln!("COMPROMISED {name}: unexpected error: {error:#}");
            }
        }
    }

    if !all_defended {
        std::process::exit(1);
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
/// See the identical helper in `server/src/main.rs` for the rationale.
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
