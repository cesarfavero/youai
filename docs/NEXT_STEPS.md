# YouAI — Guia dos Próximos Passos

> Use este arquivo quando abrir o projeto em `youai/`.  
> É o roteiro prático — o **o quê fazer, em qual ordem, e o que pedir pro assistente**.

**Leitura obrigatória antes:** [MVP.md](./MVP.md)

---

## Onde você está agora

```
youai/
├── README.md
└── docs/
    ├── MVP.md          ← visão completa
    └── NEXT_STEPS.md   ← você está aqui
```

**Fase atual:** dogfood multi-máquina — réplica + **pipeline v1–v4** ✅ · **Model Registry** tier1 em `registry/manifest.json` ✅  
**Meta imediata:** registry API no coordinator + SHA256 no node + qualidade (chat template, EOS)  
**Meta MVP:** 10–50 PCs · tier1 Spark (SmolLM2-360M) no Mac Mini · chat free · crédito básico

**Design docs normativos (ler antes de features novas):**

- [MODEL_TIERS.md](./MODEL_TIERS.md) — tiers, registry, rollout, early access
- [SECURITY_MODEL.md](./SECURITY_MODEL.md) — sandbox, E2E, PR checklist, background
- [PRODUCT.md](./PRODUCT.md) — app ChatGPT/Codex-like, onboarding, transparência

---

## Antes de codar (1 sessão · ~30 min)

Marque conforme for fazendo:

- [ ] Abrir o Cursor **na pasta `youai/`** (não na pasta `DIO/` pai)
- [ ] Confirmar nome **YouAI** (ou decidir renomear antes de criar repo)
- [ ] Criar repositório Git local:
  ```bash
  cd youai
  git init
  git add .
  git commit -m "docs: visão MVP e guia de próximos passos"
  ```
- [ ] (Opcional) Criar org no GitHub `youai-network` e push
- [ ] (Opcional) Registrar domínio `youai.dev` ou `youai.network`
- [ ] Instalar toolchain base na sua máquina:

| Ferramenta | Pra quê | Verificar |
|------------|---------|-----------|
| **Rust** | guard, node, coordinator | `rustc --version` |
| **Go** (opcional) | coordinator alternativo | `go version` |
| **Node 20+** | web chat depois | `node --version` |
| **CMake + build tools** | compilar llama.cpp | `cmake --version` |
| **CUDA** (se tiver NVIDIA) | GPU no worker | `nvidia-smi` |
| **Docker** (opcional) | testes isolados | `docker --version` |

---

## Ordem de implementação (não pule)

```
1. Docs legais          SECURITY.md · CONTRIBUTING.md
2. youai-guard       limites RAM/CPU (sem rede ainda)
3. youai-worker         llama.cpp single-node
4. Guard + Worker       integração · provar que não estoura limite
5. youai-coordinator    1 servidor · registra nós
6. youai-node CLI       install · config · start · pause
7. Rede 2–3 máquinas    2 PCs na mesma rede
8. youai-web            chat mínimo + crédito
9. Beta 10 pessoas      Discord/Telegram
```

**Regra:** não começar pelo web nem pelo mobile. Guard primeiro.

---

## Passo 1 — Docs legais (peça pro assistente)

**Objetivo:** base open source confiável antes de código.

**Pedir no Cursor:**
> "Cria `docs/SECURITY.md` e `docs/CONTRIBUTING.md` para o YouAI seguindo o MVP.md"

**Checklist:**
- [ ] `docs/SECURITY.md` — responsible disclosure, o que o nó não faz
- [ ] `docs/CONTRIBUTING.md` — como contribuir, código de conduta curto
- [ ] `docs/ARCHITECTURE.md` — diagrama das 4 camadas (pode ser rascunho)
- [ ] `.gitignore` global (Rust, Node, modelos GGUF, `.youai/`)
- [ ] `LICENSE` — Apache 2.0

---

## Passo 2 — Scaffold do monorepo (peça pro assistente)

**Objetivo:** estrutura de pastas vazia mas correta.

**Pedir no Cursor:**
> "Cria a estrutura do monorepo YouAI conforme MVP.md: youai-guard, youai-node, youai-worker, youai-coordinator, youai-web"

**Estrutura alvo:**
```
youai/
├── Cargo.toml              # workspace Rust (guard, node, coordinator)
├── youai-guard/
├── youai-node/
├── youai-worker/           # wrapper llama.cpp (pode ser Rust + FFI ou subprocess)
├── youai-coordinator/
├── youai-web/
├── docs/
├── scripts/
│   ├── install.sh
│   └── benchmark-model.sh
└── .github/workflows/      # CI básico (lint + test)
```

