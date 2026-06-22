//! YouAI Resource Guard CLI

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use youai_guard::{logging, parse_limits, run, DEFAULT_POLL_MS};

#[derive(Parser, Debug)]
#[command(name = "youai-guard", about = "YouAI resource guard", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a command under resource limits (cgroup v2 on Linux)
    Run(RunArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
    /// Maximum RAM (e.g. 8g, 512m)
    #[arg(long, default_value = "8g")]
    ram_max: String,

    /// Maximum CPU usage percentage (1–100)
    #[arg(long, default_value_t = 30)]
    cpu_percent: u8,

    /// Watchdog poll interval in milliseconds
    #[arg(long, default_value_t = DEFAULT_POLL_MS)]
    poll_ms: u64,

    /// Optional log file (default: ~/.youai/guard.log)
    #[arg(long)]
    log_file: Option<PathBuf>,

    /// Command and arguments (after `--`)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    command: Vec<String>,
}

fn main() {
    if let Err(err) = run_cli() {
        eprintln!("youai-guard error: {err:#}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(args) => exec_run(args),
    }
}

fn exec_run(args: RunArgs) -> Result<()> {
    let limits = parse_limits(&args.ram_max, args.cpu_percent, args.poll_ms)?;
    let log_path = logging::init(args.log_file)?;

    info!(
        log_file = %log_path.display(),
        ram_max = %args.ram_max,
        cpu_percent = args.cpu_percent,
        poll_ms = args.poll_ms,
        command = ?args.command,
        "youai-guard starting"
    );

    if args.command.is_empty() {
        bail!("no command specified after --");
    }

    let code = run(&args.command, limits).context("guard run failed")?;

    info!(exit_code = code, "youai-guard finished");
    std::process::exit(code);
}
