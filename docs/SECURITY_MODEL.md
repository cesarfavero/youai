# YouAI — Modelo de Segurança (design completo)

> **Status:** design v1.1 · documento normativo  
> **Última atualização:** junho 2026  
> **Relacionado:** [SECURITY.md](./SECURITY.md) (disclosure) · [MODEL_TIERS.md](./MODEL_TIERS.md) · [PRODUCT.md](./PRODUCT.md)

Este documento define **como o YouAI protege quem usa a IA** e **quem empresta o PC**. Toda feature nova deve ser avaliada contra este modelo antes de merge.

---

## 1. Princípios inegociáveis

| # | Princípio | Significado prático |
|---|-----------|---------------------|
| 1 | **Opt-in explícito** | Nada corre sem o utilizador ter aceitado limites claros (CPU, RAM, GPU, horários). |
| 2 | **Anonimato por defeito** | Chat não exige conta; identidade mínima; sem venda de dados. |
| 3 | **E2E onde importa** | Prompt e resposta nunca em plaintext nos nós voluntários. |
| 4 | **Zero inbound no nó** | Ninguém na internet liga ao PC do contribuinte. |
| 5 | **Sandbox fixo** | Worker só lê/escreve em `~/.youai/` (paths allowlist). |
| 6 | **Sem execução arbitrária** | Nó nunca corre scripts, URLs, ficheiros ou binários enviados pela rede. |
| 7 | **Guard independente** | `youai-guard` é processo separado; mata worker se ultrapassar limites. |
| 8 | **Transparência total** | Comportamento documentado; código aberto; métricas visíveis ao utilizador. |
| 9 | **Merge conservador** | Issues abertas; **PRs exigem revisão humana** + checklist de segurança. |
| 10 | **Não atrapalhar o dia a dia** | Contribuição em background; pausa automática quando o utilizador precisa do PC. |

---

## 2. Actores e superfícies de ataque

```
┌──────────────┐     E2E (fase 2+)      ┌─────────────────┐
│  Utilizador  │ ◄──────────────────► │  Coordinator /   │
│  (App Chat)  │     TLS sempre        │  Gateway         │
└──────────────┘                       └────────┬────────┘
                                                │ jobs assinados
                                                │ payloads opacos
                        ┌───────────────────────┼───────────────────────┐
                        ▼                       ▼                       ▼
                 ┌─────────────┐         ┌─────────────┐         ┌─────────────┐
                 │  Nó A       │         │  Nó B       │         │  Nó C       │
                 │  guard      │         │  guard      │         │  guard      │
                 │  worker     │         │  worker     │         │  worker     │
                 │  ~/.youai/  │         │  ~/.youai/  │         │  ~/.youai/  │
                 └─────────────┘         └─────────────┘         └─────────────┘
```

### Actores

| Actor | O que quer | Risco se comprometido |
|-------|------------|------------------------|
| **Utilizador (chat)** | Respostas úteis, privacidade | Exposição de prompts, historico |
| **Contribuinte (nó)** | Ajudar sem risco no PC | Malware, roubo de ficheiros, mineração |
| **Coordinator** | Rotear jobs, tiers, crédito | Ver prompts, falsificar jobs, DoS |
| **Atacante externo** | Explorar rede | Supply chain, MITM, nó malicioso |
| **Maintainer** | Evoluir produto com segurança | Merge acidental de backdoor |

---

## 3. O que o PC do contribuinte **nunca** faz

Estas regras são **hard requirements** para qualquer PR:

| Proibido | Porquê | Enforcement (actual / planeado) |
|----------|--------|----------------------------------|
| Abrir portas de entrada | Remote shell, RCE | Outbound-only; firewall recomendado |
| Ler `~/Documents`, email, browser, etc. | Privacidade | Path allowlist `~/.youai/` apenas |
| Escrever fora de `~/.youai/` | Persistência maliciosa | Sandbox + revisão de código |
| Executar código recebido da rede | RCE | Só binários locais assinados; jobs com schema fixo |
| Descarregar e executar URLs | Drive-by | Proibido no worker; URL fetch só no gateway (fase T3) |
| Instalar software | Supply chain | Modelos só via **Model Registry** com hash verificado |
| Usar GPU/RAM/CPU acima do teto | Atrapalhar utilizador | `youai-guard` + cgroups |
| Ver prompt do utilizador em claro | Privacidade | Pipeline v4 MVP: activações opacas (evolução E2E) |

