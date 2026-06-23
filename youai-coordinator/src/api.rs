use crate::db::{Database, StoredNode};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
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
    ChatRequest, ChatResponse, HeartbeatRequest, InferRequest, InferResponse, NodesResponse,
    RegisterNodeRequest, RegisterNodeResponse,
};

const TOKEN_HEADER: &str = "x-youai-token";

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub http: Client,
    pub rr: Arc<AtomicUsize>,
}

pub async fn serve(host: &str, port: u16, db_path: &str) -> Result<()> {
    let db = Database::open(std::path::Path::new(db_path))?;
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        http: Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .context("build HTTP client")?,
        rr: Arc::new(AtomicUsize::new(0)),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/nodes/register", post(register_node))
        .route("/api/v1/nodes/heartbeat", post(heartbeat))
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

async fn register_node(
    State(state): State<AppState>,
    Json(body): Json<RegisterNodeRequest>,
) -> Result<Json<RegisterNodeResponse>, AppError> {
    if body.worker_url.trim().is_empty() {
        return Err(AppError::bad_request("worker_url is required"));
    }

    let node_id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();

    let node = StoredNode {
        id: node_id.clone(),
        token: token.clone(),
        name: body.name,
        region: body.region,
        worker_url: body.worker_url,
        model: body.model,
        last_heartbeat: now,
        created_at: now,
    };

    state
        .db
        .lock()
        .map_err(|_| AppError::internal("db lock poisoned"))?
        .upsert_node(&node)
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

    let nodes = state
        .db
        .lock()
        .map_err(|_| AppError::internal("db lock poisoned"))?
        .online_nodes()
        .map_err(|err| AppError::internal(err.to_string()))?;

    if nodes.is_empty() {
        return Err(AppError::service_unavailable(
            "no online nodes — start youai-node on at least one machine",
        ));
    }

    let idx = state.rr.fetch_add(1, Ordering::Relaxed) % nodes.len();
    let node = &nodes[idx];

    let infer_url = format!("{}/infer", node.worker_url.trim_end_matches('/'));
    let response = state
        .http
        .post(&infer_url)
        .json(&InferRequest {
            prompt: body.prompt,
            max_tokens: body.max_tokens,
        })
        .send()
        .await
        .map_err(|err| AppError::bad_gateway(format!("worker request failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!(node_id = %node.id, %status, body = %text, "worker inference failed");
        return Err(AppError::bad_gateway(format!(
            "worker {} returned {status}: {text}",
            node.id
        )));
    }

    let infer: InferResponse = response
        .json()
        .await
        .map_err(|err| AppError::bad_gateway(format!("invalid worker response: {err}")))?;

    info!(node_id = %node.id, node_name = %node.name, "routed chat request");

    Ok(Json(ChatResponse {
        node_id: node.id.clone(),
        node_name: node.name.clone(),
        model: infer.model,
        text: infer.text,
    }))
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