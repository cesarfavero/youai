#!/usr/bin/env bash
# Test pipeline v2: distributed GGUF shards (HTTP fetch + single infer on stage 0).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COORDINATOR_URL="${1:-http://127.0.0.1:8080}"
PROMPT="${2:-Explain in one sentence what a distributed AI cluster does.}"

echo "YouAI pipeline v2 GGUF test"
echo "Coordinator: ${COORDINATOR_URL}"
echo ""

"${ROOT}/scripts/clean-coordinator.sh" "${COORDINATOR_URL}" >/dev/null

echo "== nodes (expect gguf_shard_index 0 + 1, gguf_shard_total 2) =="
curl -fsS "${COORDINATOR_URL}/api/v1/nodes" | python3 -m json.tool
echo ""

echo "== pipeline chat (mode=pipeline, expects mode=pipeline_gguf) =="
curl -fsS \
  -H 'Content-Type: application/json' \
  -d "{\"prompt\":\"${PROMPT}\",\"max_tokens\":96,\"mode\":\"pipeline\"}" \
  "${COORDINATOR_URL}/api/v1/chat" | python3 -m json.tool