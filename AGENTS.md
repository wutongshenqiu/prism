# Prism - Agent Context

Universal context for AI agents working on this project.

## Project

Prism is a Rust/Axum multi-provider AI API gateway. It routes and translates requests across Claude (Anthropic), OpenAI, Gemini (Google AI), and OpenAI-compatible providers (DeepSeek, Groq, etc.). The project follows Spec-Driven Development (SDD) methodology.

## Key Paths

| Path | Purpose |
|------|---------|
| `crates/core/` | Foundation types, config, errors, provider traits, metrics, rate limiting, cost tracking, glob, proxy, cloaking, payload rules, lifecycle |
| `crates/core/src/types/` | Provider-specific request/response types (OpenAI, Claude, Gemini) |
| `crates/provider/` | Provider executors (Claude, OpenAI, Gemini, OpenAICompat), credential routing, SSE parsing |
| `crates/translator/` | Format translation between provider APIs |
| `crates/server/` | Axum router, handlers, middleware (auth, logging, request_context, dashboard_auth, rate_limit), dispatch |
| `crates/server/src/handler/dashboard/` | Dashboard API handlers (auth, providers, auth_keys, routing, logs, config_ops, system, websocket) |
| `src/` | Binary entry point (subcommand CLI, Application struct, daemon support) |
| `web/` | React + TypeScript + Vite dashboard frontend (SPA) |
| `docs/specs/` | SDD spec registry |
| `docs/reference/` | SSOT type definitions, API surface, architecture |
| `docs/playbooks/` | How-to guides (add provider, add translator, etc.) |
| `AGENTS.md` | Universal agent context (this file) |

## Architecture

```
Client
  |
  v
Server (Axum handlers + middleware)
  |  - Auth middleware validates credentials
  |  - Request context middleware
  |  - Logging middleware
  v
Dispatch (routing + retry)
  |  - Selects provider based on model/config
  |  - Handles retry logic
  v
Translator (format conversion)
  |  - Converts between OpenAI, Claude, Gemini formats
  |  - Bidirectional translation
  v
Provider (executor)
  |  - Executes HTTP request to upstream
  |  - Parses SSE streams
  v
Upstream API (Claude, OpenAI, Gemini, etc.)
```

## Crate Responsibilities

### `crates/core/`
Foundation types shared across all crates:
- `Config` -- YAML configuration with hot-reload via `arc-swap` and `ConfigWatcher` (notify + SHA256 dedup)
- `DaemonConfig` -- Daemon settings (PID file path, shutdown timeout)
- `RateLimitConfig` -- Per-key and global RPM limits
- `ProxyError` -- Unified error type using `thiserror`, with HTTP status code mapping (includes `RateLimited` variant with `Retry-After`)
- `AuthRecord` -- Provider credential record (API key, base URL, proxy, models, circuit breaker state, cloak config, weight, region)
- `Format` enum -- Identifies API format: `OpenAI`, `Claude`, `Gemini`, `OpenAICompat`
- `WireApi` enum -- OpenAI-compatible wire protocol: `Chat` (default) or `Responses`
- `CloakConfig` -- Claude request cloaking (system prompt injection, user_id generation, sensitive word obfuscation)
- `PayloadConfig` -- Request payload manipulation (default/override/filter rules with model glob matching)
- `ProviderExecutor` trait -- Async trait for provider execution (execute, execute_stream, supported_models)
- `types/` -- Provider-specific request/response types (`openai.rs`, `claude.rs`, `gemini.rs`)
- `lifecycle/` -- Application lifecycle management:
  - `Lifecycle` trait -- Readiness notification (ForegroundLifecycle, SystemdLifecycle)
  - `signal` -- SignalHandler for SIGTERM/SIGINT shutdown and SIGHUP config reload
  - `pid_file` -- RAII PidFile with flock advisory locking
  - `daemon` -- Process daemonization via `fork::daemon()`
  - `logging` -- Tracing initialization with optional daily file rotation
  - `notify` -- sd-notify wrappers for systemd integration
