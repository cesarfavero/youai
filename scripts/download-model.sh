#!/usr/bin/env bash
# Download the default tiny YouAI test model (SmolLM2-360M-Instruct Q4_K_M, ~220 MB).
set -euo pipefail

MODEL_DIR="${HOME}/.youai/models"
MODEL_FILE="smollm2-360m-instruct-q4_k_m.gguf"
MODEL_PATH="${MODEL_DIR}/${MODEL_FILE}"

# bartowski quant on Hugging Face
MODEL_URL="https://huggingface.co/bartowski/SmolLM2-360M-Instruct-GGUF/resolve/main/SmolLM2-360M-Instruct-Q4_K_M.gguf"

mkdir -p "${MODEL_DIR}"

if [[ -f "${MODEL_PATH}" ]]; then
  echo "Model already exists: ${MODEL_PATH}"
  ls -lh "${MODEL_PATH}"
  exit 0
fi

echo "Downloading SmolLM2-360M-Instruct Q4_K_M (~220 MB)..."
echo "Destination: ${MODEL_PATH}"

if command -v curl >/dev/null 2>&1; then
  curl -fL --progress-bar "${MODEL_URL}" -o "${MODEL_PATH}"
elif command -v wget >/dev/null 2>&1; then
  wget -O "${MODEL_PATH}" "${MODEL_URL}"
else
  echo "Error: need curl or wget" >&2
  exit 1
fi

echo ""
echo "Done."
ls -lh "${MODEL_PATH}"
echo ""
echo "Test locally:"
echo "  youai-worker infer --prompt 'Hello from YouAI'"