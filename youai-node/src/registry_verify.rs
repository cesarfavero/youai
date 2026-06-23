//! Verify local model SHA256 against registry manifest before loading.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub fn verify_model_against_manifest(model_path: &Path, model_name: &str) -> Result<()> {
    if std::env::var("YOUAI_SKIP_MODEL_VERIFY")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        return Ok(());
    }

    let manifest_path = resolve_manifest_path();
    if !manifest_path.is_file() {
        tracing::warn!(
            path = %manifest_path.display(),
            "registry manifest not found — skipping SHA256 verify"
        );
        return Ok(());
    }

    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let manifest: serde_json::Value = serde_json::from_str(&raw).context("parse manifest")?;

    let expected = find_model_sha256(&manifest, model_name, model_path)?;
    let Some(expected) = expected else {
        tracing::warn!(model = %model_name, "model not listed in registry manifest — skip verify");
        return Ok(());
    };

    let actual = sha256_file(model_path)?;
    if actual != expected {
        anyhow::bail!(
            "model hash mismatch for {}\n  expected: {expected}\n  actual:   {actual}\n  Refuse to load — run ./scripts/download-model.sh or youai-node models update",
            model_path.display()
        );
    }

    tracing::info!(model = %model_name, "registry SHA256 verified");
    Ok(())
}

fn resolve_manifest_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("YOUAI_REGISTRY_MANIFEST") {
        return std::path::PathBuf::from(p);
    }
    for candidate in ["registry/manifest.json", "../registry/manifest.json"] {
        let p = std::path::PathBuf::from(candidate);
        if p.is_file() {
            return p;
        }
    }
    std::path::PathBuf::from("registry/manifest.json")
}

fn find_model_sha256(
    manifest: &serde_json::Value,
    model_name: &str,
    model_path: &Path,
) -> Result<Option<String>> {
    let tiers = manifest
        .get("tiers")
        .and_then(|v| v.as_object())
        .context("manifest missing tiers")?;

    let filename = model_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Pass 1: pipeline stage GGUFs (exact filename) — before fuzzy id match on tier0 full model.
    for tier in tiers.values() {
        let Some(models) = tier.get("models").and_then(|m| m.as_array()) else {
            continue;
        };
        for model in models {
            if let Some(stage_hash) = stage_file_sha256(model, filename) {
                return Ok(Some(stage_hash));
            }
        }
    }

    // Pass 2: full model entry
    for tier in tiers.values() {
        let Some(models) = tier.get("models").and_then(|m| m.as_array()) else {
            continue;
        };
        for model in models {
            let id = model.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let fname = model.get("filename").and_then(|v| v.as_str()).unwrap_or("");
            if id.contains(model_name)
                || model_name.contains(id)
                || fname == filename
                || id.replace('-', "_") == model_name.replace('-', "_")
            {
                if let Some(hash) = model.get("sha256").and_then(|v| v.as_str()) {
                    if !hash.is_empty() {
                        return Ok(Some(hash.to_string()));
                    }
                }
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn stage_gguf_uses_stage_hash_not_full_model() {
        let raw = include_str!("../../registry/manifest.json");
        let manifest: serde_json::Value = serde_json::from_str(raw).unwrap();
        let path = Path::new("smollm2-360m-instruct-q4_k_m-stage00-of-02.gguf");
        let hash = find_model_sha256(&manifest, "smollm2-360m-instruct", path)
            .unwrap()
            .unwrap();
        assert_eq!(hash, "9d186b6e6b5aadc1ffc8bc1c3458f6bd36745b7c237f7b1839d79ff2dd8549a7");
    }
}

fn stage_file_sha256(model: &serde_json::Value, filename: &str) -> Option<String> {
    let stages = model
        .get("pipeline")
        .and_then(|p| p.get("stage_files"))
        .and_then(|s| s.as_array())?;
    for stage in stages {
        let stage_name = stage.get("filename").and_then(|v| v.as_str()).unwrap_or("");
        if stage_name == filename {
            return stage
                .get("sha256")
                .and_then(|v| v.as_str())
                .filter(|h| !h.is_empty())
                .map(|h| h.to_string());
        }
    }
    None
}