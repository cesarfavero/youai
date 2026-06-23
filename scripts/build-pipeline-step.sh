#!/usr/bin/env bash
# Build youai-pipeline-step (llama.cpp native tool for pipeline v3).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${ROOT}/native/build-$(uname -s)-$(uname -m)"
OUT="${ROOT}/target/release/youai-pipeline-step"
if [[ -n "${YOUAI_PIPELINE_STEP_OUT:-}" ]]; then
  OUT="${YOUAI_PIPELINE_STEP_OUT}"
fi

if [[ ! -f "${HOME}/.youai/llama.cpp/include/llama.h" ]]; then
  echo "llama.cpp not found. Run: ./scripts/setup-llama.sh" >&2
  exit 1
fi

mkdir -p "${BUILD_DIR}"
cmake -S "${ROOT}/native" -B "${BUILD_DIR}" -DCMAKE_BUILD_TYPE=Release
cmake --build "${BUILD_DIR}" -j "$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"

install -m 755 "${BUILD_DIR}/youai-pipeline-step" "${OUT}"
echo "Built ${OUT}"