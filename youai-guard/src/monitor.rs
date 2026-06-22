use crate::limits::ResourceLimits;
use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Samples resource usage for watchdog decisions.
pub trait UsageSampler: Send {
    fn sample_memory_bytes(&self) -> Result<u64>;
    fn sample_cpu_usage_percent(&mut self, elapsed: Duration) -> Result<f64>;
}

/// Watch a child PID until it exits or breaches limits.
pub struct Watchdog<'a> {
    limits: ResourceLimits,
    sampler: &'a mut dyn UsageSampler,
    /// Allow brief CPU spikes above limit (sampling noise).
    cpu_grace_percent: f64,
}

impl<'a> Watchdog<'a> {
    pub fn new(limits: ResourceLimits, sampler: &'a mut dyn UsageSampler) -> Self {
        Self {
            limits,
            sampler,
            cpu_grace_percent: 2.0,
        }
    }

    pub fn run<F>(&mut self, mut is_running: F) -> Result<WatchdogOutcome>
    where
        F: FnMut() -> bool,
    {
        let mut last_tick = Instant::now();

        while is_running() {
            std::thread::sleep(self.limits.poll_interval);
            let elapsed = last_tick.elapsed();
            last_tick = Instant::now();

            let memory = self.sampler.sample_memory_bytes()?;
            if memory > self.limits.ram_max_bytes {
                error!(
                    memory_bytes = memory,
                    limit_bytes = self.limits.ram_max_bytes,
                    "RAM limit breached — sending SIGKILL"
                );
                return Ok(WatchdogOutcome::KilledRam {
                    memory_bytes: memory,
                });
            }

            let cpu = self.sampler.sample_cpu_usage_percent(elapsed)?;
            let threshold = f64::from(self.limits.cpu_percent) + self.cpu_grace_percent;
            if cpu > threshold {
                error!(
                    cpu_percent = cpu,
                    limit_percent = self.limits.cpu_percent,
                    "CPU limit breached — sending SIGKILL"
                );
                return Ok(WatchdogOutcome::KilledCpu { cpu_percent: cpu });
            }

            info!(
                memory_bytes = memory,
                memory_limit_bytes = self.limits.ram_max_bytes,
                cpu_percent = cpu,
                cpu_limit_percent = self.limits.cpu_percent,
                "guard tick"
            );
        }

        info!("child exited before watchdog kill");
        Ok(WatchdogOutcome::ChildExited)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatchdogOutcome {
    ChildExited,
    KilledRam { memory_bytes: u64 },
    KilledCpu { cpu_percent: f64 },
}

impl WatchdogOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ChildExited => 0,
            Self::KilledRam { .. } | Self::KilledCpu { .. } => 137, // 128 + SIGKILL(9)
        }
    }
}

pub fn kill_process(pid: u32) -> Result<()> {
    let pid = pid as i32;
    let rc = unsafe { libc::kill(pid, libc::SIGKILL) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        // ESRCH = already dead
        if err.raw_os_error() == Some(libc::ESRCH) {
            warn!(pid, "process already exited");
            return Ok(());
        }
        return Err(err.into());
    }
    warn!(pid, "sent SIGKILL");
    Ok(())
}
