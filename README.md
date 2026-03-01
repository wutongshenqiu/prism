# ai-proxy

Multi-provider AI API gateway written in Rust. Routes requests across Claude, OpenAI, Gemini, and any OpenAI-compatible provider with automatic credential rotation, format translation, and streaming support.

## Features

- **Multi-provider**: Claude, OpenAI, Gemini, and OpenAI-compatible (DeepSeek, Groq, etc.)
- **Format translation**: Send OpenAI-format requests, get routed to any provider transparently
- **Credential rotation**: Round-robin or fill-first strategy with weighted load balancing across multiple API keys
- **Streaming**: SSE passthrough with keepalive, bootstrap retry, and cross-format stream translation
- **Model fallback**: Request-level `models` array — automatically try the next model if one fails
- **Rate limiting**: Per-key and global RPM limits with sliding window, `x-ratelimit-*` headers, HTTP 429 + `Retry-After`
- **Cost tracking**: Built-in price table for 30+ models, per-request cost calculation, configurable price overrides
- **Debug mode**: `x-debug: true` header returns routing details (provider, model, attempts) in response headers
- **Web dashboard**: React SPA with real-time metrics, request logs, provider/auth-key management, and WebSocket push
- **Daemon mode**: Run as background service with `run`, `stop`, `status`, `reload` subcommands
- **Retry & cooldown**: Automatic retry with exponential backoff, per-credential cooldowns for 429/5xx/network errors
- **Hot reload**: Config file watcher — update credentials without restart
- **Auth keys**: Client-side API key authentication with `sk-proxy-` prefixed keys
- **TLS**: Optional HTTPS with rustls
- **Cloaking**: Request masquerading for Claude API compliance
- **Payload rules**: Per-model field overrides, defaults, and filters
- **Responses API**: Transparent Chat Completions ↔ OpenAI Responses API conversion via `wire-api: responses`

## Quick Start

```bash
# Build
cargo build --release

# Configure
cp config.example.yaml config.yaml
# Edit config.yaml with your API keys

# Run (foreground)
./target/release/ai-proxy run --config config.yaml

# Run (daemon mode)
./target/release/ai-proxy run --daemon --config config.yaml

# Management commands
./target/release/ai-proxy status    # Check if daemon is running
./target/release/ai-proxy reload    # Hot-reload config (SIGHUP)
./target/release/ai-proxy stop      # Graceful shutdown
```

The server starts on `http://0.0.0.0:8317` by default.

### Docker

```bash
# Build and run
docker compose up -d --build

# Or manually
docker build -t ai-proxy:local .
docker run -d --name ai-proxy -p 8317:8317 \
  -v ./config.yaml:/etc/ai-proxy/config.yaml:ro \
  ai-proxy:local

# Logs
docker logs -f ai-proxy
```

## Usage

### Chat Completions (OpenAI format)

```bash
curl http://localhost:8317/v1/chat/completions \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-6",
    "messages": [{"role": "user", "content": "Hello"}],
    "max_tokens": 100
  }'
```

### Model Fallback

Specify multiple models — the gateway tries each in order until one succeeds:

```bash
curl http://localhost:8317/v1/chat/completions \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "models": ["claude-sonnet-4-6", "gpt-4o", "gemini-2.0-flash"],
    "messages": [{"role": "user", "content": "Hello"}],
    "max_tokens": 100
  }'
```

### Debug Mode

Add `x-debug: true` to see routing details in response headers:

```bash
curl -v http://localhost:8317/v1/chat/completions \
  -H "x-debug: true" \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "test"}]}'

# Response headers include:
# x-debug-provider: openai
# x-debug-model: gpt-4o
# x-debug-attempts: 1
```

### Streaming

Add `"stream": true` to any request.

### Endpoints

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/health` | GET | No | Health check |
| `/metrics` | GET | No | Metrics snapshot (requests, tokens, cost) |
| `/v1/models` | GET | API key | List available models |
| `/v1/chat/completions` | POST | API key | OpenAI Chat Completions (routes to any provider) |
| `/v1/messages` | POST | API key | Claude Messages API |
| `/v1/responses` | POST | API key | OpenAI Responses API |
| `/admin/config` | GET | No | Current config (redacted) |
| `/admin/metrics` | GET | No | Detailed metrics |
| `/admin/models` | GET | No | All models with provider info |

## Dashboard

The web dashboard provides real-time monitoring and configuration management.

### Setup

```yaml
# config.yaml
dashboard:
  enabled: true
  username: "admin"
  password-hash: "$2b$12$..."   # bcrypt hash of your password
  jwt-secret: "your-secret"     # or set DASHBOARD_JWT_SECRET env var