**Checklist:**
- [ ] Workspace Rust compila (`cargo build`)
- [ ] README atualizado com comandos de dev
- [ ] CI roda `cargo test` (mesmo que vazio)

---

## Passo 3 — `youai-guard` POC (prioridade #1)

**Objetivo:** provar que limites funcionam **antes** de qualquer IA na rede.

**Pedir no Cursor:**
> "Implementa youai-guard POC em Rust: limite de RAM e CPU% no Linux com cgroups, watchdog que mata processo filho se furar"

**Escopo mínimo:**
- [ ] CLI: `youai-guard run --ram-max 8g --cpu-percent 30 -- <comando>`
- [ ] cgroup v2: memory.max + cpu.max
- [ ] Loop a cada 500ms medindo uso real
- [ ] Se passar do limite → SIGKILL no filho
- [ ] Log local em `~/.youai/guard.log`
- [ ] Teste: rodar `stress` ou loop pesado e verificar que é morto

**Plataformas MVP:**
- [ ] Linux (primeiro)
- [ ] macOS (best-effort · limites mais conservadores)
- [ ] Windows (fase 1b · Job Objects)

**Critério de sucesso:** 0 violações em 1h de teste com limite em 30% CPU.

---

## Passo 4 — `youai-worker` single-node

**Objetivo:** rodar Nex-N2-mini local via llama.cpp, sem rede.

**Pedir no Cursor:**
> "Setup youai-worker com llama.cpp para rodar Nex-N2-mini quantizado localmente"

**Ações manuais:**
- [ ] Clonar/buildar llama.cpp:
  ```bash
  git clone https://github.com/ggml-org/llama.cpp
  # build com CUDA se tiver GPU
  ```
- [ ] Baixar quant GGUF do N2-mini (Hugging Face · filtrar por tamanho que cabe na sua RAM)
- [ ] Script `scripts/benchmark-model.sh`:
  - tokens/s
  - RAM pico
  - VRAM pico (se GPU)

**Anote os números** — vão pro MVP (latência realista):

| Métrica | Sua máquina | Data |
|---------|-------------|------|
| Modelo / quant | | |
| RAM pico | | |
| Tokens/s | | |
| GPU? | | |

**Critério de sucesso:** prompt → resposta coerente no terminal.

---

## Passo 5 — Integrar Guard + Worker

**Objetivo:** inferência rodando **dentro** do sandbox.

**Pedir no Cursor:**
> "Integra youai-worker para sempre rodar sob youai-guard com os limites do config"

**Checklist:**
- [ ] `youai-node start` (primeira versão) só faz: guard → worker
- [ ] Config em `~/.youai/config.toml`:
  ```toml
  [resources]
  cpu_percent = 30
  ram_max = "8g"
  gpu_percent = 50   # se tiver NVML
  vram_max = "6g"
  ```
- [ ] `youai-node pause` mata worker em < 2s
- [ ] `youai-node status` mostra uso atual vs limite

**Critério de sucesso:** com jogo ou stress no PC, guard pausa ou mantém teto.

---

## Passo 6 — `youai-coordinator` básico

**Objetivo:** servidor central que sabe quais nós existem (ainda sem MoE complexo).

**Pedir no Cursor:**
> "Implementa youai-coordinator mínimo: registro de nó, heartbeat, lista de nós online"

**Escopo mínimo:**
- [ ] API HTTP ou gRPC
- [ ] `POST /nodes/register` — id, região, recursos, modelo
- [ ] `POST /nodes/heartbeat` — a cada 30s
- [ ] `GET /nodes` — nós vivos
- [ ] SQLite ou Postgres local pro MVP
- [ ] Auth simples: token por nó (gerado no register)

**Rodar local:**
```bash
youai-coordinator --port 8080
```

**Critério de sucesso:** 2 terminais registram nó e aparecem como online.

---

## Passo 7 — Rede com 2–3 máquinas

**Objetivo:** provar cluster na prática (mesma Wi-Fi ou VPS).

**Checklist:**
- [ ] Coordinator num PC fixo ou VPS (IP estável)
- [ ] 2 PCs com `youai-node start --coordinator http://IP:8080`
- [ ] Firewall: porta 8080 aberta só pro que precisa
- [ ] Testar queda: matar 1 nó → coordinator marca offline em < 60s

**Pedir no Cursor:**
> "Adiciona ao youai-node reconexão automática e logs claros de conexão com coordinator"

---