---

## 4. Sandbox do nó contribuinte

### 4.1 Árvore de ficheiros permitida

```
~/.youai/
├── config.toml              # config do utilizador (limites, tier, node id)
├── models/                  # GGUF verificados (hash no registry)
├── pipeline-stages/         # layer-splits (pipeline v3/v4)
├── shards/                  # GGUF splits v2
├── runtime.json             # pid do worker (pause rápido)
├── coordinator.db           # (só no coordinator, não no nó)
└── sessions/                # estado de pipeline (por sessão, TTL)
```

**Tudo o resto do filesystem é inacessível** ao worker. O `model_path` em produção deve apontar apenas para ficheiros dentro desta árvore cujo hash conste no registry.

### 4.2 Processos permitidos

| Processo | Função | Quem inicia |
|----------|--------|-------------|
| `youai-node` | CLI, registo, heartbeat | Utilizador |
| `youai-guard` | Limites, watchdog | `youai-node start` |
| `youai-worker` | HTTP local, inferência | guard |
| `youai-pipeline-step` | Forward de camadas (subprocess ou `--daemon`) | worker |
| `llama-completion` | Inferência réplica / v1 RPC | worker |

Nenhum outro binário é invocado com input da rede.

### 4.3 Rede do nó

| Direção | Permitido | Destino |
|---------|-----------|---------|
| Outbound | ✅ | Coordinator (register, heartbeat) |
| Outbound | ✅ | Outros workers **apenas** em jobs assinados de pipeline v2 (fetch shard) |
| Inbound | ❌ | Nada na internet |
| Localhost | ✅ | Worker `:7741` (health); não exposto à LAN por defeito |

---

## 5. Protecção do dia a dia (background sem atrapalhar)

O contribuinte usa o PC para trabalho, jogos, vídeo, etc. O YouAI **não pode** roubar a máquina.

### 5.1 Limites por defeito (Mac Mini / desktop)

| Recurso | Defeito dogfood | Defeito produção (alvo) |
|---------|-----------------|-------------------------|
| CPU | 30% | 30% (configurável 10–50%) |
| RAM | 2 GB (tier 1) | Até `ram_max` no config |
| GPU | off no tier 1 CPU | 50% se opt-in |
| Disco (modelos) | ~250 MB tier 1 | Por tier no registry |

### 5.2 Pausa inteligente (roadmap)

| Sinal | Acção |
|-------|--------|
| Utilizador corre `youai-node pause` | Paragem em < 2 s |
| CPU do sistema > 85% (utilizador a trabalhar) | Pausa automática temporária |
| Bateria baixa (laptop) | Pausa + notificação |
| Jogo em fullscreen / app na lista de exclusão | Pausa |
| Horário “não contribuir” (noite, reunião) | Agenda no app |

### 5.3 Prioridade de processo

- Worker corre com **nice** elevado / prioridade baixa no OS.
- Guard monitoriza a cada **500 ms**; kill imediato se RAM/CPU > teto por janela sustentada.

---

## 6. Privacidade e criptografia

### 6.1 Camadas de protecção de dados

| Dado | Onde circula | MVP (hoje) | Alvo (produção) |
|------|--------------|------------|-----------------|
| Prompt / resposta | App ↔ Coordinator | TLS; plaintext no coordinator | **E2E** (chaves no cliente) |
| Activations pipeline | Coordinator ↔ workers | TLS + base64 (dogfood) | Blob cifrado; chave só no coordinator/cliente |
| Modelo GGUF | Disco do nó | Ficheiro local | Hash SHA-256 vs registry assinado |
| Registo de nó | Coordinator DB | token UUID; sem PII | Igual; retenção mínima |
| Métricas | Coordinator | uptime, tier, recursos agregados | Anónimo; sem fingerprinting |

### 6.2 Anonimato do utilizador de chat

- **Sem email obrigatório** na fase inicial.
- Identidade = `device_id` aleatório ou par de chaves gerado no primeiro arranque do app.
- Histórico: opt-in; TTL máximo configurável; apagar conta = apagar chaves locais.
- Coordinator **não** vende nem partilha prompts com terceiros.

### 6.3 Criptografia alvo (app desktop / web)

```
App gera par de chaves (X25519)
        │
        ▼
Handshake com Coordinator → chave de sessão (HKDF)
        │
        ▼
Prompt cifrado (AEAD ChaCha20-Poly1305) → Coordinator
        │
        ▼
Jobs para nós = activações / tensors opacos (sem plaintext)
        │
        ▼
Resposta cifrada → App
```

