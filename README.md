# YouAI

**AI by you, for you.**

Rede global open source onde cada PC, celular e VPS contribui com uma fração do hardware para rodar modelos frontier gratuitos — GLM-5.2, Nex-N2-Pro e mais.

## Documentação

- [**Próximos passos**](docs/NEXT_STEPS.md) — o que fazer agora, em ordem
- [**Pipeline distribuído**](docs/PIPELINE.md) — réplica vs pipeline, RPC v1, teste 2 máquinas
- [MVP & Visão do Produto](docs/MVP.md) — arquitetura, roadmap, escopo
- [Arquitetura](docs/ARCHITECTURE.md) — camadas e componentes
- [Segurança](docs/SECURITY.md) — política de disclosure e limites do nó
- [Contribuindo](docs/CONTRIBUTING.md) — como contribuir

## Status

✅ **Dogfood local** — guard + worker + node CLI + coordinator + chat mínimo  
✅ **Cluster 2 máquinas** — Mac + Colima VM (réplica round-robin)  
✅ **Pipeline v1** — split real de tensores via llama.cpp RPC (`docs/PIPELINE.md`)  
🔜 **Pipeline v2** — GGUF partido por camadas (`llama-gguf-split`)

**Repo:** [github.com/cesarfavero/youai](https://github.com/cesarfavero/youai) · público · issues e PRs bem-vindos

## Começando sozinho (ordem certa)

Sem equipe, o caminho é **dogfood em 1 máquina** — não pule etapas:

1. **Guard** — provar que limites RAM/CPU funcionam (`youai-guard`)
2. **Worker** — llama.cpp local, sem rede (`youai-worker` + benchmark)
3. **Node CLI** — `start` / `pause` / `status` integrando os dois
4. **Coordinator** — só depois que 1 máquina está estável
5. **Web** — por último

Cada passo gera um commit pequeno, CI verde, e algo demonstrável. Isso atrai os primeiros contributors — projeto vivo > visão no papel.

Ver [NEXT_STEPS.md](docs/NEXT_STEPS.md) passo a passo.

## Estrutura do monorepo

```
youai/
├── youai-guard/         # limites RAM/CPU/GPU · watchdog
├── youai-node/          # CLI · config · start/pause/status
├── youai-worker/        # llama.cpp wrapper
├── youai-coordinator/   # registro de nós · roteamento · crédito
├── youai-web/           # chat mínimo (fase 9)
├── docs/
└── scripts/
```

## Desenvolvimento

**Pré-requisitos:** Rust stable (1.75+), Node 20+ (para web, depois)

```bash
# Clonar e compilar
git clone https://github.com/cesarfavero/youai.git
cd youai
cargo build --workspace

# Testes e lint
cargo test --workspace
cargo fmt --all
cargo clippy --workspace -- -D warnings

# Binários (scaffold — ainda sem lógica completa)
cargo run -p youai-guard -- --help
cargo run -p youai-node -- status
cargo run -p youai-coordinator -- --port 8080
```

Scripts utilitários:

```bash
chmod +x scripts/*.sh
./scripts/setup-llama.sh          # llama.cpp + rpc-server
./scripts/download-model.sh       # SmolLM2-360M (dogfood)
./scripts/benchmark-model.sh

# Cluster 2 máquinas (Mac + Colima)
./scripts/setup-pipeline-mac.sh
./scripts/ubuntu-test-vm.sh start-node
./scripts/test-shard-pipeline.sh
```

Ver [docs/PIPELINE.md](docs/PIPELINE.md) para detalhes do pipeline v1.

## Licença

[Apache License 2.0](LICENSE)