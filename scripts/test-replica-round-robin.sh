#!/usr/bin/env bash
# DOGFOOD ONLY — replica round-robin test. Real cluster: test-shard-pipeline-activation.sh
#
# Requires: 2+ replica-capable nodes online and NO complete default-pipeline chain
# (pause pipeline nodes or run ./scripts/clean-coordinator.sh after switching topology).
set -euo pipefail

COORDINATOR_URL="${1:-http://127.0.0.1:8080}"
N="${2:-6}"

echo "Coordinator: ${COORDINATOR_URL}"
echo "Requests:    ${N} (mode=replica, expect distribution across replica nodes)"
echo ""

names=()
modes=()
for i in $(seq 1 "${N}"); do
  resp="$(curl -fsS \
    -H 'Content-Type: application/json' \
    -d "{\"prompt\":\"ping-${i}\",\"max_tokens\":8,\"mode\":\"replica\"}" \
    "${COORDINATOR_URL}/api/v1/chat")"
  node="$(echo "${resp}" | python3 -c 'import sys,json; print(json.load(sys.stdin)["node_name"])')"
  mode="$(echo "${resp}" | python3 -c 'import sys,json; print(json.load(sys.stdin)["mode"])')"
  if [[ "${mode}" != "replica" ]]; then
    echo "FAIL: request #${i} returned mode=${mode} (expected replica)" >&2
    echo "  Hint: pipeline chain may still be online — pause pipeline nodes first." >&2
    exit 1
  fi
  names+=("${node}")
  modes+=("${mode}")
  echo "  #${i}  node=${node}  mode=${mode}"
done

echo ""
echo "== summary =="
printf '%s\n' "${names[@]}" | python3 -c "
import sys
from collections import Counter
names = [l.strip() for l in sys.stdin if l.strip()]
if not names:
    print('no responses')
    sys.exit(1)
unique = len(set(names))
for name, count in Counter(names).most_common():
    print(f'  {name}: {count}/{len(names)}')
if unique < 2:
    print(f'WARN: only {unique} distinct node(s) — expected 2 for round-robin dogfood')
"