Os nós voluntários **nunca** recebem a chave de sessão do chat.

### 6.4 Rede fechada — só contribuintes acedem à infra

A infraestrutura YouAI (coordinator, jobs de inferência, registry de modelos para nós) **não é API pública aberta**.

| Quem | Acesso | Como |
|------|--------|------|
| **Nó contribuinte** | ✅ register, heartbeat, jobs, registry | Token de nó + requests assinados |
| **App oficial (chat)** | ✅ chat via coordinator | Credencial de dispositivo + E2E + requests assinados |
| **Pessoa de fora / HTTP anónimo** | ❌ | Sem token → `401`; assinatura inválida → `403` |
| **Scrapers / bots** | ❌ | Rate limit + assinatura obrigatória |

**O que isto significa na prática:**

- Não existe endpoint público tipo `POST /chat` sem autenticação.
- Registar nó exige **opt-in de contribuição** (app ou CLI) — não é open relay.
- Utilizadores só-chat usam o **app oficial**; não ligam directamente aos workers voluntários.
- Workers **nunca** aceitam conexões da internet — só jobs assinados do coordinator (localhost).

Utilizadores não-contribuintes podem usar chat com quota/crédito **através do app**, mas **não** acedem à camada de infra nem aos nós.

---

### 6.5 Blockchain — não usar no caminho crítico

**Decisão:** YouAI **não** usa blockchain para segurança core.

| Problema | Blockchain resolve? | Solução YouAI |
|----------|---------------------|---------------|
| Prompt privado nos nós | ❌ Não | **E2E** (ChaCha20-Poly1305) |
| Job adulterado em trânsito | ❌ Overkill | **Ed25519** + timestamp + nonce |
| Manifest de modelo trocado | Parcial, caro | **Manifest assinado** + SHA256 + git público |
| Replay de request | ❌ Não nativo | **Nonce cache** + `expires_at` curto |
| Auditoria imutável | Sim, mas lento | **Transparency log** (Merkle append-only) — opcional, fase 2 |

**Porquê não blockchain:**

- Latência e custo inaceitáveis para cada token de inferência
- Não substitui E2E — dados ainda passariam em claro sem cifra de aplicação
- Complexidade operacional (gas, wallets, forks) sem ganho de segurança proporcional
- Open source + manifests assinados + CI público já dão auditabilidade

**Conclusão:** **E2E para conteúdo** + **assinatura Ed25519/HMAC para integridade de requests** = stack correcta. Blockchain só se no futuro houver requisito legal de ledger distribuído (improvável).

---

### 6.6 Requests imutáveis — assinatura, timestamp, nonce

Objectivo: **nenhum byte do request pode ser alterado** sem invalidar a assinatura. Replay e jobs expirados são rejeitados.

#### Camadas (defesa em profundidade)

```
┌─────────────────────────────────────────────────────────┐
│ 1. TLS 1.3          — transporte cifrado                │
│ 2. E2E (chat)       — prompt/resposta opacos no coord.  │
│ 3. Assinatura app   — integridade + autenticidade       │
│ 4. Job signing      — worker só executa jobs assinados  │
│ 5. Hash registry    — modelo imutável no disco do nó    │
└─────────────────────────────────────────────────────────┘
```

#### Formato canónico do request (v1)

Todos os campos assinados em **JSON canónico** (chaves ordenadas, sem whitespace):

```json
{
  "v": 1,
  "type": "chat.completion",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "issued_at": 1719158400,
  "expires_at": 1719158460,
  "nonce": "8f3a2b1c4d5e6f70",
  "actor": {
    "kind": "node",
    "id": "node-uuid",
    "token_id": "tok_abc"
  },
  "body_hash": "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
  "body": { "tier": "tier1", "mode": "pipeline_activation_v4", "blob": "..." }
}
```

| Campo | Função |
|-------|--------|
| `v` | Versão do schema — worker rejeita desconhecido |
| `id` | ID único do request (idempotência) |
| `issued_at` | Unix segundos UTC — relógio do emissor |
| `expires_at` | TTL curto (30–120 s jobs; 5 min register) |
| `nonce` | 64 bits aleatórios — anti-replay |
| `body_hash` | SHA256 do `body` serializado — detecta alteração parcial |
| `actor` | Quem assina (nó, app, coordinator) |

