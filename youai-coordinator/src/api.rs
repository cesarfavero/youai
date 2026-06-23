use crate::db::{Database, StoredNode};
use crate::pipeline::{run_pipeline, worker_is_healthy};
use anyhow::{Context, Result};
use axum::{
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
    ChatRequest, ChatResponse, ChatRoutingMode, HeartbeatRequest, InferRequest, InferResponse,
    NodesResponse, PruneNodesResponse, RegisterNodeRequest, RegisterNodeResponse,
    DEFAULT_PIPELINE_GROUP,
};

const TOKEN_HEADER: &str = "x-youai-token";
const WORKER_HEALTH_TIMEOUT_SECS: u64 = 3;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub http: Client,
    pub health_http: Client,
    pub rr: Arc<AtomicUsize>,
}

pub async fn serve(host: &str, port: u16, db_path: &str) -> Result<()> {
    let db = Database::open(std::path::Path::new(db_path))?;
    {
        let removed = db.prune_nodes()?;
        if removed > 0 {
            info!(removed, "pruned stale/duplicate nodes on startup");
        }
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
    };

    let app = Router::new()
        .route("/", get(chat_ui))
        .route("/health", get(health))
        .route("/api/v1/nodes/register", post(register_node))
        .route("/api/v1/nodes/heartbeat", post(heartbeat))
        .route("/api/v1/nodes/prune", post(prune_nodes))
        .route("/api/v1/nodes", get(list_nodes))
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
        };
        db.upsert_node(&updated)
            .map_err(|err| AppError::internal(err.to_string()))?;
        let removed = db
            .delete_nodes_by_worker_url_except(&body.worker_url, &existing.id)
            .map_err(|err| AppError::internal(err.to_string()))?;
        if removed > 0 {
            info!(
                removed,
                worker = %body.worker_url,
                "removed duplicate registrations for worker_url"
            );
        }
        info!(
            node_id = %existing.id,
            name = %updated.name,
            worker = %updated.worker_url,
            "node re-registered (reused id)"
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
    };

    db.upsert_node(&node)
        .map_err(|err| AppError::internal(err.to_string()))?;
    let _ = db
        .delete_nodes_by_worker_url_except(&body.worker_url, &node_id)
        .map_err(|err| AppError::internal(err.to_string()))?;

    info!(node_id = %node_id, name = %node.name, worker = %node.worker_url, "node registered");
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
    Json(body): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    if body.prompt.trim().is_empty() {
        return Err(AppError::bad_request("prompt is required"));
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

    let want_pipeline = matches!(body.mode, ChatRoutingMode::Pipeline)
        || (body.mode == ChatRoutingMode::Auto && pipeline_chain.is_some());

    if want_pipeline {
        let pipeline = pipeline_chain.ok_or_else(|| {
            AppError::service_unavailable(
                "pipeline mode requested but no complete shard chain is online",
            )
        })?;

        return chat_pipeline(&state, &pipeline, &body.prompt, body.max_tokens).await;
    }

    chat_replica(&state, &online, &body.prompt, body.max_tokens).await
}

async fn chat_pipeline(
    state: &AppState,
    pipeline: &[StoredNode],
    prompt: &str,
    max_tokens: u32,
) -> Result<Json<ChatResponse>, AppError> {
    let (text, stages, model) = run_pipeline(
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
        rpc_backends = pipeline.iter().skip(1).filter(|n| !n.rpc_url.is_empty()).count(),
        "routed pipeline chat request (RPC tensor split)"
    );

    Ok(Json(ChatResponse {
        node_id: last.id.clone(),
        node_name: last.name.clone(),
        model,
        text,
        mode: "pipeline".to_string(),
        stages,
    }))
}

async fn chat_replica(
    state: &AppState,
    nodes: &[StoredNode],
    prompt: &str,
    max_tokens: u32,
) -> Result<Json<ChatResponse>, AppError> {
    let start = state.rr.fetch_add(1, Ordering::Relaxed);
    let mut last_err: Option<String> = None;

    for attempt in 0..nodes.len() {
        let idx = (start + attempt) % nodes.len();
        let node = &nodes[idx];

        if !worker_is_healthy(&state.health_http, &node.worker_url).await {
            warn!(
                node_id = %node.id,
                worker = %node.worker_url,
                "skipping unhealthy worker"
            );
            last_err = Some(format!("worker {} unreachable", node.worker_url));
            continue;
        }

        match run_infer(&state.http, node, prompt, max_tokens).await {
            Ok(infer) => {
                info!(node_id = %node.id, node_name = %node.name, "routed replica chat request");
                return Ok(Json(ChatResponse {
                    node_id: node.id.clone(),
                    node_name: node.name.clone(),
                    model: infer.model,
                    text: infer.text,
                    mode: "replica".to_string(),
                    stages: vec![],
                }));
            }
            Err(err) => {
                warn!(node_id = %node.id, error = %err, "worker inference failed, trying next");
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
