# YouAI — Model Tiers & Registry

> **Status:** design v1.0 · documento normativo  
> **Última atualização:** junho 2026  
> **Relacionado:** [PRODUCT.md](./PRODUCT.md) · [SECURITY_MODEL.md](./SECURITY_MODEL.md) · [PIPELINE.md](./PIPELINE.md)

Este documento define **qual modelo a rede usa**, **quando sobe de tier**, **como se actualiza**, e **quem acede primeiro** a melhorias. O **Model Registry** é o sítio central que gere tudo isto.

---

## 1. Filosofia

1. **Qualidade de texto depende do modelo** — não adianta pipeline perfeito com modelo fraco para tudo.
2. **Tier 1 tem de correr no Mac Mini sem incomodar** — pouca RAM, CPU em background, sem GPU obrigatória.
3. **A rede escala o modelo** — mais nós saudáveis com recursos compatíveis → coordinator libera tier superior.
4. **Transparência** — utilizador vê tier actual, modelo, requisitos, e porquê ainda não subiu.
5. **Contribuintes primeiro** — quem empresta hardware acede a tiers/features novas antes; utilizadores só-chat seguem quando a rede aguenta.
6. **Updates controlados** — nunca `curl | bash` de modelo; só downloads do registry com hash verificado.

---

## 2. Model Registry (gestão central)

### 2.1 O que é

O **Model Registry** é um serviço lógico (hoje: ficheiro + API no coordinator; amanhã: serviço dedicado) que:

- Publica **manifestos** de modelos por tier
- Define **requisitos mínimos** por nó (RAM, CPU, pipeline stages)
- Define **limiares de rede** para activar cada tier
- Assina manifests (Ed25519) para o nó verificar integridade
- Orquestra **rollout** gradual (canary → full)

### 2.2 Localização (evolução)

| Fase | Implementação |
|------|----------------|
| **Agora (dogfood)** | `registry/manifest.json` no repo + script `download-model.sh` |
| **Alpha** | `GET /api/v1/registry/manifest` no coordinator |
| **Beta** | Registry CDN + coordinator como cache/autoridade |
| **Produção** | `registry.youai.network` com assinatura e espelhos |

### 2.3 Manifesto (schema)

```json
{
  "version": 1,
  "issued_at": "2026-06-23T00:00:00Z",
  "signature": "ed25519:...",
  "default_tier": "tier1",
  "tiers": {
    "tier1": {
      "id": "tier1",
      "display_name": "YouAI Spark",
      "description": "Leve — Mac Mini, laptops, background",
      "models": [
        {
          "id": "smollm2-360m-instruct-q4_k_m",
          "filename": "smollm2-360m-instruct-q4_k_m.gguf",
          "url": "https://huggingface.co/...",
          "sha256": "abc123...",
          "size_bytes": 230000000,
          "pipeline": {
            "kind": "activation",
            "stages": 2,
            "stage_files": [
              "smollm2-360m-instruct-q4_k_m-stage00-of-02.gguf",
              "smollm2-360m-instruct-q4_k_m-stage01-of-02.gguf"
            ]
          },
          "runtime": {
            "ram_min_mb": 512,
            "ram_recommended_mb": 1024,
            "cpu_percent_default": 30,
            "gpu_required": false
          }
        }
      ],
      "network_requirements": {
        "min_contributors_online": 2,
        "min_total_ram_gb": 4,
        "min_pipeline_chains": 1
      },
      "features": ["chat", "pipeline_v4"]
    }
  }
}
```

### 2.4 Fluxo de update de modelo

```
Maintainers publicam novo manifest (ex: tier2)
        │
        ▼
Coordinator valida assinatura
        │
        ▼
Avalia métricas da rede (N nós, RAM, chains)
        │
        ├── Limiar não atingido → mantém tier actual + mensagem transparente
        │
        └── Limiar atingido → rollout:
                1. Canary (5% nós contribuintes voluntários / reputação alta)
                2. Download automático para ~/.youai/models/ (hash check)
                3. youai-node reload / próximo start
                4. Full rollout quando erro rate < threshold
```

