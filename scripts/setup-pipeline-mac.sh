#!/usr/bin/env bash
# Configure Mac node as pipeline stage 0/2.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE="${ROOT}/target/release/youai-node"

"${NODE}" config \
  --name m1-mac \
  --coordinator http://127.0.0.1:8080 \
  --worker-host 127.0.0.1 \
  --worker-port 7741 \
  --shard-group default-pipeline \
  --shard-stage 0 \
  --shard-total-stages 2 \
  --cpu-percent 30 \
  --ram-max 2g

echo "Mac pipeline stage 0 configured."