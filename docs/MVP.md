# YouAI — MVP & Visão do Produto

> **Status:** rascunho v0.1 · documento vivo  
> **Última atualização:** junho 2026  
> **Licença planejada:** Apache 2.0 (código 100% open source)

---

## Nome do projeto: **YouAI**

### Por que YouAI

| Critério | YouAI |
|----------|-------|
| **Fácil de falar** | 2 sílabas + AI — "iú-êi-ai" |
| **Multilíngue** | "You" é universal em tech; "AI" é global |
| **Memorável** | Curto, brandável, domínio provável (`youai.dev`, `youai.network`) |
| **Significado** | *AI feita por você, para você* — contribui com hardware, usa de graça |
| **Tom** | Acolhedor, não corporativo (vs. nomes frios tipo DIO, Mesh, Shard) |

### Alternativas consideradas (descartadas)

| Nome | Prós | Contras |
|------|------|---------|
| **WeAI** | Ênfase comunitária ("nós") | Soa mais institucional, menos pessoal |
| **MyAI** | Pessoal | Parece app privado, não rede coletiva |
| **GoAI** | Ação, movimento | Confunde com linguagem Go; genérico demais |
| **HiveAI** | Metáfora da colmeia | Marca "Hive" já saturada (crypto, etc.) |
| **MeshAI** | Técnico, preciso | Difícil para usuário leigo |
| **DIO** | Interno | Parece sigla obscura; difícil de decorar |

### Identidade

```
Nome:     YouAI
Tagline:  AI by you, for you.
PT:       IA sua, feita por você.
CLI:      youai-node
Pacote:   youai-node, youai-guard, youai-coordinator
Repo:     github.com/youai-network/youai (placeholder)
```

---

## Visão em uma frase

**YouAI** é uma rede global open source onde cada celular, PC e VPS é um **nó**. Quem contribui com uma fração do hardware **alimenta** modelos frontier gratuitos (GLM-5.2, Nex-N2-Pro). Quem usa, usa **free** — pagando com hardware ocioso ou crédito de contribuição.

Não é somar RAM magicamente. É uma **colmeia MoE**: milhões de nós fracos, roteamento inteligente, modelo gigante no centro.

---

## Princípios inegociáveis

1. **Opt-in total** — usuário define teto; sistema nunca ultrapassa
2. **Open source desde o dia 0** — código, builds reproduzíveis, auditoria pública
3. **Segurança primeiro** — sandbox, guard independente, prompt nunca cru no nó alheio
4. **Free por padrão** — uso básico sem cartão; crédito por contribuição
5. **Nó fraco é válido** — celular 10% conta; VPS T4 50% conta mais
6. **Falha é normal** — celular dorme, PC desliga; rede continua com réplicas

---

## Arquitetura (4 camadas)

```
┌─────────────────────────────────────────────────────────┐
│  CAMADA 4 — USUÁRIO                                     │
│  App Web · Desktop · Mobile · API pública               │
│  Chat · Agentes · Devs                                  │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│  CAMADA 3 — YOUAI COORDINATOR                           │
│  Auth · crédito · roteamento MoE · fila · agent harness │
│  Réplicas em super-nós + âncoras regionais              │
└───────────────────────────┬─────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ CAMADA 2      │   │ CAMADA 2      │   │ CAMADA 2      │
│ Super-nó      │   │ Super-nó      │   │ Super-nó      │
│ região/local  │   │ (PC forte /   │   │ (universidade │
│               │   │  servidor)    │   │  / empresa)   │
└───────┬───────┘   └───────┬───────┘   └───────┬───────┘
        │                   │                   │
   ┌────┴────┐         ┌────┴────┐         ┌────┴────┐
   ▼    ▼    ▼         ▼    ▼    ▼         ▼    ▼    ▼
┌─────────────────────────────────────────────────────────┐
│  CAMADA 1 — YOUAI NODE (milhões de contribuidores)      │
│  📱 celular  💻 notebook  🖥️ PC  ☁️ VPS                 │
│  cada um = shard ou expert MoE · só quando permitido     │
└─────────────────────────────────────────────────────────┘
```

