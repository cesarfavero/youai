use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::info;
use youai_common::PipelineStepRequest;

#[derive(Debug, Deserialize)]
struct StepJson {
    ok: bool,
    #[serde(default)]
    op: String,
    #[serde(default)]
    n_past: i32,
    #[serde(default)]
    n_embd: i32,
    token_id: Option<u32>,
    text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineStepConfig {
    pub pipeline_step_bin: PathBuf,
    pub model_path: PathBuf,
    pub session_root: PathBuf,
    pub timeout: Duration,
}

pub struct PipelineStepOutcome {
    pub op: String,
    pub n_past: i32,
    pub n_embd: i32,
    pub token_id: Option<u32>,
    pub text: Option<String>,
    pub activation_b64: Option<String>,
}

pub fn run_pipeline_step(
    config: &PipelineStepConfig,
    req: &PipelineStepRequest,
    daemon: &mut Option<crate::pipeline_daemon::PipelineDaemon>,
) -> Result<PipelineStepOutcome> {
    if !config.pipeline_step_bin.is_file() {
        bail!(
            "youai-pipeline-step not found: {} (run ./scripts/build-pipeline-step.sh)",
            config.pipeline_step_bin.display()
        );
    }
    if !config.model_path.is_file() {
        bail!("model not found: {}", config.model_path.display());
    }

    let session_dir = config.session_root.join(&req.session_id).join("worker");
    fs::create_dir_all(&session_dir)
        .with_context(|| format!("create {}", session_dir.display()))?;

    let activation_out = session_dir.join("activation-out.bin");
    let activation_in = session_dir.join("activation-in.bin");
    let activation_out_str = activation_out.to_string_lossy().to_string();
    let activation_in_str = activation_in.to_string_lossy().to_string();
    let session_dir_str = session_dir.to_string_lossy().to_string();

    if req.op == "forward-activation" {
        if req.activation_b64.is_empty() {
            bail!("forward-activation requires activation_b64");
        }
        let bytes = STANDARD
            .decode(req.activation_b64.as_bytes())
            .context("decode activation_b64")?;
        fs::write(&activation_in, &bytes)
            .with_context(|| format!("write {}", activation_in.display()))?;
    }

    if let Some(mut outcome) = crate::pipeline_daemon::try_run_via_daemon(
        daemon,
        config,
        req,
        &session_dir_str,
        &activation_out_str,
        &activation_in_str,
    )? {
        if outcome.activation_b64.is_none() && activation_out.is_file() {
            let out_path = activation_out.display().to_string();
            let bytes = fs::read(&activation_out).with_context(|| format!("read {out_path}"))?;
            outcome.activation_b64 = Some(STANDARD.encode(bytes));
        }
    return Ok(outcome);
}

run_pipeline_step_subprocess(
        config,
        req,
        &session_dir,
        &activation_out,
        &activation_in,
    )
}

fn run_pipeline_step_subprocess(
    config: &PipelineStepConfig,
    req: &PipelineStepRequest,
    session_dir: &std::path::Path,
    activation_out: &std::path::Path,
    activation_in: &std::path::Path,
) -> Result<PipelineStepOutcome> {
    let mut cmd = Command::new(&config.pipeline_step_bin);
    cmd.arg("-m")
        .arg(&config.model_path)
        .arg("--session-dir")
        .arg(session_dir)
        .arg("--op")
        .arg(&req.op);

    match req.op.as_str() {
        "prefill-prompt" => {
            if req.prompt.trim().is_empty() {
                bail!("prefill-prompt requires prompt");
            }
            cmd.arg("-p")
                .arg(&req.prompt)
                .arg("--activation-out")
                .arg(activation_out);
        }
        "decode-token" => {
            cmd.arg("--token-id")
                .arg(req.token_id.to_string())
                .arg("--activation-out")
                .arg(activation_out);
        }
        "forward-activation" => {
            cmd.arg("--activation-in")
                .arg(activation_in)
                .arg("--sample")
                .arg(if req.sample { "1" } else { "0" });
        }
        other => bail!("unsupported pipeline op: {other}"),
    }

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    info!(
        op = %req.op,
        session = %req.session_id,
        model = %config.model_path.display(),
        "running pipeline step"
    );

    let output = run_with_timeout(cmd, config.timeout)?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!("pipeline-step exited with {}: {stdout}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_line = stdout
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{'))
        .ok_or_else(|| anyhow::anyhow!("pipeline-step produced no JSON output"))?;

    let parsed: StepJson = serde_json::from_str(json_line)
        .with_context(|| format!("parse pipeline JSON: {json_line}"))?;
    if !parsed.ok {
        bail!("pipeline-step returned ok=false");
    }

    let activation_b64 = if activation_out.is_file() {
        let bytes = fs::read(activation_out)
            .with_context(|| format!("read {}", activation_out.display()))?;
        Some(STANDARD.encode(bytes))
    } else {
        None
    };

    Ok(PipelineStepOutcome {
        op: parsed.op,
        n_past: parsed.n_past,
        n_embd: parsed.n_embd,
        token_id: parsed.token_id,
        text: parsed.text,
        activation_b64,
    })
}

fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<std::process::Output> {
    let mut child = cmd.spawn().context("spawn youai-pipeline-step")?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait().context("wait on pipeline-step")? {
            Some(_) => {
                return child
                    .wait_with_output()
                    .context("collect pipeline-step output")
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("pipeline-step timed out after {:?}", timeout);
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

pub fn default_pipeline_step_bin() -> PathBuf {
    if let Ok(path) = std::env::var("YOUAI_PIPELINE_STEP_BIN") {
        return PathBuf::from(path);
    }
    if let Ok(dir) = std::env::var("YOUAI_BIN_DIR") {
        let candidate = PathBuf::from(dir).join("youai-pipeline-step");
        if candidate.is_file() {
            return candidate;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("youai-pipeline-step");
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("youai-pipeline-step")
}

pub fn default_session_root() -> PathBuf {
    std::env::var_os("YOUAI_PIPELINE_SESSIONS")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("youai-pipeline-sessions"))
}
