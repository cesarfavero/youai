#!/usr/bin/env bash
# DOGFOOD ONLY — replica round-robin (each machine holds the full small model).
# NOT the product cluster. For the real cluster (one model, many PCs): setup-pipeline-cluster.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Switching to replica dogfood — pipeline nodes must be paused first:"
echo "  youai-node pause   # Mac"
echo "  ./scripts/ubuntu-test-vm.sh pause"
echo "  ${ROOT}/scripts/clean-coordinator.sh http://127.0.0.1:8080"
echo ""

echo "== Mac replica node =="
"${ROOT}/scripts/setup-replica-mac.sh"

echo ""
echo "== Ubuntu replica node (Colima) =="
"${ROOT}/scripts/ubuntu-test-vm.sh" start-node-replica

echo ""
echo "Start Mac node (if not running):"
echo "  YOUAI_BIN_DIR=${ROOT}/target/release ${ROOT}/target/release/youai-node start"
echo ""
echo "Test round-robin:"
echo "  ${ROOT}/scripts/test-replica-round-robin.sh"