# YouAI Architecture

> Draft v0.1 — June 2026  
> See [MVP.md](./MVP.md) for full product vision.

## Overview

YouAI is a four-layer distributed system. Each layer has a single responsibility; layers communicate over TLS with signed jobs.

```
┌─────────────────────────────────────────────────────────┐
│  LAYER 4 — USER                                         │
│  youai-web · Desktop GUI (Tauri) · Mobile · Public API  │
│  Chat · agents · developer tools                        │
└───────────────────────────┬─────────────────────────────┘
                            │ HTTPS / SSE
┌───────────────────────────▼─────────────────────────────┐
│  LAYER 3 — COORDINATOR (youai-coordinator)              │
│  Auth · credit · MoE routing · job queue · agent harness│
│  Regional super-nodes · replica selection               │
└───────────────────────────┬─────────────────────────────┘
                            │ gRPC + TLS 1.3
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ LAYER 2       │   │ LAYER 2       │   │ LAYER 2       │
│ Super-node    │   │ Super-node    │   │ Super-node    │
│ (anchor)      │   │ (strong PC)   │   │ (org / VPS)   │
└───────┬───────┘   └───────┬───────┘   └───────┬───────┘
        │                   │                   │
   ┌────┴────┐         ┌────┴────┐         ┌────┴────┐
   ▼    ▼    ▼         ▼    ▼    ▼         ▼    ▼    ▼
┌─────────────────────────────────────────────────────────┐
│  LAYER 1 — NODE (youai-node)                            │
│  Millions of contributors: phone · laptop · PC · VPS    │
│  Each node = shard or MoE expert · opt-in only          │
└─────────────────────────────────────────────────────────┘
```

## Monorepo Components

| Crate / App | Layer | Responsibility |
|-------------|-------|----------------|
| `youai-guard` | 1 | RAM/CPU/GPU caps · cgroup sandbox · watchdog |
| `youai-worker` | 1 | llama.cpp inference · reads `~/.youai/shards/` |
| `youai-node` | 1 | CLI/GUI lib · config · start/pause/status |
| `youai-coordinator` | 3 | Node registry · heartbeat · routing · credit |
| `youai-web` | 4 | Chat UI · login · credit balance |

## Node Process Model

On a contributor machine, three processes cooperate:

```
┌──────────────────────────────────────────┐
│  youai-guard (Rust · ~5 MB)           │
│  ├── polls resources every 500ms         │
│  ├── SIGKILL worker on limit breach      │
│  └── logs to ~/.youai/guard.log          │
└───────────────┬──────────────────────────┘
                │ spawns & supervises
┌───────────────▼──────────────────────────┐
│  youai-worker (cgroup sandbox)           │
│  ├── llama.cpp / CUDA                    │
│  └── filesystem: ~/.youai/ only          │
└───────────────┬──────────────────────────┘
                │ TLS outbound only
┌───────────────▼──────────────────────────┐
│  youai-node agent (network)              │
│  ├── register · heartbeat · signed jobs  │
│  └── no remote shell · no inbound ports  │
└──────────────────────────────────────────┘
```

## MVP Data Flow

### Réplica (default quando pipeline indisponível)

```
User (browser)
    │
    ▼ POST /api/v1/chat
youai-coordinator
    │
    ▼ round-robin entre nós com mesmo modelo
youai-node → guard → youai-worker → llama-completion
    │
    ▼ tokens
User ← coordinator
```

### Pipeline (multi-máquina)

O coordinator resolve a cadeia `default-pipeline` (stages 0..N-1) e escolhe o backend:

| Backend | Modo | Resumo |
|---------|------|--------|
| RPC v1 | `pipeline_rpc` | Um `POST /infer` no head + `--rpc` |
| GGUF v2 | `pipeline_gguf` | Stage 0 fetch shards HTTP + infer local |
| Activation v4 | `pipeline_activation_v4` | Coordinator encadeia `POST /pipeline/step` com activações base64 |

```
POST /api/v1/chat  mode=pipeline|auto
        │
        ▼
youai-coordinator (resolve_pipeline → detect_backend)
        │
        ├── v1: POST /infer + rpc_servers
        ├── v2: POST /infer + remote_shards
        └── v4: POST /pipeline/step (prefill → forward → decode loop)
                    │
                    ▼
              youai-pipeline-step --daemon  (modelo quente no worker)
```

Ver [PIPELINE.md](./PIPELINE.md) para setup e troubleshooting.

## MVP Scope vs Future

| Feature | MVP (phase 1) | Later |
|---------|---------------|-------|
| Routing | Replica + pipeline v1–v4 | MoE expert sharding · 3+ stages · activações comprimidas (v5) |
| Models | Nex-N2-mini | N2-Pro, GLM-5.2 |
| Mobile | — | Phase 4 |
| GPU guard | Basic / NVML | Full thermal + pause |
| Auth | Anonymous + device ID | OAuth, enterprise |

## Configuration

Local node config: `~/.youai/config.toml`

```toml
[resources]
cpu_percent = 30
ram_max = "8g"
gpu_percent = 50   # when NVML available
vram_max = "6g"

[coordinator]
url = "https://coordinator.youai.network"
region = "sa-east-1"

[models]
default = "nex-n2-mini"
```

## Network Protocol (planned)

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/nodes/register` | POST | Register node · receive auth token |
| `/nodes/heartbeat` | POST | Every 30s · liveness |
| `/nodes` | GET | List online nodes |
| `/inference` | POST | Signed job dispatch |

Transport: HTTP/JSON for MVP; gRPC for production scale.

## Security Boundaries

See [SECURITY.md](./SECURITY.md). Summary:

- Guard is **independent** of worker — cannot be disabled by inference code
- Worker runs in cgroup with `memory.max` and `cpu.max`
- Coordinator never sends shell commands to nodes
- Prompts are not exposed raw on community nodes (MVP limitation documented)

---

*Draft — will evolve as implementation progresses. See [NEXT_STEPS.md](./NEXT_STEPS.md) for current phase.*