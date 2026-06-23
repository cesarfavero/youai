#!/usr/bin/env bash
# Benchmark a local GGUF model via youai-worker + llama.cpp
# Usage: ./scripts/benchmark-model.sh [--model PATH] [--prompt TEXT]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL=""
PROMPT="Hello, YouAI."

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model)
      MODEL="$2"
      shift 2
      ;;
    --prompt)
      PROMPT="$2"
      shift 2
      ;;
    -h|--help)
      echo "Usage: $0 [--model PATH] [--prompt TEXT]"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "${MODEL}" ]]; then
  MODEL="${HOME}/.youai/models/smollm2-360m-instruct-q4_k_m.gguf"
fi

if [[ ! -f "${MODEL}" ]]; then
  echo "Error: model file not found: ${MODEL}" >&2
  echo "Run: ./scripts/download-model.sh" >&2
  exit 1
fi

WORKER_BIN="${ROOT}/target/release/youai-worker"
if [[ ! -f "${WORKER_BIN}" ]]; then
  WORKER_BIN="${ROOT}/target/debug/youai-worker"
fi
if [[ ! -f "${WORKER_BIN}" ]]; then
  echo "Building youai-worker..."
  cargo build -p youai-worker --manifest-path "${ROOT}/Cargo.toml"
  WORKER_BIN="${ROOT}/target/debug/youai-worker"
fi

echo "YouAI model benchmark"
echo "Model:  ${MODEL}"
echo "Prompt: ${PROMPT}"
echo ""

START="$(date +%s)"
OUT="$("${WORKER_BIN}" infer --model "${MODEL}" --prompt "${PROMPT}" --max-tokens 64)"
END="$(date +%s)"
ELAPSED="$((END - START))"

echo "Response (${ELAPSED}s):"
echo "${OUT}"
echo ""
echo "Record in docs/NEXT_STEPS.md benchmark table:"
echo "  model: ${MODEL}"
echo "  elapsed_s: ${ELAPSED}"