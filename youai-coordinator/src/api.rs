use crate::auth::{self, NonceStore};
use crate::cache::ResponseCache;
use crate::gateway;
use crate::priority;
use crate::db::{Database, StoredNode};
use crate::pipeline::{run_pipeline, worker_is_healthy};
use crate::registry::{load_manifest, resolve_manifest_path, select_active_tier, RegistryManifest};
use anyhow::{Context, Result};
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use reqwest::Client;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};
use uuid::Uuid;
use youai_common::{
    chat_template::clean_assistant_response,
    compute::node_compute_units, signing, ChatRequest, ChatResponse, ChatRoutingMode,
    HeartbeatRequest, InferRequest, InferResponse, NetworkComputeResponse, NodesResponse,
    PruneNodesResponse, RegisterNodeRequest, RegisterNodeResponse, RegistryTierResponse,
    SignedChatRequest, DEFAULT_PIPELINE_GROUP,
};

const TOKEN_HEADER: &str = "x-youai-token";
const WORKER_HEALTH_TIMEOUT_SECS: u64 = 3;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub http: Client,
    pub health_http: Client,
    pub rr: Arc<AtomicUsize>,
    pub cache: Arc<Mutex<ResponseCache>>,
    pub nonces: NonceStore,
    pub manifest: Arc<RegistryManifest>,
}

pub async fn serve(host: &str, port: u16, db_path: &str) -> Result<()> {
    let db = Database::open(std::path::Path::new(db_path))?;
    {
        let removed = db.prune_nodes()?;
        if removed > 0 {
            info!(removed, "pruned stale/duplicate nodes on startup");
        }
    }

    let manifest_path = resolve_manifest_path();
    let manifest = load_manifest(&manifest_path)
        .with_context(|| format!("load registry from {}", manifest_path.display()))?;
    info!(
        path = %manifest_path.display(),
        tiers = manifest.tiers.len(),
        "registry manifest loaded"
    );

    if auth::dev_mode_enabled() {
        warn!("YOUAI_DEV_MODE=1 — HMAC verification disabled (dogfood only)");
    }

    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        http: Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .context("build HTTP client")?,
        health_http: Client::builder()
            .timeout(std::time::Duration::from_secs(WORKER_HEALTH_TIMEOUT_SECS))
            .build()
            .context("build health HTTP client")?,
        rr: Arc::new(AtomicUsize::new(0)),
        cache: Arc::new(Mutex::new(ResponseCache::new())),
        nonces: NonceStore::new(),
        manifest: Arc::new(manifest),
    };

    let app = Router::new()
        .route("/", get(chat_ui))
        .route("/health", get(health))
        .route("/api/v1/nodes/register", post(register_node))
        .route("/api/v1/nodes/heartbeat", post(heartbeat))
        .route("/api/v1/nodes/prune", post(prune_nodes))
        .route("/api/v1/nodes", get(list_nodes))
        .route("/api/v1/registry/manifest", get(registry_manifest))
        .route("/api/v1/registry/tier", get(registry_tier))
        .route("/api/v1/network/compute", get(network_compute))
        .route("/api/v1/gateway/upload", post(gateway::upload_not_ready))
        .route("/api/v1/gateway/url", post(gateway::url_not_ready))
        .route("/api/v1/gateway/search", post(gateway::search_not_ready))
        .route("/api/v1/chat", post(chat))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .context("invalid coordinator listen address")?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind coordinator on {addr}"))?;

    info!(%addr, db = %db_path, "youai-coordinator listening");
    axum::serve(listener, app)
        .await
        .context("coordinator server stopped")?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn chat_ui() -> impl IntoResponse {
    let mut resp = Html(include_str!("../../youai-web/public/index.html")).into_response();
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    resp
}

fn network_snapshot(
    state: &AppState,
) -> Result<(Vec<StoredNode>, u64, u32, crate::registry::TierSelection), AppError> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db lock poisoned"))?;
    let online = db
        .online_nodes()
        .map_err(|err| AppError::internal(err.to_string()))?;
    let network_cu = db.network_compute_units(&online);
    let chains = db.count_pipeline_chains(&online);
    let tier = select_active_tier(&state.manifest, network_cu, chains);
    Ok((online, network_cu, chains, tier))
}

async fn registry_manifest(State(_state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let path = resolve_manifest_path();
    let raw = std::fs::read_to_string(&path)
        .map_err(|err| AppError::internal(format!("read manifest: {err}")))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|err| AppError::internal(format!("parse manifest: {err}")))?;
    Ok(Json(value))
}