## Passo 8 — Sharding / pipeline (MVP rede) ✅

**Feito:**

- [x] **Réplica** — round-robin com health check (`mode=replica` ou `auto`)
- [x] **Pipeline v1** — RPC tensor split (`mode=pipeline_rpc`)
- [x] **Pipeline v2** — GGUF distribuído (`mode=pipeline_gguf`)
- [x] **Pipeline v3** — activações entre layer-splits (`youai-pipeline-step`)
- [x] **Pipeline v4** — daemon com modelo quente (`--daemon`, `mode=pipeline_activation_v4`)
- [x] Docs: [PIPELINE.md](./PIPELINE.md)

**Testes:**

```bash
./scripts/test-shard-pipeline.sh              # v1
./scripts/test-shard-pipeline-gguf.sh         # v2
./scripts/test-shard-pipeline-activation.sh   # v4
```

## Passo 8b — Model Registry e tiers (prioridade agora)

**Objectivo:** gerir modelos centralmente — tier1 no Mac Mini hoje; subir tier quando a rede aguenta.

**Já feito:**

- [x] `registry/manifest.json` — tier1 Spark (SmolLM2-360M Q4_K_M + pipeline stages + SHA256)
- [x] Docs: [MODEL_TIERS.md](./MODEL_TIERS.md), [PRODUCT.md](./PRODUCT.md), [SECURITY_MODEL.md](./SECURITY_MODEL.md)
- [x] README como índice central

**Próximo (pedir no Cursor):**

- [ ] `GET /api/v1/registry/manifest` e `GET /api/v1/registry/tier` no coordinator (estático a partir do JSON)
- [ ] Verificação SHA256 em `youai-node start` antes de carregar modelo
- [ ] `youai-node models status` / `models update` / `tier`
- [ ] Métricas de rede → `tier_ativo` automático (com histerese)
- [ ] Canary rollout + `contributor_score` para early access
- [ ] Assinatura Ed25519 no manifest

**Referência:** [MODEL_TIERS.md](./MODEL_TIERS.md) · [registry/manifest.json](../registry/manifest.json)

---

## Passo 8c — App desktop + onboarding (fase beta)

**Objectivo:** app tipo ChatGPT/Codex com contribuição automática mas transparente.

**Escopo:**

- [ ] Shell desktop (Tauri ou Electron) com `youai-node` embutido
- [ ] Onboarding: opt-in explícito, sliders CPU/RAM, download registry
- [ ] Bandeja: ícone verde/amarelo, pausa em 1 clique
- [ ] Ecrãs: Chat, Contribuir, Rede (tier + progresso), Privacidade
- [ ] E2E antes de beta pública

**Referência:** [PRODUCT.md](./PRODUCT.md) · [SECURITY_MODEL.md](./SECURITY_MODEL.md)

---

### Pipeline v5 — escala e qualidade

**Objetivo:** pipeline útil em produção (texto coerente, 3+ máquinas, latência aceitável).

**Escopo sugerido:**

- [ ] Chat template SmolLM2 no `prefill-prompt` (coordinator ou worker)
- [ ] Paragem em token EOS (`<|im_end|>`)
- [ ] 3+ stages: `forward-activation` exporta activação nos intermediários
- [ ] Compressão de activações (fp16 ou zstd) no coordinator
- [ ] Fila / timeout dedicado para `mode=pipeline` (requests lentos)
- [ ] UI web: mostrar `mode` + `stages` na resposta do chat
- [ ] Modelo maior que 1 máquina (validar com 4 stages)

**Referência:** [PIPELINE.md](./PIPELINE.md)

---

## Passo 9 — Crédito + `youai-web`

**Objetivo:** contribui → ganha token de uso → gasta no chat.

**Pedir no Cursor:**
> "Cria youai-web mínimo: login simples, saldo de crédito, chat que chama coordinator"

**Escopo mínimo web:**
- [ ] Página chat (Next.js ou HTML simples)
- [ ] Conta: email ou anonymous id
- [ ] Crédito: +1000/h por nó online (número fake no início, ajustar depois)
- [ ] Quota diária baixa pra quem não contribui
- [ ] Streaming de resposta (SSE)

**Critério de sucesso:** você contribui no PC A e usa o chat no PC B.

---

## Passo 10 — Beta fechado (10 pessoas)

**Objetivo:** validar que PC de outras pessoas não explode kkk.

**Checklist:**
- [ ] Canal Discord ou Telegram "YouAI Beta"
- [ ] Guia de 1 página: instalar · configurar limites · pausar
- [ ] Formulário de feedback (Google Form ou GitHub Discussions)
- [ ] Métricas que importam:
  - alguém furou limite de GPU/RAM? (tem que ser **não**)
  - tempo até pausar
  - tokens/s percebido
  - crashes

