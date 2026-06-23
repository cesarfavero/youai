use crate::db::StoredNode;
use reqwest::Client;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tracing::warn;
use uuid::Uuid;
use youai_common::{
    chat_template::{
        clean_assistant_response, format_instruct_prompt, is_eos_piece, should_skip_piece,
    },
    ChatStageInfo, InferRequest, InferResponse, PipelineStepRequest, PipelineStepResponse,
    RemoteShardSource, PIPELINE_KIND_ACTIVATION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineBackend {
    /// v3: layer-split GGUFs with activation passing between stages.
    Activation,
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
        PipelineBackend::Activation => {
            run_pipeline_activation(client, health_client, stages, user_prompt, max_tokens).await
        }
        PipelineBackend::Gguf => {
            run_pipeline_gguf(client, health_client, stages, user_prompt, max_tokens).await
        }
        PipelineBackend::Rpc => {
            run_pipeline_rpc(client, health_client, stages, user_prompt, max_tokens).await
        }
    }
}

fn detect_backend(stages: &[StoredNode]) -> Result<PipelineBackend, String> {
    let activation_count = stages
        .iter()
        .filter(|n| n.pipeline_kind == PIPELINE_KIND_ACTIVATION)
        .count();
    if activation_count > 0 {
        if activation_count != stages.len() {
            return Err(
                "pipeline activation backend requires pipeline_kind=activation on every stage"
                    .to_string(),
            );
        }
        return Ok(PipelineBackend::Activation);
    }

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
        "pipeline misconfigured: use pipeline_kind=activation for v3, gguf_shard_total>=2 for GGUF v2, or rpc_url on stages 1+ for RPC v1"
            .to_string(),
    )
}

async fn run_pipeline_activation(
    client: &Client,
    health_client: &Client,
    stages: &[StoredNode],
    user_prompt: &str,
    max_tokens: u32,
) -> Result<(String, Vec<ChatStageInfo>, String, &'static str), String> {
    if stages.len() != 2 {
        return Err(format!(
            "pipeline v3 activation mode supports exactly 2 stages (got {})",
            stages.len()
        ));
    }

    for node in stages {
        if !worker_is_healthy(health_client, &node.worker_url).await {
            return Err(format!(
                "pipeline stage {} worker {} is unhealthy",
                node.shard_stage, node.worker_url
            ));
        }
    }

    let session_id = Uuid::new_v4().to_string();
    let head = &stages[0];
    let tail = &stages[1];
    let instruct_prompt = format_instruct_prompt(&head.model, user_prompt);

    let prefill = post_pipeline_step(
        client,
        head,
        &PipelineStepRequest {
            session_id: session_id.clone(),
            op: "prefill-prompt".to_string(),
            prompt: instruct_prompt,
            token_id: 0,
            activation_b64: String::new(),
            sample: false,
        },
    )
    .await?;

    let mut activation_b64 = prefill
        .activation_b64
        .ok_or_else(|| "stage 0 prefill produced no activation".to_string())?;

    let mut generated = String::new();
    let mut stage_texts = vec![String::new(); stages.len()];

    for token_idx in 0..max_tokens {
        let sampled = post_pipeline_step(
            client,
            tail,
            &PipelineStepRequest {
                session_id: session_id.clone(),
                op: "forward-activation".to_string(),
                prompt: String::new(),
                token_id: 0,
                activation_b64: activation_b64.clone(),
                sample: true,
            },
        )
        .await?;

        let token_id = sampled
            .token_id
            .ok_or_else(|| "tail stage produced no token_id".to_string())?;
        if let Some(piece) = sampled.text {
            if is_eos_piece(&piece) {
                break;
            }
            if !should_skip_piece(&piece) {
                generated.push_str(&piece);
                stage_texts[1].push_str(&piece);
            }
        }

        if is_eos_piece(&generated) || token_idx + 1 >= max_tokens {
            break;
        }

        let decoded = post_pipeline_step(
            client,
            head,
            &PipelineStepRequest {
                session_id: session_id.clone(),
                op: "decode-token".to_string(),
                prompt: String::new(),
                token_id,
                activation_b64: String::new(),
                sample: false,
            },
        )
        .await?;

        activation_b64 = decoded
            .activation_b64
            .ok_or_else(|| "stage 0 decode produced no activation".to_string())?;
    }

    stage_texts[0] = format!("[activation prefill+decode @ {}]", head.worker_url);

    let stages_out: Vec<ChatStageInfo> = stages
        .iter()
        .enumerate()
        .map(|(idx, node)| ChatStageInfo {
            node_id: node.id.clone(),
            node_name: node.name.clone(),
            shard_stage: node.shard_stage,
            partial_text: stage_texts[idx].clone(),
        })
        .collect();

    Ok((
        clean_assistant_response(&generated),
        stages_out,
        head.model.clone(),
        "pipeline_activation_v4",
    ))
}

async fn post_pipeline_step(
    client: &Client,
    node: &StoredNode,
    req: &PipelineStepRequest,
) -> Result<PipelineStepResponse, String> {
    let url = format!("{}/pipeline/step", node.worker_url.trim_end_matches('/'));
    let response = client
        .post(&url)
        .json(req)
        .send()
        .await
        .map_err(|err| format!("pipeline step request failed: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!(
            "pipeline step on {} (stage {}) returned {status}: {text}",
            node.name, node.shard_stage
        ));
    }

    let body = response
        .json::<PipelineStepResponse>()
        .await
        .map_err(|err| format!("invalid pipeline step response: {err}"))?;
    if !body.ok {
        return Err(body
            .error
            .unwrap_or_else(|| "pipeline step returned ok=false".to_string()));
    }
    Ok(body)
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
