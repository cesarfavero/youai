# YouAI — Produto, App e Experiência

> **Status:** design v1.0 · documento normativo  
> **Última atualização:** junho 2026  
> **Relacionado:** [MVP.md](./MVP.md) · [MODEL_TIERS.md](./MODEL_TIERS.md) · [SECURITY_MODEL.md](./SECURITY_MODEL.md)

Este documento descreve **o que o utilizador final vê**, **como contribui com o PC**, **como escalamos modelo e features**, e **como mantemos transparência total**.

---

## 1. Visão do produto

### 1.1 Uma frase

**App de chat + IA (como ChatGPT / Codex) onde, ao instalares, podes usar modelos gratuitos — e, se quiseres, o teu PC ajuda a rede em background, com limites claros e segurança máxima.**

### 1.2 Dois papéis (podem coexistir no mesmo dispositivo)

| Papel | O que faz | O que ganha |
|-------|-----------|-------------|
| **Utilizador** | Chat, agentes, ficheiros (fases futuras) | IA free / crédito |
| **Contribuinte** | Empresta RAM/CPU em background | Crédito, early access, tier melhor mais cedo |

A mesma instalação pode ser só chat, só contribuinte, ou ambos.

---

## 2. App desktop / web (alvo)

### 2.1 Referências (não cópia — inspiração UX)

- **ChatGPT / Codex:** chat limpo, histórico, streaming
- **Diferencial YouAI:** transparência de rede, tier, contribuição, privacidade visível

### 2.2 Ecrãs principais

| Ecrã | Conteúdo |
|------|----------|
| **Chat** | Conversa, streaming, modelo/tier actual, modo (réplica/pipeline) |
| **Contribuir** | Toggle on/off, sliders CPU/RAM, horários, “não incomodar” |
| **Rede** | Tier activo, nós online (agregado), progresso para próximo tier |
| **Privacidade** | O que sai do dispositivo, E2E status, link SECURITY_MODEL |
| **Conta** | Opcional; chaves locais; apagar dados |
| **Crédito** | Saldo, histórico, bónus por contribuição (fase beta) |

### 2.3 Onboarding do contribuinte (automático mas honesto)

Fluxo alvo no primeiro arranque:

1. **Bem-vindo** — “IA by you, for you” + link documentação
2. **Como funciona** — diagrama: teu PC → activações opacas → nunca o teu prompt em claro no voluntário
3. **Limites** — sliders com defeitos seguros (30% CPU, 2 GB RAM tier1)
4. **Onde ficam os ficheiros** — só `~/.youai/`
5. **Pausa** — “Podes parar em 2 segundos”
6. **Opt-in** — botão explícito “Começar a contribuir” (não pré-marcado)
7. **Download modelo** — do registry, com barra de progresso + hash
8. **Pronto** — ícone na bandeja; corre em background

**Nunca** esconder que o PC está a contribuir.

### 2.4 Background sem atrapalhar

| Comportamento | UX |
|---------------|-----|
| Contribuição activa | Ícone bandeja verde; tooltip com CPU/RAM actual |
| Pausa automática (utilizador a trabalhar) | Ícone amarelo; notificação opcional |
| Pausa manual | Um clique; confirmação instantânea |
| Update de modelo | Notificar antes; janela de manutenção ou idle-only |

---

## 3. Criptografia e privacidade no app

| Dado | Onde fica | Utilizador vê |
|------|-----------|---------------|
| Chaves E2E | Keychain / secure storage local | “As tuas chaves nunca saem do dispositivo” |
| Histórico chat | Local por defeito; sync opt-in cifrado | Toggle + apagar |
| Prompt enviado | Cifrado antes de sair do app | Indicador “ligação segura” |
| Telemetria | Mínima; opt-in | Lista exacta na UI |

Ver implementação técnica em [SECURITY_MODEL.md](./SECURITY_MODEL.md).

---

## 4. Modelo, tiers e features (produto)

### 4.1 Regra de negócio

- **Tier baixo** no Mac Mini (tier1) — já em dogfood
- **Rede cresce** → coordinator activa tier superior via [Model Registry](./MODEL_TIERS.md)
- **Features novas** (upload, URL, busca) desbloqueiam por tier + infra gateway, não por “hack” no worker

### 4.2 O que mostrar quando o tier sobe

```
🎉 A rede YouAI atingiu Tier 2 (Glow)
   Novo modelo: SmolLM2-1.7B — respostas mais capazes
   Contribuintes: já em canary
   Todos os utilizadores: disponível em ~48h se a rede se mantiver estável
   [Saber mais]
```

