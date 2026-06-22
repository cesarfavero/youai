## Summary

<!-- What does this PR do? Link issue: Fixes #123 -->

## Component

- [ ] youai-guard
- [ ] youai-node
- [ ] youai-worker
- [ ] youai-coordinator
- [ ] youai-web
- [ ] docs / CI

## Checklist

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] Docs updated if behavior changed
- [ ] No secrets, model files (`.gguf`), or `.youai/` data committed

## Security note

If this touches guard, sandbox, or network auth, describe how limits/trust boundaries are preserved.