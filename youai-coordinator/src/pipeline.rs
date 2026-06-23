use crate::db::StoredNode;
use reqwest::Client;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tracing::warn;
use youai_common::{ChatStageInfo, InferRequest, InferResponse, RemoteShardSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineBackend {
    /// v2: distributed GGUF files; stage 0 fetches peer shards over HTTP.
    Gguf,
    /// v1: llama.cpp --rpc tensor offload.
    Rpc,
}

pub async fn worker_is_healthy(client: &Client, worker_url: &str) -> bool {
    let url = format!("{}/health", worker_url.trim_end_matches('/'));
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => true,
        Ok(resp) => {
            warn!(%url, status = %resp.status(), "worker health check failed");
            false
        }
        Err(err) => {
            warn!(%url, error = %err, "worker health check failed");
            false
        }
    }
}

pub async fn rpc_is_healthy(rpc_url: &str) -> bool {
    let endpoint = rpc_url.trim();
    if endpoint.is_empty() {
        return false;
    }
    let addr = match endpoint.parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(_) => match format!("127.0.0.1:{endpoint}").parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(err) => {
                warn!(%endpoint, error = %err, "invalid rpc_url");
                return false;
            }
        },
    };

    match tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(addr)).await {
        Ok(Ok(_)) => true,
        Ok(Err(err)) => {
            warn!(%endpoint, error = %err, "rpc health check failed");
            false
        }
        Err(_) => {
            warn!(%endpoint, "rpc health check timed out");
            false
        }
    }
}

pub async fn run_pipeline(
    client: &Client,
    health_client: &Client,
    stages: &[StoredNode],
    user_prompt: &str,
    max_tokens: u32,
) -> Result<(String, Vec<ChatStageInfo>, String, &'static str), String> {
    if stages.is_empty() {
        return Err("pipeline has no stages".to_string());
    }

    let head = &stages[0];
    if head.shard_stage != 0 {
        return Err(format!(
            "pipeline head must be stage 0, got {}",
            head.shard_stage
        ));
    }

    if !worker_is_healthy(health_client, &head.worker_url).await {
        return Err(format!(
            "pipeline stage 0 worker {} is unhealthy",
            head.worker_url
        ));
    }

    match detect_backend(stages)? {
        PipelineBackend::Gguf => {
            run_pipeline_gguf(client, health_client, stages, user_prompt, max_tokens).await
        }
        PipelineBackend::Rpc => {
            run_pipeline_rpc(client, health_client, stages, user_prompt, max_tokens).await
        }
    }
}

fn detect_backend(stages: &[StoredNode]) -> Result<PipelineBackend, String> {
    let total = stages[0].gguf_shard_total;
    if total >= 2 {
        for (expected, node) in stages.iter().enumerate() {
            if node.gguf_shard_total != total {
                return Err(format!(
                    "pipeline GGUF shard total mismatch on {} (expected {total}, got {})",
                    node.name, node.gguf_shard_total
                ));
            }
            if node.gguf_shard_index as usize != expected {
                return Err(format!(
                    "pipeline stage {} ({}) has gguf_shard_index {} but expected {expected}",
                    node.shard_stage, node.name, node.gguf_shard_index
                ));
            }
        }
        return Ok(PipelineBackend::Gguf);
    }

    let has_rpc = stages.iter().skip(1).all(|n| !n.rpc_url.trim().is_empty());
    if has_rpc && stages.len() >= 2 {
        return Ok(PipelineBackend::Rpc);
    }

    Err(
        "pipeline misconfigured: use gguf_shard_total>=2 for GGUF v2 or rpc_url on stages 1+ for RPC v1"
            .to_string(),
    )
}