async fn registry_tier(State(state): State<AppState>) -> Result<Json<RegistryTierResponse>, AppError> {
    let (_, network_cu, chains, tier) = network_snapshot(&state)?;
    Ok(Json(RegistryTierResponse {
        active_tier: tier.active_tier.clone(),
        display_name: tier.display_name.clone(),
        model_id: tier.model_id.clone(),
        network_compute_units: network_cu,
        min_compute_units: tier.min_compute_units,
        next_tier: tier.next_tier.clone(),
        next_tier_compute_needed: tier
            .next_tier_min_cu
            .map(|next| next.saturating_sub(network_cu)),
        pipeline_chains: chains,
        selection_basis: "compute_units".to_string(),
    }))
}

async fn network_compute(State(state): State<AppState>) -> Result<Json<NetworkComputeResponse>, AppError> {
    let (online, network_cu, chains, tier) = network_snapshot(&state)?;
    Ok(Json(NetworkComputeResponse {
        total_compute_units: network_cu,
        online_nodes: online.len() as u32,
        pipeline_chains: chains,
        active_tier: tier.active_tier,
    }))
}

async fn register_node(
    State(state): State<AppState>,
    Json(body): Json<RegisterNodeRequest>,
) -> Result<Json<RegisterNodeResponse>, AppError> {
    if body.worker_url.trim().is_empty() {
        return Err(AppError::bad_request("worker_url is required"));
    }

    let now = Utc::now().timestamp();
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db lock poisoned"))?;

    let cpu_percent = body.cpu_percent;
    let ram_max_mb = body.ram_max_mb;

    if let Some(existing) = db
        .find_node_by_identity(&body.name, &body.worker_url)
        .map_err(|err| AppError::internal(err.to_string()))?
    {
        let updated = StoredNode {
            id: existing.id.clone(),
            token: existing.token.clone(),
            name: body.name,
            region: body.region,
            worker_url: body.worker_url.clone(),
            model: body.model,
            last_heartbeat: now,
            created_at: existing.created_at,
            shard_group: body.shard_group.clone(),
            shard_stage: body.shard_stage,
            shard_total_stages: body.shard_total_stages,
            rpc_url: body.rpc_url.clone(),
            gguf_shard_index: body.gguf_shard_index,
            gguf_shard_total: body.gguf_shard_total,
            pipeline_kind: body.pipeline_kind.clone(),
            cpu_percent,
            ram_max_mb,
            contributor_score: existing.contributor_score,
        };
        db.upsert_node(&updated)
            .map_err(|err| AppError::internal(err.to_string()))?;
        let _ = db
            .delete_nodes_by_worker_url_except(&body.worker_url, &existing.id)
            .map_err(|err| AppError::internal(err.to_string()))?;
        info!(
            node_id = %existing.id,
            compute_units = node_compute_units(cpu_percent, ram_max_mb),
            "node re-registered"
        );
        return Ok(Json(RegisterNodeResponse {
            node_id: existing.id,
            token: existing.token,
        }));
    }

    let node_id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let node = StoredNode {
        id: node_id.clone(),
        token: token.clone(),
        name: body.name,
        region: body.region,
        worker_url: body.worker_url.clone(),
        model: body.model,
        last_heartbeat: now,
        created_at: now,
        shard_group: body.shard_group,
        shard_stage: body.shard_stage,
        shard_total_stages: body.shard_total_stages,
        rpc_url: body.rpc_url,
        gguf_shard_index: body.gguf_shard_index,
        gguf_shard_total: body.gguf_shard_total,
        pipeline_kind: body.pipeline_kind,
        cpu_percent,
        ram_max_mb,
        contributor_score: 0.0,
    };

    db.upsert_node(&node)
        .map_err(|err| AppError::internal(err.to_string()))?;
    let _ = db
        .delete_nodes_by_worker_url_except(&body.worker_url, &node_id)
        .map_err(|err| AppError::internal(err.to_string()))?;

    info!(
        node_id = %node_id,
        compute_units = node_compute_units(cpu_percent, ram_max_mb),
        "node registered"
    );
    Ok(Json(RegisterNodeResponse { node_id, token }))
}

async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<HeartbeatRequest>,
) -> Result<StatusCode, AppError> {
    let token = headers
        .get(TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("missing x-youai-token"))?;

    let body_json = serde_json::to_value(&body).unwrap_or_default();
    {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::internal("db lock poisoned"))?;
        auth::verify_node_hmac(&headers, "POST", "/api/v1/nodes/heartbeat", &body_json, &db)
            .map_err(|err| AppError::unauthorized(err.to_string()))?;
    }

    if auth::require_signing() {
        let nonce = headers
            .get(signing::HEADER_NONCE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if !nonce.is_empty() && !state.nonces.check_and_insert(nonce) {
            return Err(AppError::conflict("replay detected"));
        }
    }

    let ok = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db lock poisoned"))?
        .heartbeat(&body.node_id, token)
        .map_err(|err| AppError::internal(err.to_string()))?;

    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::unauthorized("invalid node_id or token"))
    }
}

