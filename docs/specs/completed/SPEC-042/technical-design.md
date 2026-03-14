# Technical Design: Crate Structure Refactoring

| Field     | Value                        |
|-----------|------------------------------|
| Spec ID   | SPEC-042                     |
| Title     | Crate Structure Refactoring  |
| Author    | AI Agent                     |
| Status    | Draft                        |
| Created   | 2026-03-12                   |
| Updated   | 2026-03-12                   |

## Overview

Refactor the workspace from 4 crates (core as god crate) to 6 crates with clean layering. This is a pure structural refactor — zero behavior change. Executed in 4 phases, each independently landable.

Reference: [SPEC-042 PRD](prd.md)

## Current Dependency Graph

```
prism-core  (22 modules, 30+ deps including moka, reqwest, notify, fork, libc, sd-notify, axum...)
    |
    +-- prism-provider   (depends on full core)
    +-- prism-translator (depends on full core)
    +-- prism-server     (depends on full core + provider + translator)
    |
prism (binary: depends on all 4 + rustls, hyper, hyper-util, tower)
```

## Target Dependency Graph

```
prism-types  (lightweight: serde, thiserror, uuid, chrono, bytes, async-trait, tokio-stream)
    |
    +-- prism-translator  (types + serde_json)
    +-- prism-provider    (types + reqwest, tokio-stream)
    +-- prism-core        (types + moka, notify, arc-swap, reqwest, serde_yaml_ng, sha2, rand, regex)
    +-- prism-lifecycle   (tokio, tracing, fork, sd-notify, libc, tracing-appender, tracing-subscriber)
    |
    +-- prism-server      (types + core + provider + translator + lifecycle + axum, tower, jsonwebtoken, bcrypt)
    |
prism (binary: server + lifecycle + tokio + clap + dotenvy + anyhow)
```

## API Design

No API changes. All HTTP endpoints, request/response shapes, and headers remain identical.

## Backend Implementation

### Phase 1: Cleanup — Remove Zombie Dependencies & Eliminate Duplication

**Goal**: Quick wins with minimal structural change.

#### 1a. Remove zombie deps from core

Edit `crates/core/Cargo.toml`:
```diff
-jsonwebtoken = { workspace = true }
-bcrypt = { workspace = true }
```

Verify: `cargo check --workspace` must pass (confirming these are truly unused).

#### 1b. Server reuses core's HTTP client builder

In `crates/server/src/handler/dashboard/providers.rs`, replace the local `build_reqwest_client()` with `prism_core::proxy::build_http_client_with_timeout()`:

```rust
// Before (server/handler/dashboard/providers.rs)
fn build_reqwest_client(proxy_url: Option<&str>) -> Result<reqwest::Client, ...> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(10));
    if let Some(proxy) = proxy_url {
        builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }
    Ok(builder.build()?)
}

// After
use prism_core::proxy::build_http_client_with_timeout;
// Call: build_http_client_with_timeout(proxy_url, Duration::from_secs(10))
```

#### 1c. Server reuses core's YAML helpers

Add to `crates/core/src/config.rs`:
```rust
/// Serialize a Config to YAML string.
pub fn to_yaml(config: &Config) -> Result<String, anyhow::Error> {
    Ok(serde_yaml_ng::to_string(config)?)
}

/// Deserialize a Config from YAML string (with sanitization).
pub fn from_yaml(yaml: &str) -> Result<Config, anyhow::Error> {
    let mut config: Config = serde_yaml_ng::from_str(yaml)?;
    config.sanitize();
    Ok(config)
}
```

Server's `providers.rs` calls these instead of using `serde_yaml_ng` directly. Then remove `serde_yaml_ng` from `crates/server/Cargo.toml`.

---

### Phase 2: Extract `prism-types` Crate

**Goal**: Create lightweight shared types crate, migrate translator and provider to depend on it instead of core.

#### New crate: `crates/types/`

