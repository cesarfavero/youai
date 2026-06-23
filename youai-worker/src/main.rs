//! YouAI Inference Worker — llama.cpp wrapper with HTTP serve mode.

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;
use youai_common::{resolve_llama_cli, resolve_model_path, NodeConfig};
use youai_worker::{run_inference, serve, InferenceConfig, WorkerState};

#[derive(Parser, Debug)]
#[command(
    name = "youai-worker",
    about = "YouAI inference worker (llama.cpp)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Single-shot inference to stdout (dev/testing)
    Infer {
        #[arg(long)]
        model: Option<String>,
        #[arg(short, long)]
        prompt: String,
        #[arg(long, default_value_t = 128)]
        max_tokens: u32,
        #[arg(long)]
        llama_cli: Option<String>,
    },
    /// HTTP server for coordinator/node requests
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = youai_common::DEFAULT_WORKER_PORT)]
        port: u16,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        llama_cli: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("youai-worker error: {err:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Infer {
            model,
            prompt,
            max_tokens,
            llama_cli,
        } => exec_infer(model, prompt, max_tokens, llama_cli).await,
        Commands::Serve {
            host,
            port,
            model,
            llama_cli,
        } => exec_serve(host, port, model, llama_cli).await,
    }
}

async fn exec_infer(
    model: Option<String>,
    prompt: String,
    max_tokens: u32,
    llama_cli: Option<String>,
) -> Result<()> {
    let mut config = NodeConfig::default();
    if let Some(model) = model {
        config.model.path = Some(model);
    }
    if let Some(llama_cli) = llama_cli {
        config.runtime.llama_cli = Some(llama_cli);
    }

    let llama_cli = resolve_llama_cli(&config)?;
    let model_path = resolve_model_path(&config)?;

    let text = run_inference(&InferenceConfig {
        llama_cli,
        model_path,
        model_name: config.model.name,
        prompt,
        max_tokens,
        timeout: youai_worker::llama::default_timeout(),
        rpc_servers: vec![],
        remote_shards: vec![],
    })?;

    println!("{text}");
    Ok(())
}

async fn exec_serve(
    host: String,
    port: u16,
    model: Option<String>,
    llama_cli: Option<String>,
) -> Result<()> {
    let mut config = NodeConfig::default();
    if let Some(model) = model {
        config.model.path = Some(model);
    }
    if let Some(llama_cli) = llama_cli {
        config.runtime.llama_cli = Some(llama_cli);
    }

    let llama_cli = resolve_llama_cli(&config)?;
    let model_path = resolve_model_path(&config)?;
    let model_name = config.model.name.clone();

    youai_worker::llama::validate_paths(&llama_cli, &model_path)?;

    info!(
        host = %host,
        port,
        model = %model_path.display(),
        "starting worker HTTP server"
    );

    serve(
        &host,
        port,
        WorkerState {
            llama_cli,
            model_path,
            model_name,
            pipeline_daemon: std::sync::Arc::new(std::sync::Mutex::new(None)),
        },
    )
    .await
}