async fn prune_nodes(State(state): State<AppState>) -> Result<Json<PruneNodesResponse>, AppError> {
    let removed = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db lock poisoned"))?
        .prune_nodes()
        .map_err(|err| AppError::internal(err.to_string()))?;
    info!(removed, "nodes pruned");
    Ok(Json(PruneNodesResponse { removed }))
}

async fn list_nodes(State(state): State<AppState>) -> Result<Json<NodesResponse>, AppError> {
    let nodes = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db lock poisoned"))?
        .list_nodes()
        .map_err(|err| AppError::internal(err.to_string()))?;
    Ok(Json(NodesResponse { nodes }))
}

async fn chat(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ChatResponse>, AppError> {
    let (chat_body, contributor, priority_label, active_tier, contributor_score) =
        parse_chat_request(&state, &headers, &body)?;

    let _wait = priority::apply_chat_priority(contributor, contributor_score).await;

    if chat_body.prompt.trim().is_empty() {
        return Err(AppError::bad_request("prompt is required"));
    }

    let mode_str = match chat_body.mode {
        ChatRoutingMode::Auto => "auto",
        ChatRoutingMode::Replica => "replica",
        ChatRoutingMode::Pipeline => "pipeline",
    };
    let cache_key = ResponseCache::cache_key(
        &chat_body.prompt,
        chat_body.max_tokens,
        &active_tier,
        mode_str,
    );

    if let Ok(mut cache) = state.cache.lock() {
        if let Some(mut cached) = cache.get(&cache_key) {
            cached.active_tier = active_tier.clone();
            cached.priority = priority_label.clone();
            info!(tier = %active_tier, priority = %priority_label, "cache hit");
            return Ok(Json(cached));
        }
    }

    let online = {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::internal("db lock poisoned"))?;
        db.online_nodes()
            .map_err(|err| AppError::internal(err.to_string()))?
    };

    if online.is_empty() {
        return Err(AppError::service_unavailable(
            "no online nodes — start youai-node on at least one machine",
        ));
    }

    let pipeline_chain = {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::internal("db lock poisoned"))?;
        db.resolve_pipeline(&online, Some(DEFAULT_PIPELINE_GROUP))
    };

    let want_pipeline = matches!(chat_body.mode, ChatRoutingMode::Pipeline)
        || (chat_body.mode == ChatRoutingMode::Auto
            && youai_common::resolve_auto_chat_dispatch(pipeline_chain.is_some())
                == youai_common::AutoChatDispatch::Pipeline);

    let response = if want_pipeline {
        let pipeline = pipeline_chain.ok_or_else(|| {
            AppError::service_unavailable(
                "pipeline mode requested but no complete shard chain is online",
            )
        })?;
        chat_pipeline(&state, &pipeline, &chat_body.prompt, chat_body.max_tokens, &active_tier, &priority_label)
            .await?
    } else {
        chat_replica(
            &state,
            &online,
            &chat_body.prompt,
            chat_body.max_tokens,
            contributor,
            &active_tier,
            &priority_label,
        )
        .await?
    };

    let mut final_resp = response.0;
    final_resp.text = clean_assistant_response(&final_resp.text);

    if let Ok(mut cache) = state.cache.lock() {
        cache.put(cache_key, final_resp.clone(), contributor);
    }

    Ok(Json(final_resp))
}

fn parse_chat_request(
    state: &AppState,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<(ChatRequest, bool, String, String, f64), AppError> {
    let (_, _, _, tier_sel) = network_snapshot(state)?;
    let contributor_score = headers
        .get(TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|token| {
            state
                .db
                .lock()
                .ok()
                .and_then(|db| db.contributor_score_for_token(token).ok())
        })
        .unwrap_or(0.0);

    if let Ok(envelope) = serde_json::from_slice::<SignedChatRequest>(body) {
        let verified = {
            let db = state
                .db
                .lock()
                .map_err(|_| AppError::internal("db lock poisoned"))?;
            auth::verify_chat_hmac(
                headers,
                "/api/v1/chat",
                &envelope,
                &state.nonces,
                &db,
            )
            .map_err(|err| AppError::unauthorized(err.to_string()))?
        };
        let priority = if verified.contributor {
            "contributor".to_string()
        } else {
            "standard".to_string()
        };
        return Ok((
            envelope.body,
            verified.contributor,
            priority,
            tier_sel.active_tier,
            contributor_score,
        ));
    }

    if let Ok(plain) = serde_json::from_slice::<ChatRequest>(body) {
        if auth::require_signing() {
            let token = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok());
            if let Some(token) = token {
                let db = state.db.lock().map_err(|_| AppError::internal("db lock"))?;
                if db.node_id_for_token(token).ok().flatten().is_some() {
                    let score = db.contributor_score_for_token(token).unwrap_or(0.0);
                    let priority = if score > 10.0 {
                        "contributor-high".to_string()
                    } else {
                        "contributor".to_string()
                    };
                    return Ok((plain, true, priority, tier_sel.active_tier, score));
                }
            }
            return Err(AppError::unauthorized(
                "signed chat envelope or contributor token required",
            ));
        }
        return Ok((
            plain,
            false,
            "dev".to_string(),
            tier_sel.active_tier,
            contributor_score,
        ));
    }

    Err(AppError::bad_request("invalid chat request body"))
}

