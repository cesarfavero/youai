use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tracing::{error, info, warn};
use youai_common::{
    clear_runtime_state, compute::parse_ram_max_mb, is_process_alive, load_runtime_state,
    save_config, save_runtime_state, signing, worker_health_url, worker_url, HeartbeatRequest,
    NodeConfig, RegisterNodeRequest, RegisterNodeResponse, RuntimeState, HEARTBEAT_INTERVAL_SECS,
    PIPELINE_KIND_ACTIVATION,
};

pub struct NodeRuntime {
    config: NodeConfig,
    worker_child: Child,
    pub worker_url: String,
    coordinator_url: String,
    http: Client,
}

impl NodeRuntime {
    pub async fn start(mut config: NodeConfig) -> Result<Self> {
        if let Some(state) = load_runtime_state()? {
            if is_process_alive(state.pid) {
                anyhow::bail!(
                    "node already running (pid {}). Run: youai-node pause",
                    state.pid
                );
            }
            let _ = clear_runtime_state();
        }

        let model_path = youai_common::resolve_model_path(&config)?;
        crate::registry_verify::verify_model_against_manifest(
            &model_path,
            &config.model.name,
        )?;
        let worker_url = worker_url(&config.worker_host, config.worker_port);
        let coordinator_url = config.coordinator_url.clone();

        info!(
            worker = %worker_url,
            coordinator = %coordinator_url,
            model = %model_path.display(),
            ram_max = %config.resources.ram_max,
            cpu_percent = config.resources.cpu_percent,
            "spawning worker under guard"
        );

        let worker_child = spawn_worker_child(&config, &model_path)?;
        wait_for_worker_health(&worker_health_url(&config.worker_host, config.worker_port)).await?;

        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("build HTTP client")?;

        let advertise_url = config
            .worker_advertise_url
            .clone()
            .unwrap_or_else(|| worker_url.clone());
        let registration =
            register_or_reuse(&http, &coordinator_url, &config, &advertise_url).await?;
        config.node.node_id = Some(registration.node_id.clone());
        config.node.token = Some(registration.token.clone());
        save_config(&config)?;

        let runtime = Self {
            config,
            worker_child,
            worker_url,
            coordinator_url,
            http,
        };

        runtime.persist_state()?;
        runtime.send_heartbeat().await?;
        Ok(runtime)
    }

    fn persist_state(&self) -> Result<()> {
        save_runtime_state(&RuntimeState {
            pid: self.worker_child.id(),
            worker_url: self.worker_url.clone(),
            coordinator_url: self.coordinator_url.clone(),
            started_at: Utc::now().timestamp(),
        })
    }