```
crates/types/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── provider.rs       # Format, WireApi, AuthRecord, ModelEntry, ProviderExecutor trait,
    |                     # ProviderRequest, ProviderResponse, StreamChunk, StreamResult, ModelInfo
    ├── error.rs          # ProxyError (with u16 status_code instead of axum::StatusCode)
    ├── context.rs        # RequestContext
    ├── types/
    |   ├── mod.rs
    |   ├── openai.rs     # OpenAI request/response types
    |   ├── claude.rs     # Claude request/response types
    |   └── gemini.rs     # Gemini request/response types
    ├── request_record.rs # TokenUsage, RequestRecord, AttemptSummary, LogDetailLevel
    ├── request_log.rs    # LogStore trait, LogQuery, LogPage, StatsQuery, LogStats
    ├── auth_key.rs       # AuthKeyEntry, AuthKeyStore, KeyRateLimitConfig, BudgetConfig
    ├── glob.rs           # glob_match()
    └── secret.rs         # resolve()
```

#### Cargo.toml

```toml
[package]
name = "prism-types"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true }
bytes = { workspace = true }
chrono = { workspace = true }
async-trait = { workspace = true }
tokio-stream = { workspace = true }
tokio = { workspace = true }  # for broadcast in request_log
tracing = { workspace = true }
```

No `moka`, `reqwest`, `notify`, `fork`, `sd-notify`, `axum`, `tracing-appender`.

#### error.rs key change

```rust
// prism-types/src/error.rs
impl ProxyError {
    /// HTTP status code as u16 (no axum dependency).
    pub fn status_code_u16(&self) -> u16 {
        match self {
            Self::Auth(_) => 401,
            Self::ModelNotAllowed(_) | Self::KeyExpired(_) => 403,
            Self::ModelNotFound(_) | Self::BadRequest(_) => 400,
            Self::RateLimited { .. } => 429,
            Self::NoCredentials(_) | Self::ModelCooldown { .. } | Self::Upstream { .. } => 503,
            _ => 500,
        }
    }
}
```

In `prism-server`, add a conversion helper:

```rust
// prism-server/src/error_ext.rs
use axum::http::StatusCode;
use prism_types::error::ProxyError;

pub fn status_code(err: &ProxyError) -> StatusCode {
    StatusCode::from_u16(err.status_code_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}
```

#### Migration of downstream crates

**prism-translator** `Cargo.toml`:
```diff
-prism-core = { workspace = true }
+prism-types = { workspace = true }
```

All imports change: `prism_core::error::ProxyError` -> `prism_types::error::ProxyError`, etc.

**prism-provider** `Cargo.toml`:
```diff
-prism-core = { workspace = true }
+prism-types = { workspace = true }
```

**prism-core** `Cargo.toml` adds:
```diff
+prism-types = { workspace = true }
```

Core re-exports types for backward compatibility during transition:
```rust
// crates/core/src/lib.rs
pub use prism_types::auth_key;
pub use prism_types::context;
pub use prism_types::error;
pub use prism_types::glob;
pub use prism_types::provider;
pub use prism_types::request_log;
pub use prism_types::request_record;
pub use prism_types::secret;
pub use prism_types::types;
```

This allows `prism-server` to continue using `prism_core::error::ProxyError` during gradual migration.

#### Workspace registration

```toml
# Root Cargo.toml
[workspace]
members = [
    "crates/types",
    "crates/core",
    "crates/provider",
    "crates/translator",
    "crates/server",
]

[workspace.dependencies]
prism-types = { path = "crates/types" }
```

---

### Phase 3: Extract `prism-lifecycle` Crate

**Goal**: Isolate Unix process management and logging initialization.

#### New crate: `crates/lifecycle/`

```
crates/lifecycle/
├── Cargo.toml
└── src/
    ├── lib.rs        # Lifecycle trait, ForegroundLifecycle, SystemdLifecycle, detect_lifecycle()
    ├── daemon.rs     # daemonize() — unix only
    ├── logging.rs    # init_logging(), init_logging_with_layer()
    ├── notify.rs     # sd_ready(), sd_reloading(), sd_stopping()
    ├── pid_file.rs   # PidFile — unix only
    └── signal.rs     # SignalHandler
```