### 4.3 Contribuintes primeiro

| Evento | Contribuinte | Só utilizador |
|--------|--------------|---------------|
| Novo tier | Canary imediato (se elegível) | Quando estável |
| Nova feature (ex: upload) | Beta se crédito/uptime ok | Rollout geral depois |
| Manutenção | Aviso prioritário | Aviso geral |

Se a rede tem 10k utilizadores e modelo tier5 estável, **todos** beneficiam — a diferença é só *quando* no rollout, não *se*.

---

## 5. Features futuras (roadmap de produto)

### 5.1 Upload para leitura

| Aspeto | Decisão |
|--------|---------|
| Onde processa | **Gateway YouAI** (sandbox), não no nó voluntário |
| O que o utilizador envia | Ficheiro cifrado ou chunks |
| O que o nó vê | Nada |
| Tier mínimo | tier4+ |

### 5.2 Análise de URL

| Aspeto | Decisão |
|--------|---------|
| Fetch HTTP | Gateway com allowlist, timeout, sem SSRF para rede interna |
| Conteúdo ao modelo | Resumo sanitizado |
| Tier mínimo | tier4–5 |

### 5.3 Função buscar

| Aspeto | Decisão |
|--------|---------|
| Motor de busca | API contratada / self-hosted no gateway |
| Nó voluntário | Não acede à internet arbitrária |
| Tier mínimo | tier5 |

### 5.4 Agentes (estilo Codex)

| Aspeto | Decisão |
|--------|---------|
| Execução de código | Sandbox no **dispositivo do utilizador** ou cloud YouAI — **nunca** no voluntário |
| Tools | Lista fechada assinada |

---

## 6. Crédito e fair use

| Acção | Crédito (exemplo futuro) |
|-------|--------------------------|
| Nó online 1 h (tier1) | +100 tokens equivalente |
| Nó online 1 h (tier4) | +500 |
| Chat consumido | −N por token |
| Só utilizador, rede grande | quota diária free baixa |
| Contribuinte activo | quota alta + early access |

Números são placeholders — ajustar em beta.

---

## 7. Transparência e documentação (regra clara)

### 7.1 Tudo documentado

Qualquer comportamento visível ao utilizador tem entrada em:

- `README.md` (índice)
- `docs/` (detalhe)
- Changelog de release
- UI “Ajuda” com links para GitHub

### 7.2 Issues abertas, PRs revistas

| Canal | Política |
|-------|----------|
| GitHub Issues | Abertas; templates para bug/feature/security |
| Pull Requests | **Aprovação humana obrigatória** |
| Security | SECURITY_MODEL checklist em PRs de worker/guard |
| Discussões | Roadmap público |

### 7.3 O utilizador pode auditar

- Código Apache 2.0
- Manifest de modelos com hashes públicos
- Builds reproduzíveis (alvo)
- Sem telemetria obscura

---

## 8. Arquitectura app ↔ rede (resumo)

```
┌─────────────────────────────────────────┐
│  YouAI App (Desktop / Web / Mobile)     │
│  · Chat UI · E2E · onboarding           │
│  · youai-node embutido (contribuinte)   │
└──────────────────┬──────────────────────┘
                   │ TLS (+ E2E)
                   ▼
┌─────────────────────────────────────────┐
│  Coordinator + Model Registry           │
│  · tier · routing · crédito · signing   │
└──────────────────┬──────────────────────┘
                   │ jobs opacos
         ┌─────────┴─────────┐
         ▼                   ▼
    Nó contrib. A        Nó contrib. B
    (só ~/.youai)        (só ~/.youai)
```

---

## 9. Estado actual vs visão

| Capacidade | Hoje | App alvo |
|------------|------|----------|
| Chat web mínimo | youai-web básico | App polido + streaming |
| Contribuir | CLI `youai-node` | Onboarding guiado + bandeja |
| E2E | Planeado | Obrigatório antes de beta pública |
| Model registry API | Doc + scripts | Automático no app |
| Upload/URL/busca | Não | tier4–5 + gateway |

---

## 10. Próximos passos de produto (ligados a engenharia)

1. **registry/manifest.json** tier1 publicado no repo
2. **UI de status** no web: tier, modo pipeline, nós online
3. **Onboarding** copy alinhado com SECURITY_MODEL
4. **Desktop shell** (Tauri/Electron) com node embutido — fase beta
5. **contributor_score** + early access
6. **Gateway** para features T3+ (separado do worker)

---

*YouAI — IA sua, feita por você. Transparente por defeito.*