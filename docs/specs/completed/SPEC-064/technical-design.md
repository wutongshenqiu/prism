# Technical Design: Upstream Presentation Layer

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-064                           |
| Title     | Upstream Presentation Layer        |
| Author    | AI Agent                           |
| Status    | Draft                              |
| Created   | 2026-03-14                         |
| Updated   | 2026-03-14                         |
| Parent    | GitHub Issue #203                  |
| Issues    | #206, #207, #208, #209             |

## Overview

This spec defines a greenfield upstream presentation layer for Prism. The presentation layer owns how Prism presents itself to upstream providers -- both HTTP headers and identity-driven body mutations -- through a profile-first configuration model.

The design replaces the current scattered `headers` / `cloak` / `claude-header-defaults` trio with a single `upstream-presentation` config per provider.

Reference: [PRD](prd.md)

---

## 1. Configuration Model

### 1.1 Provider-Level Config

A new `upstream-presentation` field on `ProviderKeyEntry`:

```yaml
providers:
  - name: my-claude
    format: claude
    api-key: "sk-ant-..."
    upstream-presentation:
      profile: claude-code       # native | claude-code | gemini-cli | codex-cli
      mode: auto                 # always | auto (default: always)
      # Profile-specific options (claude-code only in v1)
      strict-mode: false
      sensitive-words: ["proxy", "prism"]
      cache-user-id: true
      # Advanced: raw header overrides
      custom-headers:
        x-custom-thing: "value"
```

When `upstream-presentation` is omitted, the provider defaults to `{ profile: native }`.

### 1.2 Profile Descriptions

| Profile | Target Format | Identity Headers | Body Mutations |
|---------|--------------|-----------------|----------------|
| `native` | Any | None | None |
| `claude-code` | Claude | `user-agent`, identity markers | System prompt, `metadata.user_id`, sensitive word obfuscation |
| `gemini-cli` | Gemini | `user-agent`, `x-goog-api-client` | None in v1 |
| `codex-cli` | OpenAI | `user-agent` | None in v1 (HTTP-only) |

### 1.3 Activation Modes

| Mode | Behavior |
|------|----------|
| `always` | Always apply the profile (default for non-native profiles) |
| `auto` | Skip if the inbound request's User-Agent already matches the real client (e.g., skip `claude-code` profile if UA starts with `claude-cli` or `claude-code`) |

