#!/usr/bin/env bash
# Quick smoke test: coordinator health + chat round-robin.
set -euo pipefail

COORDINATOR_URL="${1:-http://127.0.0.1:8080}"
PROMPT="${2:-Say hello in one short sentence.}"

echo "Coordinator: ${COORDINATOR_URL}"
echo "Prompt:      ${PROMPT}"
echo ""

echo "== health =="
curl -fsS "${COORDINATOR_URL}/health"
echo ""
echo ""

echo "== nodes =="
curl -fsS "${COORDINATOR_URL}/api/v1/nodes" | python3 -m json.tool 2>/dev/null || curl -fsS "${COORDINATOR_URL}/api/v1/nodes"
echo ""
echo ""

echo "== chat =="
curl -fsS \
  -H 'Content-Type: application/json' \
  -d "{\"prompt\":\"${PROMPT}\",\"max_tokens\":64}" \
  "${COORDINATOR_URL}/api/v1/chat" | python3 -m json.tool 2>/dev/null || true