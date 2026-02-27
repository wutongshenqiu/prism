# AI Proxy Gateway - Development Guide

## Project Overview

AI Proxy Gateway is a Rust/Axum multi-provider AI API gateway. It routes and translates requests across Claude (Anthropic), OpenAI, Gemini (Google AI), and OpenAI-compatible providers (DeepSeek, Groq, etc.). The project follows Spec-Driven Development (SDD) methodology.

## Key Paths

| Path | Purpose |
|------|---------|
| `crates/core/` | Foundation types, config, errors, provider traits, metrics, glob, proxy, cloaking, payload rules |
| `crates/core/src/types/` | Provider-specific request/response types (OpenAI, Claude, Gemini) |
| `crates/provider/` | Provider executors (Claude, OpenAI, Gemini, OpenAICompat), credential routing, SSE parsing |
| `crates/translator/` | Format translation between provider APIs |
| `crates/server/` | Axum router, handlers, middleware (auth, logging, request_context), dispatch |
| `src/` | Binary entry point |
| `docs/specs/` | SDD spec registry |
| `docs/reference/` | SSOT type definitions, API surface, architecture |
| `docs/playbooks/` | How-to guides (add provider, add translator, etc.) |

## Commands

### Cargo

```sh
cargo build --release     # Production build
cargo test --workspace    # Run all tests
cargo clippy --workspace -- -D warnings  # Lint
cargo fmt                 # Format code
cargo fmt --check         # Check formatting
cargo check --workspace   # Type-check without building
cargo run -- --config config.yaml  # Run locally
```

### Make Targets

```sh
make build   # cargo build --release
make dev     # cargo run with config.yaml
make test    # cargo test --workspace
make lint    # fmt --check + clippy
make fmt     # cargo fmt
make clean   # cargo clean
make check   # cargo check --workspace
```

### Docker

```sh
make docker-build          # Build Docker image locally
make docker-run            # Run container (mounts config.yaml)
make docker-stop           # Stop and remove container
make docker-logs           # Tail container logs
make docker-compose-up     # Build & start via docker compose
make docker-compose-down   # Stop docker compose services
```

### Security

```sh
make audit   # cargo audit — check for known vulnerabilities
```

## Rules

- **Lint before commit**: Run `make lint` and fix all warnings before committing.
- **Test before commit**: Run `make test` and ensure all tests pass before committing.
- **Never commit secrets**: Do not commit `config.yaml`, `.env`, API keys, or any credentials. Use `config.example.yaml` and `.env.example` as templates.
- **Keep the lock file**: Always commit `Cargo.lock` since this is a binary project.

## Code Style

- **Rust Edition 2024**: All crates use edition 2024.
- **Error handling**: Use `thiserror` for library error types, `anyhow` for application-level errors. Define domain-specific error enums in each crate.
- **Async traits**: Use `async-trait` for trait objects that require async methods.
- **Serialization**: Use `serde` with `serde_json` and `serde_yml` for all data serialization. Derive `Serialize`/`Deserialize` on public types.
- **Configuration**: Use `arc-swap` for hot-reloadable configuration.
- **Naming**: Follow standard Rust naming conventions -- `snake_case` for functions/variables, `PascalCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.

## Git Conventions

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` -- New feature or capability
- `fix:` -- Bug fix
- `docs:` -- Documentation only changes
- `refactor:` -- Code change that neither fixes a bug nor adds a feature
- `test:` -- Adding or correcting tests
- `chore:` -- Build process, CI, or auxiliary tool changes

Examples:
```
feat: add Gemini provider streaming support
fix: correct SSE parsing for multi-line data fields
docs: update API endpoint reference
refactor: extract credential routing into CredentialRouter
test: add integration tests for translator registry
chore: update dependencies to latest versions
```

### Branch Naming

- `feature/<description>` -- New features
- `fix/<description>` -- Bug fixes
- `docs/<description>` -- Documentation changes
- `refactor/<description>` -- Refactoring work

## SDD (Spec-Driven Development)

### Spec Registry

All specifications live in `docs/specs/` with `_index.md` as the registry. Each spec is a directory (`SPEC-NNN/`) containing `prd.md` and `technical-design.md`.

### Spec Organization

```
docs/specs/
├── _index.md          # Registry table of all specs
├── _templates/        # PRD, TD, and research templates
├── active/            # In-progress specs (SPEC-NNN/ directories)
└── completed/         # Completed specs (SPEC-NNN/ directories)
```

### Spec Lifecycle

| Status | Location | Meaning |
|--------|----------|---------|
| Draft | `active/SPEC-NNN/` | Spec is being written, not yet approved |
| Active | `active/SPEC-NNN/` | Spec is approved and implementation is in progress |
| Completed | `completed/SPEC-NNN/` | Implementation matches spec, verified by tests |
| Deprecated | `completed/SPEC-NNN/` | Spec is no longer relevant, superseded or removed |

### Feature Lifecycle

- **New feature** -- Create a spec directory first (`docs/specs/active/SPEC-NNN/`), add `prd.md` + `technical-design.md`, register in `_index.md`, then implement.
- **Modify feature** -- Update the corresponding spec before or alongside code changes.
- **Deprecate feature** -- Mark the spec as Deprecated in `_index.md` with a note explaining why and what replaces it.

## Slash Commands

Available commands (`.claude/commands/`):

| Command | Purpose | Example |
|---------|---------|---------|
| `/spec` | Spec 生命周期管理（创建/列表/状态/推进/创建 TD） | `/spec create "WebSocket 支持"` |
| `/ship` | 质量检查 + 提交 + 推送 | `/ship "feat: add WebSocket support"` |
| `/pr` | 创建 Pull Request（含验证和文档检查） | `/pr` |
| `/review` | Review Pull Request | `/review 42` |
| `/doc-audit` | 文档与代码一致性审查 | `/doc-audit types` |
| `/diagnose` | 问题诊断与修复 | `/diagnose "SSE streaming drops connection"` |
| `/lint` | 代码检查/修复 | `/lint fix` |
| `/test` | 运行测试 | `/test cloak` |

## Quality Gates

Pre-commit hooks (`.claude/settings.json`) automatically run `make lint && make test` before every `git commit` and `git push`.