#### Assinatura (preferir Ed25519; HMAC onde simétrico)

**Produção — Ed25519 (assimétrico):**

```text
message = canonical_json({ v, type, id, issued_at, expires_at, nonce, actor, body_hash })
signature = Ed25519_sign(private_key, message)
header: X-YouAI-Signature: ed25519:<base64>
header: X-YouAI-Key-Id: coordinator-v1
```

**Heartbeat nó ↔ coordinator — HMAC-SHA256 (simétrico, chave por nó):**

```text
message = "{issued_at}|{nonce}|{method}|{path}|{body_hash}"
mac = HMAC-SHA256(node_secret, message)
header: X-YouAI-MAC: hmac-sha256:<base64>
```

| Canal | Algoritmo | Porquê |
|-------|-----------|--------|
| Coordinator → Worker (job) | Ed25519 | Worker só precisa da chave pública |
| App → Coordinator (chat) | Ed25519 (device key) | Sem segredo partilhado no cliente |
| Nó → Coordinator (heartbeat) | HMAC-SHA256 | Leve; segredo gerado no register |
| Registry manifest | Ed25519 | Verificável offline no nó |

#### Validação (coordinator e worker)

Ordem obrigatória — falha em qualquer passo → **rejeitar sem executar**:

1. `expires_at > now()` — senão `408 Request Expired`
2. `issued_at` dentro de janela (ex: ±60 s do relógio do servidor) — senão `400 Clock Skew`
3. `nonce` não visto nos últimos N minutos (cache Redis/SQLite) — senão `409 Replay`
4. `body_hash == SHA256(canonical(body))` — senão `400 Body Tampered`
5. Verificar assinatura Ed25519/HMAC sobre campos **sem** incluir `body` em claro duplicado
6. `actor.token_id` activo e não revogado
7. Schema `type` na allowlist — senão `400 Unknown Operation`
8. Só então executar

**Regra:** worker **nunca** altera o request — só lê `payload`/`blob` opaco e responde.

#### Job de inferência (coordinator → worker)

```json
{
  "v": 1,
  "type": "inference.pipeline_step",
  "id": "job-uuid",
  "issued_at": 1719158400,
  "expires_at": 1719158460,
  "nonce": "a1b2c3d4e5f60718",
  "coordinator_sig": "ed25519:BASE64...",
  "payload": {
    "op": "forward_activation",
    "session_id": "sess-uuid",
    "stage": 0,
    "blob": "base64-opaque-or-ciphertext"
  }
}
```

Worker rejeita: expirado, assinatura inválida, `op` desconhecido, `stage` ≠ config do nó.

#### Manifest imutável (registry)

```text
manifest_bytes = canonical_json(manifest sem campo signature)
signature = Ed25519_sign(maintainer_key, SHA256(manifest_bytes))
```

Nó recusa manifest com assinatura inválida ou `issued_at` mais antigo que o já instalado (anti-rollback).

#### Transparency log (opcional, sem blockchain)

- Maintainers publicam `SHA256(manifest)` num log append-only (ficheiro assinado ou CT-style)
- Permite auditar que ninguém trocou modelo às escondidas
- Nós podem verificar offline contra o log

---

### 6.7 Resumo: o que protege cada ameaça

| Ameaça | E2E | Assinatura + nonce | Hash registry |
|--------|-----|-------------------|---------------|
| Ler prompt no nó | ✅ | — | — |
| Alterar request MITM | parcial (TLS) | ✅ | — |
| Replay de job | — | ✅ | — |
| Modelo trocado | — | — | ✅ |
| Coordinator comprometido | minimiza exposição E2E | limita janela | — |

**E2E e assinatura são complementares — não alternativas.**

---

## 7. Threat model detalhado

