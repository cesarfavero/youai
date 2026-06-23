#!/usr/bin/env bash
# Benchmark round-robin inference across all online cluster nodes.
set -euo pipefail

COORDINATOR_URL="${1:-http://127.0.0.1:8080}"
REQUESTS="${2:-6}"
PROMPT="${3:-Say hello in one short sentence.}"

echo "YouAI cluster benchmark"
echo "Coordinator: ${COORDINATOR_URL}"
echo "Requests:    ${REQUESTS}"
echo "Prompt:      ${PROMPT}"
echo ""

echo "== nodes =="
curl -fsS "${COORDINATOR_URL}/api/v1/nodes" | python3 -m json.tool 2>/dev/null || true
echo ""

TOTAL_MS=0
RESULTS_FILE="$(mktemp)"
trap 'rm -f "${RESULTS_FILE}"' EXIT

for i in $(seq 1 "${REQUESTS}"); do
  START_MS=$(python3 -c 'import time; print(int(time.time()*1000))')
  RESP=$(curl -fsS \
    -H 'Content-Type: application/json' \
    -d "{\"prompt\":\"${PROMPT}\",\"max_tokens\":64}" \
    "${COORDINATOR_URL}/api/v1/chat")
  END_MS=$(python3 -c 'import time; print(int(time.time()*1000))')
  ELAPSED=$((END_MS - START_MS))
  TOTAL_MS=$((TOTAL_MS + ELAPSED))

  NODE=$(echo "${RESP}" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("node_name","?"))')
  TEXT=$(echo "${RESP}" | python3 -c 'import sys,json; d=json.load(sys.stdin); print((d.get("text") or "")[:60])')
  echo "${NODE}" >> "${RESULTS_FILE}"

  echo "[$i] ${ELAPSED}ms · ${NODE} · ${TEXT}"
done

AVG=$((TOTAL_MS / REQUESTS))
echo ""
echo "Summary:"
echo "  total_ms:  ${TOTAL_MS}"
echo "  avg_ms:    ${AVG}"
echo "  routing:"
sort "${RESULTS_FILE}" | uniq -c | while read -r count node; do
  echo "    - ${node}: ${count} request(s)"
done