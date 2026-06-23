#!/usr/bin/env bash
# Clone and build llama.cpp for YouAI local inference.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LLAMA_DIR="${HOME}/.youai/llama.cpp"
BUILD_TYPE="${BUILD_TYPE:-Release}"

if [[ -d "${LLAMA_DIR}/.git" ]]; then
  echo "Updating existing llama.cpp at ${LLAMA_DIR}"
  git -C "${LLAMA_DIR}" pull --ff-only
else
  echo "Cloning llama.cpp into ${LLAMA_DIR}"
  git clone --depth 1 https://github.com/ggml-org/llama.cpp "${LLAMA_DIR}"
fi

mkdir -p "${LLAMA_DIR}/build"

CMAKE_ARGS=(-DCMAKE_BUILD_TYPE="${BUILD_TYPE}" -DGGML_RPC=ON)
if [[ "$(uname -s)" == "Darwin" ]]; then
  echo "macOS detected — enabling Metal (M1/M2/M3)"
  CMAKE_ARGS+=(-DGGML_METAL=ON)
fi
echo "Building llama.cpp with RPC tensor offload (GGML_RPC=ON)"

cmake -S "${LLAMA_DIR}" -B "${LLAMA_DIR}/build" "${CMAKE_ARGS[@]}"

JOBS="$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"
cmake --build "${LLAMA_DIR}/build" --config "${BUILD_TYPE}" -j "${JOBS}"

CLI_BIN="${LLAMA_DIR}/build/bin/llama-completion"
if [[ ! -f "${CLI_BIN}" ]]; then
  CLI_BIN="${LLAMA_DIR}/build/bin/llama-cli"
fi
if [[ ! -f "${CLI_BIN}" ]]; then
  CLI_BIN="${LLAMA_DIR}/build/bin/llama"
fi

if [[ ! -f "${CLI_BIN}" ]]; then
  echo "Error: llama-completion binary not found after build" >&2
  exit 1
fi

echo ""
RPC_BIN="${LLAMA_DIR}/build/bin/rpc-server"
echo "llama.cpp ready (YouAI uses llama-completion for one-shot inference):"
echo "  ${CLI_BIN}"
if [[ -f "${RPC_BIN}" ]]; then
  echo "  ${RPC_BIN}  (RPC tensor backend for distributed inference)"
fi
echo ""
echo "Optional config (~/.youai/config.toml):"
echo "  [runtime]"
echo "  llama_cli = \"${CLI_BIN}\""