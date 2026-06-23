use crate::llama::{default_timeout, run_inference, InferenceConfig};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
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
use youai_common::{HealthResponse, InferRequest, InferResponse};

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
        .route("/infer", post(infer))
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