//! Shared types, defaults, and config paths for the YouAI workspace.

pub mod chat_template;
pub mod compute;
pub mod signing;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_COORDINATOR_PORT: u16 = 8080;
pub const DEFAULT_WORKER_PORT: u16 = 7741;
pub const DEFAULT_MODEL_NAME: &str = "smollm2-360m-instruct";
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;
pub const NODE_STALE_SECS: i64 = 90;

/// Default tiny model for 2-node dogfood (SmolLM2-360M-Instruct Q4_K_M, ~220 MB).
pub const DEFAULT_MODEL_FILENAME: &str = "smollm2-360m-instruct-q4_k_m.gguf";

/// Pipeline shard group for multi-machine single-request inference.
pub const DEFAULT_PIPELINE_GROUP: &str = "default-pipeline";

/// Default llama.cpp RPC server port (ggml tensor offload).
pub const DEFAULT_RPC_PORT: u16 = 50052;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    #[serde(default = "default_node_name")]
    pub name: String,
    #[serde(default)]
    pub region: String,
    #[serde(default = "default_coordinator_url")]
    pub coordinator_url: String,
    #[serde(default = "default_worker_host")]
    pub worker_host: String,
    #[serde(default = "default_worker_port")]
    pub worker_port: u16,
    /// URL sent to coordinator (use when worker bind host differs from reachable address).
    #[serde(default)]
    pub worker_advertise_url: Option<String>,
    #[serde(default)]
    pub resources: ResourceConfig,
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub node: PersistedNodeState,
    #[serde(default)]
    pub shard: ShardConfig,
    /// This node's llama.cpp RPC listen/advertise address (stage 1+ backends).
    #[serde(default)]
    pub rpc_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    /// Pipeline group id (empty = replica-only node).
    #[serde(default)]
    pub group: String,
    /// Zero-based stage index in the pipeline.
    #[serde(default)]
    pub stage: u8,
    /// Total stages in this pipeline (1 = no pipeline).
    #[serde(default = "default_shard_total_stages")]
    pub total_stages: u8,
    /// GGUF split index (0-based) when using pipeline v2.
    #[serde(default)]
    pub gguf_shard_index: u16,
    /// Total GGUF splits for this model (1 = full unsplit model).
    #[serde(default = "default_gguf_shard_total")]
    pub gguf_shard_total: u16,
    /// Pipeline backend: "" (auto), "gguf", or "activation" (v3).
    #[serde(default)]
    pub pipeline_kind: String,
}

/// Pipeline v3: pass activations between stages (no full GGUF assembly on stage 0).
pub const PIPELINE_KIND_ACTIVATION: &str = "activation";

fn default_gguf_shard_total() -> u16 {
    1
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            group: String::new(),
            stage: 0,
            total_stages: default_shard_total_stages(),
            gguf_shard_index: 0,
            gguf_shard_total: default_gguf_shard_total(),
            pipeline_kind: String::new(),
        }
    }
}

fn default_shard_total_stages() -> u8 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedNodeState {
    pub node_id: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    #[serde(default = "default_cpu_percent")]
    pub cpu_percent: u8,
    #[serde(default = "default_ram_max")]
    pub ram_max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    #[serde(default = "default_model_name")]
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub llama_cli: Option<String>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            name: default_node_name(),
            region: String::new(),
            coordinator_url: default_coordinator_url(),
            worker_host: default_worker_host(),
            worker_port: default_worker_port(),
            worker_advertise_url: None,
            resources: ResourceConfig::default(),
            model: ModelConfig::default(),
            runtime: RuntimeConfig::default(),
            node: PersistedNodeState::default(),
            shard: ShardConfig::default(),
            rpc_url: None,
        }
    }
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            cpu_percent: default_cpu_percent(),
            ram_max: default_ram_max(),
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            name: default_model_name(),
            path: None,
        }
    }
}