- `glob` -- Wildcard pattern matching for model names
- `proxy` -- HTTP proxy client builder (http/https/socks5)
- `context` -- `RequestContext` (request ID, start time, client IP, api_key_id, tenant_id, auth_key, client_region)
- `metrics` -- Atomic counters for requests, errors, latency, token usage, cost (micro-USD)
- `rate_limit` -- `RateLimiter` with sliding window algorithm, per-key and global RPM tracking
- `cost` -- `CostCalculator` with built-in model price table (30+ models) and user overrides
- `audit` -- `AuditBackend` trait, `FileAuditBackend` (append-only JSONL), `NoopAuditBackend`
- `cache` -- `ResponseCacheBackend` trait, `MokaCache` in-memory cache, `CacheKey` builder
- `circuit_breaker` -- `CircuitBreakerPolicy` trait, `ThreeStateCircuitBreaker` (Closed→Open→HalfOpen), `NoopCircuitBreaker`
- `prometheus` -- Prometheus text format renderer for metrics, cache stats, circuit breaker states
- `secret` -- Secret resolver (`env://`, `file://` prefixes for sensitive config values)
- `request_log` -- `RequestLogStore` ring buffer for recent request/response log entries

### `crates/provider/`
Provider-specific execution logic:
- `ClaudeExecutor` -- Anthropic Claude API executor
- `OpenAICompatExecutor` -- Generic executor for OpenAI-format APIs (also used for OpenAI itself via `openai::new_openai_executor()`)
- `GeminiExecutor` -- Google Gemini API executor
- `ExecutorRegistry` -- Registry of all executor instances
- `CredentialRouter` -- Credential selection with round-robin/fill-first/latency-aware/geo-aware routing, circuit breaker tracking, and latency EWMA
- `sse` -- SSE (Server-Sent Events) stream parsing (`SseEvent`, `parse_sse_stream`)

### `crates/translator/`
Format translation between provider APIs:
- `TranslatorRegistry` -- Registry of request and response translators
- OpenAI → Claude request translation + Claude → OpenAI response translation
- OpenAI → Gemini request translation + Gemini → OpenAI response translation
- Handles both streaming (via `TranslateState`) and non-streaming response translation

### `crates/server/`
HTTP server and request dispatch:
- Axum router setup with middleware stack
- Handlers: `chat_completions`, `messages`, `responses`, `models`, `admin`, `health`
- Auth: `auth.rs` -- Bearer token / x-api-key validation (top-level module)
- Middleware: `request_logging`, `request_context`, `dashboard_auth` (JWT), `rate_limit` (in `middleware/` directory)
- `dispatch/` -- Core routing logic (split into `mod.rs`, `helpers.rs`, `streaming.rs`, `retry.rs`): credential rotation, translation, cloaking, payload rules, model fallback (`models` array), debug mode (`x-debug` header), cost calculation, token usage extraction, and keepalive body builder
- `streaming` -- SSE response builder
- `handler/dashboard/` -- Dashboard API handlers:
  - `auth` -- Login (bcrypt verify + JWT), token refresh
  - `providers` -- Provider CRUD with API key masking and atomic config write-back
  - `auth_keys` -- Auth key management (create `sk-proxy-` prefix, list masked, delete)
  - `routing` -- Routing strategy get/update
  - `logs` -- Request log query and stats
  - `config_ops` -- Config validation (dry-run), hot-reload, get current sanitized config
  - `system` -- System health (uptime, version), application log viewer
  - `websocket` -- WebSocket at `/ws/dashboard` with metrics and request_log subscription channels
  - `tenant` -- Tenant listing and per-tenant metrics

### `web/` (Dashboard Frontend)
React 19 + TypeScript + Vite SPA:
- `services/api.ts` -- Axios client with JWT interceptor and auto-refresh
- `services/websocket.ts` -- Auto-reconnecting WebSocket manager
- `stores/` -- Zustand stores (auth, metrics, logs)
- `pages/` -- Overview, Metrics, RequestLogs, Providers, AuthKeys, Routing, System, Logs

### `src/` (binary entry point)
Subcommand architecture with daemon support:
- `cli.rs` -- CLI parsing: subcommands `run`, `stop`, `status`, `reload` with `RunArgs` and `PidArgs`
- `app.rs` -- `Application` struct: encapsulates config loading, provider/router/translator assembly, and HTTP/TLS serving
- `main.rs` -- Entry point: subcommand dispatch, daemonization (before tokio), logging init, runtime creation

## API Endpoints

### Public (no auth)
- `GET /health` -- Health check
- `GET /metrics` -- Metrics (custom JSON format)
- `GET /metrics/prometheus` -- Metrics (Prometheus text format)

### Admin (no auth required)
- `GET /admin/config` -- Current configuration
- `GET /admin/metrics` -- Detailed metrics
- `GET /admin/models` -- Available models

