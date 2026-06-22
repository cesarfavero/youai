#!/usr/bin/env bash
# Benchmark a local GGUF model via youai-worker + llama.cpp
# Usage: ./scripts/benchmark-model.sh --model ~/.youai/models/n2-mini.gguf
set -euo pipefail

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
      echo "Usage: $0 --model PATH [--prompt TEXT]"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "${MODEL}" ]]; then
  echo "Error: --model is required" >&2
  echo "Example: $0 --model ~/.youai/models/n2-mini.gguf" >&2
  exit 1
fi

if [[ ! -f "${MODEL}" ]]; then
  echo "Error: model file not found: ${MODEL}" >&2
  exit 1
fi

echo "YouAI model benchmark — scaffold"
echo "Model:  ${MODEL}"
echo "Prompt: ${PROMPT}"
echo ""
echo "TODO (docs/NEXT_STEPS.md passo 4):"
echo "  1. Build llama.cpp"
echo "  2. Run inference and measure tokens/s, RAM peak, VRAM peak"
echo "  3. Record results in docs/NEXT_STEPS.md benchmark table"