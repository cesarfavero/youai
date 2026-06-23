# YouAI — Pipeline distribuído (sharding)

> Guia prático: réplica vs pipeline, v1–v4 implementados, setup e troubleshooting.

## Modos de inferência

| Modo | O que faz | Quando usar |
|------|-----------|-------------|
| **replica** | Mesmo modelo em N nós; coordinator faz round-robin | Mais throughput (vários chats em paralelo) |
| **pipeline** | Um pedido passa por uma cadeia de estágios | Uma inferência usa várias máquinas |
| **auto** (default) | Pipeline se a cadeia `default-pipeline` estiver completa; senão réplica | Chat normal |

O coordinator expõe `POST /api/v1/chat` com `mode`: `auto` | `replica` | `pipeline`.

## Evolução do pipeline

| Versão | Modo na resposta | Ideia |
|--------|------------------|-------|
| **v1 RPC** | `pipeline_rpc` | Um infer no stage 0; tensores offload via `rpc-server` |
| **v2 GGUF** | `pipeline_gguf` | Cada nó tem um shard GGUF; stage 0 junta splits via HTTP |
| **v3 Activ.** | `pipeline_activation` (legado) | Layer-split GGUF + activações base64 entre estágios (subprocess por step) |
| **v4 Daemon** | `pipeline_activation_v4` | Igual v3, mas modelo fica carregado (`--daemon`) — sem reload por token |

O coordinator escolhe o backend automaticamente:

- `pipeline_kind=activation` em todos os estágios → **v4** (activações)
- `gguf_shard_total >= 2` → **v2** (GGUF distribuído)
- `rpc_url` nos stages 1+ → **v1** (RPC)

---

### v1 — RPC tensor split ✅

```
Mac (stage 0)  llama-completion -m model.gguf --rpc 127.0.0.1:50052
        │ TCP (GGML RPC)
VM (stage 1)   rpc-server -H 0.0.0.0 -p 50052
```

```bash
./scripts/setup-pipeline-mac.sh
./scripts/ubuntu-test-vm.sh start-node
./scripts/test-shard-pipeline.sh
```

### v2 — GGUF distribuído ✅

Cada nó guarda só o seu `*-0000N-of-0000M.gguf`. Stage 0 faz `GET /model/shard` nos peers e corre um único infer.

```bash
./scripts/split-model.sh MODEL 2 ~/.youai/shards
./scripts/setup-pipeline-gguf-mac.sh
./scripts/ubuntu-test-vm.sh start-node-gguf
./scripts/test-shard-pipeline-gguf.sh
```

### v3 — Activações por camada ✅

Cada estágio carrega um **GGUF standalone** com só as suas camadas (`split-model-layers.py`). O coordinator encadeia:

1. Stage 0: `prefill-prompt` → activação f32 (base64)
2. Stage 1: `forward-activation` → amostra token
3. Stage 0: `decode-token` → próxima activação
4. Repete até `max_tokens`

**Sem** montar o GGUF completo no stage 0. **Sem** cache de shards v2 no stage 0.

```bash
./scripts/build-pipeline-step.sh
./scripts/setup-pipeline-activation-mac.sh
YOUAI_BIN_DIR=$PWD/target/release youai-node start   # terminal dedicado

./scripts/ubuntu-test-vm.sh start-node-activation
./scripts/test-shard-pipeline-activation.sh
```

Ficheiros de modelo: `~/.youai/pipeline-stages/*-stageNN-of-MM.gguf`

### v4 — Daemon (modelo quente) ✅

Mesma lógica que v3, mas o worker mantém um processo `youai-pipeline-step --daemon` com o modelo **já carregado**. Cada `/pipeline/step` envia uma linha JSON ao stdin do daemon em vez de spawnar um subprocesso novo.

| v3 (subprocess) | v4 (daemon) |
|-----------------|-------------|
| ~1–10 s/step (reload modelo) | ~50–200 ms/step (só forward) |
| Metal destructor → `_exit(0)` por step | Modelo vivo; sem teardown por token |

Resposta do chat: `mode: "pipeline_activation_v4"`.

Desactivar daemon (fallback v3): `YOUAI_PIPELINE_DAEMON=0`.

---

## Configuração pipeline v3/v4 (2 máquinas)

### Pré-requisitos

