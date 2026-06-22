# Contributing to YouAI

Thank you for your interest in contributing to **YouAI** — a global open-source network where everyone contributes hardware and uses AI for free.

## Code of Conduct

Be respectful, inclusive, and constructive. We follow the spirit of the [Contributor Covenant](https://www.contributor-covenant.org/):

- **Be welcoming** — newcomers and non-native speakers are valued
- **Be respectful** — disagree on ideas, not people
- **Be constructive** — explain the "why" in reviews and issues
- **No harassment** — zero tolerance for discrimination, threats, or personal attacks

Report conduct issues to the maintainers via GitHub or security@youai.network.

## How to Contribute

### 1. Find something to work on

- Check [GitHub Issues](https://github.com/cesarfavero/youai/issues) for `good first issue` or `help wanted`
- Read [MVP.md](./MVP.md) and [NEXT_STEPS.md](./NEXT_STEPS.md) for the current phase
- Ask in GitHub Discussions before starting large changes

### 2. Set up your environment

**Prerequisites:**

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | stable (1.75+) | governor, node, coordinator |
| Node.js | 20+ | youai-web (later phases) |
| CMake | 3.20+ | llama.cpp builds |
| CUDA | optional | GPU inference |

```bash
git clone https://github.com/cesarfavero/youai.git
cd youai
cargo build --workspace
cargo test --workspace
```

### 3. Branch and commit

```bash
git checkout -b feat/short-description
# ... make changes ...
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
git commit -m "feat(governor): add cgroup v2 memory cap"
```

**Commit message format** (Conventional Commits):

```
<type>(<scope>): <description>

[optional body]
```

| Type | Use for |
|------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code change without feature/fix |
| `test` | Tests only |
| `chore` | Build, CI, tooling |

**Scopes:** `governor`, `node`, `worker`, `coordinator`, `web`, `ci`, `docs`

### 4. Open a Pull Request

- Fill in the PR template (if available)
- Link related issues (`Fixes #123`)
- Keep PRs focused — one concern per PR
- Ensure CI passes

## Project Structure

```
youai/
├── youai-governor/      # Resource limits · sandbox · watchdog
├── youai-node/          # CLI · install · config · start/pause
├── youai-worker/        # llama.cpp wrapper · inference
├── youai-coordinator/   # Routing · credit · node registry
├── youai-web/           # Chat UI · credit dashboard
├── docs/                # Architecture · security · guides
└── scripts/             # install.sh · benchmark-model.sh
```

**Implementation order matters.** See [NEXT_STEPS.md](./NEXT_STEPS.md):

1. Governor (resource limits) — **first**
2. Worker (local inference)
3. Node CLI (integrate governor + worker)
4. Coordinator (network)
5. Web (chat + credit)

Do not skip ahead without maintainer agreement.

## Coding Standards

### Rust

- `cargo fmt` and `cargo clippy` must pass with no warnings
- Prefer `Result` over panics in library code
- Document public APIs with `///` doc comments
- Platform-specific code behind `#[cfg(target_os = "...")]`

### General

- Security-sensitive paths require tests
- No secrets in code or commits
- User-configurable limits must never be bypassed in code

## Testing

```bash
# All workspace tests
cargo test --workspace

# Single crate
cargo test -p youai-governor

# With output
cargo test --workspace -- --nocapture
```

Integration tests for governor **must** verify that resource limits are enforced (see NEXT_STEPS.md, Passo 3).

## Documentation

- Update `docs/` when changing architecture or security behavior
- Add inline comments only where logic is non-obvious
- Keep README commands in sync with CLI changes

## Security

- Read [SECURITY.md](./SECURITY.md) before touching governor, sandbox, or network code
- Never commit API keys, tokens, or model files (`.gguf`)
- Report vulnerabilities privately — do not open public issues

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](../LICENSE).

---

*YouAI — AI by you, for you.*