    pub async fn run_until_stopped(mut self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(err) = self.send_heartbeat().await {
                        warn!(error = %err, "heartbeat failed");
                    }
                    if let Ok(Some(status)) = self.worker_child.try_wait() {
                        error!(?status, "worker exited unexpectedly — restarting");
                        if let Err(err) = self.restart_worker().await {
                            let _ = clear_runtime_state();
                            anyhow::bail!("worker restart failed: {err}");
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("received Ctrl+C — stopping node");
                    self.stop_worker()?;
                    let _ = clear_runtime_state();
                    return Ok(());
                }
            }
        }
    }

    async fn send_heartbeat(&self) -> Result<()> {
        let node_id = self
            .config
            .node
            .node_id
            .as_ref()
            .context("missing node_id in config")?;
        let token = self
            .config
            .node
            .token
            .as_ref()
            .context("missing token in config")?;

        let path = "/api/v1/nodes/heartbeat";
        let url = format!(
            "{}{}",
            self.coordinator_url.trim_end_matches('/'),
            path
        );
        let issued_at = signing::unix_now();
        let nonce = signing::fresh_nonce();
        let body = HeartbeatRequest {
            node_id: node_id.clone(),
            issued_at,
            nonce: nonce.clone(),
        };
        let body_json = serde_json::to_value(&body).context("serialize heartbeat")?;
        let signed = signing::signed_headers(token, "POST", path, &body_json);
        let mut req = self.http.post(&url).header("x-youai-token", token);
        for (name, value) in signed {
            req = req.header(name, value);
        }
        let response = req.json(&body).send().await.with_context(|| format!("POST {url}"))?;

        if response.status().is_success() {
            info!(node_id = %node_id, "heartbeat ok");
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("heartbeat failed ({status}): {body}");
        }
    }

    fn stop_worker(&mut self) -> Result<()> {
        let pid = self.worker_child.id();
        info!(pid, "stopping worker");
        let _ = self.worker_child.kill();
        let _ = self.worker_child.wait();
        Ok(())
    }

    async fn restart_worker(&mut self) -> Result<()> {
        let model_path = youai_common::resolve_model_path(&self.config)?;
        cleanup_orphan_pipeline_daemons(&self.config, &model_path);
        let _ = self.worker_child.kill();
        let _ = self.worker_child.wait();

        tokio::time::sleep(Duration::from_secs(2)).await;

        self.worker_child = spawn_worker_child(&self.config, &model_path)?;
        wait_for_worker_health(&worker_health_url(
            &self.config.worker_host,
            self.config.worker_port,
        ))
        .await?;
        self.persist_state()?;
        info!("worker restarted");
        Ok(())
    }
}

