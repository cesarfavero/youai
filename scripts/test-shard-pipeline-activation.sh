#!/usr/bin/env bash
# Test pipeline v3: activation passing between layer-split stages.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COORDINATOR_URL="${1:-http://127.0.0.1:8080}"
PROMPT="${2:-Explain in one sentence what a distributed AI cluster does.}"

echo "YouAI pipeline v3 activation test"
echo "Coordinator: ${COORDINATOR_URL}"
echo ""

"${ROOT}/scripts/clean-coordinator.sh" "${COORDINATOR_URL}" >/dev/null

echo "== nodes (expect pipeline_kind=activation, stages 0+1) =="
curl -fsS "${COORDINATOR_URL}/api/v1/nodes" | python3 -m json.tool
echo ""

echo "== pipeline chat (mode=pipeline, expects mode=pipeline_activation) =="
curl -fsS \
  -H 'Content-Type: application/json' \
  -d "{\"prompt\":\"${PROMPT}\",\"max_tokens\":8,\"mode\":\"pipeline\"}" \
  "${COORDINATOR_URL}/api/v1/chat" | python3 -m json.tool