### 2.5 API planeada (coordinator)

| Endpoint | Função |
|----------|--------|
| `GET /api/v1/registry/manifest` | Manifesto completo assinado |
| `GET /api/v1/registry/tier` | Tier activo para esta rede + razão |
| `GET /api/v1/registry/models/{id}` | Metadados + URL + hash |
| `POST /api/v1/nodes/model-status` | Nó reporta modelo instalado + hash |

---

## 3. Tabela de tiers

### 3.1 Resumo

| Tier | Nome | Modelo (referência) | RAM/nó (alvo) | Rede mínima | Features |
|------|------|---------------------|---------------|-------------|----------|
| **tier0** | Lab | SmolLM2-360M (dev) | 512 MB | 1 nó | dev only |
| **tier1** | Spark | SmolLM2-360M Q4_K_M | 1 GB | 2 contribuintes | chat, pipeline v4 |
| **tier2** | Glow | SmolLM2-1.7B Q4 ou Qwen2.5-0.5B | 2 GB | 10 contribuintes | + contexto longo |
| **tier3** | Beam | 3B class Q4 | 4 GB | 100 contribuintes | + tools gateway |
| **tier4** | Arc | 7B sharded pipeline | 4 GB/stage | 1 000 contribuintes | + upload leitura* |
| **tier5** | Horizon | 7B+ / MoE parcial | variável | 10 000 contribuintes | + URL, busca* |

\* Upload, análise de URL e busca **não** correm nos nós voluntários — correm no **gateway** com sandbox (ver [PRODUCT.md](./PRODUCT.md)).

### 3.2 Tier 1 — Spark (Mac Mini hoje)

**Objectivo:** rodar no teu Mac Mini M1/M2 em background sem atrapalhar.

| Parâmetro | Valor |
|-----------|--------|
| Modelo | `smollm2-360m-instruct-q4_k_m` (~220 MB em disco) |
| RAM em inferência | ~0.5–1.5 GB (com guard `ram_max=2g`) |
| CPU | 30% máximo (configurável) |
| GPU | Opcional; tier1 não exige |
| Pipeline | v4 activation, 2 stages (Mac + 1 VM/PC) |
| Qualidade | Adequada para dogfood e chat curto; **não** frontier |

**Comandos actuais:**

```bash
./scripts/download-model.sh
./scripts/split-model-layers.py   # para pipeline
./scripts/setup-pipeline-activation-mac.sh
export YOUAI_BIN_DIR="$PWD/target/release"
youai-node start
```

### 3.3 Critérios para subir de tier

O coordinator calcula periodicamente:

```text
tier_ativo = max { tierN | rede_satisfaz(tierN.network_requirements) }
```

| Métrica | Como medir |
|---------|------------|
| `contributors_online` | Nós com heartbeat < 90s e `shard_total_stages` ou réplica |
| `total_ram_gb` | Soma de `ram_max` declarado pelos nós online |
| `pipeline_chains` | Cadeias completas `default-pipeline` por tier |
| `error_rate` | Falhas de inferência / total (últimas 24h) |
| `p95_latency` | Tempo de resposta chat pipeline |

**Regra de histerese:** para **descer** tier, limiar inferior (ex: 80% do mínimo) evita flapping.

### 3.4 Exemplo numérico (visão 10k)

| Utilizadores | Contribuintes online (est.) | Tier provável | Modelo |
|--------------|----------------------------|---------------|--------|
| 10 | 5 | tier1 | SmolLM2-360M |
| 100 | 40 | tier2 | 1.7B Q4 |
| 1 000 | 300 | tier3–4 | 3B–7B sharded |
| 10 000 | 2 000+ | tier5 | 7B+ rede estável, features T3 |

Com 10k na rede e 2k contribuintes saudáveis, um modelo “legal” (tier4–5) **roda de boa** porque há RAM agregada e pipeline chains de sobra — exactamente como descreveste.

---

## 4. Acesso antecipado (contribuintes)

### 4.1 Regra