- llama.cpp: `./scripts/setup-llama.sh`
- Modelo: `./scripts/download-model.sh`
- Build: `cargo build --release --workspace` + `./scripts/build-pipeline-step.sh`

### 1. Coordinator (Mac)

```bash
./target/release/youai-coordinator \
  --host 0.0.0.0 --port 8080 \
  --db ~/.youai/coordinator.db
```

### 2. Stage 0 — Mac

```bash
./scripts/setup-pipeline-activation-mac.sh
export YOUAI_BIN_DIR="$PWD/target/release"
youai-node start   # não usar background — precisa de heartbeats
```

### 3. Stage 1 — VM (Colima)

```bash
./scripts/ubuntu-test-vm.sh start-node-activation
```

### 4. Teste

```bash
./scripts/test-shard-pipeline-activation.sh
# Esperado: mode=pipeline_activation_v4
```

---

## API — campos relevantes

### Registro (`POST /api/v1/nodes/register`)

Pipeline v3/v4:

```json
{
  "name": "m1-mac",
  "worker_url": "http://127.0.0.1:7741",
  "model": "smollm2-360m-instruct",
  "shard_group": "default-pipeline",
  "shard_stage": 0,
  "shard_total_stages": 2,
  "pipeline_kind": "activation",
  "gguf_shard_total": 1
}
```

### Worker (`POST /pipeline/step`)

```json
{
  "session_id": "uuid-da-sessão",
  "op": "prefill-prompt | decode-token | forward-activation",
  "prompt": "...",
  "token_id": 0,
  "activation_b64": "...",
  "sample": true
}
```

### CLI do nó

```bash
youai-node config \
  --shard-group default-pipeline \
  --shard-stage 0 \
  --shard-total-stages 2 \
  --pipeline-kind activation \
  --gguf-shard-total 1 \
  --clear-rpc-url \
  --model-path ~/.youai/pipeline-stages/...-stage00-of-02.gguf
```

---

## Scripts

| Script | Função |
|--------|--------|
| `scripts/build-pipeline-step.sh` | Compila `youai-pipeline-step` (v3 single-shot + v4 `--daemon`) |
| `scripts/split-model-layers.py` | GGUF standalone por stage (v3/v4) |
| `scripts/setup-pipeline-activation-mac.sh` | Mac stage 0 (v3/v4) |
| `scripts/ubuntu-test-vm.sh start-node-activation` | VM stage 1 (v3/v4) |
| `scripts/test-shard-pipeline-activation.sh` | E2E v4 |
| `scripts/setup-pipeline-gguf-mac.sh` | Mac stage 0 (v2) |
| `scripts/ubuntu-test-vm.sh start-node-gguf` | VM stage 1 (v2) |
| `scripts/setup-pipeline-mac.sh` | Mac stage 0 (v1 RPC) |
| `scripts/ubuntu-test-vm.sh start-node` | VM stage 1 (v1 RPC) |

---

## Troubleshooting

| Sintoma | Causa provável | Fix |
|---------|----------------|-----|
| `no complete shard chain` | Falta stage 0 ou 1 online | `youai-node start` no Mac (terminal dedicado); verificar heartbeats |
| `pipeline_kind` vazio no Mac | Node registrou antes do config v3 | `youai-node pause && youai-node start` |
| `pipeline step failed` / empty reply | `YOUAI_BIN_DIR` sem `youai-pipeline-step` | `export YOUAI_BIN_DIR=$PWD/target/release` |
| Worker healthy mas node offline | Só worker a correr, sem `youai-node` | Reiniciar `youai-node start` |
| Texto gibberish | Sem chat template no prefill | Próximo passo: template SmolLM2 no coordinator |
| Só 2 stages | v3/v4 MVP | 3+ stages precisa export de activação nos intermediários |
| Daemon lento no 1º token | Cold start do daemon | Normal; tokens seguintes são rápidos |

---

## Limitações actuais (v4)

- **2 stages** apenas (SmolLM2 360M partido ao meio)
- **Greedy decode** no último stage
- **Activations** em base64 via coordinator (OK para dogfood LAN)
- **Chat template** ainda não aplicado no prefill

## Próximo (v5+)

Ver [NEXT_STEPS.md](./NEXT_STEPS.md): 3+ stages, chat template, EOS, compressão de activações, fila/crédito para pipeline lento.