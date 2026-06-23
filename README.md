# YouAI

**AI by you, for you.**

Rede global open source onde cada PC contribui com uma fração do hardware para rodar modelos de IA gratuitos — começando leve no teu Mac Mini e escalando conforme a rede cresce.

> **Regra de transparência:** se um comportamento não está documentado, é bug. Tudo o que o utilizador vê ou o PC faz deve estar explicado em `docs/` ou neste README.

---

## Índice

1. [Visão](#visão)
2. [Como funciona](#como-funciona)
3. [Model Tiers e Registry](#model-tiers-e-registry)
4. [Segurança e background](#segurança-e-background)
5. [Produto e app](#produto-e-app)
6. [Status do projeto](#status-do-projeto)
7. [Começar no Mac Mini (Tier 1)](#começar-no-mac-mini-tier-1)
8. [Desenvolvimento](#desenvolvimento)
9. [Estrutura do monorepo](#estrutura-do-monorepo)
10. [Documentação completa](#documentação-completa)
11. [Contribuir](#contribuir)
12. [Licença](#licença)

---

## Visão

O YouAI é uma rede onde:

- **Utilizadores** usam chat e IA gratuitos (como ChatGPT ou Codex).
- **Contribuintes** emprestam RAM/CPU em **background**, com limites claros, para a rede rodar modelos maiores.
- **A qualidade do modelo escala com o tamanho da rede** — poucos nós = modelo leve (Tier 1); dezenas de milhares = modelos capazes (Tier 5).
- **Quem contribui com hardware acede primeiro** a novos tiers e features; todos recebem as melhorias quando a rede está estável.

Não é crypto, não é mineração disfarçada, não é execução arbitrária no PC de ninguém. É inferência distribuída com sandbox, opt-in explícito e código aberto.

**Repo:** [github.com/cesarfavero/youai](https://github.com/cesarfavero/youai) · público · issues abertas · PRs com revisão humana obrigatória

---

## Como funciona

```
┌─────────────────────────────────────────┐
│  App YouAI (chat + contribuição)        │
│  · streaming · E2E (roadmap)            │
│  · youai-node embutido (opcional)       │
└──────────────────┬──────────────────────┘
                   │ TLS (+ E2E futuro)
                   ▼
┌─────────────────────────────────────────┐
│  Coordinator + Model Registry         │
│  · tier activo · routing · crédito      │
│  · manifestos assinados · rollout       │
└──────────────────┬──────────────────────┘
                   │ jobs assinados / activações opacas
         ┌─────────┴─────────┐
         ▼                   ▼
    Nó A (Mac Mini)      Nó B (VM/PC)
    guard + worker       guard + worker
    só ~/.youai/         só ~/.youai/
```

### Papéis

| Papel | O que faz | O que ganha |
|-------|-----------|-------------|
| **Utilizador** | Chat, agentes (fases futuras) | IA free / crédito |
| **Contribuinte** | Empresta RAM/CPU em background | Crédito, early access, tier melhor mais cedo |

A mesma instalação pode ser só chat, só contribuinte, ou ambos.

### Pipeline distribuído (estado actual)

| Versão | Modo | Descrição |
|--------|------|-----------|
| v1 | `pipeline_rpc` | Split de tensores via llama.cpp RPC |
| v2 | `pipeline_gguf` | GGUF partido entre máquinas |
| v3 | `pipeline_activation` | Activations entre layer-splits (`youai-pipeline-step`) |
| v4 | `pipeline_activation_v4` | Modelo quente em daemon (`--daemon`) — **dogfood actual** |

Detalhe técnico: [docs/PIPELINE.md](docs/PIPELINE.md)

---

## Model Tiers e Registry

A rede **não** tenta rodar um modelo frontier com 2 PCs. O **Model Registry** define qual modelo usar, quando subir de tier, e como actualizar com segurança.

### Tiers (resumo)

| Tier | Nome | Modelo (referência) | RAM/nó | Rede mínima | Estado |
|------|------|---------------------|--------|-------------|--------|
| tier0 | Lab | SmolLM2-360M | 512 MB | 1 nó | dev |
| **tier1** | **Spark** | **SmolLM2-360M Q4_K_M** | **~1 GB** | **2 contribuintes** | **✅ hoje** |
| tier2 | Glow | SmolLM2-1.7B / Qwen2.5-0.5B | 2 GB | 10 | planeado |
| tier3 | Beam | 3B class | 4 GB | 100 | planeado |
| tier4 | Arc | 7B sharded | 4 GB/stage | 1 000 | planeado |
| tier5 | Horizon | 7B+ / MoE | variável | 10 000 | planeado |

**Tier 1 (Spark)** é o alvo para o teu Mac Mini: ~220 MB em disco, ~0.5–1.5 GB RAM em inferência, **30% CPU** por defeito, sem GPU obrigatória.

### Model Registry

O registry é o sítio central que gere modelos e updates:

| Fase | Onde |
|------|------|
| **Agora** | [`registry/manifest.json`](registry/manifest.json) no repo + `scripts/download-model.sh` |
| Alpha | `GET /api/v1/registry/manifest` no coordinator |
| Beta | CDN + coordinator como cache |
| Produção | `registry.youai.network` com assinatura Ed25519 |

**Fluxo de update:**

1. Maintainers publicam novo manifest (ex: tier2).
2. Coordinator valida assinatura e métricas da rede.
3. Se o limiar não é atingido → mantém tier actual + mensagem transparente ao utilizador.
4. Se atingido → canary (5% contribuintes) → download com verificação SHA256 → rollout completo.

**Comandos planeados:**

```bash
youai-node models list      # tiers e modelos disponíveis
youai-node models status    # instalado + hash local
youai-node models update    # sync com registry
youai-node tier             # tier activo + o que falta para subir
```

Documento completo: [docs/MODEL_TIERS.md](docs/MODEL_TIERS.md)

### Escala com a rede

| Utilizadores | Contribuintes (est.) | Tier provável |
|--------------|---------------------|---------------|
| 10 | 5 | tier1 |
| 100 | 40 | tier2 |
| 1 000 | 300 | tier3–4 |
| 10 000 | 2 000+ | tier5 |

Com 10k na rede e contribuintes saudáveis, um modelo “legal” roda de boa — há RAM agregada e cadeias de pipeline de sobra.

### Acesso antecipado (contribuintes)

| Papel | Novo tier / feature |
|-------|---------------------|
| **Contribuinte activo** | Canary imediato (uptime, sem violações guard) |
| **Só utilizador** | Quando tier está estável (pós-canary) |
| **Todos** | Melhorias finais quando rollout completo |

Contribuintes **não** vêem prompts de outros (E2E). Benefício = modelo melhor + features antes, não dados alheios.

---

## Segurança e background

### Princípios inegociáveis

| # | Princípio |
|---|-----------|
| 1 | **Opt-in explícito** — nada corre sem aceitar limites (CPU, RAM, horários) |
| 2 | **Anonimato por defeito** — chat sem conta obrigatória |
| 3 | **E2E onde importa** — prompt nunca em plaintext nos nós voluntários (roadmap) |
| 4 | **Zero inbound** — ninguém na internet liga ao teu PC |
| 5 | **Sandbox fixo** — worker só acede `~/.youai/` |
| 6 | **Sem execução arbitrária** — nó nunca corre scripts/URLs da rede |
| 7 | **Guard independente** — `youai-guard` mata worker se furar limites |
| 8 | **Transparência total** — comportamento documentado, código aberto |
| 9 | **PRs revistos** — merge humano obrigatório em código sensível |
| 10 | **Não atrapalhar o dia a dia** — background com pausa automática |

### O que o PC do contribuinte **nunca** faz

- Abrir portas de entrada
- Ler `~/Documents`, email, browser, etc.
- Escrever fora de `~/.youai/`
- Executar código recebido da rede
- Descarregar e executar URLs arbitrárias
- Usar CPU/RAM/GPU acima do teto configurado

### Background sem incomodar

| Comportamento | Como |
|---------------|------|
| CPU por defeito | 30% (configurável) |
| RAM tier1 | `ram_max=2g` recomendado no Mac Mini |
| Pausa manual | `youai-node pause` em < 2s |
| Pausa automática | Quando utilizador precisa do PC (roadmap: carga do sistema) |
| Update de modelo | Notificar antes; idle-only quando possível |

Upload, análise de URL e busca **não** correm nos nós voluntários — só no **gateway YouAI** (sandbox), a partir de tier4–5.

Documentos: [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) (design completo) · [docs/SECURITY.md](docs/SECURITY.md) (disclosure)

### Estado honesto da segurança

| Capacidade | Estado |
|------------|--------|
| Guard RAM/CPU | ✅ Parcial (dogfood) |
| Outbound-only | ✅ Por design |
| Activations opacas (pipeline) | ⚠️ LAN dogfood; E2E planeado |
| E2E chat | ❌ Planeado |
| Job signing | ❌ Planeado |
| Verificação hash no node | ❌ Planeado (manifest já no repo) |
| Pausa inteligente | ❌ Planeado |

---

## Produto e app

**Visão:** app de chat + IA (como ChatGPT / Codex) onde, ao instalares, usas modelos gratuitos — e, se quiseres, o teu PC ajuda a rede em background, com limites claros e segurança máxima.

### Ecrãs principais (alvo)

| Ecrã | Conteúdo |
|------|----------|
| Chat | Conversa, streaming, tier/modelo actual |
| Contribuir | Toggle, sliders CPU/RAM, horários, “não incomodar” |
| Rede | Tier activo, progresso para próximo tier |
| Privacidade | O que sai do dispositivo, E2E, link docs |
| Crédito | Saldo, bónus por contribuição |

### Onboarding do contribuinte

1. Explicar como funciona (diagrama, links docs)
2. Sliders com defeitos seguros (30% CPU, 2 GB RAM)
3. Opt-in **explícito** — botão “Começar a contribuir” **não** pré-marcado
4. Download do modelo via registry com barra + hash
5. Ícone na bandeja; corre em background

**Nunca** esconder que o PC está a contribuir.

Documento completo: [docs/PRODUCT.md](docs/PRODUCT.md)

---

## Status do projeto

| Componente | Estado |
|------------|--------|
| `youai-guard` | ✅ Limites RAM/CPU, watchdog |
| `youai-worker` | ✅ llama.cpp local + pipeline step |
| `youai-node` | ✅ CLI start/pause/status/config |
| `youai-coordinator` | ✅ Registo, heartbeat, chat, pipeline v4 |
| Pipeline v1–v4 | ✅ Réplica + RPC + GGUF + activation + daemon |
| Model Registry | ✅ `registry/manifest.json` tier1 · API planeado |
| `youai-web` | 🔜 Chat mínimo |
| App desktop | 🔜 Tauri/Electron + node embutido |
| E2E / job signing | 🔜 Alpha/Beta |

**Fase actual:** dogfood multi-máquina — Mac Mini (stage 0) + VM/PC (stage 1), modo `pipeline_activation_v4`.

**Meta imediata:** qualidade (chat template, EOS) + registry API + verificação SHA256 no node.

---

## Começar no Mac Mini (Tier 1)

Objectivo: rodar **sem consumir muito** — background-friendly no M1/M2.

### Pré-requisitos

- macOS com Rust stable (1.75+)
- ~2 GB RAM livre para o nó
- Terminal dedicado para `youai-node start` (processos em background podem receber SIGTERM)

### Passo a passo

```bash
# 1. Clonar e compilar
git clone https://github.com/cesarfavero/youai.git
cd youai
cargo build --release --workspace

# 2. Modelo tier1 (~220 MB) — verifica hash no manifest
./scripts/download-model.sh

# 3. llama.cpp + pipeline step nativo
./scripts/setup-llama.sh
./scripts/build-pipeline-step.sh

# 4. Layer-splits para pipeline v4 (2 stages)
python3 scripts/split-model-layers.py \
  ~/.youai/models/smollm2-360m-instruct-q4_k_m.gguf 2 \
  ~/.youai/pipeline-stages

# 5. Configurar Mac como stage 0 (30% CPU, 2 GB RAM)
./scripts/setup-pipeline-activation-mac.sh

# 6. Coordinator (outro terminal)
cargo run -p youai-coordinator -- --port 8080

# 7. Arrancar nó (terminal dedicado — importante)
export YOUAI_BIN_DIR="$PWD/target/release"
youai-node start
```

### Segundo nó (VM Ubuntu / outro PC)

```bash
./scripts/ubuntu-test-vm.sh start-node-activation
```

### Testar pipeline v4

```bash
./scripts/test-shard-pipeline-activation.sh
# Esperado: mode=pipeline_activation_v4
```

### Limites recomendados (Mac Mini)

```toml
# ~/.youai/config.toml
[resources]
cpu_percent = 30
ram_max = "2g"
```

### O que vês no status (alvo)

```
Rede YouAI
  Tier activo:     tier1 (Spark)
  Modelo:          smollm2-360m-instruct-q4_k_m
  Próximo tier:    tier2 (Glow) — faltam N contribuintes
  Recursos:        CPU 30% · RAM 2 GB
  Pipeline:        activation v4 · 2 stages
```

---

## Desenvolvimento

**Pré-requisitos:** Rust stable (1.75+), CMake 3.20+, Python 3 (split scripts), Node 20+ (web, depois)

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### Scripts úteis

```bash
chmod +x scripts/*.sh

./scripts/setup-llama.sh                    # llama.cpp + rpc-server
./scripts/download-model.sh                 # SmolLM2-360M tier1
./scripts/benchmark-model.sh                # tokens/s, RAM pico
./scripts/setup-pipeline-activation-mac.sh  # Mac stage 0
./scripts/test-shard-pipeline-activation.sh # teste v4
```

### Ordem de implementação (não pular)

1. **Guard** — limites RAM/CPU
2. **Worker** — inferência local
3. **Node CLI** — integração
4. **Coordinator** — rede
5. **Registry API** — tier + hash verify
6. **Web / App** — chat + onboarding

Ver [docs/NEXT_STEPS.md](docs/NEXT_STEPS.md) para o roteiro completo.

---

## Estrutura do monorepo

```
youai/
├── youai-guard/           # limites RAM/CPU/GPU · watchdog
├── youai-node/            # CLI · config · start/pause/status
├── youai-worker/          # llama.cpp wrapper · pipeline daemon
├── youai-coordinator/     # nós · routing · crédito · pipeline
├── youai-web/             # chat mínimo (fase 9)
├── native/                # youai-pipeline-step (C++/llama.cpp)
├── registry/              # manifest.json — Model Registry
├── docs/                  # design docs normativos
└── scripts/               # setup, benchmark, testes cluster
```

---

## Documentação completa

### Design (normativos — ler antes de implementar features)

| Documento | Conteúdo |
|-----------|----------|
| [**MODEL_TIERS.md**](docs/MODEL_TIERS.md) | Tiers, registry, rollout, early access, features por tier |
| [**SECURITY_MODEL.md**](docs/SECURITY_MODEL.md) | Sandbox, E2E, threat model, checklist PR, background |
| [**PRODUCT.md**](docs/PRODUCT.md) | App, onboarding, transparência, crédito, gateway features |

### Técnico e operação

| Documento | Conteúdo |
|-----------|----------|
| [**NEXT_STEPS.md**](docs/NEXT_STEPS.md) | Roteiro prático — o que fazer agora |
| [**PIPELINE.md**](docs/PIPELINE.md) | Pipeline v1–v4, testes, daemon |
| [**ARCHITECTURE.md**](docs/ARCHITECTURE.md) | Camadas e componentes |
| [**MVP.md**](docs/MVP.md) | Visão MVP, roadmap, escopo |

### Comunidade e segurança

| Documento | Conteúdo |
|-----------|----------|
| [**CONTRIBUTING.md**](docs/CONTRIBUTING.md) | Como contribuir, checklist de segurança em PRs |
| [**SECURITY.md**](docs/SECURITY.md) | Responsible disclosure, versões suportadas |

### Registry

| Ficheiro | Conteúdo |
|----------|----------|
| [**registry/manifest.json**](registry/manifest.json) | Manifesto tier1 (Spark) com URLs e SHA256 |

---

## Contribuir

Issues abertas. Pull requests **exigem revisão humana** — especialmente em `guard`, `worker`, `node`, `coordinator`, `native/`.

Antes de tocar em sandbox ou rede: ler [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) e usar a checklist de PR em [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

```bash
git checkout -b feat/short-description
cargo fmt --all && cargo clippy --workspace -- -D warnings && cargo test --workspace
```

---

## Licença

[Apache License 2.0](LICENSE)

---

*YouAI — IA sua, feita por você. O modelo cresce com a rede; a rede cresce com a confiança.*