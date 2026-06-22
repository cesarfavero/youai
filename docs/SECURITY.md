# Security Policy

> YouAI — AI by you, for you.  
> Last updated: June 2026

YouAI is a distributed, open-source network where contributors share idle hardware to run AI models. Security is a core design principle — not an afterthought.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x (pre-MVP) | ✅ Active development |
| < 0.1.0 | ❌ Not supported |

Security fixes are backported to the latest minor release on the `main` branch.

## Reporting a Vulnerability

We follow **responsible disclosure**. Please do **not** open public GitHub issues for security vulnerabilities.

### How to report

1. Email: **security@youai.network** (placeholder — update when domain is live)
2. Or use [GitHub Security Advisories](https://github.com/cesarfavero/youai/security/advisories/new) (private report)

### What to include

- Description of the vulnerability and its impact
- Steps to reproduce (proof-of-concept if available)
- Affected component (`youai-governor`, `youai-node`, `youai-worker`, `youai-coordinator`, `youai-web`)
- Your environment (OS, version, hardware)
- Your contact info for follow-up (optional: PGP key)

### What to expect

| Timeline | Action |
|----------|--------|
| **48 hours** | Acknowledgment of your report |
| **7 days** | Initial assessment and severity classification |
| **30 days** | Target fix or mitigation plan (may extend for complex issues) |
| **After fix** | Coordinated disclosure with credit to reporter (if desired) |

We will not take legal action against researchers who follow this policy in good faith.

## Security Model

### What a YouAI node **cannot** do

These constraints are enforced by design and must hold in every release:

| Constraint | Enforcement |
|------------|-------------|
| Execute arbitrary code from the network | Jobs are signed; no remote shell execution |
| Read files outside `~/.youai/` | Sandbox + filesystem isolation |
| Open arbitrary inbound ports | Outbound-only connections to coordinator |
| Install unsigned binaries | Verified releases with published build hashes |
| Exceed user-configured resource limits | Independent `youai-governor` with hard caps |

### What a YouAI node **can** do (with explicit opt-in)

- Run inference workloads inside a cgroup / sandbox
- Connect outbound to the coordinator over TLS 1.3
- Store model shards and cache in `~/.youai/`
- Report anonymized metrics (uptime, resource usage) to the coordinator

### Prompt privacy

User prompts are **not** sent in plaintext to arbitrary contributor nodes. The coordinator aggregates requests and distributes opaque intermediate activations or signed job payloads. Full prompt exposure on community nodes is a known limitation of early MVP versions and is documented in release notes.

## Threat Model (MVP)

| Threat | Mitigation |
|--------|------------|
| Malicious node returns bad inference | Replica voting + consistency checks (phase 2) |
| Resource limit bypass | Independent governor process; cgroup v2 hard caps; watchdog SIGKILL |
| Model weight theft | Encrypted shards (future); TEE (future) |
| Coordinator compromise | TLS, per-node auth tokens, minimal data retention |
| Supply chain attack | Reproducible builds; signed releases; public CI logs |

## Resource Governor Guarantees

The `youai-governor` is a **separate process** from the inference worker:

- Monitors CPU, RAM, GPU (when available), and temperature every 500ms
- Kills the worker immediately if limits are exceeded
- Circuit breaker: repeated violations → automatic pause
- User can pause in < 2 seconds via `youai-node pause`

If you observe a governor failure (resource limit exceeded without worker termination), treat it as a **critical** security issue and report immediately.

## Safe Defaults

| Setting | Default | Rationale |
|---------|---------|-----------|
| CPU max | 30% | Leaves headroom for user workloads |
| RAM max | 8 GB | Conservative for typical home PCs |
| GPU max | 50% | Half GPU for gaming/editing coexistence |
| Auto-pause on heavy app | on | Protects interactive use |
| Mobile contribution | off (MVP) | Mobile not in MVP scope |

## Auditing

- All source code is Apache 2.0 — public audit welcome
- Build artifacts publish SHA-256 hashes on each release
- CI runs on every PR: `cargo test`, `cargo clippy`, dependency audit

## Security Contacts

| Role | Contact |
|------|---------|
| Security reports | security@youai.network |
| General questions | GitHub Discussions |
| Code of conduct | See [CONTRIBUTING.md](./CONTRIBUTING.md) |

---

*YouAI — AI by you, for you.*