# YouAI — Pipeline distribuído (sharding)

> Guia prático: réplica vs pipeline, v0/v1 implementados, e roadmap v2.

## Modos de inferência

| Modo | O que faz | Quando usar |
|------|-----------|-------------|
| **replica** | Mesmo modelo em N nós; coordinator faz round-robin | Mais throughput (vários chats em paralelo) |
| **pipeline** | Um pedido passa por uma cadeia de estágios | Uma inferência usa várias máquinas |
| **auto** (default) | Pipeline se a cadeia `default-pipeline` estiver completa; senão réplica | Chat normal |

O coordinator expõe `POST /api/v1/chat` com `mode`: `auto` | `replica` | `pipeline`.

## Evolução do pipeline

### v0 — prompt chain (legado, removido no v1)

Cada estágio rodava o **modelo completo** com prompts diferentes (simulava prefill → decode). Útil para validar roteamento multi-máquina, mas **não** partia tensores.

### v1 — RPC tensor split (atual) ✅

Uma única inferência no **stage 0** com `llama.cpp --rpc`:

```
┌─────────────────────────────────────────────────────────┐
│  Mac (stage 0)                                          │
│  youai-worker → llama-completion -m model.gguf          │
│                 --rpc 127.0.0.1:50052                   │
└───────────────────────────┬─────────────────────────────┘
                            │ TCP (GGML RPC)
┌───────────────────────────▼─────────────────────────────┐
│  VM (stage 1)                                           │
│  rpc-server -H 0.0.0.0 -p 50052                         │
│  (tensores offload — CPU/Metal conforme memória)          │
└─────────────────────────────────────────────────────────┘
```

O llama.cpp distribui **pesos e KV cache** entre dispositivos locais e remotos proporcionalmente à RAM livre. Não é atribuição manual de camadas.

**Componentes novos:**

- `rpc_url` no registro do nó (stage 1+)
- `InferRequest.rpc_servers` → worker passa `--rpc` ao `llama-completion`
- `pipeline.rs` — um `POST /infer` no head + health TCP nos backends RPC
- `scripts/ubuntu-test-vm.sh` — sobe `rpc-server`, port-forward `:50052`

### v2 — GGUF por camadas (próximo)

Split explícito com `llama-gguf-split`: cada nó carrega um subconjunto de camadas do GGUF. Requer loader custom ou múltiplos ficheiros + orquestração de activações entre estágios.

---

## Configuração do pipeline (2 máquinas)

### Pré-requisitos

- llama.cpp com `GGML_RPC=ON`: `./scripts/setup-llama.sh`
- Modelo: `./scripts/download-model.sh` (default: SmolLM2-360M)
- Mac: Rust release build `cargo build --release --workspace`
- VM (opcional): Colima + `./scripts/ubuntu-test-vm.sh create`

### 1. Coordinator (Mac)

```bash
./target/release/youai-coordinator \
  --host 0.0.0.0 --port 8080 \
  --db ~/.youai/coordinator.db
```

### 2. Stage 0 — Mac

```bash
./scripts/setup-pipeline-mac.sh
./target/release/youai-node start
```

Config gerada em `~/.youai/config.toml`:

- `shard.group = "default-pipeline"`
- `shard.stage = 0`, `shard.total_stages = 2`

### 3. Stage 1 — VM Ubuntu (Colima)

```bash
./scripts/ubuntu-test-vm.sh start-node
```

Isto:

1. Compila YouAI + llama.cpp (RPC) na VM
2. Port-forward Mac `127.0.0.1:7742` → worker VM
3. Port-forward Mac `127.0.0.1:50052` → `rpc-server` VM
4. Registra nó com `rpc_url = "127.0.0.1:50052"`

### 4. Teste

```bash
./scripts/test-shard-pipeline.sh
# ou
curl -fsS -H 'Content-Type: application/json' \
  -d '{"prompt":"Olá","max_tokens":64,"mode":"pipeline"}' \
  http://127.0.0.1:8080/api/v1/chat | jq
```

Resposta esperada:

- `mode`: `"pipeline"`
- `stages[0]`: texto da inferência (Mac)
- `stages[1]`: `"[rpc tensor backend @ 127.0.0.1:50052]"`

---

## API — campos de shard e RPC

### Registro (`POST /api/v1/nodes/register`)

```json
{
  "name": "ubuntu-colima",
  "worker_url": "http://127.0.0.1:7742",
  "model": "smollm2-360m-instruct",
  "shard_group": "default-pipeline",
  "shard_stage": 1,
  "shard_total_stages": 2,
  "rpc_url": "127.0.0.1:50052"
}
```

Stage 0 deixa `rpc_url` vazio. Stages 1+ **devem** expor `rpc_url` para pipeline v1.

### CLI do nó

```bash
youai-node config \
  --shard-group default-pipeline \
  --shard-stage 1 \
  --shard-total-stages 2 \
  --rpc-url 127.0.0.1:50052
```

### Inferência no worker (`POST /infer`)

```json
{
  "prompt": "...",
  "max_tokens": 128,
  "rpc_servers": ["127.0.0.1:50052"]
}
```

---

## Scripts de operação

| Script | Função |
|--------|--------|
| `scripts/setup-pipeline-mac.sh` | Configura Mac como stage 0 |
| `scripts/ubuntu-test-vm.sh` | VM Colima: bootstrap, `rpc-server`, node stage 1 |
| `scripts/test-shard-pipeline.sh` | Teste end-to-end pipeline v1 |
| `scripts/clean-coordinator.sh` | Prune nós stale/duplicados |
| `scripts/benchmark-cluster.sh` | Benchmark réplica no cluster |
| `scripts/setup-llama.sh` | Build llama.cpp + `rpc-server` (GGML_RPC) |

---

## Troubleshooting

| Sintoma | Causa provável | Fix |
|---------|----------------|-----|
| `pipeline mode requested but no complete shard chain` | Faltam estágios ou modelo diferente | Verificar `GET /api/v1/nodes`, stages 0..N-1 online |
| `stage N has no rpc_url` | VM sem `--rpc-url` no registro | `ubuntu-test-vm.sh start-node` ou `youai-node config --rpc-url ...` |
| `rpc ... is unreachable` | Port-forward :50052 morto | Reiniciar `ubuntu-test-vm.sh start-node` |
| `connection refused` no register | Coordinator não está em `0.0.0.0:8080` | Subir coordinator antes dos nós |
| VM não alcança coordinator | Colima | Usar `http://host.lima.internal:8080` (já no script) |

---

## Segurança (RPC)

O backend RPC do llama.cpp está em **prova de conceito** — não expor `rpc-server` na internet aberta. No YouAI, o forward fica em `127.0.0.1` no Mac; uso apenas em rede de teste local.

---

## Próximo: v2

Ver [NEXT_STEPS.md](./NEXT_STEPS.md#pipeline-v2--gguf-por-camadas). Objetivo: ficheiros GGUF partidos por intervalo de camadas e activações explícitas entre nós (sem depender só de memória proporcional do RPC).