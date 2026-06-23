#!/usr/bin/env bash
# Prune stale and duplicate node registrations from the coordinator DB.
set -euo pipefail

COORDINATOR_URL="${1:-http://127.0.0.1:8080}"

echo "Pruning nodes at ${COORDINATOR_URL}..."
curl -fsS -X POST "${COORDINATOR_URL}/api/v1/nodes/prune" | python3 -m json.tool 2>/dev/null || \
  curl -fsS -X POST "${COORDINATOR_URL}/api/v1/nodes/prune"
echo ""
echo "== nodes after prune =="
curl -fsS "${COORDINATOR_URL}/api/v1/nodes" | python3 -m json.tool 2>/dev/null || \
  curl -fsS "${COORDINATOR_URL}/api/v1/nodes"