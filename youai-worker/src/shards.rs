use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::info;
use youai_common::{gguf_split_sibling_path, parse_gguf_split_filename, RemoteShardSource};

pub fn ensure_gguf_shards_local(
    local_model_path: &Path,
    remote_shards: &[RemoteShardSource],
) -> Result<PathBuf> {
    let (local_index, total) = parse_gguf_split_filename(local_model_path)
        .context("local model is not a GGUF split file")?;
    if total < 2 {
        return Ok(local_model_path.to_path_buf());
    }

    let cache_dir = local_model_path
        .parent()
        .context("model path has no parent directory")?;
    std::fs::create_dir_all(cache_dir)?;

    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .context("build HTTP client for shard fetch")?;

    for shard_index in 0..total {
        if shard_index == local_index {
            continue;
        }
        let dest = gguf_split_sibling_path(local_model_path, shard_index, total)?;
        if dest.is_file() {
            continue;
        }

        let source = remote_shards
            .iter()
            .find(|s| s.gguf_shard_index == shard_index)
            .with_context(|| {
                format!("no remote worker registered for GGUF shard index {shard_index}")
            })?;

        let url = format!("{}/model/shard", source.worker_url.trim_end_matches('/'));
        info!(
            %url,
            shard_index,
            dest = %dest.display(),
            "fetching remote GGUF shard"
        );

        let response = client
            .get(&url)
            .send()
            .with_context(|| format!("GET {url}"))?;
        if !response.status().is_success() {
            anyhow::bail!("shard fetch {url} returned {}", response.status());
        }

        let bytes = response.bytes().context("read shard response body")?;
        std::fs::write(&dest, &bytes).with_context(|| format!("write {}", dest.display()))?;
        info!(path = %dest.display(), bytes = bytes.len(), "GGUF shard cached");
    }

    Ok(local_model_path.to_path_buf())
}