```

Generate a password hash:

```bash
htpasswd -nbBC 12 "" "your-password" | cut -d: -f2
```

### Pages

| Page | Description |
|------|-------------|
| Overview | Request/error rates, active providers, recent activity |
| Metrics | Token usage, cost breakdown, latency percentiles |
| Request Logs | Searchable log with model, provider, tokens, cost per request |
| Providers | Add/edit/delete provider credentials |
| Auth Keys | Manage client API keys |
| Routing | View/change routing strategy |
| System | Uptime, version, config reload |
| App Logs | Application log viewer |

### Dashboard API

All dashboard endpoints are under `/api/dashboard/` and require JWT authentication (obtained via login).

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/dashboard/auth/login` | POST | Login (returns JWT) |
| `/api/dashboard/auth/refresh` | POST | Refresh JWT token |
| `/api/dashboard/providers` | GET/POST | List / create providers |
| `/api/dashboard/providers/{id}` | GET/PATCH/DELETE | Provider CRUD |
| `/api/dashboard/auth-keys` | GET/POST | List / create auth keys |
| `/api/dashboard/auth-keys/{id}` | DELETE | Delete auth key |
| `/api/dashboard/routing` | GET/PATCH | Get / update routing |
| `/api/dashboard/logs` | GET | Query request logs |
| `/api/dashboard/logs/stats` | GET | Log statistics |
| `/api/dashboard/config/reload` | POST | Hot-reload config |
| `/api/dashboard/system/health` | GET | System health |
| `/ws/dashboard` | WebSocket | Real-time metrics & log push |

## Configuration

See [`config.example.yaml`](config.example.yaml) for all options. Key sections:

```yaml
# Server
host: "0.0.0.0"
port: 8317

# Client authentication (leave empty to allow all)
api-keys:
  - "your-client-api-key"

# Routing strategy: round-robin | fill-first
routing:
  strategy: round-robin

# Rate limiting
rate-limit:
  enabled: true
  global-rpm: 60        # Global requests per minute (0 = unlimited)
  per-key-rpm: 30       # Per-API-key RPM (0 = unlimited)

# Retry
request-retry: 3

# Cost tracking (override built-in prices)
model-prices:
  my-custom-model:
    input: 1.0           # USD per 1M input tokens
    output: 2.0

# Provider credentials (multiple keys per provider)
claude-api-key:
  - api-key: "sk-ant-..."
    models:
      - id: "claude-sonnet-4-6"
        alias: "sonnet"

openai-api-key:
  - api-key: "sk-..."

gemini-api-key:
  - api-key: "..."

# OpenAI-compatible providers
openai-compatibility:
  - api-key: "..."
    base-url: "https://api.deepseek.com"
    prefix: "deepseek/"
    models:
      - id: "deepseek-chat"
```

### Per-key Options

| Option | Description |
|--------|-------------|
| `api-key` | API key (required) |
| `base-url` | Custom API endpoint |
| `proxy-url` | Per-key proxy (overrides global; `""` = direct) |
| `prefix` | Model name prefix for routing (e.g., `"deepseek/"`) |
| `models` | Available models with optional aliases |
| `excluded-models` | Models to exclude (supports `*` glob) |
| `headers` | Custom HTTP headers |
| `weight` | Routing weight for weighted round-robin (default: 1) |
| `wire-api` | `chat` (default) or `responses` (OpenAI Responses API) |
| `disabled` | Disable this credential |

## Architecture

```
ai-proxy (binary)
├── ai-proxy-core       # Config, errors, provider traits, metrics, rate limiting, cost tracking
├── ai-proxy-provider   # Claude/Gemini/OpenAI executors, credential routing
├── ai-proxy-translator # Cross-format request/response translation
└── ai-proxy-server     # Axum HTTP server, dispatch, middleware, dashboard API
```

## Development

```bash
make dev           # Run locally (foreground)
make test          # Run all tests
make lint          # Format check + clippy
make fmt           # Auto-format code
make web-install   # Install frontend dependencies
make web-dev       # Start frontend dev server
make web-build     # Build frontend for production
```

## Requirements

- Rust 1.85+ (Edition 2024)
- Node.js 18+ (for dashboard frontend)

## License

MIT