| Ameaça | Cenário | Impacto | Mitigação | Fase |
|--------|---------|---------|-----------|------|
| **T1** Nó malicioso devolve inferência errada | Atacante regista nó | Respostas corruptas | Réplica + votação; reputação; ban | Beta |
| **T2** Bypass de limites RAM/CPU | Bug no guard | PC travado | Guard independente; testes; cgroup v2 | MVP |
| **T3** Leitura de ficheiros pessoais | Worker comprometido | Privacidade | Allowlist `~/.youai/` | MVP |
| **T4** RCE via payload de rede | Job malformado | Controlo total PC | Schema fixo; sem eval; assinatura | Alpha |
| **T5** Roubo de pesos do modelo | Cópia de GGUF | IP / custo | Licença modelo; TEE futuro | Longo prazo |
| **T6** Coordinator comprometido | Hack central | Ver todos os prompts | E2E; minimização de dados; HSM futuro | Beta |
| **T7** MITM | Rede Wi‑Fi hostil | Interceptar chat | TLS 1.3; pinning no app | Alpha |
| **T8** Supply chain | Dependência/npm/rust malicioso | Backdoor | `cargo audit`; releases assinadas; CI público | Contínuo |
| **T9** Prompt injection em tools | Upload/URL (fase T3) | Exfiltração | Tools só no gateway; sandbox | T3+ |
| **T10** Denial of service na rede | Spam de chat | Indisponível | Rate limit; crédito; fila | Beta |

---

## 8. Pipeline distribuído e segurança

| Versão | Risco específico | Mitigação |
|--------|------------------|-----------|
| v1 RPC | Porta RPC exposta | Só localhost + forward controlado |
| v2 GGUF | Fetch de shard malicioso | Hash no registry; HTTPS entre workers conhecidos |
| v3/v4 Activation | Activations leak info do prompt | Comprimir + cifrar; minimizar passagem pelo coordinator |
| v4 Daemon | Processo long-lived comprometido | Mesmo sandbox; restart periódico; sem shell |

---

## 9. Governança de código (PRs e issues)

### 9.1 Política de merge

- **Issues:** abertas à comunidade.
- **Pull requests:** **sempre** revisão de pelo menos um maintainer.
- **Nenhum auto-merge** em código que toque: `youai-guard`, `youai-worker`, `youai-node`, `youai-coordinator`, `native/`, `scripts/` de deploy.

### 9.2 Checklist obrigatória em PR (segurança)

Copiar para a descrição do PR:

```markdown
## Security checklist
- [ ] Não lê filesystem fora de ~/.youai/
- [ ] Não executa código/binários recebidos da rede
- [ ] Não abre portas inbound novas
- [ ] Não loga prompt ou PII em claro
- [ ] Limites guard/cpu/ram respeitados ou documentada excepção
- [ ] Novos downloads só via Model Registry (hash)
- [ ] Documentação actualizada (README ou docs/)
- [ ] Testes passam (cargo test, clippy)
```

PRs que falham itens críticos **não são merged** até correcção.

---

## 10. Transparência para utilizadores

O utilizador e o contribuinte devem **sempre** poder ver:

| Informação | Onde |
|------------|------|
| O que o nó está a fazer | `youai-node status` + app dashboard |
| Limites activos | `config.toml` + UI |
| Modelo carregado e tier | Registry + status |
| Dados enviados à rede | Política de privacidade + docs |
| Código que corre no PC | GitHub público |
| Histórico de releases | CHANGELOG + hashes |

**Regra:** se um comportamento não está documentado, é bug de transparência.

---

## 11. Roadmap de implementação de segurança

| Prioridade | Item | Componente |
|------------|------|------------|
| P0 | Path allowlist no worker | youai-worker |
| P0 | Checklist PR no CONTRIBUTING | docs |
| P1 | Request signing (Ed25519 + HMAC + nonce) | coordinator + worker + app |
| P1 | Rede fechada — auth obrigatória | coordinator |
| P1 | Model hash verification | node + registry |
| P2 | E2E no app chat | youai-web / desktop |
| P2 | Activation encryption | pipeline |
| P3 | Réplica voting | coordinator |
| P3 | Pausa por carga do sistema | youai-node |
| P4 | TEE / enclave | pesquisa |

---

## 12. Estado actual (honestidade)

| Capacidade | Estado |
|------------|--------|
| Guard RAM/CPU | ✅ Parcial (dogfood) |
| Outbound-only | ✅ Por design |
| Prompt opaco nos nós (pipeline) | ⚠️ Activations em claro no coordinator (LAN) |
| E2E chat | ❌ Planeado |
| Job signing (Ed25519/HMAC) | ❌ Planeado — spec em §6.6 |
| Rede fechada (só contribuintes) | ❌ Planeado — spec em §6.4 |
| Model registry com hash | ⚠️ `registry/manifest.json` tier1 · verify no node planeado |
| Pausa inteligente | ❌ Planeado |

Tudo o que está ❌ ou ⚠️ deve aparecer nas release notes até ser resolvido.

---

*YouAI — IA sua, feita por você. Segurança não é feature; é permissão para existir.*