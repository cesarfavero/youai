#!/usr/bin/env bash
# Test distributed pipeline inference (Mac stage 0 + VM stage 1).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COORDINATOR_URL="${1:-http://127.0.0.1:8080}"
PROMPT="${2:-Explain in one sentence what a distributed AI cluster does.}"

echo "YouAI pipeline shard test"
echo "Coordinator: ${COORDINATOR_URL}"
echo "Prompt:      ${PROMPT}"
echo ""

"${ROOT}/scripts/clean-coordinator.sh" "${COORDINATOR_URL}" >/dev/null

echo "== nodes (expect 2 online, stage 0 Mac + stage 1 VM with rpc_url) =="
curl -fsS "${COORDINATOR_URL}/api/v1/nodes" | python3 -m json.tool
echo ""

echo "== RPC reachability (Mac -> VM tensor backend) =="
if command -v nc >/dev/null 2>&1; then
  nc -z -w 2 127.0.0.1 50052 && echo "  127.0.0.1:50052 OK" || echo "  127.0.0.1:50052 FAIL (run ubuntu-test-vm.sh start-node)"
else
  echo "  (skip — nc not installed)"
fi
echo ""

echo "== pipeline chat v1 RPC (single infer on stage 0, --rpc backends) =="
curl -fsS \
  -H 'Content-Type: application/json' \
  -d "{\"prompt\":\"${PROMPT}\",\"max_tokens\":96,\"mode\":\"pipeline\"}" \
  "${COORDINATOR_URL}/api/v1/chat" | python3 -m json.tool