async fn run_pipeline_gguf(
    client: &Client,
    health_client: &Client,
    stages: &[StoredNode],
    user_prompt: &str,
    max_tokens: u32,
) -> Result<(String, Vec<ChatStageInfo>, String, &'static str), String> {
    let head = &stages[0];
    let mut remote_shards = Vec::new();
    let mut stage_results = Vec::new();
    let mut rpc_servers = Vec::new();

    for node in stages.iter().skip(1) {
        if !worker_is_healthy(health_client, &node.worker_url).await {
            return Err(format!(
                "pipeline stage {} worker {} is unhealthy",
                node.shard_stage, node.worker_url
            ));
        }
        remote_shards.push(RemoteShardSource {
            worker_url: node.worker_url.clone(),
            gguf_shard_index: node.gguf_shard_index,
        });
        let rpc = node.rpc_url.trim();
        if !rpc.is_empty() && rpc_is_healthy(rpc).await {
            rpc_servers.push(rpc.to_string());
        }
        stage_results.push(ChatStageInfo {
            node_id: node.id.clone(),
            node_name: node.name.clone(),
            shard_stage: node.shard_stage,
            partial_text: format!(
                "[gguf shard {} @ {}]",
                node.gguf_shard_index, node.worker_url
            ),
        });
    }

    let infer = run_infer(
        client,
        head,
        user_prompt,
        max_tokens,
        &rpc_servers,
        &remote_shards,
    )
    .await?;

    let mut stages_out = vec![ChatStageInfo {
        node_id: head.id.clone(),
        node_name: head.name.clone(),
        shard_stage: head.shard_stage,
        partial_text: infer.text.clone(),
    }];
    stages_out.append(&mut stage_results);

    Ok((infer.text, stages_out, infer.model, "pipeline_gguf"))
}

async fn run_pipeline_rpc(
    client: &Client,
    health_client: &Client,
    stages: &[StoredNode],
    user_prompt: &str,
    max_tokens: u32,
) -> Result<(String, Vec<ChatStageInfo>, String, &'static str), String> {
    let head = &stages[0];
    let mut rpc_servers = Vec::new();
    let mut stage_results = Vec::new();

    for node in stages.iter().skip(1) {
        if !worker_is_healthy(health_client, &node.worker_url).await {
            return Err(format!(
                "pipeline stage {} worker {} is unhealthy",
                node.shard_stage, node.worker_url
            ));
        }
        let rpc = node.rpc_url.trim();
        if rpc.is_empty() {
            return Err(format!(
                "pipeline stage {} ({}) has no rpc_url",
                node.shard_stage, node.name
            ));
        }
        if !rpc_is_healthy(rpc).await {
            return Err(format!(
                "pipeline stage {} rpc {} is unreachable",
                node.shard_stage, rpc
            ));
        }
        rpc_servers.push(rpc.to_string());
        stage_results.push(ChatStageInfo {
            node_id: node.id.clone(),
            node_name: node.name.clone(),
            shard_stage: node.shard_stage,
            partial_text: format!("[rpc tensor backend @ {rpc}]"),
        });
    }

    let infer = run_infer(client, head, user_prompt, max_tokens, &rpc_servers, &[]).await?;
    let mut stages_out = vec![ChatStageInfo {
        node_id: head.id.clone(),
        node_name: head.name.clone(),
        shard_stage: head.shard_stage,
        partial_text: infer.text.clone(),
    }];
    stages_out.append(&mut stage_results);

    Ok((infer.text, stages_out, infer.model, "pipeline_rpc"))
}

async fn run_infer(
    client: &Client,
    node: &StoredNode,
    prompt: &str,
    max_tokens: u32,
    rpc_servers: &[String],
    remote_shards: &[RemoteShardSource],
) -> Result<InferResponse, String> {
    let infer_url = format!("{}/infer", node.worker_url.trim_end_matches('/'));
    let response = client
        .post(&infer_url)
        .json(&InferRequest {
            prompt: prompt.to_string(),
            max_tokens,
            rpc_servers: rpc_servers.to_vec(),
            remote_shards: remote_shards.to_vec(),
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
