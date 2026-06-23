#!/usr/bin/env bash
# Pipeline v2 stage 0: local GGUF shard 00001-of-00002.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE="${ROOT}/target/release/youai-node"
SHARDS="${HOME}/.youai/shards"

"${ROOT}/scripts/split-model.sh" "${HOME}/.youai/models/smollm2-360m-instruct-q4_k_m.gguf" 2 "${SHARDS}"

SHARD0="$(ls "${SHARDS}"/*-00001-of-00002.gguf | head -1)"

"${NODE}" pause 2>/dev/null || true
pkill -f 'youai-worker serve --host 127.0.0.1 --port 7741' 2>/dev/null || true
pkill -f 'youai-guard.*7741' 2>/dev/null || true

"${NODE}" config \
  --name m1-mac \
  --coordinator http://127.0.0.1:8080 \
  --worker-host 127.0.0.1 \
  --worker-port 7741 \
  --shard-group default-pipeline \
  --shard-stage 0 \
  --shard-total-stages 2 \
  --gguf-shard-index 0 \
  --gguf-shard-total 2 \
  --clear-rpc-url \
  --model-path "${SHARD0}" \
  --cpu-percent 30 \
  --ram-max 2g

echo "Mac pipeline v2 stage 0 configured (GGUF shard 0)."
echo "  shard: ${SHARD0}"