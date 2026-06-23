//! Model registry loader and compute-based tier selection.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use youai_common::compute::node_compute_units;

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryManifest {
    pub version: u32,
    pub default_tier: String,
    pub tiers: std::collections::BTreeMap<String, TierDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TierDef {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub models: Vec<ModelDef>,
    pub network_requirements: NetworkRequirements,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelDef {
    pub id: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkRequirements {
    #[serde(default)]
    pub min_contributors_online: u32,
    #[serde(default)]
    pub min_total_ram_gb: u32,
    #[serde(default)]
    pub min_compute_units: u64,
    #[serde(default)]
    pub min_pipeline_chains: u32,
}

#[derive(Debug, Clone)]
pub struct TierSelection {
    pub active_tier: String,
    pub display_name: String,
    pub model_id: String,
    pub min_compute_units: u64,
    pub next_tier: Option<String>,
    pub next_tier_min_cu: Option<u64>,
}

pub fn resolve_manifest_path() -> PathBuf {
    if let Ok(path) = std::env::var("YOUAI_REGISTRY_MANIFEST") {
        return PathBuf::from(path);
    }
    for candidate in [
        PathBuf::from("registry/manifest.json"),
        PathBuf::from("../registry/manifest.json"),
    ] {
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("registry/manifest.json")
}

pub fn load_manifest(path: &Path) -> Result<RegistryManifest> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read registry manifest {}", path.display()))?;
    serde_json::from_str(&raw).context("parse registry manifest")
}

/// Effective min compute units — prefers `min_compute_units`, falls back from RAM GB.
pub fn tier_min_compute(req: &NetworkRequirements) -> u64 {
    if req.min_compute_units > 0 {
        return req.min_compute_units;
    }
    u64::from(req.min_total_ram_gb).saturating_mul(1024)
}

pub fn select_active_tier(
    manifest: &RegistryManifest,
    network_cu: u64,
    pipeline_chains: u32,
) -> TierSelection {
    let mut ordered: Vec<&TierDef> = manifest.tiers.values().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));

    let mut active: Option<&TierDef> = None;
    for tier in &ordered {
        if tier.status.as_deref() == Some("planned") && tier.models.is_empty() {
            continue;
        }
        let min_cu = tier_min_compute(&tier.network_requirements);
        let chains_ok = pipeline_chains >= tier.network_requirements.min_pipeline_chains;
        if network_cu >= min_cu && chains_ok {
            active = Some(tier);
        }
    }

    let fallback_id = manifest.default_tier.clone();
    let active = active.unwrap_or_else(|| {
        manifest
            .tiers
            .get(&fallback_id)
            .expect("default_tier must exist in manifest")
    });

    let model_id = active
        .models
        .first()
        .map(|m| m.id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let active_min = tier_min_compute(&active.network_requirements);
    let next = ordered
        .iter()
        .find(|t| tier_min_compute(&t.network_requirements) > active_min)
        .map(|t| {
            (
                t.id.clone(),
                tier_min_compute(&t.network_requirements),
            )
        });

    TierSelection {
        active_tier: active.id.clone(),
        display_name: active.display_name.clone(),
        model_id,
        min_compute_units: active_min,
        next_tier: next.as_ref().map(|(id, _)| id.clone()),
        next_tier_min_cu: next.map(|(_, cu)| cu),
    }
}

pub fn node_cu(cpu_percent: u8, ram_max_mb: u32) -> u64 {
    node_compute_units(cpu_percent, ram_max_mb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier1_with_two_mac_minis() {
        let path = resolve_manifest_path();
        if !path.is_file() {
            return;
        }
        let manifest = load_manifest(&path).unwrap();
        let cu = node_compute_units(30, 2048) * 2;
        let sel = select_active_tier(&manifest, cu, 1);
        assert_eq!(sel.active_tier, "tier1");
    }
}