| Papel | Acesso a novo tier/feature |
|-------|---------------------------|
| **Contribuinte activo** (nó online > X h/semana) | Canary + fila prioritária |
| **Utilizador só-chat** | Quando tier está **estável** na rede (pós-canary) |
| **Não contribuinte** | Mesmo tier, possível fila/crédito mais baixo (fase crédito) |

### 4.2 Implementação (roadmap)

- `contributor_score` no coordinator: uptime, jobs completados, sem violações guard
- Flag `early_access: true` no registo do nó
- App mostra: *“Estás em canary do Tier 2 — obrigado por contribuir”*

### 4.3 Justiça

Contribuintes **não** vêem prompts de outros (E2E). Benefício = **modelo melhor + features antes**, não dados alheios.

---

## 5. Features por tier (produto)

| Feature | tier1 | tier2 | tier3 | tier4 | tier5 |
|---------|-------|-------|-------|-------|-------|
| Chat texto | ✅ | ✅ | ✅ | ✅ | ✅ |
| Pipeline multi-PC | ✅ | ✅ | ✅ | ✅ | ✅ |
| Contexto longo | curto | médio | longo | longo | longo |
| Upload ficheiros (leitura) | — | — | gateway | gateway | gateway |
| Análise URL | — | — | — | gateway | gateway |
| Busca web | — | — | — | — | gateway |
| Agentes / tools | — | — | limitado | sim | sim |

**Importante:** upload/URL/busca usam **infra YouAI** (gateway), não o PC do voluntário.

---

## 6. Operação no nó (download e verify)

### 6.1 Fluxo no `youai-node`

```
start
  │
  ├─► GET /api/v1/registry/tier
  │
  ├─► Compara modelo local ~/.youai/models/ vs manifest.sha256
  │
  ├─► Se falta ou hash errado → download + verify
  │
  ├─► Se pipeline → split/layers conforme manifest
  │
  └─► worker com modelo allowlisted
```

### 6.2 Comandos CLI (planeados)

```bash
youai-node models list          # tiers e modelos disponíveis
youai-node models status        # o que está instalado + hash
youai-node models update        # sync com registry (verify hash)
youai-node tier                 # tier activo na rede + requisitos em falta
```

### 6.3 Protecção

- Download só de URLs no manifest assinado
- `sha256` obrigatório antes de carregar no worker
- Falha de hash → recusa carregar + log claro

---

## 7. Qualidade de texto vs infraestrutura

| Factor | Responsabilidade |
|--------|------------------|
| Coerência, raciocínio, idioma | **Modelo (tier)** |
| Latência, multi-PC | Pipeline v4, daemon |
| Disponibilidade | Número de contribuintes |
| Privacidade | SECURITY_MODEL + E2E |
| Chat template, EOS | Coordinator / app (melhora qualidade sem subir tier) |

Subir tier é a alavanca principal; template SmolLM2 no tier1 melhora instruct sem mais RAM.

---

## 8. Roadmap de implementação

| # | Entrega | Componente |
|---|---------|------------|
| 1 | `registry/manifest.json` tier1 | repo |
| 2 | `GET /api/v1/registry/tier` (estático) | coordinator |
| 3 | Verificação SHA256 no node start | youai-node |
| 4 | Métricas de rede → escolha tier | coordinator |
| 5 | `youai-node models update` | CLI |
| 6 | Canary rollout | coordinator |
| 7 | contributor_score + early access | coordinator + app |
| 8 | tier2 manifest + download automático | registry |

---

## 9. Transparência (o que o utilizador vê)

No app / `youai-node status`:

```
Rede YouAI
  Tier activo:     tier1 (Spark)
  Modelo:          smollm2-360m-instruct-q4_k_m
  Próximo tier:    tier2 (Glow) — faltam 6 contribuintes online
  Teu papel:       contribuinte · early_access: sim
  Recursos:        CPU 30% · RAM 2 GB · modelo em ~/.youai/models/
  Privacidade:     E2E em implementação · ver SECURITY_MODEL.md
```

---

*YouAI — o modelo cresce com a rede; a rede cresce com a confiança.*