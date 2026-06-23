use crate::db::StoredNode;
use reqwest::Client;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tracing::warn;
use youai_common::{ChatStageInfo, InferRequest, InferResponse};

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

/// Pipeline v1: single inference on stage 0 with llama.cpp `--rpc` backends from later stages.
pub async fn run_pipeline(
    client: &Client,
    health_client: &Client,
    stages: &[StoredNode],
    user_prompt: &str,
    max_tokens: u32,
) -> Result<(String, Vec<ChatStageInfo>, String), String> {
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

    let mut rpc_servers = Vec::new();
    let mut stage_results = Vec::new();

    for node in stages.iter().skip(1) {
        let rpc = node.rpc_url.trim();
        if rpc.is_empty() {
            return Err(format!(
                "pipeline stage {} ({}) has no rpc_url — rebuild cluster with rpc-server",
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

    let infer = run_infer(client, head, user_prompt, max_tokens, &rpc_servers).await?;
    let mut stages_out = vec![ChatStageInfo {
        node_id: head.id.clone(),
        node_name: head.name.clone(),
        shard_stage: head.shard_stage,
        partial_text: infer.text.clone(),
    }];
    stages_out.append(&mut stage_results);

    Ok((infer.text, stages_out, infer.model))
}

async fn run_infer(
    client: &Client,
    node: &StoredNode,
    prompt: &str,
    max_tokens: u32,
    rpc_servers: &[String],
) -> Result<InferResponse, String> {
    let infer_url = format!("{}/infer", node.worker_url.trim_end_matches('/'));
    let response = client
        .post(&infer_url)
        .json(&InferRequest {
            prompt: prompt.to_string(),
            max_tokens,
            rpc_servers: rpc_servers.to_vec(),
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
