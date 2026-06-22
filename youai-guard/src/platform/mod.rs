use crate::limits::ResourceLimits;
use anyhow::Result;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::{run_command, RunOutcome};

#[cfg(target_os = "macos")]
pub use macos::{run_command, RunOutcome};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn run_command(_command: &[String], _limits: ResourceLimits) -> Result<RunOutcome> {
    anyhow::bail!("youai-guard run is only supported on Linux and macOS (best-effort) for now")
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[derive(Debug)]
pub enum RunOutcome {
    Exited(i32),
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl RunOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Exited(code) => *code,
        }
    }
}

pub fn run(command: &[String], limits: ResourceLimits) -> Result<i32> {
    let outcome = run_command(command, limits)?;
    Ok(outcome.exit_code())
}
