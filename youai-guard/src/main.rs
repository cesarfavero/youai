//! YouAI Resource Guard
//!
//! Independent process that enforces RAM/CPU/GPU limits on the inference worker.
//! See docs/NEXT_STEPS.md — Passo 3 for implementation plan.

use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "youai-guard", about = "YouAI resource guard", version)]
struct Args {
    /// Maximum RAM (e.g. 8g, 4096m)
    #[arg(long, default_value = "8g")]
    ram_max: String,

    /// Maximum CPU usage percentage (1–100)
    #[arg(long, default_value_t = 30)]
    cpu_percent: u8,

    /// Poll interval in milliseconds
    #[arg(long, default_value_t = 500)]
    poll_ms: u64,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    info!(
        ram_max = %args.ram_max,
        cpu_percent = args.cpu_percent,
        poll_ms = args.poll_ms,
        "youai-guard scaffold — cgroup enforcement not yet implemented"
    );

    eprintln!(
        "youai-guard v{} — POC pending (see docs/NEXT_STEPS.md passo 3)",
        env!("CARGO_PKG_VERSION")
    );
}