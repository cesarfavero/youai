use crate::limits::ResourceLimits;
use crate::monitor::{UsageSampler, Watchdog, WatchdogOutcome};
use anyhow::{bail, Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tracing::{info, warn};

const CGROUP_BASE: &str = "/sys/fs/cgroup";

pub struct CgroupGuard {
    path: PathBuf,
}

impl CgroupGuard {
    pub fn create(name: &str, limits: ResourceLimits) -> Result<Self> {
        let path = PathBuf::from(CGROUP_BASE).join(name);
        if path.exists() {
            Self::cleanup_tree(&path)?;
        }
        fs::create_dir_all(&path).with_context(|| format!("create cgroup {}", path.display()))?;

        enable_controllers(&path)?;
        write_limit(&path.join("memory.max"), limits.ram_max_bytes)?;
        write_cpu_max(&path, limits.cpu_percent)?;

        info!(
            cgroup = %path.display(),
            ram_max_bytes = limits.ram_max_bytes,
            cpu_percent = limits.cpu_percent,
            "cgroup v2 limits applied"
        );

        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn cleanup_tree(path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        // Kill processes still in cgroup
        let procs = path.join("cgroup.procs");
        if procs.exists() {
            if let Ok(content) = fs::read_to_string(&procs) {
                for line in content.lines() {
                    if let Ok(pid) = line.trim().parse::<i32>() {
                        if pid > 0 {
                            let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
                        }
                    }
                }
            }
        }
        // Remove nested cgroups first (best effort)
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let _ = Self::cleanup_tree(&p);
                }
            }
        }
        let _ = fs::remove_dir(path);
        Ok(())
    }
}

impl Drop for CgroupGuard {
    fn drop(&mut self) {
        if let Err(err) = Self::cleanup_tree(&self.path) {
            warn!(error = %err, "failed to cleanup cgroup");
        }
    }
}

fn enable_controllers(cgroup_path: &Path) -> Result<()> {
    let parent = cgroup_path.parent().context("cgroup has no parent")?;

    let subtree = parent.join("cgroup.subtree_control");
    if subtree.exists() {
        let controllers = fs::read_to_string(parent.join("cgroup.controllers")).unwrap_or_default();
        let mut to_enable = Vec::new();
        if controllers.contains("memory") {
            to_enable.push("+memory");
        }
        if controllers.contains("cpu") {
            to_enable.push("+cpu");
        }
        if !to_enable.is_empty() {
            let value = to_enable.join(" ");
            let result = OpenOptions::new()
                .write(true)
                .open(&subtree)
                .and_then(|mut f| f.write_all(value.as_bytes()));
            if let Err(err) = result {
                warn!(
                    error = %err,
                    "could not enable cgroup controllers on parent — may need sudo or user delegation"
                );
            }
        }
    }

    Ok(())
}

fn write_limit(path: &Path, value: u64) -> Result<()> {
    fs::write(path, value.to_string()).with_context(|| format!("write {}", path.display()))
}

fn write_cpu_max(cgroup_path: &Path, cpu_percent: u8) -> Result<()> {
    // cpu.max format: QUOTA PERIOD (microseconds). Period = 100ms.
    let period = 100_000u64;
    let quota = (period * u64::from(cpu_percent)) / 100;
    let value = format!("{quota} {period}");
    fs::write(cgroup_path.join("cpu.max"), value)
        .with_context(|| format!("write {}", cgroup_path.join("cpu.max").display()))
}

pub struct CgroupSampler {
    cgroup_path: PathBuf,
    last_cpu_usec: u64,
    num_cpus: u32,
}

impl CgroupSampler {
    pub fn new(cgroup_path: PathBuf) -> Result<Self> {
        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        let last_cpu_usec = read_cpu_usage_usec(&cgroup_path)?;
        Ok(Self {
            cgroup_path,
            last_cpu_usec,
            num_cpus,
        })
    }
}

impl UsageSampler for CgroupSampler {
    fn sample_memory_bytes(&self) -> Result<u64> {
        let raw = fs::read_to_string(self.cgroup_path.join("memory.current"))
            .with_context(|| "read memory.current")?;
        let value = raw.trim().parse::<u64>().unwrap_or(0);
        Ok(value)
    }

    fn sample_cpu_usage_percent(&mut self, elapsed: Duration) -> Result<f64> {
        let usage = read_cpu_usage_usec(&self.cgroup_path)?;
        let delta = usage.saturating_sub(self.last_cpu_usec);
        self.last_cpu_usec = usage;

        let elapsed_usec = elapsed.as_micros().max(1) as u64;
        let capacity = elapsed_usec.saturating_mul(u64::from(self.num_cpus));
        if capacity == 0 {
            return Ok(0.0);
        }
        Ok((delta as f64 / capacity as f64) * 100.0)
    }
}