### Por que MoE é o coração

Modelos escolhidos são **Mixture of Experts** — só uma fração dos parâmetros é ativa por token:

| Modelo | Total | Ativo/token | Papel no YouAI |
|--------|-------|-------------|----------------|
| **Nex-N2-mini** | 35B (Qwen3.5-35B-A3B) | ~3B | Tier rápido · MVP |
| **Nex-N2-Pro** | 397B-A17B | ~17B | Tier agente · coding · tools |
| **GLM-5.2** | 744B | ~40B | Tier frontier · contexto 1M |

```
Token do usuário
      │
      ▼
[Coordinator escolhe experts 3, 17, 42, 89]
      │
 ┌────┴────┬────────┬────────┐
 ▼         ▼        ▼        ▼
Nó A     Nó B     Nó C     Nó D
(expert) (expert) (expert) (expert)
      │
      ▼
Resposta → usuário
```

Cada expert existe em **3–5 réplicas** — nó caiu, roteador usa cópia viva.

---

## YouAI Node — o produto que a pessoa instala

### Três canais, um core

| Canal | Público | Interface |
|-------|---------|-----------|
| **Desktop GUI** | usuário comum | sliders, pausar, logs |
| **CLI** | dev, VPS, servidor | `youai-node start --gpu 50%` |
| **Mobile** | celular | app com regras mais duras |

### Configuração (GUI conceitual)

```
┌─────────────────────────────────────────────┐
│  YouAI Node · open source                   │
├─────────────────────────────────────────────┤
│  Status: 🟢 Contribuindo (modo ocioso)      │
│                                             │
│  GPU            [████████░░] 50%  máx       │
│  CPU            [███░░░░░░░] 30%  máx       │
│  RAM reservada  [====] 16 GB      máx       │
│  Disco (cache)  [==] 40 GB                  │
│                                             │
│  ☑ Só quando plugado na tomada              │
│  ☑ Pausar se app pesado (jogo, edição)      │
│  ☑ Só Wi-Fi (mobile)                        │
│                                             │
│  Expert: GLM-5.2 #42 · Crédito hoje: 12.4k  │
│                                             │
│  [Pausar agora]  [Logs]  [Remover nó]       │
└─────────────────────────────────────────────┘
```

**"Pausar agora"** — para em < 2 segundos. Sem exceção.

### CLI (VPS / terminal)

```bash
# Instalar
curl -fsSL https://get.youai.network | sh

# Configurar limites
youai-node config \
  --gpu-percent 50 \
  --vram-max 14g \
  --cpu-percent 40 \
  --ram-max 32g \
  --disk-cache 80g \
  --region sa-east-1

# VPS com T4 dedicando metade
youai-node start \
  --gpu-percent 50 \
  --model nex-n2-mini \
  --expert auto \
  --daemon

# Verificar que limites estão sendo respeitados
youai-node status
youai-node metrics --last 1h
```

### Enterprise / VPS em escala

```yaml
# youai-node.enterprise.yaml
contributor:
  name: "Acme Cloud"
  tier: enterprise
resources:
  gpu_percent: 50
  gpu_model: NVIDIA T4
  instances: 100
  regions: [sa-east-1, us-east-1]
schedule:
  contribute: "24/7"
models:
  - nex-n2-mini:experts:1-8
  - glm-5.2:experts:12-45
incentives:
  credit_multiplier: 2.0
  public_badge: true
```

---

## Resource Guard — controle para não explodir o PC

Camada **independente** do worker de inferência. Processo pequeno (Rust), alta prioridade de monitoramento, mata worker se furar limite.

### Limites configuráveis