### Authenticated
- `GET /v1/models` -- List available models
- `POST /v1/chat/completions` -- OpenAI Chat Completions format
- `POST /v1/messages` -- Anthropic Messages format
- `POST /v1/responses` -- OpenAI Responses API format

### Dashboard (no auth)
- `POST /api/dashboard/auth/login` -- Dashboard login (bcrypt + JWT)

### Dashboard (JWT auth required)
- `POST /api/dashboard/auth/refresh` -- Refresh JWT token
- `GET/POST /api/dashboard/providers` -- List / create providers
- `GET/PATCH/DELETE /api/dashboard/providers/{id}` -- Get / update / delete provider
- `GET/POST /api/dashboard/auth-keys` -- List / create auth keys
- `PATCH/DELETE /api/dashboard/auth-keys/{id}` -- Update / delete auth key
- `GET/PATCH /api/dashboard/routing` -- Get / update routing config
- `GET /api/dashboard/logs` -- Query request logs
- `GET /api/dashboard/logs/stats` -- Request log statistics
- `POST /api/dashboard/config/validate` -- Validate config (dry-run)
- `POST /api/dashboard/config/reload` -- Hot-reload config
- `GET /api/dashboard/config/current` -- Get current sanitized config
- `GET /api/dashboard/system/health` -- System health details
- `GET /api/dashboard/system/logs` -- Application log viewer
- `GET /api/dashboard/tenants` -- List tenants
- `GET /api/dashboard/tenants/{id}/metrics` -- Tenant metrics

### WebSocket
- `GET /ws/dashboard` -- Real-time metrics and request log push (JWT via query param)

## Provider Matrix

| Provider | Format | Default Base URL | Notes |
|----------|--------|------------------|-------|
| Claude | `Format::Claude` | `https://api.anthropic.com` | Auth via `x-api-key` header |
| OpenAI | `Format::OpenAI` | `https://api.openai.com` | Uses `OpenAICompatExecutor` with OpenAI defaults |
| Gemini | `Format::Gemini` | `https://generativelanguage.googleapis.com` | Auth via `x-goog-api-key` header |
| OpenAI-compatible | `Format::OpenAICompat` | (must be configured) | For DeepSeek, Groq, etc. Supports `wire-api: chat\|responses` |

Models are not hardcoded — any model name can be routed if configured in `config.yaml`.

## Commands

### Cargo

```sh
cargo build --release     # Production build
cargo test --workspace    # Run all tests
cargo clippy --workspace --tests -- -D warnings  # Lint (includes test code)
cargo fmt                 # Format code
cargo fmt --check         # Check formatting
cargo check --workspace   # Type-check without building
cargo run -- run --config config.yaml         # Run locally (foreground)
cargo run -- run --daemon --config config.yaml # Run as daemon
cargo run -- status                           # Check daemon status
cargo run -- reload                           # Reload config (SIGHUP)
cargo run -- stop                             # Graceful shutdown
```

### Make Targets

```sh
make build              # cargo build --release
make dev                # cargo run -- run --config config.yaml
make test               # cargo test --workspace
make lint               # fmt --check + clippy
make fmt                # cargo fmt
make clean              # cargo clean
make check              # cargo check --workspace
make web-install        # npm install (web/)
make web-dev            # npm run dev (web/)
make web-build          # npm run build (web/)
make test-e2e-docker    # Run Docker E2E CLI tool tests (requires E2E_BAILIAN_API_KEY)
```

### E2E Docker Tests

Docker-based end-to-end tests for coding agent CLI tools. Tests live in `tests/e2e-docker/cases/<name>/test.sh`.

```sh
make test-e2e-docker                              # Run quick tests (default)
TEST_LEVEL=full make test-e2e-docker              # Run all tests (quick + full)
TEST_FILTER=cline TEST_LEVEL=full make test-e2e-docker  # Run only cline test
```

Each `test.sh` declares a level via `# @level: quick|full` metadata on line 2:
- `quick` -- runs on every push to main (smoke tests, minimal API cost)
- `full` -- runs on manual dispatch and weekly schedule (all tools, all models)

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

## Slash Commands

Agent commands defined in `.claude/commands/`. Portable to OpenCode (`.opencode/commands/`) and Codex (`.agents/skills/`) via `/sync-commands`.

