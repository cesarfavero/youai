//! Gateway stubs — upload / URL / search run on YouAI infra, not volunteer nodes.

use axum::{http::StatusCode, Json};

pub async fn upload_not_ready() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "upload gateway not enabled",
            "detail": "File reading runs on YouAI gateway (tier4+), never on contributor PCs",
            "doc": "docs/PRODUCT.md#51-upload-para-leitura"
        })),
    )
}

pub async fn url_not_ready() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "url gateway not enabled",
            "detail": "URL fetch runs on sandboxed YouAI gateway (tier4+)",
            "doc": "docs/PRODUCT.md#52-análise-de-url"
        })),
    )
}

pub async fn search_not_ready() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "search gateway not enabled",
            "detail": "Web search runs on YouAI gateway (tier5)",
            "doc": "docs/PRODUCT.md#53-função-buscar"
        })),
    )
}