**Pedir no Cursor:**
> "Cria docs/BETA_GUIDE.md para os 10 primeiros testadores"

---

## O que pedir em cada sessão no Cursor

Copie e cole conforme a fase:

| Fase | Prompt sugerido |
|------|-----------------|
| Docs | `Lê docs/MVP.md e cria SECURITY.md + CONTRIBUTING.md + .gitignore` |
| Scaffold | `Cria monorepo YouAI com workspace Rust conforme docs/MVP.md` |
| Guard | `Implementa youai-guard POC Linux cgroups conforme docs/NEXT_STEPS.md passo 3` |
| Worker | `Setup youai-worker com llama.cpp + script de benchmark` |
| Integração | `Integra guard + worker no youai-node start/pause/status` |
| Coordinator | `Implementa coordinator mínimo: register, heartbeat, nodes online` |
| Web | `Cria youai-web chat mínimo com crédito` |
| Beta | `Cria BETA_GUIDE.md e checklist de métricas` |

**Sempre comece a sessão com:**
> `Lê docs/MVP.md e docs/NEXT_STEPS.md — estamos na fase X`

---

## Decisões que você precisa tomar (não delegar)

| # | Decisão | Opções | Recomendação MVP |
|---|---------|--------|------------------|
| 1 | Domínio / GitHub org | youai.dev · github.com/youai-network | criar antes do beta |
| 2 | Modelo MVP | N2-mini · outro GGUF menor | **Nex-N2-mini** ou fallback Qwen2.5 7B se RAM apertar |
| 3 | Coordinator em | sua máquina · VPS $5 | VPS com IP fixo |
| 4 | Auth web | email · GitHub OAuth · anonymous | anonymous + device id no MVP |
| 5 | Sharding vs réplica | A ou B acima | **réplica** (mais simples) |
| 6 | Licença | Apache 2.0 · MIT | Apache 2.0 |

---

## Armadilhas — não faça isso ainda

- ❌ App mobile
- ❌ GLM-5.2 full distribuído
- ❌ Token crypto / blockchain
- ❌ MoE routing complexo antes de réplica funcionar
- ❌ GUI Tauri antes da CLI estável
- ❌ Enterprise YAML multi-instância
- ❌ Integrar 10 modelos ao mesmo tempo

---

## Cronograma sugerido (realista)

| Semana | Entrega |
|--------|---------|
| **1** | Docs legais + scaffold + guard POC |
| **2** | Worker local + benchmark Nex-N2-mini |
| **3** | youai-node CLI + guard integrado |
| **4** | Coordinator + 2 máquinas online |
| **5** | Réplica round-robin + crédito básico |
| **6** | Web chat + beta 10 pessoas |
| **7–8** | Bugfix · GPU guard · estabilidade |

---

## Comandos úteis (futuro — referência)

```bash
# Desenvolvimento
cargo build --workspace
cargo test --workspace

# Nó contribuinte
youai-node config --cpu-percent 30 --ram-max 8g
youai-node start --coordinator https://coordinator.youai.network
youai-node pause
youai-node status
youai-node metrics --last 1h

# Coordinator (servidor)
youai-coordinator --port 8080 --db youai.db

# Benchmark local
./scripts/benchmark-model.sh --model ~/.youai/models/n2-mini.gguf
```

---

## Definição de "MVP pronto"

O MVP está pronto quando **todos** forem verdade:

- [ ] ≥ 20 nós estáveis ao mesmo tempo
- [ ] Guard: **0** casos de furar limite de RAM/GPU em teste de 24h
- [ ] Pausar em < 2 segundos
- [ ] Chat free funciona com crédito por contribuição
- [ ] Código 100% open source no GitHub com SECURITY.md
- [ ] 10 beta testers externos usaram sem incidente de hardware

---

## Links rápidos internos

- [README (índice central)](../README.md)
- [Model Tiers & Registry](./MODEL_TIERS.md)
- [Modelo de Segurança](./SECURITY_MODEL.md)
- [Produto & App](./PRODUCT.md)
- [Pipeline distribuído (v1–v4)](./PIPELINE.md)
- [Manifest tier1](../registry/manifest.json)
- [Visão e arquitetura](./MVP.md)

---

*Quando abrir o Cursor em `youai/`, comece por: **Passo 1** se ainda não tiver docs legais, ou **Passo 3** se já tiver scaffold.*