| Command | Description | Example |
|---------|-------------|---------|
| `/ship` | End-to-end commit pipeline (lint, test, commit, push, PR, CI) | `/ship --merge "feat: xxx"` |
| `/audit` | Full codebase review + batch fix | `/audit --fix security` |
| `/lint` | Run formatting + clippy checks | `/lint fix` |
| `/test` | Run tests (unit, e2e, docker) | `/test unit` |
| `/spec` | Manage spec lifecycle (create/list/advance/td) | `/spec create "Title"` |
| `/implement` | Implement a spec end-to-end | `/implement SPEC-008` |
| `/issues` | Generate GitHub issues from spec | `/issues SPEC-009` |
| `/review` | Review a pull request | `/review 114` |
| `/diagnose` | Diagnose and fix a project problem | `/diagnose "SSE timeout"` |
| `/deps` | Dependency management (merge/fix/update) | `/deps merge` |
| `/merge` | Batch merge multiple PRs | `/merge 81 85` |
| `/doc-audit` | Audit docs vs code consistency | `/doc-audit full --fix` |
| `/retro` | Retrospective: improve commands/workflow | `/retro 3` |
| `/sync-commands` | Sync command definitions across agent tools | `/sync-commands` |

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

### When a Spec Is Required

**Needs a spec:**
- New features that affect API surface or user-facing behavior
- Architecture changes (new crate, major restructure)
- New provider or translator implementation

**Does NOT need a spec:**
- CI/CD pipeline changes
- Dependency upgrades
- Documentation-only updates
- Bug fixes
- Small refactoring (no API/behavior change)
- Infrastructure and tooling (Makefile, Docker, GitHub Actions, hooks)

### Feature Lifecycle

- **New feature** -- Create a spec directory first (`docs/specs/active/SPEC-NNN/`), add `prd.md` + `technical-design.md`, register in `_index.md`, then implement.
- **Modify feature** -- Update the corresponding spec before or alongside code changes.
- **Deprecate feature** -- Mark the spec as Deprecated in `_index.md` with a note explaining why and what replaces it.

## Quality Gates

Pre-commit hook (`.claude/settings.json`) automatically runs `make fmt && make lint && make test` before every `git commit`. Push does not trigger hooks -- quality is guaranteed at commit time. When `web/` files are staged, the hook additionally runs `npx tsc --noEmit` to type-check the frontend.

## CI/CD

### Workflows

| Workflow | Trigger | Jobs |
|----------|---------|------|
| **CI** (`ci.yml`) | Push to `main`, PR to `main` | Format check, Clippy lint, Test |
| **CD** (`cd.yml`) | Push to `main`, tags `v*` | CI (reusable) → Build & push Docker image to GHCR |
| **Security** (`security.yml`) | Push to `main`, PR to `main`, weekly cron | `cargo audit`, Trivy image scan |
| **E2E** (`e2e.yml`) | Push to `main`, manual dispatch, weekly cron | E2E cargo tests, Docker CLI tool tests (quick on push, full on dispatch/schedule) |
| **Claude Code** (`claude.yml`) | `@claude` mention in issues/PRs | Automated agent response |

### Dependabot

Configured in `.github/dependabot.yml`:
- **Cargo dependencies**: Weekly updates, grouped as `rust-deps`
- **GitHub Actions**: Weekly updates

## E2E Docker Test Framework

Docker-based end-to-end tests for coding agent CLI tools (`tests/e2e-docker/`).

Convention-based discovery: `cases/<name>/test.sh` = auto-discovered test case. Each test declares `# @level: quick|full` metadata.

| Case | Level | Protocol | Tool |
|------|-------|----------|------|
| `opencode` | quick | OpenAI `/v1/chat/completions` | opencode-ai |
| `opencode-full` | full | OpenAI `/v1/chat/completions` | opencode-ai (7 models) |
| `cline` | full | OpenAI `/v1/chat/completions` | Cline CLI |
| `aider` | full | OpenAI `/v1/chat/completions` | Aider |
| `claude-code` | full | Anthropic `/v1/messages` | Claude Code CLI |

See `docs/playbooks/add-e2e-test.md` for adding new test cases.

## Key Dependencies

- `axum` -- HTTP framework (with `ws` feature for WebSocket)
- `tokio` -- Async runtime
- `serde` / `serde_json` / `serde_yml` -- Serialization
- `thiserror` / `anyhow` -- Error handling
- `async-trait` -- Async trait support
- `arc-swap` -- Hot-reloadable configuration
- `reqwest` -- HTTP client for upstream calls
- `jsonwebtoken` -- Dashboard JWT authentication
- `bcrypt` -- Dashboard password hashing
- `fork` -- Process daemonization (unix)
- `sd-notify` -- systemd readiness notification
- `tracing-appender` -- File-based log rotation