| Recurso | Controle |
|---------|----------|
| GPU | % máxima + VRAM hard cap |
| CPU | % cores + prioridade mínima do SO (nice baixo) |
| RAM | cgroup / Job Object — OOM mata só o worker |
| Disco | cache de shards com quota + LRU |
| Rede | Mbps upload/download máx |
| Temperatura | GPU > 85°C → pausa automática |
| Bateria (mobile) | < 80% ou desplugado → off |

### Pausa automática

```
SE processo_pesado_usuario (jogo, Blender, etc.)
   OU gpu_temp > limite
   OU cpu_uso_usuario > 70%
   OU bateria (mobile)
ENTÃO → pausa imediata · job redireciona para outro nó
```

### Circuit breaker

| Evento | Ação |
|--------|------|
| 3 picos de temperatura em 10 min | pausa 30 min |
| OOM no worker | restart com -10% RAM, não sobe de novo |
| 2 crashes seguidos | nó sai da rede até revisão manual |
| Worker não responde 30s | watchdog SIGKILL |

### Processo no PC

```
┌──────────────────────────────────────────┐
│  youai-guard (Rust · ~5 MB)           │
│  ├── mede GPU/CPU/RAM/temp a cada 500ms  │
│  ├── mata worker se furar limite         │
│  └── métricas locais para o usuário      │
└───────────────┬──────────────────────────┘
                │
┌───────────────▼──────────────────────────┐
│  youai-worker (sandbox cgroup)           │
│  ├── llama.cpp / CUDA                    │
│  └── só lê ~/.youai/shards/              │
└───────────────┬──────────────────────────┘
                │
┌───────────────▼──────────────────────────┐
│  youai-agent (rede)                      │
│  ├── TLS → coordinator                   │
│  ├── jobs assinados                      │
│  └── nunca executa shell remoto          │
└──────────────────────────────────────────┘
```

### Regras mobile (mais duras que PC)

| Regra | Default |
|-------|---------|
| CPU/NPU máx | 10–15% |
| Só Wi-Fi | obrigatório |
| Só carregando | obrigatório |
| Bateria mínima | 80% |
| Notificação | sempre visível · "12% · toque para pausar" |
| Papel | expert pequeno ou relay — nunca tier GLM sozinho |

---

## Economia free (crédito, não cartão)

| Ação | Crédito |
|------|---------|
| Nó online 1h contribuindo | +X tokens de uso |
| PC com GPU 20–50% | +5–10X |
| Celular 10% | +1X |
| VPS enterprise T4 50% | +2X (multiplier) |
| Só usar, nunca contribuir | quota diária baixa |
| Contribuidor histórico | prioridade na fila |

**Free de verdade** = paga com hardware ocioso. Quem não pode contribuir ainda usa, com limite — modelo Wikipedia.

---

## Tiers de modelo para o usuário

```
┌─────────────────────────────────────────────┐
│  TIER RÁPIDO — Nex-N2-mini                   │
│  Poucos nós · resposta mais rápida            │
│  Chat, perguntas simples                      │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│  TIER AGENTE — Nex-N2-Pro                    │
│  Coding · tools · pesquisa · tarefas longas  │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│  TIER FRONTIER — GLM-5.2                     │
│  Raciocínio pesado · contexto 1M             │
│  Rede grande · fila maior · mais lento       │
└─────────────────────────────────────────────┘
```

---

## Segurança & confiança (open source auditável)

### O nó NÃO pode

- Executar código arbitrário da rede
- Ler arquivos fora de `~/.youai/`
- Abrir portas inbound aleatórias
- Instalar binários sem assinatura verificada

### Pipeline de confiança

1. Shards assinados (chave pública da comunidade YouAI)
2. Builds reproduzíveis (GitHub Actions + hash publicado)
3. Prompt do usuário **não** passa cru no nó contribuinte
4. Logs locais sanitizados — usuário vê tudo
5. Anti-malícia: 2+ réplicas do mesmo expert · votação de consistência
6. Reputação por uptime honesto

### Privacidade

- Nó comunitário recebe **ativações intermediárias** ou jobs opacos
- Coordinator agrega e devolve resposta
- Futuro: TEE / homomorphic para casos sensíveis

