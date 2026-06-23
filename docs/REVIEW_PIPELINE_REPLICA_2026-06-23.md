# Review + testes — pipeline vs réplica (2026-06-23)

> Sessão de validação antes de commitar o trabalho local não commitado.
> Mudanças em análise: separação cluster real (pipeline) vs dogfood (réplica).

---

## Resumo executivo

| Área | Resultado |
|------|-----------|
| `cargo test --workspace` | ✅ 13/13 passam |
| `cargo build --release` | ✅ Compila (7 warnings dead_code no coordinator) |
| E2E pipeline (`test-shard-pipeline-activation.sh`) | ✅ `pipeline_activation_v4`, 2 stages |
| `mode=auto` com cadeia completa | ✅ Sempre `pipeline_activation_v4` |
| `mode=replica` só com nodes pipeline | ✅ 503 com mensagem clara |
| Estabilidade pipeline (3 runs frescos) | ✅ 3/3 após restart do Mac node |
| Estabilidade sem supervisão ativa | ❌ Mac node morreu ~30s após worker crash |

**Veredito:** A lógica de routing e SHA256 está correta. O cluster pipeline funciona quando ambos os nós estão online e supervisionados. Há gaps operacionais (scripts, paths hardcoded, teste réplica) e um bug de resiliência no `youai-node` quando o worker cai.

---

## O que mudou (diff local)

### Rust

- **`youai-common`**: `is_replica_eligible()`, `resolve_auto_chat_dispatch()`, testes
- **`youai-coordinator`**: `mode=auto` prefere pipeline; réplica filtra shards de pipeline
- **`youai-node`**: SHA256 de stage GGUF via `stage_files` no manifest (antes do match fuzzy do modelo completo)

### Scripts novos

| Script | Propósito |
|--------|-----------|
| `setup-pipeline-cluster.sh` | **Default produto** — Mac stage 0 + Ubuntu stage 1 |
| `setup-replica-cluster.sh` | **Dogfood** — modelo completo em cada máquina |
| `setup-replica-mac.sh` | Config Mac para réplica |
| `test-replica-round-robin.sh` | Teste round-robin réplica |

### Docs

- `README.md` + `PIPELINE.md` — pipeline como cluster default; réplica só para throughput test

---

## Resultados dos testes

### Unitários

```
youai-common:     8 passed (incl. auto_prefers_pipeline_when_chain_online)
youai-coordinator: 1 passed
youai-node:        1 passed (stage_gguf_uses_stage_hash_not_full_model)
youai-guard:       2 passed
youai-worker:      1 passed
```

### E2E — pipeline (cluster real)

**Setup:** Coordinator `YOUAI_DEV_MODE=1` · Mac `m1-mac` stage 0 · Ubuntu `ubuntu-colima` stage 1

```bash
./scripts/test-shard-pipeline-activation.sh
# → mode=pipeline_activation_v4, stages=[m1-mac, ubuntu-colima]
```

**`mode=auto` (3 pedidos):** todos `pipeline_activation_v4` em `ubuntu-colima` (último stage reporta).

**`mode=replica` (só pipeline nodes):**

```json
{
  "error": "no replica-capable nodes online — pipeline stage shards cannot serve replica chat; configure a node with the full GGUF model (see scripts/setup-replica-mac.sh)"
}
```

HTTP 503 — comportamento esperado ✅

### Estabilidade

| Cenário | Resultado |
|---------|-----------|
| 3× `mode=pipeline` com cache limpo, nós online | 3/3 OK (~5–15s cada) |
| 5× `mode=pipeline` após Mac node supervisor morrer | 0/5 — cadeia incompleta (503) ou 502 |

**Causa raiz observada:** O processo `youai-node` no Mac terminou com:

```
ERROR youai_node::runtime: worker exited unexpectedly status=ExitStatus(unix_wait_status(256))
youai-node error: worker process exited
```

O `youai-worker` e `youai-pipeline-step --daemon` ficaram órfãos (health em `:7741` ainda OK), mas o coordinator marcou `m1-mac` como `online=false` → cadeia pipeline quebrada.

Isto explica os 502/503 intermitentes da sessão anterior.

### Réplica — não testado E2E

Não foi montado cluster réplica nesta sessão (exigiria reconfigurar Mac + Ubuntu + prune coordinator). O script `test-replica-round-robin.sh` tem um bug conhecido: usa `mode=auto` mas espera round-robin réplica — ver review abaixo.

