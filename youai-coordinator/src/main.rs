//! YouAI Coordinator — node registry, heartbeat, and round-robin chat routing.

mod api;
mod auth;
mod cache;
mod db;
mod gateway;
mod pipeline;
mod priority;
mod registry;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use youai_common::DEFAULT_COORDINATOR_PORT;

#[derive(Parser, Debug)]
#[command(
    name = "youai-coordinator",
    about = "YouAI network coordinator",
    version
)]
struct Args {
    /// HTTP listen host (use 0.0.0.0 for LAN testing)
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// HTTP listen port
    #[arg(long, default_value_t = DEFAULT_COORDINATOR_PORT)]
    port: u16,

    /// SQLite database path
    #[arg(long, default_value = "youai.db")]
    db: String,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("youai-coordinator error: {err:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    info!(
        host = %args.host,
        port = args.port,
        db = %args.db,
        "starting coordinator"
    );

    api::serve(&args.host, args.port, &args.db).await
}
