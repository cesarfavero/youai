#!/usr/bin/env bash
# YouAI local installer — build binaries and optional model/llama.cpp setup.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "YouAI installer"
echo "Repository: ${ROOT}"
echo ""

echo "== 1/3 Build Rust workspace =="
cargo build --release --manifest-path "${ROOT}/Cargo.toml"

BIN_DIR="${ROOT}/target/release"
echo ""
echo "Binaries:"
echo "  ${BIN_DIR}/youai-guard"
echo "  ${BIN_DIR}/youai-node"
echo "  ${BIN_DIR}/youai-worker"
echo "  ${BIN_DIR}/youai-coordinator"
echo ""

echo "== 2/3 llama.cpp (optional but required for inference) =="
read -r -p "Build llama.cpp now? [Y/n] " BUILD_LLAMA
BUILD_LLAMA="${BUILD_LLAMA:-Y}"
if [[ "${BUILD_LLAMA}" =~ ^[Yy]$ ]]; then
  "${ROOT}/scripts/setup-llama.sh"
else
  echo "Skipped. Run ./scripts/setup-llama.sh later."
fi

echo ""
echo "== 3/3 Tiny test model (~220 MB) =="
read -r -p "Download SmolLM2-360M now? [Y/n] " DOWNLOAD_MODEL
DOWNLOAD_MODEL="${DOWNLOAD_MODEL:-Y}"
if [[ "${DOWNLOAD_MODEL}" =~ ^[Yy]$ ]]; then
  "${ROOT}/scripts/download-model.sh"
else
  echo "Skipped. Run ./scripts/download-model.sh later."
fi

echo ""
echo "Done. Add to PATH for this session:"
echo "  export PATH=\"${BIN_DIR}:\$PATH\""
echo "  export YOUAI_BIN_DIR=\"${BIN_DIR}\""
echo ""
echo "Quick 2-machine test:"
echo "  1) Coordinator (machine A): youai-coordinator --host 0.0.0.0 --port 8080"
echo "  2) Node (each machine):"
echo "       youai-node config --coordinator http://IP_A:8080 --worker-host LAN_IP --name m1"
echo "       youai-node start"
echo "  3) Chat: ./scripts/test-cluster.sh http://IP_A:8080"