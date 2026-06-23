#!/usr/bin/env bash
# Split a GGUF model into N parts for distributed pipeline (v2).
# Uses llama-gguf-split; output files: <prefix>-00001-of-0000N.gguf
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INPUT="${1:-${HOME}/.youai/models/smollm2-360m-instruct-q4_k_m.gguf}"
SPLITS="${2:-2}"
OUT_DIR="${3:-${HOME}/.youai/shards}"

LLAMA_SPLIT="${HOME}/.youai/llama.cpp/build/bin/llama-gguf-split"
if [[ ! -f "${LLAMA_SPLIT}" ]]; then
  echo "llama-gguf-split not found. Run: ./scripts/setup-llama.sh" >&2
  exit 1
fi
if [[ ! -f "${INPUT}" ]]; then
  echo "Model not found: ${INPUT}" >&2
  exit 1
fi

mkdir -p "${OUT_DIR}"
BASENAME="$(basename "${INPUT}" .gguf)"
PREFIX="${OUT_DIR}/${BASENAME}"

# Dry-run to count tensors, then pick max-tensors per split for exactly N parts.
N_TENSORS="$("${LLAMA_SPLIT}" --dry-run "${INPUT}" "${PREFIX}" 2>&1 | awk '/with a total of/ {print $(NF-1)}')"
if [[ -z "${N_TENSORS}" || "${N_TENSORS}" -lt 1 ]]; then
  echo "Could not read tensor count from dry-run" >&2
  exit 1
fi
MAX_TENSORS=$(( (N_TENSORS + SPLITS - 1) / SPLITS ))

echo "Splitting ${INPUT}"
echo "  tensors: ${N_TENSORS} → ${SPLITS} parts (max ${MAX_TENSORS} tensors/split)"
echo "  output:  ${PREFIX}-00001-of-$(printf '%05d' "${SPLITS}").gguf ..."

"${LLAMA_SPLIT}" --split --split-max-tensors "${MAX_TENSORS}" "${INPUT}" "${PREFIX}"

echo ""
echo "Shard files:"
ls -lh "${PREFIX}"-*.gguf
echo ""
echo "Stage 0: --model-path ${PREFIX}-00001-of-$(printf '%05d' "${SPLITS}").gguf --gguf-shard-index 0 --gguf-shard-total ${SPLITS}"
echo "Stage 1: --model-path ${PREFIX}-00002-of-$(printf '%05d' "${SPLITS}").gguf --gguf-shard-index 1 --gguf-shard-total ${SPLITS}"