//! HMAC verification and nonce replay protection.

use crate::db::Database;
use anyhow::{anyhow, Result};
use axum::http::HeaderMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use youai_common::signing::{
    hmac_message, sha256_json_hex, verify_hmac_sha256, DEFAULT_CLOCK_SKEW_SECS, HEADER_MAC,
    HEADER_NONCE, HEADER_TIMESTAMP,
};

#[derive(Clone)]
struct NonceRecord {
    #[allow(dead_code)]
    seen_at: Instant,
}

#[derive(Clone)]
pub struct NonceStore {
    inner: Arc<Mutex<std::collections::HashMap<String, NonceRecord>>>,
    retention: Duration,
}

impl NonceStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(std::collections::HashMap::new())),
            retention: Duration::from_secs(youai_common::signing::NONCE_RETENTION_SECS as u64),
        }
    }

    pub fn check_and_insert(&self, nonce: &str) -> bool {
        let mut guard = self.inner.lock().expect("nonce store lock");
        let now = Instant::now();
        guard.retain(|_, r| now.duration_since(r.seen_at) < self.retention);
        if guard.contains_key(nonce) {
            return false;
        }
        guard.insert(
            nonce.to_string(),
            NonceRecord { seen_at: now },
        );
        true
    }
}

impl Default for NonceStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct VerifiedRequest {
    pub node_id: Option<String>,
    pub contributor: bool,
}

pub fn dev_mode_enabled() -> bool {
    std::env::var("YOUAI_DEV_MODE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub fn require_signing() -> bool {
    !dev_mode_enabled()
}

pub fn verify_node_hmac(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &serde_json::Value,
    db: &Database,
) -> Result<()> {
    if !require_signing() {
        return Ok(());
    }

    let token = headers
        .get(youai_common::signing::HEADER_TOKEN)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("missing {}", youai_common::signing::HEADER_TOKEN))?;

    let issued_at: i64 = headers
        .get(HEADER_TIMESTAMP)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("missing or invalid {}", HEADER_TIMESTAMP))?;

    let nonce = headers
        .get(HEADER_NONCE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("missing {}", HEADER_NONCE))?;

    let mac = headers
        .get(HEADER_MAC)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("missing {}", HEADER_MAC))?;

    let now = youai_common::signing::unix_now();
    if !youai_common::signing::within_clock_skew(issued_at, now, DEFAULT_CLOCK_SKEW_SECS) {
        return Err(anyhow!("clock skew too large"));
    }

    let node_id = body
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing node_id in body"))?;

    let stored_token = db
        .node_token(node_id)?
        .ok_or_else(|| anyhow!("unknown node_id"))?;

    if stored_token != token {
        return Err(anyhow!("token mismatch"));
    }

    let body_hash = sha256_json_hex(body);
    let message = hmac_message(issued_at, nonce, method, path, &body_hash);
    if !verify_hmac_sha256(&stored_token, &message, mac) {
        return Err(anyhow!("invalid HMAC"));
    }

    Ok(())
}

pub fn verify_chat_hmac(
    headers: &HeaderMap,
    path: &str,
    envelope: &youai_common::SignedChatRequest,
    nonce_store: &NonceStore,
    db: &Database,
) -> Result<VerifiedRequest> {
    if !require_signing() {
        return Ok(VerifiedRequest {
            node_id: lookup_contributor(headers, db).ok(),
            contributor: headers
                .get(youai_common::signing::HEADER_TOKEN)
                .is_some(),
        });
    }

    if let Ok(node_id) = lookup_contributor(headers, db) {
        return Ok(VerifiedRequest {
            node_id: Some(node_id),
            contributor: true,
        });
    }

    let token = headers
        .get(youai_common::signing::HEADER_TOKEN)
        .and_then(|v| v.to_str().ok());

    let mac = headers
        .get(HEADER_MAC)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("missing chat auth"))?;

    let now = youai_common::signing::unix_now();
    if !youai_common::signing::not_expired(envelope.expires_at, now) {
        return Err(anyhow!("request expired"));
    }
    if !youai_common::signing::within_clock_skew(envelope.issued_at, now, DEFAULT_CLOCK_SKEW_SECS)
    {
        return Err(anyhow!("clock skew"));
    }
    if !nonce_store.check_and_insert(&envelope.nonce) {
        return Err(anyhow!("replay detected"));
    }

    let body_json = serde_json::to_value(&envelope.body).unwrap_or_default();
    let body_hash = sha256_json_hex(&body_json);
    let signing_payload = serde_json::json!({
        "v": envelope.v,
        "id": envelope.id,
        "issued_at": envelope.issued_at,
        "expires_at": envelope.expires_at,
        "nonce": envelope.nonce,
        "body_hash": body_hash,
    });
    let payload_hash = sha256_json_hex(&signing_payload);
    let message = hmac_message(
        envelope.issued_at,
        &envelope.nonce,
        "POST",
        path,
        &payload_hash,
    );

    let device_secret =
        std::env::var("YOUAI_CHAT_DEVICE_SECRET").unwrap_or_else(|_| "youai-dev-chat".to_string());
    let secret = token.unwrap_or(&device_secret);

    if !verify_hmac_sha256(secret, &message, mac) {
        return Err(anyhow!("invalid chat HMAC"));
    }

    Ok(VerifiedRequest {
        node_id: None,
        contributor: false,
    })
}

fn lookup_contributor(headers: &HeaderMap, db: &Database) -> Result<String> {
    let token = headers
        .get(youai_common::signing::HEADER_TOKEN)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("no token"))?;
    db.node_id_for_token(token)?
        .ok_or_else(|| anyhow!("invalid contributor token"))
}