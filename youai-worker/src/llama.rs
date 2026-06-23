use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct InferenceConfig {
    pub llama_cli: std::path::PathBuf,
    pub model_path: std::path::PathBuf,
    pub model_name: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub timeout: Duration,
    pub rpc_servers: Vec<String>,
}

pub fn run_inference(config: &InferenceConfig) -> Result<String> {
    if !config.llama_cli.is_file() {
        bail!("llama binary not found: {}", config.llama_cli.display());
    }
    if !config.model_path.is_file() {
        bail!("model not found: {}", config.model_path.display());
    }
    if config.prompt.trim().is_empty() {
        bail!("prompt is empty");
    }

    info!(
        binary = %config.llama_cli.display(),
        model = %config.model_path.display(),
        max_tokens = config.max_tokens,
        rpc_servers = ?config.rpc_servers,
        "running llama.cpp inference"
    );

    let mut cmd = Command::new(&config.llama_cli);
    cmd.args([
        "-m",
        config
            .model_path
            .to_str()
            .context("model path is not UTF-8")?,
        "-p",
        &config.prompt,
        "-n",
        &config.max_tokens.to_string(),
        "--temp",
        "0.7",
    ]);
    if !config.rpc_servers.is_empty() {
        cmd.arg("--rpc").arg(config.rpc_servers.join(","));
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let child = cmd
        .spawn()
        .with_context(|| format!("spawn {}", config.llama_cli.display()))?;

    let output = wait_with_timeout(child, config.timeout).context("llama.cpp execution failed")?;

    if !output.status.success() {
        bail!("llama.cpp exited with {}", output.status);
    }

    let text = extract_response_text(&output.stdout);
    if text.is_empty() {
        warn!("llama.cpp returned empty text");
    }
    Ok(text)
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output> {
    let started = std::time::Instant::now();
    loop {
        match child.try_wait().context("wait on llama.cpp")? {
            Some(_) => {
                return child.wait_with_output().context("collect llama.cpp output");
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("llama.cpp timed out after {:?}", timeout);
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn extract_response_text(stdout: &[u8]) -> String {
    let raw = String::from_utf8_lossy(stdout);
    if let Some(rest) = raw.split_once("assistant\n") {
        return rest
            .1
            .lines()
            .map(str::trim)
            .take_while(|line| {
                !line.starts_with('>') && !line.eq_ignore_ascii_case("user") && !line.is_empty()
            })
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
    }

    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("user"))
        .filter(|line| !line.starts_with('>'))
        .filter(|line| !line.contains("[end of text]"))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub fn default_timeout() -> Duration {
    Duration::from_secs(180)
}

pub fn validate_paths(llama_cli: &Path, model_path: &Path) -> Result<()> {
    if !llama_cli.is_file() {
        bail!("llama binary not found: {}", llama_cli.display());
    }
    if !model_path.is_file() {
        bail!("model not found: {}", model_path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_assistant_block() {
        let raw = "user\nSay hi\nassistant\nHello there\n> EOF by user\n";
        assert_eq!(extract_response_text(raw.as_bytes()), "Hello there");
    }
}
