//! YouAI Node CLI — config, start, pause, status for contributor nodes.

mod runtime;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;
use youai_common::{load_config, save_config, worker_url, NodeConfig};

#[derive(Parser, Debug)]
#[command(name = "youai-node", about = "YouAI contributor node CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show node status and cluster overview
    Status,
    /// Pause contribution immediately
    Pause,
    /// Start contributing (worker + coordinator registration)
    Start {
        /// Coordinator base URL (e.g. http://192.168.1.10:8080)
        #[arg(long)]
        coordinator: Option<String>,
        /// Worker listen host (LAN IP for multi-machine tests)
        #[arg(long)]
        worker_host: Option<String>,
        /// Worker listen port
        #[arg(long)]
        worker_port: Option<u16>,
        /// URL advertised to coordinator (if different from worker bind address)
        #[arg(long)]
        worker_advertise_url: Option<String>,
        /// Pipeline shard group (empty = replica-only)
        #[arg(long)]
        shard_group: Option<String>,
        #[arg(long)]
        shard_stage: Option<u8>,
        #[arg(long)]
        shard_total_stages: Option<u8>,
        /// Node display name
        #[arg(long)]
        name: Option<String>,
        /// llama.cpp RPC endpoint advertised to coordinator (host:port)
        #[arg(long)]
        rpc_url: Option<String>,
        /// Clear stored rpc_url (GGUF v2 nodes should not advertise RPC).
        #[arg(long)]
        clear_rpc_url: bool,
        #[arg(long)]
        gguf_shard_index: Option<u16>,
        #[arg(long)]
        gguf_shard_total: Option<u16>,
    },
    /// Configure resource limits and defaults
    Config {
        #[arg(long)]
        cpu_percent: Option<u8>,
        #[arg(long)]
        ram_max: Option<String>,
        #[arg(long)]
        coordinator: Option<String>,
        #[arg(long)]
        worker_host: Option<String>,
        #[arg(long)]
        worker_port: Option<u16>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        model_path: Option<String>,
        #[arg(long)]
        llama_cli: Option<String>,
        #[arg(long)]
        worker_advertise_url: Option<String>,
        #[arg(long)]
        shard_group: Option<String>,
        #[arg(long)]
        shard_stage: Option<u8>,
        #[arg(long)]
        shard_total_stages: Option<u8>,
        #[arg(long)]
        region: Option<String>,
        #[arg(long)]
        rpc_url: Option<String>,
        /// Clear stored rpc_url (GGUF v2 nodes should not advertise RPC).
        #[arg(long)]
        clear_rpc_url: bool,
        #[arg(long)]
        gguf_shard_index: Option<u16>,
        #[arg(long)]
        gguf_shard_total: Option<u16>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("youai-node error: {err:#}");
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
        Commands::Status => {
            let config = load_config().context("load config")?;
            runtime::show_status(&config).await?;
        }
        Commands::Pause => {
            runtime::pause_running_node()?;
            println!("node paused");
        }
        Commands::Start {
            coordinator,
            worker_host,
            worker_port,
            worker_advertise_url,
            shard_group,
            shard_stage,
            shard_total_stages,
            name,
            rpc_url,
            clear_rpc_url,
            gguf_shard_index,
            gguf_shard_total,
        } => {
            let mut config = load_config().context("load config")?;
            apply_overrides(
                &mut config,
                coordinator,
                worker_host,
                worker_port,
                name,
                None,
                None,
                None,
                None,
                None,
            );
            apply_shard_overrides(
                &mut config,
                shard_group,
                shard_stage,
                shard_total_stages,
                gguf_shard_index,
                gguf_shard_total,
            );
            if let Some(url) = worker_advertise_url {
                config.worker_advertise_url = Some(url);
            }
            apply_rpc_url(&mut config, rpc_url, clear_rpc_url);
            save_config(&config)?;

            info!(
                name = %config.name,
                coordinator = %config.coordinator_url,
                worker = %worker_url(&config.worker_host, config.worker_port),
                rpc = ?config.rpc_url,
                gguf_shard = config.shard.gguf_shard_index,
                gguf_total = config.shard.gguf_shard_total,
                "starting node"
            );

            let node = runtime::NodeRuntime::start(config).await?;
            println!("node started — worker {} · Ctrl+C to stop", node.worker_url);
            node.run_until_stopped().await?;
        }
        Commands::Config {
            cpu_percent,
            ram_max,
            coordinator,
            worker_host,
            worker_port,
            name,
            model,
            model_path,
            llama_cli,
            worker_advertise_url,
            shard_group,
            shard_stage,
            shard_total_stages,
            region,
            rpc_url,
            clear_rpc_url,
            gguf_shard_index,
            gguf_shard_total,
        } => {
            let mut config = load_config().context("load config")?;
            apply_overrides(
                &mut config,
                coordinator,
                worker_host,
                worker_port,
                name,
                cpu_percent,
                ram_max,
                model,
                model_path,
                llama_cli,
            );
            apply_shard_overrides(
                &mut config,
                shard_group,
                shard_stage,
                shard_total_stages,
                gguf_shard_index,
                gguf_shard_total,
            );
            if let Some(url) = worker_advertise_url {
                config.worker_advertise_url = Some(url);
            }
            if let Some(region) = region {
                config.region = region;
            }
            apply_rpc_url(&mut config, rpc_url, clear_rpc_url);
            save_config(&config)?;
            println!("config saved to {}", youai_common::config_path()?.display());
            runtime::show_status(&config).await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_overrides(
    config: &mut NodeConfig,
    coordinator: Option<String>,
    worker_host: Option<String>,
    worker_port: Option<u16>,
    name: Option<String>,
    cpu_percent: Option<u8>,
    ram_max: Option<String>,
    model: Option<String>,
    model_path: Option<String>,
    llama_cli: Option<String>,
) {
    if let Some(coordinator) = coordinator {
        config.coordinator_url = coordinator;
    }
    if let Some(worker_host) = worker_host {
        config.worker_host = worker_host;
    }
    if let Some(worker_port) = worker_port {
        config.worker_port = worker_port;
    }
    if let Some(name) = name {
        config.name = name;
    }
    if let Some(cpu_percent) = cpu_percent {
        config.resources.cpu_percent = cpu_percent;
    }
    if let Some(ram_max) = ram_max {
        config.resources.ram_max = ram_max;
    }
    if let Some(model) = model {
        config.model.name = model;
    }
    if let Some(model_path) = model_path {
        config.model.path = Some(model_path);
    }
    if let Some(llama_cli) = llama_cli {
        config.runtime.llama_cli = Some(llama_cli);
    }
}

fn apply_rpc_url(config: &mut NodeConfig, rpc_url: Option<String>, clear_rpc_url: bool) {
    if clear_rpc_url {
        config.rpc_url = None;
        return;
    }
    if let Some(url) = rpc_url {
        config.rpc_url = Some(url);
    }
}

fn apply_shard_overrides(
    config: &mut NodeConfig,
    shard_group: Option<String>,
    shard_stage: Option<u8>,
    shard_total_stages: Option<u8>,
    gguf_shard_index: Option<u16>,
    gguf_shard_total: Option<u16>,
) {
    if let Some(group) = shard_group {
        config.shard.group = group;
    }
    if let Some(stage) = shard_stage {
        config.shard.stage = stage;
    }
    if let Some(total) = shard_total_stages {
        config.shard.total_stages = total;
    }
    if let Some(index) = gguf_shard_index {
        config.shard.gguf_shard_index = index;
    }
    if let Some(total) = gguf_shard_total {
        config.shard.gguf_shard_total = total;
    }
}