#### Cargo.toml

```toml
[package]
name = "prism-lifecycle"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { workspace = true }
tracing = { workspace = true }
tracing-appender = { workspace = true }
tracing-subscriber = { workspace = true }
sd-notify = { workspace = true }
anyhow = { workspace = true }

[target.'cfg(unix)'.dependencies]
libc = { workspace = true }
fork = { workspace = true }
```

#### Core cleanup

Remove from `crates/core/Cargo.toml`:
```diff
-tracing-appender = { workspace = true }
-tracing-subscriber = { workspace = true }
-sd-notify = { workspace = true }
-
-[target.'cfg(unix)'.dependencies]
-libc = { workspace = true }
-fork = { workspace = true }
```

Remove `crates/core/src/lifecycle/` directory entirely. Core re-exports for backward compat:
```rust
// crates/core/src/lib.rs
pub use prism_lifecycle as lifecycle;
```

Or, server and binary switch imports directly to `prism_lifecycle::*`.

#### Binary and server migration

`src/main.rs`:
```diff
-use prism_core::lifecycle::daemon::daemonize;
+use prism_lifecycle::daemon::daemonize;
```

---

### Phase 4: Move TLS/Serve Logic into Server

**Goal**: Binary becomes a thin CLI shell.

#### Move `src/app.rs` -> `crates/server/src/app.rs`

Move `Application` struct, `serve_http()`, `serve_tls()` into `prism-server`. Server gains these dependencies:

```diff
# crates/server/Cargo.toml
+rustls = { workspace = true }
+rustls-pki-types = { workspace = true }
+tokio-rustls = { workspace = true }
+hyper = { workspace = true }
+hyper-util = { workspace = true }
```

Remove from binary `Cargo.toml`:
```diff
-rustls = { workspace = true }
-rustls-pki-types = { workspace = true }
-tokio-rustls = { workspace = true }
-axum = { workspace = true }
-arc-swap = { workspace = true }
-hyper = { workspace = true }
-hyper-util = { workspace = true }
-tower = { workspace = true }
```

#### Simplified binary

```rust
// src/main.rs
mod cli;

use clap::Parser;
use cli::{Cli, Command, RunArgs};

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Run(RunArgs::default()));

    match command {
        Command::Run(args) => prism_server::run(args.into()),
        Command::Stop(args) => prism_lifecycle::pid_file::cmd_stop(&args.pid_file, args.timeout),
        Command::Status(args) => prism_lifecycle::pid_file::cmd_status(&args.pid_file),
        Command::Reload(args) => prism_lifecycle::pid_file::cmd_reload(&args.pid_file),
    }
}
```

Binary dependencies reduce to: `prism-server`, `prism-lifecycle`, `tokio`, `clap`, `dotenvy`, `anyhow`, `libc`.

---

## Configuration Changes

None. All configuration files, environment variables, and YAML schemas remain identical.

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | No change | Pure refactor |
| Claude   | No change | Pure refactor |
| Gemini   | No change | Pure refactor |
| OpenAI-compat | No change | Pure refactor |

## Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| Feature flags in core instead of prism-types | No new crate, less workspace churn | Feature flags add compile complexity, conditional compilation bugs, poor IDE support | Rejected |
| Big-bang single PR | One review cycle | Too large to review (~50 files), high risk of merge conflicts | Rejected |
| Keep lifecycle in core with cfg attributes | Less crates to manage | Still pollutes core's Cargo.toml with unix-specific deps | Rejected |
| Only extract types, skip lifecycle | Less work | Misses the opportunity to cleanly isolate unix deps | Rejected — do both |

## Task Breakdown