fn default_node_name() -> String {
    std::env::var("YOUAI_NODE_NAME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "youai-node".to_string())
}

fn default_coordinator_url() -> String {
    format!("http://127.0.0.1:{DEFAULT_COORDINATOR_PORT}")
}

fn default_worker_host() -> String {
    "127.0.0.1".to_string()
}

fn default_worker_port() -> u16 {
    DEFAULT_WORKER_PORT
}

fn default_cpu_percent() -> u8 {
    30
}

fn default_ram_max() -> String {
    "2g".to_string()
}

fn default_model_name() -> String {
    DEFAULT_MODEL_NAME.to_string()
}

pub fn youai_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not resolve home directory")?;
    Ok(home.join(".youai"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(youai_dir()?.join("config.toml"))
}

pub fn models_dir() -> Result<PathBuf> {
    Ok(youai_dir()?.join("models"))
}

pub fn shards_dir() -> Result<PathBuf> {
    Ok(youai_dir()?.join("shards"))
}

pub fn default_model_path() -> Result<PathBuf> {
    Ok(models_dir()?.join(DEFAULT_MODEL_FILENAME))
}

pub fn runtime_state_path() -> Result<PathBuf> {
    Ok(youai_dir()?.join("node.runtime.json"))
}

pub fn load_config() -> Result<NodeConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(NodeConfig::default());
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let config: NodeConfig = toml::from_str(&raw).context("parse config.toml")?;
    Ok(config)
}

pub fn save_config(config: &NodeConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let raw = toml::to_string_pretty(config).context("serialize config.toml")?;
    std::fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn resolve_model_path(config: &NodeConfig) -> Result<PathBuf> {
    if let Some(path) = &config.model.path {
        let path = expand_tilde(path);
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!("model file not found: {}", path.display());
    }
    let default = default_model_path()?;
    if default.exists() {
        return Ok(default);
    }
    anyhow::bail!(
        "model not found at {}. Run: ./scripts/download-model.sh",
        default.display()
    );
}

pub fn resolve_llama_cli(config: &NodeConfig) -> Result<PathBuf> {
    if let Some(path) = &config.runtime.llama_cli {
        let path = expand_tilde(path);
        if path.is_file() {
            return Ok(path);
        }
        anyhow::bail!("llama-cli not found: {}", path.display());
    }

    let candidates = [
        youai_dir()?.join("llama.cpp/build/bin/llama-completion"),
        youai_dir()?.join("llama.cpp/build/bin/llama-cli"),
        youai_dir()?.join("llama.cpp/build/bin/llama"),
        PathBuf::from("/usr/local/bin/llama-completion"),
        PathBuf::from("/usr/local/bin/llama-cli"),
    ];

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    for name in ["llama-completion", "llama-cli"] {
        if let Ok(path) = which_binary(name) {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "llama-completion not found. Run: ./scripts/setup-llama.sh or set runtime.llama_cli in ~/.youai/config.toml"
    );
}

pub fn resolve_rpc_server() -> Result<PathBuf> {
    let candidates = [
        youai_dir()?.join("llama.cpp/build/bin/rpc-server"),
        PathBuf::from("/usr/local/bin/rpc-server"),
    ];

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    if let Ok(path) = which_binary("rpc-server") {
        return Ok(path);
    }

    anyhow::bail!("rpc-server not found. Rebuild llama.cpp with RPC: ./scripts/setup-llama.sh");
}

/// Build sibling path for a llama.cpp multi-part GGUF (e.g. `*-00002-of-00002.gguf`).
pub fn gguf_split_sibling_path(
    local: &std::path::Path,
    target_index: u16,
    total: u16,
) -> Result<PathBuf> {
    let (prefix, _, file_total) = parse_gguf_split_parts(local)?;
    if file_total != total {
        anyhow::bail!("split total mismatch: file has {file_total}, expected {total}");
    }
    let file_idx = target_index.saturating_add(1);
    let sibling = format!("{prefix}-{file_idx:05}-of-{total:05}.gguf");
    Ok(local.with_file_name(sibling))
}

/// Zero-based GGUF split index (0 = `*-00001-of-*.gguf`).
pub fn parse_gguf_split_filename(path: &std::path::Path) -> Option<(u16, u16)> {
    let (_, file_idx, total) = parse_gguf_split_parts(path).ok()?;
    Some((file_idx.saturating_sub(1), total))
}

fn parse_gguf_split_parts(path: &std::path::Path) -> Result<(String, u16, u16)> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("model path has no filename")?;
    let rest = name
        .strip_suffix(".gguf")
        .context("model file must end with .gguf")?;
    let (left, total_str) = rest
        .rsplit_once("-of-")
        .context("model is not a GGUF split (*-NNNNN-of-MMMMM.gguf)")?;
    let (prefix, index_str) = left
        .rsplit_once('-')
        .context("invalid GGUF split index in filename")?;
    let index: u16 = index_str.parse().context("invalid GGUF split index")?;
    let total: u16 = total_str.parse().context("invalid GGUF split count")?;
    Ok((prefix.to_string(), index, total))
}

fn which_binary(name: &str) -> Result<PathBuf> {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .with_context(|| format!("run which {name}"))?;
    if !output.status.success() {
        anyhow::bail!("{name} not in PATH");
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        anyhow::bail!("empty which output");
    }
    Ok(PathBuf::from(path))
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

pub fn worker_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}")
}

/// Host for local health checks (0.0.0.0 / :: are bind-all, not dialable).
pub fn worker_local_health_host(host: &str) -> &str {
    match host {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        other => other,
    }
}

pub fn worker_health_url(host: &str, port: u16) -> String {
    format!("http://{}:{port}", worker_local_health_host(host))
}

/// True when the node loads a complete model suitable for standalone `/infer` (replica chat).
pub fn is_replica_eligible(
    shard_total_stages: u8,
    gguf_shard_total: u16,
    pipeline_kind: &str,
) -> bool {
    if shard_total_stages > 1 {
        return false;
    }
    if gguf_shard_total > 1 {
        return false;
    }
    if pipeline_kind == PIPELINE_KIND_ACTIVATION {
        return false;
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoChatDispatch {
    Replica,
    Pipeline,
}

/// Auto mode: pipeline when a full shard chain is online (one model, many PCs);
/// else replica on whatever standalone workers are available.
pub fn resolve_auto_chat_dispatch(pipeline_chain_available: bool) -> AutoChatDispatch {
    if pipeline_chain_available {
        AutoChatDispatch::Pipeline
    } else {
        AutoChatDispatch::Replica
    }
}

// --- Coordinator API types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterNodeRequest {
    pub name: String,
    pub region: String,
    pub worker_url: String,
    pub model: String,
    #[serde(default)]
    pub shard_group: String,
    #[serde(default)]
    pub shard_stage: u8,
    #[serde(default = "default_shard_total_stages")]
    pub shard_total_stages: u8,
    #[serde(default)]
    pub rpc_url: String,
    #[serde(default)]
    pub gguf_shard_index: u16,
    #[serde(default = "default_gguf_shard_total")]
    pub gguf_shard_total: u16,
    #[serde(default)]
    pub pipeline_kind: String,
    /// CPU cap offered to the network (for compute-unit tiering).
    #[serde(default = "default_cpu_percent")]
    pub cpu_percent: u8,
    /// RAM cap in megabytes (parsed from config `ram_max`).
    #[serde(default = "default_ram_max_mb")]
    pub ram_max_mb: u32,
}

fn default_ram_max_mb() -> u32 {
    2048
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterNodeResponse {
    pub node_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub node_id: String,
    #[serde(default)]
    pub issued_at: i64,
    #[serde(default)]
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub name: String,
    pub region: String,
    pub worker_url: String,
    pub model: String,
    pub online: bool,
    pub last_heartbeat: i64,
    #[serde(default)]
    pub shard_group: String,
    #[serde(default)]
    pub shard_stage: u8,
    #[serde(default = "default_shard_total_stages")]
    pub shard_total_stages: u8,
    /// llama.cpp RPC endpoint (host:port) for tensor offload backends.
    #[serde(default)]
    pub rpc_url: String,
    #[serde(default)]
    pub gguf_shard_index: u16,
    #[serde(default = "default_gguf_shard_total")]
    pub gguf_shard_total: u16,
    #[serde(default)]
    pub pipeline_kind: String,
    #[serde(default = "default_cpu_percent")]
    pub cpu_percent: u8,
    #[serde(default = "default_ram_max_mb")]
    pub ram_max_mb: u32,
    #[serde(default)]
    pub compute_units: u64,
    #[serde(default)]
    pub contributor_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodesResponse {
    pub nodes: Vec<NodeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneNodesResponse {
    pub removed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub prompt: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// auto: pipeline when a full shard chain is online, else replica.
    #[serde(default)]
    pub mode: ChatRoutingMode,
}

/// Signed chat envelope (integrity + anti-replay).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedChatRequest {
    pub v: u8,
    pub id: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub nonce: String,
    pub body: ChatRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryTierResponse {
    pub active_tier: String,
    pub display_name: String,
    pub model_id: String,
    pub network_compute_units: u64,
    pub min_compute_units: u64,
    pub next_tier: Option<String>,
    pub next_tier_compute_needed: Option<u64>,
    pub pipeline_chains: u32,
    pub selection_basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkComputeResponse {
    pub total_compute_units: u64,
    pub online_nodes: u32,
    pub pipeline_chains: u32,
    pub active_tier: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatRoutingMode {
    #[default]
    Auto,
    Replica,
    Pipeline,
}

fn default_max_tokens() -> u32 {
    128
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub node_id: String,
    pub node_name: String,
    pub model: String,
    pub text: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub stages: Vec<ChatStageInfo>,
    #[serde(default)]
    pub cached: bool,
    #[serde(default)]
    pub active_tier: String,
    #[serde(default)]
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStageInfo {
    pub node_id: String,
    pub node_name: String,
    pub shard_stage: u8,
    pub partial_text: String,
}

// --- Worker API types ---

/// Remote GGUF shard hosted on another pipeline stage worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteShardSource {
    pub worker_url: String,
    pub gguf_shard_index: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferRequest {
    pub prompt: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Remote llama.cpp RPC servers (host:port) for real tensor split.
    #[serde(default)]
    pub rpc_servers: Vec<String>,
    /// Pipeline v2: fetch missing GGUF splits from peer workers before infer.
    #[serde(default)]
    pub remote_shards: Vec<RemoteShardSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferResponse {
    pub text: String,
    pub model: String,
}

/// Worker pipeline v3 step (activation passing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStepRequest {
    pub session_id: String,
    /// prefill-prompt | decode-token | forward-activation
    pub op: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub token_id: u32,
    /// Base64 f32 activation bytes (forward-activation input from coordinator).
    #[serde(default)]
    pub activation_b64: String,
    #[serde(default = "default_true")]
    pub sample: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineStepResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub op: String,
    #[serde(default)]
    pub n_past: i32,
    #[serde(default)]
    pub n_embd: i32,
    #[serde(default)]
    pub token_id: Option<u32>,
    #[serde(default)]
    pub text: Option<String>,
    /// Base64 f32 activation produced by this stage (for coordinator handoff).
    #[serde(default)]
    pub activation_b64: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub pid: u32,
    pub worker_url: String,
    pub coordinator_url: String,
    pub started_at: i64,
}

pub fn load_runtime_state() -> Result<Option<RuntimeState>> {
    let path = runtime_state_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let state: RuntimeState = serde_json::from_str(&raw).context("parse node.runtime.json")?;
    Ok(Some(state))
}

pub fn save_runtime_state(state: &RuntimeState) -> Result<()> {
    let path = runtime_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(state).context("serialize node.runtime.json")?;
    std::fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn clear_runtime_state() -> Result<()> {
    let path = runtime_state_path()?;
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        let rc = unsafe { libc::kill(pid as i32, 0) };
        if rc == 0 {
            return true;
        }
        let err = std::io::Error::last_os_error();
        err.raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrips_toml() {
        let config = NodeConfig::default();
        let raw = toml::to_string(&config).unwrap();
        let parsed: NodeConfig = toml::from_str(&raw).unwrap();
        assert_eq!(parsed.model.name, DEFAULT_MODEL_NAME);
    }

    #[test]
    fn auto_prefers_pipeline_when_chain_online() {
        assert_eq!(
            resolve_auto_chat_dispatch(true),
            AutoChatDispatch::Pipeline
        );
        assert_eq!(
            resolve_auto_chat_dispatch(false),
            AutoChatDispatch::Replica
        );
    }

    #[test]
    fn replica_eligible_only_for_standalone_full_gguf() {
        assert!(is_replica_eligible(1, 1, ""));
        assert!(!is_replica_eligible(2, 1, ""));
        assert!(!is_replica_eligible(1, 2, ""));
        assert!(!is_replica_eligible(1, 1, PIPELINE_KIND_ACTIVATION));
    }

    #[test]
    fn gguf_split_sibling_paths() {
        let local = PathBuf::from("/tmp/smollm2-360m-instruct-q4_k_m-00001-of-00002.gguf");
        assert_eq!(parse_gguf_split_filename(&local), Some((0, 2)));
        let s2 = gguf_split_sibling_path(&local, 1, 2).unwrap();
        assert_eq!(
            s2,
            PathBuf::from("/tmp/smollm2-360m-instruct-q4_k_m-00002-of-00002.gguf")
        );
    }
}
