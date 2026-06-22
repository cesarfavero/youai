//! YouAI Inference Worker
//!
//! Wraps llama.cpp for local GGUF model inference.
//! See docs/NEXT_STEPS.md — Passo 4 for implementation plan.

use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "youai-worker",
    about = "YouAI inference worker (llama.cpp)",
    version
)]
struct Args {
    /// Path to GGUF model file
    #[arg(long)]
    model: Option<String>,

    /// Prompt for single-shot inference (dev/testing)
    #[arg(short, long)]
    prompt: Option<String>,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    info!(
        model = ?args.model,
        prompt = ?args.prompt,
        "youai-worker scaffold — llama.cpp integration pending"
    );

    eprintln!(
        "youai-worker v{} — llama.cpp wrapper pending (see docs/NEXT_STEPS.md passo 4)",
        env!("CARGO_PKG_VERSION")
    );
}
