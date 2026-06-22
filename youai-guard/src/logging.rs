use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Default log path: `~/.youai/guard.log`
pub fn default_log_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".youai").join("guard.log"))
}

/// Initialize tracing to stderr and append-only log file.
pub fn init(log_path: Option<PathBuf>) -> Result<PathBuf> {
    let path = log_path.unwrap_or_else(|| default_log_path().expect("HOME not set"));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create log directory {}", parent.display()))?;
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open log file {}", path.display()))?;

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_writer(file).with_ansi(false))
        .init();

    Ok(path)
}