---

## Code review

### Blockers

Nenhum na lógica de routing.

### Major

1. **`test-replica-round-robin.sh` usa `mode=auto`** — com cadeia pipeline online, nunca testa réplica. Corrigir para `mode=replica`.
2. **Paths hardcoded** em `ubuntu-test-vm.sh`: `/Users/cesarfavero/.youai/...` — quebra para outros devs.
3. **Sem cleanup ao trocar topologia** — pipeline ↔ réplica deixa nós stale no coordinator; `mode=auto` pode surpreender.
4. **Docs desatualizados** — README ainda diz "Verificação hash no node | ❌ Planeado"; PIPELINE.md ainda menciona chat template como pendente (já implementado no commit `7084978`).

### Minor

5. `is_replica_eligible` sem testes unitários (só `resolve_auto_chat_dispatch` tem).
6. `find_model_sha256` pass-2 com match fuzzy pode confundir tiers futuros.
7. `sha256_file` lê GGUF inteiro em RAM (~270 MB tier1).
8. Scripts de cluster só configuram — não verificam health nem correm testes.

### Pontos fortes

- Separação produto/dogfood clara nos scripts e docs
- Política de routing centralizada em `youai-common`
- Réplica não pode servir inferência em shards de pipeline
- Fix SHA256 stage-before-full com teste de regressão
- Mensagem de erro 503 aponta para `setup-replica-mac.sh`

---

## Recomendações antes de commit

### Obrigatório

1. Corrigir `test-replica-round-robin.sh` → `mode=replica` + assert `mode=="replica"`
2. Parameterizar paths VM: `YOUAI_MAC_HOME` ou `$HOME/.youai`
3. Atualizar README (hash verify ✅) e PIPELINE.md (remover itens obsoletos)

### Desejável

4. Investigar/fixar resiliência do `youai-node` quando worker morre (restart automático vs sair)
5. Testes `is_replica_eligible` em `youai-common`
6. Nota nos scripts de cluster: correr `clean-coordinator.sh` ao trocar topologia
7. Commits separados:
   - `feat(coordinator): replica eligibility + auto pipeline preference`
   - `feat(node): stage-aware registry SHA256 verify`
   - `chore(scripts): pipeline vs replica cluster entrypoints`
   - `docs: default cluster is pipeline`

### Não bloqueia commit

- Warnings dead_code no coordinator
- Streaming SHA256 para tiers grandes (roadmap)

---

## Fixes aplicados (pós-review)

- `test-replica-round-robin.sh` — `mode=replica` + assert de modo
- `ubuntu-test-vm.sh` — paths via `YOUAI_MAC_HOME` / `YOUAI_MAC_DATA`
- `youai-node` — restart automático do worker + cleanup de daemons órfãos
- `youai-common` — testes `is_replica_eligible`
- `README.md` / `PIPELINE.md` — docs alinhados com implementação
- Scripts de cluster — notas de cleanup ao trocar topologia

---

## Como continuar (opções)

| Prioridade | Tarefa | Esforço |
|------------|--------|---------|
| 🔴 | Fix resiliência worker/node (restart vs exit) | Médio |
| 🟠 | Fixes operacionais (test réplica, paths, docs) | Baixo |
| 🟠 | Commit do diff local com splits acima | Baixo |
| 🟡 | Teste E2E réplica completo | Médio |
| 🟡 | Registry API REST (meta imediata restante) | Médio-alto |
| 🟢 | Qualidade resposta pipeline (`text: "...... and."` — template/EOS) | Médio |

---

## Comandos para reproduzir

```bash
# Build + testes
cargo test --workspace
cargo build --release

# Coordinator
YOUAI_DEV_MODE=1 ./target/release/youai-coordinator \
  --host 0.0.0.0 --port 8080 --db ~/.youai/coordinator.db

# Cluster pipeline (já configurado nesta sessão)
YOUAI_BIN_DIR=$PWD/target/release ./target/release/youai-node start
./scripts/ubuntu-test-vm.sh status
./scripts/test-shard-pipeline-activation.sh

# Verificar routing
curl -s -H 'Content-Type: application/json' \
  -d '{"prompt":"ping","max_tokens":4,"mode":"auto"}' \
  http://127.0.0.1:8080/api/v1/chat | python3 -m json.tool
```