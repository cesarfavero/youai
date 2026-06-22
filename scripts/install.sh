#!/usr/bin/env bash
# YouAI node installer (scaffold)
# Future: curl -fsSL https://get.youai.network | sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "YouAI installer — scaffold only"
echo "Build from source:"
echo "  cd ${ROOT}"
echo "  cargo build --release --workspace"
echo ""
echo "Binaries will be in: ${ROOT}/target/release/"
echo "  - youai-governor"
echo "  - youai-node"
echo "  - youai-worker"
echo "  - youai-coordinator"