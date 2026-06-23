#!/usr/bin/env bash
# Pipeline v3 stage 0: layer-split GGUF (layers 0..N/2-1) + activation export.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE="${ROOT}/target/release/youai-node"
STAGES="${HOME}/.youai/pipeline-stages"
MODEL="${HOME}/.youai/models/smollm2-360m-instruct-q4_k_m.gguf"

"${ROOT}/scripts/build-pipeline-step.sh"
python3 "${ROOT}/scripts/split-model-layers.py" "${MODEL}" 2 "${STAGES}"

STAGE0="$(ls "${STAGES}"/*-stage00-of-02.gguf | head -1)"

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
  --pipeline-kind activation \
  --gguf-shard-index 0 \
  --gguf-shard-total 1 \
  --clear-rpc-url \
  --model-path "${STAGE0}" \
  --cpu-percent 30 \
  --ram-max 2g

export YOUAI_BIN_DIR="${ROOT}/target/release"

echo "Mac pipeline v3 stage 0 configured (activation passing)."
echo "  stage gguf: ${STAGE0}"
echo "  pipeline-step: ${YOUAI_BIN_DIR}/youai-pipeline-step"
echo ""
echo "Start with: YOUAI_BIN_DIR=${YOUAI_BIN_DIR} ${NODE} start"