### Phase 1: Cleanup (PR #1)
- [x] Verify `jsonwebtoken` and `bcrypt` are unused in core (confirmed by analysis)
- [ ] Remove `jsonwebtoken` and `bcrypt` from `crates/core/Cargo.toml`
- [ ] Refactor `crates/server/src/handler/dashboard/providers.rs` to use `prism_core::proxy::build_http_client_with_timeout()`
- [ ] Add `Config::to_yaml()` and `Config::from_yaml()` helpers to `crates/core/src/config.rs`
- [ ] Refactor `crates/server/src/handler/dashboard/providers.rs` to use config YAML helpers
- [ ] Remove `serde_yaml_ng` from `crates/server/Cargo.toml`
- [ ] Run `make lint && make test`

### Phase 2: Extract `prism-types` (PR #2)
- [ ] Create `crates/types/` with `Cargo.toml` and module structure
- [ ] Move `provider.rs`, `error.rs`, `context.rs`, `types/`, `request_record.rs`, `request_log.rs`, `auth_key.rs`, `glob.rs`, `secret.rs` from core to types
- [ ] Change `ProxyError::status_code()` from `axum::StatusCode` to `u16`
- [ ] Add `status_code()` conversion helper in `prism-server`
- [ ] Update `prism-translator` to depend on `prism-types` instead of `prism-core`
- [ ] Update `prism-provider` to depend on `prism-types` instead of `prism-core`
- [ ] Add `prism-types` dependency to `prism-core`, add re-exports for backward compat
- [ ] Remove `axum` from `crates/core/Cargo.toml`
- [ ] Register `prism-types` in workspace `Cargo.toml`
- [ ] Update all import paths in translator and provider
- [ ] Run `make lint && make test`

### Phase 3: Extract `prism-lifecycle` (PR #3)
- [ ] Create `crates/lifecycle/` with `Cargo.toml` and module structure
- [ ] Move `lifecycle/` directory from core to lifecycle crate
- [ ] Remove `fork`, `sd-notify`, `libc`, `tracing-appender`, `tracing-subscriber` from `crates/core/Cargo.toml`
- [ ] Add re-export `pub use prism_lifecycle as lifecycle;` in core (or migrate imports directly)
- [ ] Update `src/main.rs` imports
- [ ] Register `prism-lifecycle` in workspace `Cargo.toml`
- [ ] Run `make lint && make test`

### Phase 4: Move Serve Logic to Server (PR #4)
- [ ] Move `src/app.rs` content (`Application`, `serve_http`, `serve_tls`) into `crates/server/src/app.rs`
- [ ] Add `rustls`, `tokio-rustls`, `hyper`, `hyper-util` to `crates/server/Cargo.toml`
- [ ] Remove `rustls`, `tokio-rustls`, `hyper`, `hyper-util`, `tower`, `axum`, `arc-swap` from root binary `Cargo.toml`
- [ ] Simplify `src/main.rs` to thin CLI dispatch
- [ ] Export `prism_server::run()` or `prism_server::Application` as public API
- [ ] Run `make lint && make test`

## Test Strategy

- **Unit tests:** All existing tests must pass unmodified in every phase. No new tests required (pure refactor).
- **Integration tests:** E2E tests (`tests/e2e/main.rs`) must pass — validates no behavior regression.
- **Compilation checks:** `cargo check --workspace` after each phase to verify dependency graph is correct.
- **Lint:** `cargo clippy --workspace --tests -- -D warnings` must pass — catches unused imports, dead code from migration.
- **Manual verification:** Start server with `cargo run -- run --config config.yaml`, verify `/health`, `/v1/chat/completions`, and dashboard endpoints respond correctly.

## Rollout Plan

1. **Phase 1** — Land as single PR. Low risk, no structural change.
2. **Phase 2** — Land as single PR. Largest change, most import path rewrites. Core re-exports provide safety net.
3. **Phase 3** — Land as single PR. After Phase 2 stabilizes.
4. **Phase 4** — Land as single PR. After Phase 3 stabilizes.
5. **Cleanup PR** — Remove backward-compat re-exports from core once all imports are migrated. Update `CLAUDE.md` to reflect new crate structure.

Each phase is independently revertable.
