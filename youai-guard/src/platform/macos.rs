use crate::limits::ResourceLimits;
use crate::monitor::{UsageSampler, Watchdog, WatchdogOutcome};
use anyhow::{bail, Context, Result};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::warn;

/// macOS has no cgroup v2 — best-effort via setrlimit + ps-based watchdog.
pub fn run_command(command: &[String], limits: ResourceLimits) -> Result<RunOutcome> {
    if command.is_empty() {
        bail!("empty command");
    }

    warn!("macOS: no cgroup v2 — using setrlimit(RLIMIT_AS) + CPU watchdog (best-effort)");

    let ram_bytes = limits.ram_max_bytes;
    let program = &command[0];
    let args = &command[1..];

    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    unsafe {
        cmd.pre_exec(move || {
            // Best-effort: macOS may reject RLIMIT_AS before the binary maps.
            let _ = set_rlimit_as(ram_bytes);
            Ok(())
        });
    }

    let mut child = cmd.spawn().with_context(|| format!("spawn {program}"))?;
    let pid = child.id();

    let mut sampler = ProcessSampler::new(pid)?;
    let mut watchdog = Watchdog::new(limits, &mut sampler);

    let outcome = watchdog.run(|| match child.try_wait() {
        Ok(None) => true,
        Ok(Some(_)) => false,
        Err(_) => false,
    })?;

    match outcome {
        WatchdogOutcome::ChildExited => {
            let status = child.wait()?;
            Ok(RunOutcome::Exited(status.code().unwrap_or(1)))
        }
        WatchdogOutcome::KilledRam { .. } | WatchdogOutcome::KilledCpu { .. } => {
            crate::monitor::kill_process(pid)?;
            let _ = child.wait();
            Ok(RunOutcome::KilledByGuard(outcome))
        }
    }
}

#[derive(Debug)]
pub enum RunOutcome {
    Exited(i32),
    KilledByGuard(WatchdogOutcome),
}

impl RunOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Exited(code) => *code,
            Self::KilledByGuard(o) => o.exit_code(),
        }
    }
}

struct ProcessSampler {
    pid: u32,
    last_cpu_time: f64,
    num_cpus: u32,
}

impl ProcessSampler {
    fn new(pid: u32) -> Result<Self> {
        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        Ok(Self {
            pid,
            last_cpu_time: process_cpu_time(pid)?,
            num_cpus,
        })
    }
}

impl UsageSampler for ProcessSampler {
    fn sample_memory_bytes(&self) -> Result<u64> {
        // ps rss is in kilobytes on macOS
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &self.pid.to_string()])
            .output()
            .context("ps rss")?;
        if !output.status.success() {
            return Ok(0);
        }
        let kb: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0);
        Ok(kb * 1024)
    }

    fn sample_cpu_usage_percent(&mut self, elapsed: Duration) -> Result<f64> {
        let cpu_time = process_cpu_time(self.pid)?;
        let delta = (cpu_time - self.last_cpu_time).max(0.0);
        self.last_cpu_time = cpu_time;

        let elapsed_secs = elapsed.as_secs_f64().max(0.001);
        let capacity = elapsed_secs * f64::from(self.num_cpus);
        Ok((delta / capacity) * 100.0)
    }
}

fn process_cpu_time(pid: u32) -> Result<f64> {
    // ps time is elapsed CPU seconds (user+sys)
    let output = Command::new("ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .context("ps time")?;
    if !output.status.success() {
        return Ok(0.0);
    }
    parse_ps_time(String::from_utf8_lossy(&output.stdout).trim())
}

fn parse_ps_time(raw: &str) -> Result<f64> {
    let parts: Vec<&str> = raw.split(':').collect();
    match parts.len() {
        2 => {
            let mins: f64 = parts[0].parse().unwrap_or(0.0);
            let secs: f64 = parts[1].parse().unwrap_or(0.0);
            Ok(mins * 60.0 + secs)
        }
        3 => {
            let hours: f64 = parts[0].parse().unwrap_or(0.0);
            let mins: f64 = parts[1].parse().unwrap_or(0.0);
            let secs: f64 = parts[2].parse().unwrap_or(0.0);
            Ok(hours * 3600.0 + mins * 60.0 + secs)
        }
        _ => Ok(0.0),
    }
}

fn set_rlimit_as(bytes: u64) -> std::io::Result<()> {
    let rlim = libc::rlimit {
        rlim_cur: bytes,
        rlim_max: bytes,
    };
    let rc = unsafe { libc::setrlimit(libc::RLIMIT_AS, &rlim) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
