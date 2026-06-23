//! Request signing helpers — HMAC-SHA256 for node↔coordinator integrity.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub const HEADER_TIMESTAMP: &str = "x-youai-timestamp";
pub const HEADER_NONCE: &str = "x-youai-nonce";
pub const HEADER_MAC: &str = "x-youai-mac";
pub const HEADER_TOKEN: &str = "x-youai-token";

pub const DEFAULT_CLOCK_SKEW_SECS: i64 = 60;
pub const DEFAULT_REQUEST_TTL_SECS: i64 = 120;
pub const NONCE_RETENTION_SECS: i64 = 300;

/// SHA256 hex digest of raw bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

/// SHA256 hex digest of a JSON value (compact serialization).
pub fn sha256_json_hex(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    sha256_hex(&bytes)
}

/// Build the HMAC message: `{issued_at}|{nonce}|{method}|{path}|{body_hash}`
pub fn hmac_message(issued_at: i64, nonce: &str, method: &str, path: &str, body_hash: &str) -> String {
    format!("{issued_at}|{nonce}|{method}|{path}|{body_hash}")
}

/// Compute HMAC-SHA256 hex using the node token as secret.
pub fn hmac_sha256_hex(secret: &str, message: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verify HMAC-SHA256 (constant-time compare via hex decode).
pub fn verify_hmac_sha256(secret: &str, message: &str, mac_hex: &str) -> bool {
    let expected = hmac_sha256_hex(secret, message);
    constant_time_eq(expected.as_bytes(), mac_hex.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn fresh_nonce() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Headers for an HMAC-signed coordinator request.
/// Uses `issued_at` and `nonce` from `body` when present (must match serialized body).
pub fn signed_headers(
    token: &str,
    method: &str,
    path: &str,
    body: &serde_json::Value,
) -> [(&'static str, String); 3] {
    let issued_at = body
        .get("issued_at")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(unix_now);
    let nonce = body
        .get("nonce")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(fresh_nonce);
    let body_hash = sha256_json_hex(body);
    let message = hmac_message(issued_at, &nonce, method, path, &body_hash);
    let mac = hmac_sha256_hex(token, &message);
    [
        (HEADER_TIMESTAMP, issued_at.to_string()),
        (HEADER_NONCE, nonce),
        (HEADER_MAC, mac),
    ]
}

pub fn within_clock_skew(issued_at: i64, now: i64, skew_secs: i64) -> bool {
    (now - issued_at).abs() <= skew_secs
}

pub fn not_expired(expires_at: i64, now: i64) -> bool {
    expires_at > now
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_roundtrip() {
        let msg = hmac_message(1719158400, "abc123", "POST", "/api/v1/nodes/heartbeat", "deadbeef");
        let mac = hmac_sha256_hex("secret-token", &msg);
        assert!(verify_hmac_sha256("secret-token", &msg, &mac));
        assert!(!verify_hmac_sha256("wrong", &msg, &mac));
    }
}