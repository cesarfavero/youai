//! YouAI Coordinator
//!
//! Central server: node registration, heartbeat, routing, credit.
//! See docs/NEXT_STEPS.md — Passo 6 for implementation plan.

use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "youai-coordinator",
    about = "YouAI network coordinator",
    version
)]
struct Args {
    /// HTTP listen port
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// SQLite database path
    #[arg(long, default_value = "youai.db")]
    db: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    info!(
        port = args.port,
        db = %args.db,
        "youai-coordinator scaffold — API pending"
    );

    eprintln!(
        "youai-coordinator v{} — HTTP API pending (see docs/NEXT_STEPS.md passo 6)",
        env!("CARGO_PKG_VERSION")
    );
}