fn read_cpu_usage_usec(cgroup_path: &Path) -> Result<u64> {
    let stat = fs::read_to_string(cgroup_path.join("cpu.stat")).with_context(|| "read cpu.stat")?;
    for line in stat.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() == Some("usage_usec") {
            if let Some(value) = parts.next() {
                return Ok(value.parse().unwrap_or(0));
            }
        }
    }
    Ok(0)
}

pub fn spawn_in_cgroup(
    cgroup: &CgroupGuard,
    command: &[String],
    ram_max_bytes: u64,
) -> Result<Child> {
    if command.is_empty() {
        bail!("empty command");
    }

    let program = &command[0];
    let args = &command[1..];
    let procs_path = cgroup.path().join("cgroup.procs");
    let procs = procs_path
        .to_str()
        .context("cgroup.procs path is not valid UTF-8")?
        .to_owned();

    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    unsafe {
        cmd.pre_exec(move || {
            let mut file = OpenOptions::new()
                .write(true)
                .open(&procs)
                .map_err(std::io::Error::other)?;
            writeln!(file, "{}", std::process::id())?;
            let rlim = libc::rlimit {
                rlim_cur: ram_max_bytes,
                rlim_max: ram_max_bytes,
            };
            // Best-effort; cgroup memory.max is the primary enforcer on Linux.
            let _ = libc::setrlimit(libc::RLIMIT_AS, &rlim);
            Ok(())
        });
    }

    cmd.spawn().with_context(|| format!("spawn {program}"))
}

pub fn run_command(command: &[String], limits: ResourceLimits) -> Result<RunOutcome> {
    let name = format!("youai-guard-{}", std::process::id());
    match CgroupGuard::create(&name, limits) {
        Ok(cgroup) => run_with_cgroup(&cgroup, command, limits),
        Err(err) => {
            warn!(
                error = %err,
                "cgroup v2 unavailable — falling back to setrlimit + ps watchdog (best-effort)"
            );
            run_without_cgroup(command, limits)
        }
    }
}

fn run_with_cgroup(
    cgroup: &CgroupGuard,
    command: &[String],
    limits: ResourceLimits,
) -> Result<RunOutcome> {
    let mut child = spawn_in_cgroup(cgroup, command, limits.ram_max_bytes)?;
    let pid = child.id();

    info!(pid, command = ?command, "child started under cgroup");

    let mut sampler = CgroupSampler::new(cgroup.path().to_path_buf())?;
    supervise_child(&mut child, pid, limits, &mut sampler)
}

fn run_without_cgroup(command: &[String], limits: ResourceLimits) -> Result<RunOutcome> {
    if command.is_empty() {
        bail!("empty command");
    }

    let program = &command[0];
    let args = &command[1..];
    let ram_bytes = limits.ram_max_bytes;

    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    unsafe {
        cmd.pre_exec(move || {
            let rlim = libc::rlimit {
                rlim_cur: ram_bytes,
                rlim_max: ram_bytes,
            };
            let _ = libc::setrlimit(libc::RLIMIT_AS, &rlim);
            Ok(())
        });
    }

    let mut child = cmd.spawn().with_context(|| format!("spawn {program}"))?;
    let pid = child.id();
    info!(pid, command = ?command, "child started without cgroup");

    let mut sampler = ProcessSampler::new(pid)?;
    supervise_child(&mut child, pid, limits, &mut sampler)
}

fn supervise_child(
    child: &mut Child,
    pid: u32,
    limits: ResourceLimits,
    sampler: &mut dyn UsageSampler,
) -> Result<RunOutcome> {
    let mut watchdog = Watchdog::new(limits, sampler);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::parse_limits;

    fn cgroup_available() -> bool {
        Path::new(CGROUP_BASE).join("cgroup.controllers").exists()
    }

    #[test]
    fn creates_cgroup_and_runs_true() {
        if !cgroup_available() {
            return;
        }

        let limits = parse_limits("64m", 50, 100).unwrap();
        let result = run_command(&["true".to_string()], limits);

        match result {
            Ok(RunOutcome::Exited(0)) => {}
            Err(err) => {
                // Permission errors are acceptable in CI without cgroup delegation
                let msg = err.to_string();
                assert!(
                    msg.contains("Permission denied") || msg.contains("cgroup"),
                    "unexpected error: {err}"
                );
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}
