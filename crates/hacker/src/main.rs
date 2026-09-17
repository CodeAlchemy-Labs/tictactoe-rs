//! Binary entry point for the adversarial actor.

use clap::Parser;
use tracing_subscriber::EnvFilter;

use hacker::config::{Cli, ScenarioChoice};
use hacker::scenarios;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();

    let scenario_names: Vec<&'static str> = match cli.scenario {
        ScenarioChoice::All => vec!["session_hijack", "port_reuse", "flood"],
        ScenarioChoice::SessionHijack => vec!["session_hijack"],
        ScenarioChoice::PortReuse => vec!["port_reuse"],
        ScenarioChoice::Flood => vec!["flood"],
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
        .init();
}
