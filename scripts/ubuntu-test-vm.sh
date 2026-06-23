#!/usr/bin/env bash
# Lightweight Ubuntu for YouAI 2-machine test.
# Uses Colima (Ubuntu 24.04 VM) — install: brew install colima && colima start
#
# Usage:
#   ./scripts/ubuntu-test-vm.sh create      # bootstrap tools + build
#   ./scripts/ubuntu-test-vm.sh start-node  # run node (coordinator on Mac)
#   ./scripts/ubuntu-test-vm.sh shell       # SSH into Ubuntu
#   ./scripts/ubuntu-test-vm.sh status
#   ./scripts/ubuntu-test-vm.sh pause
#   ./scripts/ubuntu-test-vm.sh destroy     # stop colima VM
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
YOUAI_MAC_HOME="${YOUAI_MAC_HOME:-$HOME}"
YOUAI_MAC_DATA="${YOUAI_MAC_HOME}/.youai"
# Repo is virtiofs-mounted from macOS — never use ROOT/target/release inside the VM
# (those are Mach-O binaries). Build Linux artifacts in the VM home instead.
VM_TARGET_DIR="${YOUAI_VM_TARGET_DIR:-\$HOME/.youai/cargo-target}"
COORDINATOR_URL="${YOUAI_COORDINATOR_URL:-http://host.lima.internal:8080}"
WORKER_PORT="${YOUAI_WORKER_PORT:-7741}"
# Colima vz networking is not routable from macOS — forward VM worker to Mac localhost.
WORKER_FORWARD_PORT="${YOUAI_WORKER_FORWARD_PORT:-7742}"
WORKER_ADVERTISE="${YOUAI_WORKER_ADVERTISE:-http://127.0.0.1:${WORKER_FORWARD_PORT}}"
RPC_PORT="${YOUAI_RPC_PORT:-50052}"
RPC_FORWARD_PORT="${YOUAI_RPC_FORWARD_PORT:-50052}"
RPC_ADVERTISE="${YOUAI_RPC_ADVERTISE:-127.0.0.1:${RPC_FORWARD_PORT}}"
LIMA_SSH_CONFIG="${HOME}/.colima/_lima/colima/ssh.config"

require_colima() {
  if ! command -v colima >/dev/null 2>&1; then
    echo "Colima not installed. Run: brew install colima lima && colima start --cpu 2 --memory 4 --disk 25" >&2
    exit 1
  fi
  if ! colima status >/dev/null 2>&1; then
    echo "Starting Colima (Ubuntu VM)..."
    colima start --cpu 2 --memory 4 --disk 25
  fi
}

colima_sh() {
  colima ssh -- "$@"
}

ensure_rpc_port_forward() {
  if [[ ! -f "${LIMA_SSH_CONFIG}" ]]; then
    echo "Lima SSH config missing at ${LIMA_SSH_CONFIG}" >&2
    exit 1
  fi
  if lsof -i ":${RPC_FORWARD_PORT}" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "RPC port-forward already listening on 127.0.0.1:${RPC_FORWARD_PORT}"
    return 0
  fi
  echo "Forwarding Mac 127.0.0.1:${RPC_FORWARD_PORT} -> VM rpc-server :${RPC_PORT}..."
  ssh -F "${LIMA_SSH_CONFIG}" -N -L "${RPC_FORWARD_PORT}:127.0.0.1:${RPC_PORT}" lima-colima &
  echo $! > /tmp/youai-vm-rpc-forward.pid
  sleep 1
}

ensure_worker_port_forward() {
  if [[ ! -f "${LIMA_SSH_CONFIG}" ]]; then
    echo "Lima SSH config missing at ${LIMA_SSH_CONFIG}" >&2
    exit 1
  fi
  if lsof -i ":${WORKER_FORWARD_PORT}" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "Worker port-forward already listening on 127.0.0.1:${WORKER_FORWARD_PORT}"
    return 0
  fi
  echo "Forwarding Mac 127.0.0.1:${WORKER_FORWARD_PORT} -> VM worker :${WORKER_PORT}..."
  ssh -F "${LIMA_SSH_CONFIG}" -N -L "${WORKER_FORWARD_PORT}:127.0.0.1:${WORKER_PORT}" lima-colima &
  echo $! > /tmp/youai-vm-forward.pid
  sleep 1
}

