#!/usr/bin/env bash
# DOGFOOD ONLY — replica throughput test (full GGUF per machine).
# Production cluster: use ./scripts/setup-pipeline-cluster.sh (one model split across PCs).
# Do NOT use this as the default 2-machine setup.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE="${ROOT}/target/release/youai-node"
MODEL="${HOME}/.youai/models/smollm2-360m-instruct-q4_k_m.gguf"

if [[ ! -f "${MODEL}" ]]; then
  echo "Full model not found: ${MODEL}" >&2
  echo "Run: ./scripts/download-model.sh" >&2
  exit 1
fi

"${NODE}" pause 2>/dev/null || true
pkill -f 'youai-worker serve --host 127.0.0.1 --port 7741' 2>/dev/null || true
pkill -f 'youai-guard.*7741' 2>/dev/null || true

"${NODE}" config \
  --name m1-mac \
  --coordinator http://127.0.0.1:8080 \
  --worker-host 127.0.0.1 \
  --worker-port 7741 \
  --shard-group "" \
  --shard-stage 0 \
  --shard-total-stages 1 \
  --gguf-shard-index 0 \
  --gguf-shard-total 1 \
  --pipeline-kind "" \
  --clear-rpc-url \
  --model-path "${MODEL}" \
  --cpu-percent 30 \
  --ram-max 2g

export YOUAI_BIN_DIR="${ROOT}/target/release"

echo "Mac replica node configured (full model)."
echo "  model: ${MODEL}"
echo ""
echo "Start with: YOUAI_BIN_DIR=${YOUAI_BIN_DIR} ${NODE} start"