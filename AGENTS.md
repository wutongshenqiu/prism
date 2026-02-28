# AI Proxy Gateway - Agent Context

Universal context for AI agents working on this project.

## Project

AI Proxy Gateway is a Rust/Axum multi-provider AI API gateway that routes requests to Claude, OpenAI, Gemini, and OpenAI-compatible providers. It translates between provider-specific API formats, handles authentication, and supports both streaming (SSE) and non-streaming responses.

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
- `ProxyError` -- Unified error type using `thiserror`, with HTTP status code mapping
- `AuthRecord` -- Provider credential record (API key, base URL, proxy, models, cooldown state, cloak config)
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
- `context` -- `RequestContext` (request ID, start time, client IP)
- `metrics` -- Atomic counters for requests, errors, latency, token usage

### `crates/provider/`
Provider-specific execution logic:
- `ClaudeExecutor` -- Anthropic Claude API executor
- `OpenAICompatExecutor` -- Generic executor for OpenAI-format APIs (also used for OpenAI itself via `openai::new_openai_executor()`)
- `GeminiExecutor` -- Google Gemini API executor
- `ExecutorRegistry` -- Registry of all executor instances
- `CredentialRouter` -- Credential selection with round-robin/fill-first routing and cooldown tracking
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
- Middleware: `request_logging`, `request_context`, `dashboard_auth` (JWT) (in `middleware/` directory)
- `dispatch` -- Core routing logic with retry, credential rotation, translation, cloaking, and payload rules
- `streaming` -- SSE response builder with keepalive
- `handler/dashboard/` -- Dashboard API handlers:
  - `auth` -- Login (bcrypt verify + JWT), token refresh
  - `providers` -- Provider CRUD with API key masking and atomic config write-back
  - `auth_keys` -- Auth key management (create `sk-proxy-` prefix, list masked, delete)
  - `routing` -- Routing strategy get/update
  - `logs` -- Request log query and stats
  - `config_ops` -- Config validation (dry-run), hot-reload, get current sanitized config
  - `system` -- System health (uptime, version), application log viewer
  - `websocket` -- WebSocket at `/ws/dashboard` with metrics and request_log subscription channels

### `web/` (Dashboard Frontend)
React 19 + TypeScript + Vite SPA:
- `services/api.ts` -- Axios client with JWT interceptor and auto-refresh
- `services/websocket.ts` -- Auto-reconnecting WebSocket manager
- `stores/` -- Zustand stores (auth, metrics, logs)
- `pages/` -- Overview, Metrics, RequestLogs, Providers, AuthKeys, Routing, System, Logs

## API Endpoints

### Public (no auth)
- `GET /health` -- Health check
- `GET /metrics` -- Metrics (custom JSON format)

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
- `DELETE /api/dashboard/auth-keys/{id}` -- Delete auth key
- `GET/PATCH /api/dashboard/routing` -- Get / update routing config
- `GET /api/dashboard/logs` -- Query request logs
- `GET /api/dashboard/logs/stats` -- Request log statistics
- `POST /api/dashboard/config/validate` -- Validate config (dry-run)
- `POST /api/dashboard/config/reload` -- Hot-reload config
- `GET /api/dashboard/config/current` -- Get current sanitized config
- `GET /api/dashboard/system/health` -- System health details
- `GET /api/dashboard/system/logs` -- Application log viewer

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

### `src/` (binary entry point)
Subcommand architecture with daemon support:
- `cli.rs` -- CLI parsing: subcommands `run`, `stop`, `status`, `reload` with `RunArgs` and `PidArgs`
- `app.rs` -- `Application` struct: encapsulates config loading, provider/router/translator assembly, and HTTP/TLS serving
- `main.rs` -- Entry point: subcommand dispatch, daemonization (before tokio), logging init, runtime creation

## Quick Start

```sh
# Build
cargo build

# Create config from example
cp config.example.yaml config.yaml
# Edit config.yaml with your API keys and settings

# Run (foreground)
cargo run -- run --config config.yaml

# Run (daemon mode)
cargo run -- run --daemon --config config.yaml

# Management commands
cargo run -- status                    # Check if daemon is running
cargo run -- reload                    # Send SIGHUP to reload config
cargo run -- stop                      # Graceful shutdown

# Or use Make
make dev
```

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