async fn chat_pipeline(
    state: &AppState,
    pipeline: &[StoredNode],
    prompt: &str,
    max_tokens: u32,
    active_tier: &str,
    priority: &str,
) -> Result<Json<ChatResponse>, AppError> {
    let (text, stages, model, mode) = run_pipeline(
        &state.http,
        &state.health_http,
        pipeline,
        prompt,
        max_tokens,
    )
    .await
    .map_err(AppError::bad_gateway)?;

    let last = pipeline.last().expect("pipeline has stages");
    info!(
        node_id = %last.id,
        stages = pipeline.len(),
        %mode,
        tier = %active_tier,
        "routed pipeline chat"
    );

    Ok(Json(ChatResponse {
        node_id: last.id.clone(),
        node_name: last.name.clone(),
        model,
        text,
        mode: mode.to_string(),
        stages,
        cached: false,
        active_tier: active_tier.to_string(),
        priority: priority.to_string(),
    }))
}

async fn chat_replica(
    state: &AppState,
    nodes: &[StoredNode],
    prompt: &str,
    max_tokens: u32,
    contributor: bool,
    active_tier: &str,
    priority: &str,
) -> Result<Json<ChatResponse>, AppError> {
    let eligible: Vec<&StoredNode> = nodes
        .iter()
        .filter(|node| {
            youai_common::is_replica_eligible(
                node.shard_total_stages,
                node.gguf_shard_total,
                &node.pipeline_kind,
            )
        })
        .collect();

    if eligible.is_empty() {
        return Err(AppError::service_unavailable(
            "no replica-capable nodes online — pipeline stage shards cannot serve replica chat; \
             configure a node with the full GGUF model (see scripts/setup-replica-mac.sh)",
        ));
    }

    let mut ordered: Vec<&StoredNode> = eligible;
    if contributor {
        ordered.sort_by(|a, b| {
            b.contributor_score
                .partial_cmp(&a.contributor_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let start = state.rr.fetch_add(1, Ordering::Relaxed);
    let mut last_err: Option<String> = None;

    for attempt in 0..ordered.len() {
        let idx = (start + attempt) % ordered.len();
        let node = ordered[idx];

        if !worker_is_healthy(&state.health_http, &node.worker_url).await {
            warn!(node_id = %node.id, worker = %node.worker_url, "skipping unhealthy worker");
            last_err = Some(format!("worker {} unreachable", node.worker_url));
            continue;
        }

        match run_infer(&state.http, node, prompt, max_tokens).await {
            Ok(infer) => {
                info!(node_id = %node.id, tier = %active_tier, "routed replica chat");
                return Ok(Json(ChatResponse {
                    node_id: node.id.clone(),
                    node_name: node.name.clone(),
                    model: infer.model,
                    text: infer.text,
                    mode: "replica".to_string(),
                    stages: vec![],
                    cached: false,
                    active_tier: active_tier.to_string(),
                    priority: priority.to_string(),
                }));
            }
            Err(err) => {
                warn!(node_id = %node.id, error = %err, "worker inference failed");
                last_err = Some(err);
            }
        }
    }

    Err(AppError::bad_gateway(
        last_err.unwrap_or_else(|| "all workers failed".to_string()),
    ))
}

async fn run_infer(
    client: &Client,
    node: &StoredNode,
    prompt: &str,
    max_tokens: u32,
) -> Result<InferResponse, String> {
    let infer_url = format!("{}/infer", node.worker_url.trim_end_matches('/'));
    let response = client
        .post(&infer_url)
        .json(&InferRequest {
            prompt: prompt.to_string(),
            max_tokens,
            rpc_servers: vec![],
            remote_shards: vec![],
        })
        .send()
        .await
        .map_err(|err| format!("worker request failed: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("worker {} returned {status}: {text}", node.id));
    }

    response
        .json::<InferResponse>()
        .await
        .map_err(|err| format!("invalid worker response: {err}"))
}

#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}