---

## Stack técnica (MVP → produção)

| Peça | Tecnologia | Motivo |
|------|------------|--------|
| Guard + Agent | **Rust** | seguro, leve, cross-platform |
| Inferência | **llama.cpp** | GLM-5.2 GGUF já funciona; auditável |
| CLI | `youai-node` | mesma lib do GUI |
| GUI desktop | **Tauri** | leve vs Electron |
| Mobile | Kotlin (Android) primeiro | background + notificação |
| Coordinator | **Rust** ou Go | escala, simples de auditar |
| Protocolo | gRPC + TLS 1.3 | jobs assinados, heartbeat |
| Licença | Apache 2.0 | empresa + comunidade |

---

## MVP — escopo fechado

### Objetivo do MVP

Provar em **4–8 semanas** que:

1. 10–50 PCs podem formar um cluster estável
2. Guard **nunca** fura limite configurado
3. Usuário contribui → ganha crédito → usa o chat free
4. Tudo open source e auditável

### Fora do MVP (explicitamente)

- ❌ Celular (fase 4)
- ❌ GLM-5.2 full (fase 3)
- ❌ Enterprise multi-instância (fase 3)
- ❌ Token on-chain
- ❌ iOS

### MVP — entregáveis

| # | Entregável | Descrição |
|---|------------|-----------|
| 1 | `youai-guard` | limites GPU/RAM/CPU · circuit breaker · watchdog |
| 2 | `youai-node` CLI | install, config, start, status, pause |
| 3 | `youai-worker` | llama.cpp integrado · 1 shard/expert |
| 4 | `youai-coordinator` | auth simples · roteamento · fila · crédito básico |
| 5 | `youai-web` | chat mínimo · login · saldo de crédito |
| 6 | Modelo | **Nex-N2-mini** quantizado |
| 7 | Docs | este arquivo + `CONTRIBUTING.md` + `SECURITY.md` |

### MVP — modelo e escala

- **Modelo:** Nex-N2-mini (35B-A3B) — menor barreira de entrada
- **Nós mínimos:** 10 PCs com 8+ GB RAM livre
- **Ideal:** 30–50 nós para redundância
- **Latência aceitável:** 3–15s para primeira token
- **Região:** uma região geográfica (ex.: Brasil · `sa-east`)

### MVP — fluxo do usuário

```
1. Baixa youai-node (site / GitHub releases)
2. Define limites: --gpu 30 --ram 8g
3. Guard valida · worker sobe · agent registra no coordinator
4. Recebe shard/expert automático
5. Ganha crédito por hora online
6. Acessa youai.network/chat · gasta crédito
7. "Pausar agora" a qualquer momento
```

---

## Roadmap de evolução

### Fase 0 — Dogfood (semana 1–2)
- [ ] Repo GitHub público
- [ ] Guard em Rust: RAM cap + CPU % no Linux
- [ ] llama.cpp local single-node (sem rede)
- [ ] 3 devs, mesma rede Wi-Fi

### Fase 1 — MVP rede (semana 3–6)
- [ ] Coordinator básico
- [ ] 10–50 nós · Nex-N2-mini sharded
- [ ] Crédito: hora online → tokens
- [ ] Web chat free
- [ ] CLI `youai-node` estável
- [ ] Windows + Linux (macOS best-effort)

### Fase 2 — Produto (mês 2–3)
- [ ] GUI Tauri com sliders
- [ ] Detecção de app pesado · pausa automática
- [ ] GPU guard (NVML)
- [ ] Nex-N2-Pro tier agente
- [ ] API pública · rate limit
- [ ] Discord/Telegram comunidade · 500 beta

### Fase 3 — Escala (mês 4–6)
- [ ] GLM-5.2 tier frontier (shards distribuídos)
- [ ] Enterprise YAML · VPS multi-instância
- [ ] Super-nós regionais
- [ ] Réplicas automáticas · reputação
- [ ] Agent harness (tools, sandbox) para N2-Pro