fn spawn_worker_child(config: &NodeConfig, model_path: &std::path::Path) -> Result<Child> {
    let guard_bin = resolve_binary("youai-guard")?;
    let worker_bin = resolve_binary("youai-worker")?;
    let llama_cli = youai_common::resolve_llama_cli(config)?;

    let mut worker_cmd = Command::new(&guard_bin);
    worker_cmd
        .arg("run")
        .arg("--ram-max")
        .arg(&config.resources.ram_max)
        .arg("--cpu-percent")
        .arg(config.resources.cpu_percent.to_string())
        .arg("--")
        .arg(&worker_bin)
        .args([
            "serve",
            "--host",
            &config.worker_host,
            "--port",
            &config.worker_port.to_string(),
            "--model",
            model_path.to_str().context("model path is not UTF-8")?,
            "--llama-cli",
            llama_cli.to_str().context("llama-cli path is not UTF-8")?,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Ok(dir) = std::env::var("YOUAI_BIN_DIR") {
        worker_cmd.env("YOUAI_BIN_DIR", dir);
    }
    worker_cmd
        .spawn()
        .with_context(|| format!("spawn {} -> {}", guard_bin.display(), worker_bin.display()))
}

fn cleanup_orphan_pipeline_daemons(config: &NodeConfig, model_path: &std::path::Path) {
    if config.shard.pipeline_kind != PIPELINE_KIND_ACTIVATION {
        return;
    }
    let Some(model) = model_path.to_str() else {
        return;
    };
    let pattern = format!("youai-pipeline-step.*{model}");
    let _ = Command::new("pkill")
        .args(["-f", &pattern])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn pause_running_node() -> Result<()> {
    let state = load_runtime_state()?.context("node is not running")?;
    if !is_process_alive(state.pid) {
        let _ = clear_runtime_state();
        anyhow::bail!("stale runtime state — worker is not running");
    }

    let pid = state.pid as i32;
    let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::ESRCH) {
            return Err(err.into());
        }
    }

    std::thread::sleep(Duration::from_millis(500));
    if is_process_alive(state.pid) {
        let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    }

    clear_runtime_state()?;
    info!(pid = state.pid, "node paused");
    Ok(())
}

pub async fn show_status(config: &NodeConfig) -> Result<()> {
    let runtime = load_runtime_state()?;
    let worker_alive = runtime
        .as_ref()
        .map(|s| is_process_alive(s.pid))
        .unwrap_or(false);

    println!("YouAI Node status");
    println!("  name:          {}", config.name);
    println!("  coordinator:   {}", config.coordinator_url);
    println!(
        "  worker:        {}",
        worker_url(&config.worker_host, config.worker_port)
    );
    println!("  model:         {}", config.model.name);
    println!(
        "  resources:     cpu {}% · ram {}",
        config.resources.cpu_percent, config.resources.ram_max
    );
    println!(
        "  registered:    {}",
        config.node.node_id.as_deref().unwrap_or("(not yet)")
    );
    if let Some(rpc) = &config.rpc_url {
        println!("  rpc_url:       {rpc}");
    }
    println!(
        "  running:       {}",
        if worker_alive { "yes" } else { "no" }
    );

    if let Some(state) = runtime {
        println!("  worker pid:    {}", state.pid);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("build HTTP client")?;

    let nodes_url = format!(
        "{}/api/v1/nodes",
        config.coordinator_url.trim_end_matches('/')
    );
    match client.get(&nodes_url).send().await {
        Ok(response) if response.status().is_success() => {
            let body: youai_common::NodesResponse = response.json().await?;
            let online = body.nodes.iter().filter(|n| n.online).count();
            println!(
                "  cluster:       {online}/{} nodes online",
                body.nodes.len()
            );
            for node in body.nodes {
                let mark = if node.online { "online" } else { "offline" };
                println!("    - {} ({}) · {}", node.name, mark, node.worker_url);
            }
        }
        Ok(response) => {
            println!(
                "  cluster:       coordinator returned {}",
                response.status()
            );
        }
        Err(err) => {
            println!("  cluster:       unreachable ({err})");
        }
    }

    Ok(())
}

async fn wait_for_worker_health(worker_url: &str) -> Result<()> {
    let health_url = format!("{}/health", worker_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("build HTTP client")?;

    for attempt in 1..=30 {
        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => {
                info!(attempt, %health_url, "worker is healthy");
                return Ok(());
            }
            Ok(response) => {
                warn!(attempt, status = %response.status(), "worker health not ready");
            }
            Err(err) => {
                warn!(attempt, error = %err, "worker health check failed");
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    anyhow::bail!("worker did not become healthy at {health_url}");
}

async fn register_or_reuse(
    http: &Client,
    coordinator_url: &str,
    config: &NodeConfig,
    worker_url: &str,
) -> Result<RegisterNodeResponse> {
    let url = format!(
        "{}/api/v1/nodes/register",
        coordinator_url.trim_end_matches('/')
    );

    let ram_max_mb = parse_ram_max_mb(&config.resources.ram_max).unwrap_or(2048);
    let response = http
        .post(&url)
        .json(&RegisterNodeRequest {
            name: config.name.clone(),
            region: config.region.clone(),
            worker_url: worker_url.to_string(),
            model: config.model.name.clone(),
            shard_group: config.shard.group.clone(),
            shard_stage: config.shard.stage,
            shard_total_stages: config.shard.total_stages,
            rpc_url: config.rpc_url.clone().unwrap_or_default(),
            gguf_shard_index: config.shard.gguf_shard_index,
            gguf_shard_total: config.shard.gguf_shard_total,
            pipeline_kind: config.shard.pipeline_kind.clone(),
            cpu_percent: config.resources.cpu_percent,
            ram_max_mb,
        })
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("register failed ({status}): {body}");
    }

    response
        .json::<RegisterNodeResponse>()
        .await
        .context("parse register response")
}

fn resolve_binary(name: &str) -> Result<PathBuf> {
    if let Ok(path) = std::env::var("YOUAI_BIN_DIR") {
        let candidate = PathBuf::from(path).join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    let local = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join(name)));
    if let Some(candidate) = local {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .with_context(|| format!("resolve {name} binary"))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    anyhow::bail!(
        "{name} not found in PATH. Build with `cargo build --release` and add target/release to PATH, or set YOUAI_BIN_DIR"
    );
}