bootstrap() {
  require_colima
  echo "Bootstrapping Ubuntu in Colima (first run ~10-15 min)..."
  colima_sh bash -s <<BOOT
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
export HOME="\${HOME:-/home/\$(whoami)}"

if [[ ! -f "\$HOME/.youai-bootstrapped" ]]; then
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends \
    build-essential cmake git curl ca-certificates pkg-config python3
  sudo rm -rf /var/lib/apt/lists/*
  if ! command -v rustc >/dev/null 2>&1; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y
  fi
  touch "\$HOME/.youai-bootstrapped"
fi

source "\$HOME/.cargo/env"
export CARGO_TARGET_DIR=${VM_TARGET_DIR}
cd ${ROOT}

echo "Building YouAI (Linux target: ${VM_TARGET_DIR})..."
cargo build --release --workspace

if [[ ! -f "\$HOME/.youai/llama.cpp/build/bin/llama-completion" ]] \
   || [[ ! -f "\$HOME/.youai/llama.cpp/build/bin/rpc-server" ]]; then
  echo "Building llama.cpp (with RPC)..."
  ${ROOT}/scripts/setup-llama.sh
fi

mkdir -p "\$HOME/.youai/models"
if [[ ! -f "\$HOME/.youai/models/smollm2-360m-instruct-q4_k_m.gguf" ]]; then
  if [[ -f ${YOUAI_MAC_DATA}/models/smollm2-360m-instruct-q4_k_m.gguf ]]; then
    ln -sf ${YOUAI_MAC_DATA}/models/smollm2-360m-instruct-q4_k_m.gguf "\$HOME/.youai/models/"
  else
    ${ROOT}/scripts/download-model.sh
  fi
fi

echo "Bootstrap complete."
BOOT
}

start_node_gguf() {
  bootstrap
  ensure_worker_port_forward
  echo "Starting youai-node (GGUF v2 shard 1) in Colima Ubuntu..."
  echo "  Coordinator (Mac): ${COORDINATOR_URL}"
  echo "  Worker advertise: ${WORKER_ADVERTISE}"

  colima_sh bash -s <<START
set -euo pipefail
export HOME="\${HOME:-/home/\$(whoami)}"
source "\$HOME/.cargo/env"
export CARGO_TARGET_DIR=${VM_TARGET_DIR}
export PATH=${VM_TARGET_DIR}/release:\$PATH
export YOUAI_BIN_DIR=${VM_TARGET_DIR}/release
mkdir -p "\$HOME/.youai/shards"
MAC_SHARDS="${YOUAI_MAC_DATA}/shards"
SHARD1="\$(ls "\$MAC_SHARDS"/*-00002-of-00002.gguf 2>/dev/null | head -1 || true)"
if [[ -z "\$SHARD1" ]]; then
  echo "Mac GGUF shard 2 not found. Run: ./scripts/setup-pipeline-gguf-mac.sh" >&2
  exit 1
fi
cp -f "\$SHARD1" "\$HOME/.youai/shards/"
SHARD_VM="\$HOME/.youai/shards/\$(basename "\$SHARD1")"

youai-node pause 2>/dev/null || true
pkill -f 'youai-worker serve' 2>/dev/null || true
pkill -f 'youai-guard' 2>/dev/null || true
if [[ -f /tmp/youai-node.pid ]]; then rm -f /tmp/youai-node.pid; fi

youai-node config \
  --name ubuntu-colima \
  --coordinator ${COORDINATOR_URL} \
  --worker-host 0.0.0.0 \
  --worker-port ${WORKER_PORT} \
  --worker-advertise-url ${WORKER_ADVERTISE} \
  --shard-group default-pipeline \
  --shard-stage 1 \
  --shard-total-stages 2 \
  --gguf-shard-index 1 \
  --gguf-shard-total 2 \
  --clear-rpc-url \
  --model-path "\$SHARD_VM" \
  --cpu-percent 30 \
  --ram-max 2g

nohup youai-node start > /tmp/youai-node.log 2>&1 &
echo \$! > /tmp/youai-node.pid
sleep 2
tail -5 /tmp/youai-node.log || true
START
}

start_node_replica() {
  bootstrap
  ensure_worker_port_forward
  echo "Starting youai-node (replica — full GGUF) in Colima Ubuntu..."
  echo "  Coordinator (Mac): ${COORDINATOR_URL}"
  echo "  Worker advertise: ${WORKER_ADVERTISE}"

  colima_sh bash -s <<START
set -euo pipefail
export HOME="\${HOME:-/home/\$(whoami)}"
source "\$HOME/.cargo/env"
export CARGO_TARGET_DIR=${VM_TARGET_DIR}
export PATH=${VM_TARGET_DIR}/release:\$PATH
export YOUAI_BIN_DIR=${VM_TARGET_DIR}/release
mkdir -p "\$HOME/.youai/models"
MODEL="\$HOME/.youai/models/smollm2-360m-instruct-q4_k_m.gguf"
if [[ ! -f "\$MODEL" ]]; then
  MAC_MODEL="${YOUAI_MAC_DATA}/models/smollm2-360m-instruct-q4_k_m.gguf"
  if [[ -f "\$MAC_MODEL" ]]; then
    ln -sf "\$MAC_MODEL" "\$MODEL"
  else
    echo "Full model not found. Run on Mac: ./scripts/download-model.sh" >&2
    exit 1
  fi
fi

youai-node pause 2>/dev/null || true
pkill -f 'youai-worker serve' 2>/dev/null || true
pkill -f 'youai-guard' 2>/dev/null || true
if [[ -f /tmp/youai-node.pid ]]; then rm -f /tmp/youai-node.pid; fi

youai-node config \
  --name ubuntu-colima \
  --coordinator ${COORDINATOR_URL} \
  --worker-host 0.0.0.0 \
  --worker-port ${WORKER_PORT} \
  --worker-advertise-url ${WORKER_ADVERTISE} \
  --shard-group "" \
  --shard-stage 0 \
  --shard-total-stages 1 \
  --gguf-shard-index 0 \
  --gguf-shard-total 1 \
  --pipeline-kind "" \
  --clear-rpc-url \
  --model-path "\$MODEL" \
  --cpu-percent 30 \
  --ram-max 2g

nohup youai-node start > /tmp/youai-node.log 2>&1 &
echo \$! > /tmp/youai-node.pid
sleep 3
tail -8 /tmp/youai-node.log || true
START
}

start_node_activation() {
  bootstrap
  ensure_worker_port_forward
  echo "Starting youai-node (pipeline v3 stage 1) in Colima Ubuntu..."
  echo "  Coordinator (Mac): ${COORDINATOR_URL}"
  echo "  Worker advertise: ${WORKER_ADVERTISE}"

  colima_sh bash -s <<START
set -euo pipefail
export HOME="\${HOME:-/home/\$(whoami)}"
source "\$HOME/.cargo/env"
export CARGO_TARGET_DIR=${VM_TARGET_DIR}
export PATH=${VM_TARGET_DIR}/release:\$PATH
export YOUAI_BIN_DIR=${VM_TARGET_DIR}/release
mkdir -p "\$HOME/.youai/pipeline-stages"
MAC_STAGES="${YOUAI_MAC_DATA}/pipeline-stages"
STAGE1="\$(ls "\$MAC_STAGES"/*-stage01-of-02.gguf 2>/dev/null | head -1 || true)"
if [[ -z "\$STAGE1" ]]; then
  echo "Mac pipeline stage 1 GGUF not found. Run: ./scripts/setup-pipeline-activation-mac.sh" >&2
  exit 1
fi
cp -f "\$STAGE1" "\$HOME/.youai/pipeline-stages/"
STAGE_VM="\$HOME/.youai/pipeline-stages/\$(basename "\$STAGE1")"

export YOUAI_PIPELINE_STEP_OUT="\${YOUAI_BIN_DIR}/youai-pipeline-step"
${ROOT}/scripts/build-pipeline-step.sh

youai-node pause 2>/dev/null || true
pkill -f 'youai-worker serve' 2>/dev/null || true
pkill -f 'youai-guard' 2>/dev/null || true
if [[ -f /tmp/youai-node.pid ]]; then rm -f /tmp/youai-node.pid; fi

youai-node config \
  --name ubuntu-colima \
  --coordinator ${COORDINATOR_URL} \
  --worker-host 0.0.0.0 \
  --worker-port ${WORKER_PORT} \
  --worker-advertise-url ${WORKER_ADVERTISE} \
  --shard-group default-pipeline \
  --shard-stage 1 \
  --shard-total-stages 2 \
  --pipeline-kind activation \
  --gguf-shard-index 0 \
  --gguf-shard-total 1 \
  --clear-rpc-url \
  --model-path "\$STAGE_VM" \
  --cpu-percent 80 \
  --ram-max 4g

export YOUAI_PIPELINE_DAEMON=1
nohup youai-node start > /tmp/youai-node.log 2>&1 &
echo \$! > /tmp/youai-node.pid
sleep 2
tail -5 /tmp/youai-node.log || true
START
}

start_node() {
  bootstrap
  ensure_worker_port_forward
  ensure_rpc_port_forward
  echo "Starting rpc-server + youai-node in Colima Ubuntu..."
  echo "  Coordinator (Mac): ${COORDINATOR_URL}"
  echo "  Worker advertise: ${WORKER_ADVERTISE} (via SSH forward :${WORKER_FORWARD_PORT})"
  echo "  RPC advertise:    ${RPC_ADVERTISE} (via SSH forward :${RPC_FORWARD_PORT})"

  colima_sh bash -s <<START
set -euo pipefail
export HOME="\${HOME:-/home/\$(whoami)}"
source "\$HOME/.cargo/env"
export CARGO_TARGET_DIR=${VM_TARGET_DIR}
export PATH=${VM_TARGET_DIR}/release:\$PATH
export YOUAI_BIN_DIR=${VM_TARGET_DIR}/release
RPC_BIN="\$HOME/.youai/llama.cpp/build/bin/rpc-server"

if [[ -f /tmp/youai-rpc-server.pid ]] && kill -0 \$(cat /tmp/youai-rpc-server.pid) 2>/dev/null; then
  echo "rpc-server already running (pid \$(cat /tmp/youai-rpc-server.pid))"
else
  nohup "\$RPC_BIN" -H 0.0.0.0 -p ${RPC_PORT} > /tmp/youai-rpc-server.log 2>&1 &
  echo \$! > /tmp/youai-rpc-server.pid
  sleep 1
  echo "rpc-server PID: \$(cat /tmp/youai-rpc-server.pid)"
fi

youai-node config \
  --name ubuntu-colima \
  --coordinator ${COORDINATOR_URL} \
  --worker-host 0.0.0.0 \
  --worker-port ${WORKER_PORT} \
  --worker-advertise-url ${WORKER_ADVERTISE} \
  --rpc-url ${RPC_ADVERTISE} \
  --shard-group default-pipeline \
  --shard-stage 1 \
  --shard-total-stages 2 \
  --cpu-percent 30 \
  --ram-max 2g

nohup youai-node start > /tmp/youai-node.log 2>&1 &
echo \$! > /tmp/youai-node.pid
sleep 2
echo "youai-node PID: \$(cat /tmp/youai-node.pid)"
tail -5 /tmp/youai-node.log || true
START

  echo ""
  echo "Logs: ./scripts/ubuntu-test-vm.sh logs"
}

logs() {
  colima_sh tail -f /tmp/youai-node.log
}

status() {
  require_colima
  colima status
  echo ""
  colima_sh bash -s <<'STAT'
export HOME="${HOME:-/home/$(whoami)}"
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.youai/cargo-target/release:$PATH"
if [[ -f /tmp/youai-node.pid ]] && kill -0 $(cat /tmp/youai-node.pid) 2>/dev/null; then
  echo "youai-node: running (pid $(cat /tmp/youai-node.pid))"
else
  echo "youai-node: not running"
fi
youai-node status 2>/dev/null || true
STAT
}

pause_node() {
  if [[ -f /tmp/youai-vm-forward.pid ]]; then
    kill "$(cat /tmp/youai-vm-forward.pid)" 2>/dev/null || true
    rm -f /tmp/youai-vm-forward.pid
  fi
  if [[ -f /tmp/youai-vm-rpc-forward.pid ]]; then
    kill "$(cat /tmp/youai-vm-rpc-forward.pid)" 2>/dev/null || true
    rm -f /tmp/youai-vm-rpc-forward.pid
  fi
  colima_sh bash -s <<'PAUSE'
export HOME="${HOME:-/home/$(whoami)}"
source "$HOME/.cargo/env"
export PATH="$HOME/.youai/cargo-target/release:$PATH"
youai-node pause 2>/dev/null || true
pkill -f 'youai-worker serve' 2>/dev/null || true
pkill -f 'youai-guard' 2>/dev/null || true
if [[ -f /tmp/youai-node.pid ]]; then kill $(cat /tmp/youai-node.pid) 2>/dev/null || true; rm -f /tmp/youai-node.pid; fi
if [[ -f /tmp/youai-rpc-server.pid ]]; then kill $(cat /tmp/youai-rpc-server.pid) 2>/dev/null || true; rm -f /tmp/youai-rpc-server.pid; fi
PAUSE
  echo "Node paused"
}

shell() {
  require_colima
  colima ssh
}

destroy() {
  pause_node 2>/dev/null || true
  colima stop 2>/dev/null || true
  echo "Colima stopped. Remove completely: colima delete"
}

cmd="${1:-create}"
case "${cmd}" in
  create) bootstrap ;;
  shell) shell ;;
  start-node) start_node ;;
  start-node-gguf) start_node_gguf ;;
  start-node-activation) start_node_activation ;;
  start-node-replica) start_node_replica ;;
  status) status ;;
  logs) logs ;;
  pause) pause_node ;;
  destroy) destroy ;;
  -h|--help)
    cat <<EOF
Usage: $0 {create|start-node|start-node-gguf|start-node-activation|start-node-replica|status|logs|shell|pause|destroy}

Prereq: brew install colima lima && colima start

On Mac (other terminal):
  youai-coordinator --host 0.0.0.0 --port 8080
  youai-node config --coordinator http://127.0.0.1:8080 --worker-host 127.0.0.1 --name m1
  youai-node start
  ./scripts/test-cluster.sh http://127.0.0.1:8080
EOF
    ;;
  *)
    echo "Unknown: ${cmd}" >&2
    exit 1
    ;;
esac