### Fase 4 — Mobile & global (mês 6+)
- [ ] Android app · regras duras · notificação
- [ ] iOS sessão limitada
- [ ] Multi-região · latência otimizada
- [ ] Governança comunitária · votação de modelos
- [ ] Auditoria de segurança externa

---

## Métricas de sucesso

### MVP (fase 1)
| Métrica | Alvo |
|---------|------|
| Nós estáveis simultâneos | ≥ 20 |
| Uptime médio do cluster | ≥ 85% |
| Violações de limite (furou GPU/RAM) | **0** |
| Tempo para pausar | < 2s |
| Usuários usando chat free | ≥ 100 |
| Crashes do worker / 24h | < 5% dos nós |

### Fase 3
| Métrica | Alvo |
|---------|------|
| Nós totais registrados | ≥ 10.000 |
| Inferências / dia | ≥ 100.000 |
| Tier GLM-5.2 disponível | ≥ 95% uptime |
| Enterprise contributors | ≥ 3 |

---

## Riscos e mitigações

| Risco | Mitigação |
|-------|-----------|
| Guard falha · PC explode | watchdog independente · testes automatizados · cgroup hard cap |
| Cold start · poucos nós | MVP regional · super-nó âncora do time |
| App Store bloqueia mobile | Android primeiro · iOS sessão manual |
| Roubo de weights | shards criptografados · TEE futuro |
| Nó malicioso | réplicas + votação + reputação |
| Latência frustrante | tiers · expectativa clara · N2-mini rápido |
| Empresa abusa do "50%" | métricas públicas · auditoria · revogação |

---

## Governança open source

```
youai/
├── youai-guard/      # limites de hardware
├── youai-node/          # CLI + lib compartilhada
├── youai-worker/        # llama.cpp wrapper
├── youai-coordinator/   # roteador + crédito + fila
├── youai-web/           # chat + dashboard
├── youai-mobile/        # (fase 4)
└── docs/
    ├── MVP.md           # este documento
    ├── ARCHITECTURE.md  # (fase 1)
    ├── SECURITY.md      # (fase 1)
    └── CONTRIBUTING.md  # (fase 0)
```

- Decisões de modelo: comunidade vota (fase 4)
- Security issues: responsible disclosure · SECURITY.md
- Builds: reproduzíveis · hash em cada release

---

## Referências técnicas

| Recurso | Link / nota |
|---------|-------------|
| GLM-5.2 | Z.ai · 744B MoE · GGUF Unsloth · llama.cpp |
| Nex-N2-Pro | nex-agi/Nex-N2-Pro · 397B-A17B · agentic |
| Nex-N2-mini | nex-agi/Nex-N2-mini · 35B-A3B · MVP |
| Inspiração | Petals · Folding@home · exo |
| Runtime | llama.cpp · sglang (fork Nex para N2-Pro futuro) |

---

## Próximos passos imediatos

1. [ ] Validar nome **YouAI** · registrar domínio (`youai.network` / `youai.dev`)
2. [ ] Criar org GitHub `youai-network`
3. [ ] Scaffold `youai-guard` em Rust (Linux cgroup POC)
4. [ ] Baixar Nex-N2-mini quantizado · benchmark single-node
5. [ ] Recrutar 10 beta testers com PC
6. [ ] Escrever `SECURITY.md` e `CONTRIBUTING.md`

---

## Apêndice — evolução da conversa

Este documento consolida a visão discutida:

1. **Cluster caseiro** — 10 celulares / PCs na mesma rede
2. **Escala global** — 1M nós · MoE · réplicas · super-nós
3. **Modelos frontier open** — GLM-5.2 · Nex-N2-Pro · quantização
4. **YouAI Node** — GUI + CLI + mobile · limites · VPS enterprise
5. **Guard** — nunca explodir PC/celular · 100% open source
6. **Economia free** — crédito por contribuição · tiers de modelo

---

*YouAI — AI by you, for you.*