use crate::llama::{default_timeout, run_inference, InferenceConfig};
use crate::pipeline::{
    default_pipeline_step_bin, default_session_root, run_pipeline_step, PipelineStepConfig,
};
use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;
use youai_common::parse_gguf_split_filename;
use youai_common::{
    HealthResponse, InferRequest, InferResponse, PipelineStepRequest, PipelineStepResponse,
};

#[derive(Clone)]
pub struct WorkerState {
    pub llama_cli: PathBuf,
    pub model_path: PathBuf,
    pub model_name: String,
}

pub async fn serve(host: &str, port: u16, state: WorkerState) -> Result<()> {
    let app_state = Arc::new(state);
    let model_path = app_state.model_path.display().to_string();
    let app = Router::new()
        .route("/health", get(health))
        .route("/model/shard", get(model_shard))
        .route("/infer", post(infer))
        .route("/pipeline/step", post(pipeline_step))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .context("invalid worker listen address")?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind worker on {addr}"))?;

    info!(%addr, model = %model_path, "youai-worker listening");
    axum::serve(listener, app)
        .await
        .context("worker server stopped")?;
    Ok(())
}

async fn health(State(state): State<Arc<WorkerState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        model: state.model_name.clone(),
    })
}

async fn model_shard(State(state): State<Arc<WorkerState>>) -> impl IntoResponse {
    let path = &state.model_path;
    if !path.is_file() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: format!("shard not found: {}", path.display()),
            }),
        )
            .into_response();
    }

    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: err.to_string(),
                }),
            )
                .into_response();
        }
    };

    let mut headers = axum::http::HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str("application/octet-stream") {
        headers.insert(header::CONTENT_TYPE, value);
    }
    if let Some((index, total)) = parse_gguf_split_filename(path) {
        if let Ok(value) = HeaderValue::from_str(&index.to_string()) {
            headers.insert("x-youai-gguf-shard-index", value);
        }
        if let Ok(value) = HeaderValue::from_str(&total.to_string()) {
            headers.insert("x-youai-gguf-shard-total", value);
        }
    }

    (StatusCode::OK, headers, Body::from(bytes)).into_response()
}

async fn pipeline_step(
    State(state): State<Arc<WorkerState>>,
    Json(body): Json<PipelineStepRequest>,
) -> impl IntoResponse {
    let config = PipelineStepConfig {
        pipeline_step_bin: default_pipeline_step_bin(),
        model_path: state.model_path.clone(),
        session_root: default_session_root(),
        timeout: default_timeout(),
    };

    let result = tokio::task::spawn_blocking(move || run_pipeline_step(&config, &body)).await;

    match result {
        Ok(Ok(outcome)) => (
            StatusCode::OK,
            Json(PipelineStepResponse {
                ok: true,
                op: outcome.op,
                n_past: outcome.n_past,
                n_embd: outcome.n_embd,
                token_id: outcome.token_id,
                text: outcome.text,
                activation_b64: outcome.activation_b64,
                error: None,
            }),
        )
            .into_response(),
        Ok(Err(err)) => (
            StatusCode::BAD_GATEWAY,
            Json(PipelineStepResponse {
                ok: false,
                error: Some(err.to_string()),
                ..Default::default()
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PipelineStepResponse {
                ok: false,
                error: Some(err.to_string()),
                ..Default::default()
            }),
        )
            .into_response(),
    }
}

async fn infer(
    State(state): State<Arc<WorkerState>>,
    Json(body): Json<InferRequest>,
) -> impl IntoResponse {
    let config = InferenceConfig {
        llama_cli: state.llama_cli.clone(),
        model_path: state.model_path.clone(),
        model_name: state.model_name.clone(),
        prompt: body.prompt,
        max_tokens: body.max_tokens,
        timeout: default_timeout(),
        rpc_servers: body.rpc_servers,
        remote_shards: body.remote_shards,
    };

    let state_for_task = state.clone();
    let result = tokio::task::spawn_blocking(move || run_inference(&config)).await;

    match result {
        Ok(Ok(text)) => (
            StatusCode::OK,
            Json(InferResponse {
                text,
                model: state_for_task.model_name.clone(),
            }),
        )
            .into_response(),
        Ok(Err(err)) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorBody {
                error: err.to_string(),
            }),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: err.to_string(),
            }),
        )
            .into_response(),
    }
}

#[derive(serde::Serialize)]
struct ErrorBody {
    error: String,
}
