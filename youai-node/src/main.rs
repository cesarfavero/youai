//! YouAI Node CLI
//!
//! Entry point for contributors: config, start, pause, status.
//! See docs/NEXT_STEPS.md — Passos 5–7 for implementation plan.

use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "youai-node", about = "YouAI contributor node CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show node status and resource usage
    Status,
    /// Pause contribution immediately
    Pause,
    /// Start contributing (guard → worker → coordinator)
    Start {
        /// Coordinator URL
        #[arg(long)]
        coordinator: Option<String>,
    },
    /// Configure resource limits
    Config {
        #[arg(long)]
        cpu_percent: Option<u8>,
        #[arg(long)]
        ram_max: Option<String>,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Status => info!("status: not implemented"),
        Commands::Pause => info!("pause: not implemented"),
        Commands::Start { coordinator } => {
            info!(?coordinator, "start: not implemented");
        }
        Commands::Config {
            cpu_percent,
            ram_max,
        } => {
            info!(?cpu_percent, ?ram_max, "config: not implemented");
        }
    }

    eprintln!(
        "youai-node v{} — CLI scaffold (see docs/NEXT_STEPS.md)",
        env!("CARGO_PKG_VERSION")
    );
}