`native` profile always activates (it's a no-op).

### 1.4 Header Precedence (Bottom Wins)

```
1. Profile default headers (e.g., claude-code's user-agent)
   ↓ overridden by
2. Custom headers (operator-specified overrides)
   ↓ filtered by
3. Protected header policy (auth/protocol headers blocked)
   ↓ result stored in
4. ProviderRequest.headers
   ↓ applied by executor, then
5. Executor auth/protocol invariants (final authority)
```

### 1.5 Protected Headers

These headers cannot be set by profiles or custom headers. If attempted, they are silently dropped and recorded in the presentation trace.

**Always protected:**
- `authorization`
- `x-api-key`
- `x-goog-api-key`
- `content-type`
- `host`
- `content-length`
- `transfer-encoding`
- `connection`

**Format-specific protected (enforced by executor, not presentation):**
- Claude: `anthropic-version`, `anthropic-beta` (set by ClaudeExecutor)

Rationale: Auth and transport headers are protocol invariants owned by executors. Presentation headers are identity concerns. Keeping them separate prevents accidental auth override and keeps executors as the single source of truth for protocol compliance.

### 1.6 Backward Compatibility

During config normalization (`Config::sanitize`), legacy fields are transparently migrated:

| Legacy Config | Migration |
|---------------|-----------|
| `headers` only (no `upstream-presentation`) | `→ upstream-presentation: { profile: native, custom-headers: <headers> }` |
| `cloak` with `mode != never` (no `upstream-presentation`) | `→ upstream-presentation: { profile: claude-code, mode: <cloak.mode>, strict-mode: <cloak.strict_mode>, sensitive-words: <cloak.sensitive_words>, cache-user-id: <cloak.cache_user_id>, custom-headers: <headers> }` |
| `claude-header-defaults` (global) | Deprecated. Log warning on load. `claude-code` profile provides built-in defaults. Operators should move custom values to per-provider `custom-headers`. |

When `upstream-presentation` is explicitly set, legacy `headers` and `cloak` fields on the same provider are ignored.

---

## 2. Module Structure

### 2.1 Core: `prism_core::presentation`

```
crates/core/src/presentation/
├── mod.rs              # Public API: apply(), PresentationResult
├── config.rs           # UpstreamPresentationConfig, ProfileKind, ActivationMode
├── profile.rs          # Built-in profile definitions and header tables
├── engine.rs           # Header merge + body mutation logic
├── trace.rs            # PresentationTrace, HeaderProvenance, MutationRecord
└── protected.rs        # PROTECTED_HEADERS set, is_protected()
```

No Axum types, no reqwest types, no runtime side effects.

### 2.2 Server: `prism_server::dispatch::prepare`

```
crates/server/src/dispatch/
├── mod.rs              # dispatch() orchestration (unchanged public API)
├── prepare.rs          # NEW: request preparation pipeline
├── executor.rs         # execution, retry, response handling (simplified)
├── features.rs         # route feature extraction
├── helpers.rs          # utility functions
└── streaming.rs        # SSE response building
```

### 2.3 Dashboard

```
crates/server/src/handler/dashboard/
├── providers.rs        # Updated: upstream-presentation in detail/create/update
└── ...

web/src/pages/
├── Providers.tsx        # Updated: profile-first editing UX
└── ...
```

---

## 3. Key Types

### 3.1 Configuration Types (`config.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileKind {
    Native,
    ClaudeCode,
    GeminiCli,
    CodexCli,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationMode {
    Always,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct UpstreamPresentationConfig {
    pub profile: ProfileKind,
    pub mode: ActivationMode,
    // claude-code specific options
    pub strict_mode: bool,
    pub sensitive_words: Vec<String>,
    pub cache_user_id: bool,
    // Advanced: raw header overrides
    pub custom_headers: HashMap<String, String>,
}

impl Default for UpstreamPresentationConfig {
    fn default() -> Self {
        Self {
            profile: ProfileKind::Native,
            mode: ActivationMode::Always,
            strict_mode: false,
            sensitive_words: Vec::new(),
            cache_user_id: false,
            custom_headers: HashMap::new(),
        }
    }
}
```

### 3.2 Engine Types (`engine.rs`, `trace.rs`)

```rust
/// Input context for presentation computation.
pub struct PresentationContext<'a> {
    pub target_format: Format,
    pub model: &'a str,
    pub user_agent: Option<&'a str>,
    pub api_key: &'a str,
}

/// Output of the presentation engine.
pub struct PresentationResult {
    pub headers: HashMap<String, String>,
    pub trace: PresentationTrace,
}

/// Provenance tracking for debugging/preview.
pub struct PresentationTrace {
    pub profile: String,
    pub activated: bool,
    pub headers: Vec<HeaderProvenance>,
    pub body_mutations: Vec<MutationRecord>,
    pub protected_blocked: Vec<String>,
}

pub struct HeaderProvenance {
    pub name: String,
    pub value: String,
    pub source: HeaderSource,
}

pub enum HeaderSource {
    Profile,
    CustomOverride,
}

pub struct MutationRecord {
    pub kind: MutationKind,
    pub applied: bool,
    pub reason: Option<String>,
}

pub enum MutationKind {
    SystemPromptInjection,
    UserIdGeneration,
    SensitiveWordObfuscation,
}
```

### 3.3 Profile Definitions (`profile.rs`)

Each built-in profile is a function that returns its default headers and declares its body mutation capabilities:

```rust
pub struct ProfileDefinition {
    pub default_headers: HashMap<String, String>,
    pub body_mutations: Vec<MutationKind>,
    pub compatible_formats: Vec<Format>,
    pub auto_skip_ua_prefixes: Vec<String>,
}

pub fn claude_code_profile() -> ProfileDefinition {
    ProfileDefinition {
        default_headers: HashMap::from([
            ("user-agent".into(), "claude-code/1.0.0".into()),
            // Additional Claude Code identity headers
        ]),
        body_mutations: vec![
            MutationKind::SystemPromptInjection,
            MutationKind::UserIdGeneration,
            MutationKind::SensitiveWordObfuscation,
        ],
        compatible_formats: vec![Format::Claude],
        auto_skip_ua_prefixes: vec![
            "claude-cli".into(),
            "claude-code".into(),
        ],
    }
}

pub fn gemini_cli_profile() -> ProfileDefinition {
    ProfileDefinition {
        default_headers: HashMap::from([
            ("user-agent".into(), "gemini-cli/0.1.0".into()),
            ("x-goog-api-client".into(), "gemini-cli/0.1.0".into()),
        ]),
        body_mutations: vec![],
        compatible_formats: vec![Format::Gemini],
        auto_skip_ua_prefixes: vec!["gemini-cli".into()],
    }
}

pub fn codex_cli_profile() -> ProfileDefinition {
    ProfileDefinition {
        default_headers: HashMap::from([
            ("user-agent".into(), "codex-cli/1.0.0".into()),
        ]),
        body_mutations: vec![],
        compatible_formats: vec![Format::OpenAI],
        auto_skip_ua_prefixes: vec!["codex".into()],
    }
}

pub fn native_profile() -> ProfileDefinition {
    ProfileDefinition {
        default_headers: HashMap::new(),
        body_mutations: vec![],
        compatible_formats: vec![Format::OpenAI, Format::Claude, Format::Gemini],
        auto_skip_ua_prefixes: vec![],
    }
}
```

---

## 4. Request Processing Flow

### 4.1 Current Flow (executor.rs, lines 192-306)

```
translate → payload_rules → cloak (Claude only) → thinking_cache → claude_header_defaults → build ProviderRequest → executor
```

Presentation logic is scattered across the middle of `execute_single_attempt`.

### 4.2 Target Flow

```
translate → payload_rules → PRESENTATION → thinking_cache → build ProviderRequest → executor
```

The presentation step replaces three inline code blocks:
1. The `cloak` application (executor.rs:225-244)
2. The `claude_header_defaults` injection (executor.rs:272-280)
3. The `auth.headers` pass-through in `apply_headers` (common.rs:26-28)

### 4.3 Detailed Flow in `prepare.rs`

```rust
pub fn prepare_provider_request(
    config: &Config,
    auth: &AuthRecord,
    req: &DispatchRequest,
    target_format: Format,
    actual_model: &str,
    translated_payload: Vec<u8>,
    thinking_cache: Option<&ThinkingCache>,
) -> Result<(ProviderRequest, PresentationTrace), ProxyError> {

    // 1. Parse payload
    let mut payload_value: serde_json::Value =
        serde_json::from_slice(&translated_payload)?;

    // 2. Apply payload manipulation rules
    if payload_value.is_object() {
        apply_payload_rules(&mut payload_value, &config.payload, actual_model, Some(target_format.as_str()));
    }

    // 3. Apply upstream presentation
    let presentation_config = &auth.upstream_presentation;
    let context = PresentationContext {
        target_format,
        model: actual_model,
        user_agent: req.user_agent.as_deref(),
        api_key: &auth.api_key,
    };
    let presentation_result = presentation::apply(presentation_config, &context, &mut payload_value);

    // 4. Inject thinking cache (Claude targets)
    if target_format == Format::Claude {
        if let Some(tc) = thinking_cache {
            let tenant_id = req.tenant_id.as_deref().unwrap_or("");
            tc.inject_into_request(tenant_id, actual_model, &mut payload_value).await;
        }
    }

    // 5. Inject stream_options for OpenAI streaming
    if req.stream && target_format == Format::OpenAI {
        inject_stream_options(&mut payload_value);
    }

    // 6. Serialize and build ProviderRequest
    let payload = serde_json::to_vec(&payload_value)?;

    let provider_request = ProviderRequest {
        model: actual_model.to_string(),
        payload: Bytes::from(payload),
        source_format: req.source_format,
        stream: req.stream,
        headers: presentation_result.headers,
        original_request: Some(req.body.clone()),
    };

    Ok((provider_request, presentation_result.trace))
}
```

### 4.4 Executor Changes

With `prepare.rs` handling request shaping, `executor.rs` simplifies:

**Before (current):**
```rust
// executor.rs: execute_single_attempt -- 150+ lines of inline request building
let translated_payload = translators.translate_request(...);
let translated_payload = { /* payload rules */ };
let translated_payload = if target_format == Format::Claude { /* cloak */ };
let translated_payload = if target_format == Format::Claude { /* thinking cache */ };
let mut request_headers = Default::default();
if target_format == Format::Claude && cloak_active { /* claude_header_defaults */ };
let translated_payload = if req.stream && Format::OpenAI { /* stream_options */ };
let provider_request = ProviderRequest { headers: request_headers, ... };
```

**After:**
```rust
// executor.rs: execute_single_attempt -- calls prepare, then executes
let (provider_request, trace) = prepare::prepare_provider_request(
    &config, &auth, &req, target_format, &actual_model,
    translated_payload, thinking_cache,
)?;
// ... execute + handle response (unchanged)
```

### 4.5 Provider Executor Behavior (Unchanged)

Executors continue to own auth and protocol headers. In `common.rs::apply_headers`, the `auth.headers` parameter is no longer populated (empty HashMap) for providers using the new presentation model. All custom/identity headers now arrive via `ProviderRequest.headers`.

```
Executor sets:
  - content-type: application/json          (all formats)
  - authorization / x-api-key / x-goog-api-key  (auth)
  - anthropic-version, anthropic-beta        (Claude)

ProviderRequest.headers provides:
  - user-agent                               (from profile)
  - x-goog-api-client                        (from profile, Gemini)
  - any custom-headers                       (from operator config)
```

Since `apply_headers` applies request headers first and then auth headers, and executors set auth headers directly on the request builder before calling `apply_headers`, the executor's headers take precedence by virtue of being set last (reqwest appends; for unique headers, last wins). This is the existing behavior and does not change.

---

## 5. Presentation Engine Algorithm

```rust
// presentation/engine.rs

pub fn apply(
    config: &UpstreamPresentationConfig,
    context: &PresentationContext,
    payload: &mut serde_json::Value,
) -> PresentationResult {
    let profile_def = resolve_profile(&config.profile);
    let mut trace = PresentationTrace::new(&config.profile);

    // 1. Check activation
    if !should_activate(config, context, &profile_def) {
        trace.activated = false;
        return PresentationResult {
            headers: config.custom_headers.clone(), // still apply custom headers
            trace,
        };
    }
    trace.activated = true;

    // 2. Merge headers: profile defaults → custom overrides
    let mut headers = profile_def.default_headers.clone();
    for (k, v) in &config.custom_headers {
        headers.insert(k.to_lowercase(), v.clone());
    }

    // 3. Filter protected headers
    headers.retain(|k, _| {
        if is_protected(k) {
            trace.protected_blocked.push(k.clone());
            false
        } else {
            true
        }
    });

    // 4. Record header provenance
    for (k, v) in &headers {
        let source = if config.custom_headers.contains_key(k) {
            HeaderSource::CustomOverride
        } else {
            HeaderSource::Profile
        };
        trace.headers.push(HeaderProvenance {
            name: k.clone(), value: v.clone(), source,
        });
    }

    // 5. Apply body mutations (if format is compatible)
    if profile_def.compatible_formats.contains(&context.target_format) {
        for mutation in &profile_def.body_mutations {
            apply_body_mutation(mutation, config, context, payload, &mut trace);
        }
    }

    PresentationResult { headers, trace }
}

fn should_activate(
    config: &UpstreamPresentationConfig,
    context: &PresentationContext,
    profile_def: &ProfileDefinition,
) -> bool {
    match config.mode {
        ActivationMode::Always => true,
        ActivationMode::Auto => {
            // Skip if inbound UA matches the real client
            let ua = context.user_agent.unwrap_or("");
            !profile_def.auto_skip_ua_prefixes.iter().any(|prefix| ua.starts_with(prefix))
        }
    }
}
```

### 5.1 Body Mutations (claude-code profile)

Body mutations reuse the existing logic from `cloak.rs`, relocated into the presentation engine:

```rust
fn apply_body_mutation(
    kind: &MutationKind,
    config: &UpstreamPresentationConfig,
    context: &PresentationContext,
    payload: &mut serde_json::Value,
    trace: &mut PresentationTrace,
) {
    match kind {
        MutationKind::SystemPromptInjection => {
            inject_system_prompt(payload, config.strict_mode);
            trace.body_mutations.push(MutationRecord {
                kind: MutationKind::SystemPromptInjection,
                applied: true,
                reason: None,
            });
        }
        MutationKind::UserIdGeneration => {
            inject_user_id(payload, context.api_key, config.cache_user_id);
            trace.body_mutations.push(MutationRecord {
                kind: MutationKind::UserIdGeneration,
                applied: true,
                reason: None,
            });
        }
        MutationKind::SensitiveWordObfuscation => {
            if !config.sensitive_words.is_empty() {
                obfuscate_sensitive_words(payload, &config.sensitive_words);
                trace.body_mutations.push(MutationRecord {
                    kind: MutationKind::SensitiveWordObfuscation,
                    applied: true,
                    reason: None,
                });
            }
        }
    }
}
```

The body mutation functions (`inject_system_prompt`, `inject_user_id`, `obfuscate_sensitive_words`) are moved from `cloak.rs` into `presentation/engine.rs` (or a sub-module). `cloak.rs` is then deprecated; its public API can be replaced by thin wrappers that delegate to the presentation engine during the transition.

---

## 6. API Design

### 6.1 Provider Detail Endpoint (Updated)

**GET /api/dashboard/providers/:name** -- now includes `upstream_presentation`:

```json
{
  "name": "my-claude",
  "format": "claude",
  "api_key_masked": "sk-a****xxx",
  "base_url": "https://api.anthropic.com",
  "upstream_presentation": {
    "profile": "claude-code",
    "mode": "auto",
    "strict_mode": false,
    "sensitive_words": ["proxy"],
    "cache_user_id": true,
    "custom_headers": { "x-extra": "value" }
  },
  "models": [...],
  ...
}
```

### 6.2 Provider Create/Update (Updated)

**POST /api/dashboard/providers** and **PATCH /api/dashboard/providers/:name** accept `upstream_presentation` in the request body.

### 6.3 Presentation Preview Endpoint (New)

**POST /api/dashboard/providers/:name/presentation-preview**

Shows the effective presentation state for a sample request without actually sending anything upstream.

Request:
```json
{
  "model": "claude-sonnet-4-20250514",
  "user_agent": "python-requests/2.31.0",
  "sample_body": {
    "model": "claude-sonnet-4-20250514",
    "messages": [{"role": "user", "content": "hello"}]
  }
}
```

Response:
```json
{
  "profile": "claude-code",
  "activated": true,
  "effective_headers": {
    "user-agent": "claude-code/1.0.0",
    "x-extra": "value"
  },
  "body_mutations": [
    { "kind": "system_prompt_injection", "applied": true },
    { "kind": "user_id_generation", "applied": true },
    { "kind": "sensitive_word_obfuscation", "applied": true }
  ],
  "protected_headers_blocked": [],
  "effective_body": {
    "model": "claude-sonnet-4-20250514",
    "system": "You are Claude Code...\n\nhello",
    "messages": [...],
    "metadata": { "user_id": "user_abc123..." }
  }
}
```

### 6.4 Debug Headers (Updated)

When `x-debug: true` is sent with a request, the response includes a new header:

```
x-prism-presentation: profile=claude-code,activated=true,headers=2,mutations=3,blocked=0
```

---

## 7. ProviderKeyEntry Changes

### 7.1 New Field

```rust
pub struct ProviderKeyEntry {
    // ... existing fields ...

    // NEW: upstream presentation config
    #[serde(default)]
    pub upstream_presentation: UpstreamPresentationConfig,

    // DEPRECATED (retained for backward compat, migrated during normalize)
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub cloak: CloakConfig,
}
```

### 7.2 AuthRecord Changes

```rust
pub struct AuthRecord {
    // ... existing fields ...
    pub headers: HashMap<String, String>,           // kept but empty for migrated configs
    pub cloak: Option<CloakConfig>,                 // kept but None for migrated configs
    pub upstream_presentation: UpstreamPresentationConfig,  // NEW
}
```

### 7.3 Migration in `Config::normalize()`

```rust
fn normalize(&mut self) {
    // ... existing normalization ...

    for entry in &mut self.providers {
        // Migrate legacy cloak + headers → upstream-presentation
        if entry.upstream_presentation.profile == ProfileKind::Native
            && entry.cloak.mode != CloakMode::Never
        {
            entry.upstream_presentation = UpstreamPresentationConfig {
                profile: ProfileKind::ClaudeCode,
                mode: match entry.cloak.mode {
                    CloakMode::Always => ActivationMode::Always,
                    CloakMode::Auto => ActivationMode::Auto,
                    CloakMode::Never => ActivationMode::Always,
                },
                strict_mode: entry.cloak.strict_mode,
                sensitive_words: entry.cloak.sensitive_words.clone(),
                cache_user_id: entry.cloak.cache_user_id,
                custom_headers: entry.headers.clone(),
            };
            // Clear legacy fields to avoid double-application
            entry.cloak = CloakConfig::default();
            entry.headers.clear();
        } else if entry.upstream_presentation.profile == ProfileKind::Native
            && !entry.headers.is_empty()
            && entry.upstream_presentation.custom_headers.is_empty()
        {
            // Migrate standalone headers → native profile with custom-headers
            entry.upstream_presentation.custom_headers = entry.headers.clone();
            entry.headers.clear();
        }
    }
}
```

---

## 8. Provider Compatibility

| Provider | Profile | Headers | Body Mutations | Notes |
|----------|---------|---------|---------------|-------|
| Claude (Anthropic) | `claude-code` | `user-agent`, identity markers | System prompt, `user_id`, sensitive words | Full presentation support |
| Claude (Anthropic) | `native` | None | None | Transparent proxy |
| OpenAI / OpenAI-compat | `codex-cli` | `user-agent` | None | HTTP-only in v1 |
| OpenAI / OpenAI-compat | `native` | None | None | Transparent proxy |
| Gemini (Google) | `gemini-cli` | `user-agent`, `x-goog-api-client` | None | Header identity only |
| Gemini (Google) | `native` | None | None | Transparent proxy |
| Any | Any | `custom-headers` | None | Escape hatch |

Format compatibility check: if a profile's `compatible_formats` does not include the target format, body mutations are skipped but identity headers are still applied. This allows using `claude-code` profile with an OpenAI-compat Claude endpoint (header identity without Claude-specific body mutations).

---

## 9. Dashboard UX Changes

### 9.1 Provider List (Summary)

No change -- summary table continues to show name, format, base URL, models count, status.

### 9.2 Provider Detail/Edit

The edit modal is restructured:

```
+-- Provider Identity --+
| Name:    [my-claude  ] |
| Format:  [Claude     ] |
| Base URL: [https://...] |
| API Key: [********    ] |
+------------------------+

+-- Upstream Presentation --+
| Profile: [Claude Code  v] |  ← dropdown: Native, Claude Code, Gemini CLI, Codex CLI
|                            |
| Mode: [Auto            v] |  ← only shown for non-native profiles
|                            |
| --- Claude Code Options ---  ← shown only when profile=claude-code
| [x] Strict Mode           |
| Sensitive Words: [proxy, ] |
| [x] Cache User ID         |
|                            |
| --- Custom Headers ---     ← collapsed by default
| [+ Add Header]            |
| [user-agent   ] [value  ] |
+----------------------------+

+-- Models & Routing --+
| Models: [...]         |
| Weight: [1]           |
| Region: [us-east]     |
+----------------------+
```

### 9.3 Preview Panel

An "Inspect" button on the provider detail view opens a preview panel that calls the presentation-preview API and shows:

- Effective headers (with provenance badges: "Profile" / "Custom")
- Body mutations applied (with before/after preview for system prompt)
- Protected headers that were blocked (if any)

---

## 10. Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| **New crate (`prism_presentation`)** | Clear crate boundary, enforces no transport deps at compile time | One consumer, adds `Cargo.toml` + inter-crate coordination overhead, type duplication for config | **Rejected for v1.** Revisit when a second consumer exists. |
| **Trait-plugin system for profiles** | Maximum extensibility, custom profiles via trait impls | Over-engineering for 4 profiles, dynamic dispatch overhead, complex registration | **Rejected.** Enum is simpler, exhaustive match ensures all profiles are handled. |
| **Keep extending `headers` + `cloak`** | No new abstraction, minimal code change | Perpetuates the split between header and body identity, Claude-only, no profile semantics | **Rejected.** This is what we're replacing. |
| **Generic inbound header passthrough** | Flexible, less config needed | Security risk (leaks client headers upstream), unpredictable behavior, hard to audit | **Rejected per requirements.** |

---

## 11. Task Breakdown

### Phase 1: Core Presentation Engine (Issue #207)

- [ ] Create `crates/core/src/presentation/mod.rs` with public API
- [ ] Create `config.rs` with `ProfileKind`, `ActivationMode`, `UpstreamPresentationConfig`
- [ ] Create `profile.rs` with built-in profile definitions
- [ ] Create `engine.rs` with `apply()`, activation logic, header merge, body mutations
- [ ] Create `protected.rs` with protected header set
- [ ] Create `trace.rs` with provenance types
- [ ] Move body mutation functions from `cloak.rs` into presentation engine
- [ ] Add `upstream_presentation` field to `ProviderKeyEntry` and `AuthRecord`
- [ ] Add legacy migration in `Config::normalize()`
- [ ] Unit tests: profile resolution, activation modes, header merge, protected headers
- [ ] Unit tests: body mutations (system prompt, user_id, sensitive words)
- [ ] Unit tests: backward compatibility (legacy cloak + headers migration)

### Phase 2: Server Integration (Issue #208)

- [ ] Create `crates/server/src/dispatch/prepare.rs`
- [ ] Extract request preparation logic from `executor.rs` into `prepare.rs`
- [ ] Wire presentation engine into the prepare pipeline
- [ ] Remove inline cloak/header logic from `executor.rs`
- [ ] Remove `claude_header_defaults` usage from dispatch
- [ ] Update `common.rs::apply_headers` (auth.headers no longer populated)
- [ ] Attach presentation trace to debug output
- [ ] Integration tests: full dispatch pipeline with presentation
- [ ] Verify existing behavior unchanged for configs without `upstream-presentation`

### Phase 3: Dashboard & API (Issue #209)

- [ ] Update provider detail/create/update handlers to include `upstream_presentation`
- [ ] Add `POST /api/dashboard/providers/:name/presentation-preview` endpoint
- [ ] Add `x-prism-presentation` debug response header
- [ ] Update `Providers.tsx` with profile-first editing UX
- [ ] Add preview/inspect panel to provider detail view
- [ ] Frontend tests for profile editing and preview
- [ ] API tests for preview endpoint

### Phase 4: Cleanup

- [ ] Deprecate `claude_header_defaults` in config docs
- [ ] Deprecate `cloak` field in config docs
- [ ] Update `config.example.yaml` with `upstream-presentation` examples
- [ ] Update CLAUDE.md / AGENTS.md with new module paths
- [ ] Update reference docs (`docs/reference/types/`)

---

## 12. Test Strategy

### Unit Tests (Phase 1)

| Test | Input | Expected |
|------|-------|----------|
| Native profile resolution | `profile: native` | Empty headers, no mutations |
| Claude-code profile headers | `profile: claude-code` | `user-agent` etc. in result |
| Gemini-cli profile headers | `profile: gemini-cli` | `user-agent`, `x-goog-api-client` |
| Custom headers override | Profile headers + custom | Custom values win |
| Protected header filter | Custom with `authorization` | Blocked, recorded in trace |
| Activation: always | `mode: always` with any UA | Activated |
| Activation: auto, skip | `mode: auto`, UA = `claude-code/1.0` | Not activated (profile body mutations skipped) |
| Activation: auto, apply | `mode: auto`, UA = `python-requests` | Activated |
| Body: system prompt (prepend) | `strict_mode: false` | Prepended to existing system |
| Body: system prompt (strict) | `strict_mode: true` | Replaces existing system |
| Body: user_id | `cache_user_id: true` | Stable `metadata.user_id` |
| Body: sensitive words | `sensitive_words: ["proxy"]` | Zero-width spaces inserted |
| Body: format mismatch | `claude-code` on OpenAI target | Headers applied, body mutations skipped |
| Legacy migration: cloak | Old cloak config | Migrated to claude-code profile |
| Legacy migration: headers | Old headers config | Migrated to native + custom-headers |
| Trace provenance | Mixed profile + custom headers | Each header tagged with source |

### Integration Tests (Phase 2)

| Test | Scenario | Assertion |
|------|----------|-----------|
| Full pipeline | Translate → payload rules → presentation → ProviderRequest | Request has correct headers and body |
| No presentation | Provider without `upstream-presentation` | Behaves identically to current code |
| Debug header | `x-debug: true` | Response includes `x-prism-presentation` |

### API Tests (Phase 3)

| Test | Endpoint | Assertion |
|------|----------|-----------|
| Preview: claude-code | `POST .../presentation-preview` | Returns effective headers + mutations |
| Preview: native | `POST .../presentation-preview` | Returns empty headers, no mutations |
| Preview: protected blocked | Custom header `authorization` | Trace shows blocked |
| Create with presentation | `POST /api/dashboard/providers` | Saved and retrievable |
| Update presentation | `PATCH /api/dashboard/providers/:name` | Updated in config |

---

## 13. Rollout Plan

1. **Phase 1 (Core)**: Implement `prism_core::presentation` module. All unit tests pass. No runtime behavior changes yet.
2. **Phase 2 (Server)**: Wire presentation into dispatch. Remove inline cloak/header logic. Verify all existing tests pass. New integration tests for presentation pipeline.
3. **Phase 3 (Dashboard)**: Add profile-first editing UX, preview API. Frontend + API tests.
4. **Phase 4 (Cleanup)**: Deprecation warnings for legacy fields. Doc updates.

Each phase is independently shippable. Phase 1 can merge without affecting runtime behavior. Phase 2 is the critical behavioral change. Phase 3 and 4 are additive.
