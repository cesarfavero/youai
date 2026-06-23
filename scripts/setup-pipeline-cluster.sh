#!/usr/bin/env bash
# Real YouAI cluster: one model split across machines (pipeline activation v3/v4).
# Mac = stage 0, Ubuntu Colima = stage 1. Use mode=auto or mode=pipeline for chat.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Switching to pipeline topology — pause replica nodes and prune stale registrations if needed:"
echo "  ${ROOT}/scripts/clean-coordinator.sh http://127.0.0.1:8080"
echo ""

echo "== Pipeline cluster (Mac stage 0) =="
"${ROOT}/scripts/setup-pipeline-activation-mac.sh"

echo ""
echo "== Pipeline cluster (Ubuntu stage 1) =="
"${ROOT}/scripts/ubuntu-test-vm.sh" start-node-activation

echo ""
echo "Start Mac node (if not running):"
echo "  YOUAI_BIN_DIR=${ROOT}/target/release ${ROOT}/target/release/youai-node start"
echo ""
echo "Verify:"
echo "  ${ROOT}/scripts/test-shard-pipeline-activation.sh"