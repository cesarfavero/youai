use crate::pipeline::{PipelineStepConfig, PipelineStepOutcome};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::warn;
use youai_common::PipelineStepRequest;

#[derive(Debug, Serialize)]
struct DaemonRequest<'a> {
    id: u64,
    session_dir: &'a str,
    op: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    prompt: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    activation_out: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    activation_in: &'a str,
    #[serde(skip_serializing_if = "is_zero")]
    token_id: u32,
    sample: bool,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

#[derive(Debug, Deserialize)]
struct DaemonResponse {
    ok: bool,
    #[serde(default)]
    op: String,
    #[serde(default)]
    n_past: i32,
    #[serde(default)]
    n_embd: i32,
    token_id: Option<u32>,
    text: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

pub struct PipelineDaemon {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl PipelineDaemon {
    pub fn spawn(config: &PipelineStepConfig) -> Result<Self> {
        let mut child = Command::new(&config.pipeline_step_bin)
            .arg("-m")
            .arg(&config.model_path)
            .arg("--daemon")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn daemon {}", config.pipeline_step_bin.display()))?;

        let stdin = child
            .stdin
            .take()
            .context("daemon stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("daemon stdout unavailable")?;
        let mut daemon = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        daemon.wait_ready(Duration::from_secs(120))?;
        Ok(daemon)
    }

    fn wait_ready(&mut self, timeout: Duration) -> Result<()> {
        let started = Instant::now();
        loop {
            if started.elapsed() > timeout {
                bail!("pipeline daemon ready timeout");
            }
            if let Some(resp) = self.read_json_line()? {
                if resp.ok && resp.op == "daemon-ready" {
                    return Ok(());
                }
                if !resp.ok {
                    bail!(
                        "pipeline daemon failed during startup: {}",
                        resp.error.unwrap_or_else(|| "unknown".to_string())
                    );
                }
            }
            if self.child.try_wait()?.is_some() {
                bail!("pipeline daemon exited during startup");
            }
        }
    }

    pub fn run_step(
        &mut self,
        _config: &PipelineStepConfig,
        req: &PipelineStepRequest,
        session_dir: &str,
        activation_out: &str,
        activation_in: &str,
        timeout: Duration,
    ) -> Result<PipelineStepOutcome> {
        let id = self.next_id;
        self.next_id += 1;

        let body = DaemonRequest {
            id,
            session_dir,
            op: &req.op,
            prompt: if req.op == "prefill-prompt" {
                req.prompt.as_str()
            } else {
                ""
            },
            activation_out: if req.op == "prefill-prompt" || req.op == "decode-token" {
                activation_out
            } else {
                ""
            },
            activation_in: if req.op == "forward-activation" {
                activation_in
            } else {
                ""
            },
            token_id: if req.op == "decode-token" {
                req.token_id
            } else {
                0
            },
            sample: req.sample,
        };

        let line = serde_json::to_string(&body).context("encode daemon request")?;
        writeln!(self.stdin, "{line}")
            .and_then(|_| self.stdin.flush())
            .context("write daemon request")?;

        let started = Instant::now();
        loop {
            if started.elapsed() > timeout {
                let _ = self.child.kill();
                bail!("pipeline daemon step timed out after {:?}", timeout);
            }
            if let Some(resp) = self.read_json_line()? {
                if resp.op == "daemon-ready" {
                    continue;
                }
                if !resp.ok {
                    return Err(anyhow::anyhow!(
                        resp.error.unwrap_or_else(|| "daemon step failed".to_string())
                    ));
                }
                return Ok(PipelineStepOutcome {
                    op: resp.op,
                    n_past: resp.n_past,
                    n_embd: resp.n_embd,
                    token_id: resp.token_id,
                    text: resp.text,
                    activation_b64: None,
                });
            }
            if self.child.try_wait()?.is_some() {
                bail!("pipeline daemon exited during step");
            }
        }
    }

    fn read_json_line(&mut self) -> Result<Option<DaemonResponse>> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .context("read daemon stdout")?;
        if line.is_empty() {
            return Ok(None);
        }
        let trimmed = line.trim();
        if !trimmed.starts_with('{') {
            return Ok(None);
        }
        let parsed: DaemonResponse =
            serde_json::from_str(trimmed).with_context(|| format!("parse daemon JSON: {trimmed}"))?;
        Ok(Some(parsed))
    }
}

impl Drop for PipelineDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn daemon_enabled() -> bool {
    !matches!(
        std::env::var("YOUAI_PIPELINE_DAEMON").as_deref(),
        Ok("0") | Ok("false")
    )
}

pub fn try_run_via_daemon(
    daemon: &mut Option<PipelineDaemon>,
    config: &PipelineStepConfig,
    req: &PipelineStepRequest,
    session_dir: &str,
    activation_out: &str,
    activation_in: &str,
) -> Result<Option<PipelineStepOutcome>> {
    if !daemon_enabled() {
        return Ok(None);
    }

    if daemon.is_none() {
        match PipelineDaemon::spawn(config) {
            Ok(d) => *daemon = Some(d),
            Err(err) => {
                warn!(error = %err, "pipeline daemon unavailable, using subprocess");
                return Ok(None);
            }
        }
    }

    let Some(d) = daemon.as_mut() else {
        return Ok(None);
    };

    match d.run_step(
        config,
        req,
        session_dir,
        activation_out,
        activation_in,
        config.timeout,
    ) {
        Ok(out) => Ok(Some(out)),
        Err(err) => {
            warn!(error = %err, "pipeline daemon step failed, falling back to subprocess");
            *daemon = None;
            Ok(None)
        }
    }
}