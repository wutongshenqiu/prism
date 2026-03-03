# Playbook: Rename Project

Step-by-step guide for renaming the project (crate names, binary, env vars, Docker, docs).

## Prerequisites

- All tests passing on current main
- No open PRs that would conflict

## Steps

### 1. Cargo.toml files (5 files)

Update package names and dependency references:
- `Cargo.toml` (root) — package name, all workspace dep names
- `crates/core/Cargo.toml` — package name
- `crates/provider/Cargo.toml` — package name + deps
- `crates/translator/Cargo.toml` — package name + deps
- `crates/server/Cargo.toml` — package name + deps

### 2. Rust source code (~40 files)

Bulk find-replace across all `.rs` files:
- `old_core` → `new_core` (use statements)
- `old_provider` → `new_provider`
- `old_translator` → `new_translator`
- `old_server` → `new_server`

Also update:
- `crates/core/src/prometheus.rs` — metric name prefix strings
- `crates/core/src/proxy.rs` — user-agent string
- `crates/core/src/config.rs` — PID file default, test assertions
- `crates/core/src/lifecycle/logging.rs` — log filename
- `src/main.rs` — status print messages

### 3. CLI & env vars

- `src/cli.rs` — `#[command(name = "...")]`, env var prefixes, PID file default

### 4. Docker & deployment

- `Dockerfile` — binary name, user/group, config paths
- `docker-compose.yml` — service name, image tag, volume paths
- `docker-compose.e2e.yml` — volume paths
- `Makefile` — docker image/container name, volume paths
- `dist/*.service` — rename file, update all internal references

### 5. Config files

- `config.example.yaml` — header comment, pid-file path
- `config.test.yaml` — pid-file path
- `.env.example` — header comment, env var names, RUST_LOG filter

### 6. CI/CD workflows

- `.github/workflows/security.yml` — docker image tag

### 7. Web frontend

- `web/package.json` — name field
- Regenerate `web/package-lock.json` via `cd web && npm install`

### 8. Documentation

- `AGENTS.md` (= `CLAUDE.md`) — project name, crate names, command examples, env vars
- `README.md` — project name, binary name, all examples
- `LICENSE` — copyright holder
- `docs/reference/architecture.md` — crate names, binary name
- `docs/reference/types/*.md` — source file citations
- `docs/playbooks/*.md` — project name, import examples
- `docs/specs/completed/*/technical-design.md` — project references

### 9. Agent/command config files

- `.claude/`, `.agents/`, `.opencode/` files that reference old name

### 10. Cargo.lock regeneration

```sh
rm Cargo.lock
cargo generate-lockfile
```

## Verification Checklist

1. `cargo build --workspace` — compile succeeds
2. `make lint` — fmt + clippy pass
3. `make test` — all tests pass
4. `grep -r "old_name" --include="*.rs" --include="*.toml" --include="*.yml" --include="*.yaml" --include="*.md" --include="*.json" .` — zero results (excluding .git, target, node_modules)
5. Docker build: `docker build -t new_name:local .` — succeeds

## Post-merge

- GitHub repo rename: Settings → General → Repository name
- Update local remote: `git remote set-url origin <new-url>`
- `gh repo rename <new-name> --yes` can also be used from CLI
