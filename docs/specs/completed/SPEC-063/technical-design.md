# SPEC-063: Technical Design

## Overview

Remove `Format::OpenAICompat`, unify 4 config arrays into one `providers` array, and refactor the credential/routing chain to use provider names instead of Format as the grouping key.

## 1. Format Enum (crates/types)

### `crates/types/src/format.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    OpenAI,
    Claude,
    Gemini,
}

impl Format {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
        }
    }

    /// Canonical default base URL for this wire protocol.
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com",
            Self::Claude => "https://api.anthropic.com",
            Self::Gemini => "https://generativelanguage.googleapis.com",
        }
    }
}

impl FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "openai" => Ok(Self::OpenAI),
            "claude" => Ok(Self::Claude),
            "gemini" => Ok(Self::Gemini),
            _ => Err(format!("unknown format: {s}")),
        }
    }
}
```

Changes:
- Delete `OpenAICompat` variant
- Delete `"openai-compat"` / `"openai_compat"` from `FromStr`
- Add `default_base_url()` method
- Delete `WireApi` — move it to remain in `crates/types` but decouple from Format

## 2. Config (crates/core)

### `crates/core/src/config.rs`

```rust
pub struct ProviderKeyEntry {
    /// Unique provider name (required).
    pub name: String,
    /// Wire protocol format (required).
    pub format: Format,
    // All existing fields remain:
    pub api_key: String,
    pub base_url: Option<String>,
    pub proxy_url: Option<String>,
    pub prefix: Option<String>,
    pub models: Vec<ModelMapping>,
    pub excluded_models: Vec<String>,
    pub headers: HashMap<String, String>,
    pub disabled: bool,
    pub cloak: CloakConfig,
    pub wire_api: WireApi,
    pub weight: u32,
    pub region: Option<String>,
    pub credential_source: Option<CredentialSource>,
    pub vertex: bool,
    pub vertex_project: Option<String>,
    pub vertex_location: Option<String>,
}

pub struct Config {
    // ... non-provider fields unchanged ...

    /// Unified provider credentials.
    pub providers: Vec<ProviderKeyEntry>,
}
```

Changes:
- `name` changes from `Option<String>` to `String` (required)
- Add `format: Format` field
- Delete `claude_api_key`, `openai_api_key`, `gemini_api_key`, `openai_compatibility` fields
- Replace with single `providers: Vec<ProviderKeyEntry>`
- `all_provider_keys()` returns `&self.providers` directly
- `sanitize()` / `normalize()` operate on `self.providers` once (not 4 times)
- `validate()` adds: check `name` uniqueness, check `format` is valid

### Config validation

```rust
fn validate(&self) -> Result<(), anyhow::Error> {
    // Existing validations...

    // Provider name uniqueness
    let mut seen = HashSet::new();
    for entry in &self.providers {
        anyhow::ensure!(
            seen.insert(&entry.name),
            "duplicate provider name: {}",
            entry.name
        );
    }

    Ok(())
}
```

### Default base URL resolution

No longer stored on executor. When building AuthRecord:

```rust
fn build_auth_record(entry: &ProviderKeyEntry, cb_config: &CircuitBreakerConfig) -> AuthRecord {
    AuthRecord {
        provider: entry.format,
        provider_name: entry.name.clone(),
        base_url: entry.base_url.clone(),
        // base_url_or_default() uses format.default_base_url() as fallback
        ...
    }
}
```

## 3. AuthRecord (crates/core)

### `crates/core/src/provider.rs`

```rust
pub struct AuthRecord {
    pub id: String,
    pub provider: Format,              // wire protocol
    pub provider_name: String,         // NEW: provider identity from config
    pub api_key: String,
    pub base_url: Option<String>,
    // ... rest unchanged ...
}

impl AuthRecord {
    /// Resolve base URL: entry-level override > format default.
    pub fn base_url_or_default(&self, fallback: &str) -> &str {
        self.base_url.as_deref().unwrap_or(fallback)
    }

    /// Resolve base URL using the format's default.
    pub fn resolved_base_url(&self) -> &str {
        self.base_url
            .as_deref()
            .unwrap_or_else(|| self.provider.default_base_url())
    }
}
```

Changes:
- Add `provider_name: String` field
- Add `resolved_base_url()` convenience method
- Remove all test code that uses `Format::OpenAICompat`

## 4. CredentialRouter (crates/provider)

### `crates/provider/src/routing.rs`

**Core change: group by provider name, not Format.**

```rust
pub struct CredentialRouter {
    /// provider_name -> Vec<AuthRecord>
    credentials: RwLock<HashMap<String, Vec<AuthRecord>>>,
    /// credential_id -> (provider_name, index)
    credential_index: RwLock<HashMap<String, (String, usize)>>,
    counters: RwLock<HashMap<String, AtomicUsize>>,
    strategy: RwLock<CredentialStrategy>,
    latency_ewma: RwLock<HashMap<String, f64>>,
    ewma_alpha: RwLock<f64>,
    cb_config: RwLock<CircuitBreakerConfig>,
    cooldowns: DashMap<String, QuotaCooldown>,
}
```

```rust
impl CredentialRouter {
    /// Pick credential by provider name.
    pub fn pick(
        &self,
        provider_name: &str,
        model: &str,
        tried: &[String],
        _client_region: Option<&str>,
        allowed_credentials: &[String],
    ) -> Option<AuthRecord> {
        let creds = self.credentials.read().ok()?;
        let entries = creds.get(provider_name)?;
        // ... filter + strategy selection (unchanged logic) ...
    }

    pub fn update_from_config(&self, config: &Config) {
        let mut map: HashMap<String, Vec<AuthRecord>> = HashMap::new();
        let cb_config = config.circuit_breaker.clone();

        for entry in &config.providers {
            let auth = build_auth_record(entry, &cb_config);
            map.entry(entry.name.clone()).or_default().push(auth);
        }

        // Preserve circuit breaker state (unchanged logic)...
        // Rebuild credential index...
    }

    pub fn resolve_providers(&self, model: &str) -> Vec<(String, Format)> {
        // Returns (provider_name, format) pairs that support the model
        let mut result = Vec::new();
        if let Ok(creds) = self.credentials.read() {
            for (name, entries) in creds.iter() {
                for auth in entries {
                    if auth.is_available() && auth.supports_model(model) {
                        result.push((name.clone(), auth.provider));
                        break;
                    }
                }
            }
        }
        result
    }

    /// Credential map keyed by provider name (for ProviderCatalog).
    pub fn credential_map(&self) -> HashMap<String, Vec<AuthRecord>> {
        self.credentials.read().map(|c| c.clone()).unwrap_or_default()
    }
}
```

`pick_round_robin` key changes from `format:model` to `provider_name:model`.

## 5. ProviderCatalog (crates/provider)

### `crates/provider/src/catalog.rs`

```rust
pub fn update_from_credentials(&self, credentials: &HashMap<String, Vec<AuthRecord>>) {
    let mut providers = Vec::new();
    for (provider_name, records) in credentials {
        if records.is_empty() {
            continue;
        }
        providers.push(CatalogProvider {
            format: records[0].provider,  // all records in same group share format
            name: provider_name.clone(),  // from config, not format.as_str()
            credentials: records.iter().map(|r| CatalogCredential { record: r.clone() }).collect(),
        });
    }
    // ...
}
```

The InventorySnapshot now has provider entries with user-defined names. The route planner uses these names for provider-pin matching and policy weights.

## 6. ExecutorRegistry (crates/provider)

### `crates/provider/src/lib.rs`

```rust
pub fn build_registry(global_proxy: Option<String>, client_pool: Arc<HttpClientPool>) -> ExecutorRegistry {
    let mut executors: HashMap<String, Arc<dyn ProviderExecutor>> = HashMap::new();

    // One executor per wire format
    let openai = openai_compat::OpenAICompatExecutor {
        name: "openai".to_string(),
        format: Format::OpenAI,
        global_proxy: global_proxy.clone(),
        client_pool: client_pool.clone(),
    };
    executors.insert("openai".to_string(), Arc::new(openai));

    let claude = claude::ClaudeExecutor::new(global_proxy.clone(), client_pool.clone());
    executors.insert("claude".to_string(), Arc::new(claude));

    let gemini = gemini::GeminiExecutor::new(global_proxy.clone(), client_pool.clone());
    executors.insert("gemini".to_string(), Arc::new(gemini));

    // NO openai-compat executor — Format::OpenAI handles all OpenAI-protocol providers

    ExecutorRegistry { executors }
}
```

Delete `crates/provider/src/openai.rs` (the thin wrapper). OpenAICompatExecutor is the only OpenAI executor.

### OpenAICompatExecutor changes

Remove `default_base_url` field. Use `auth.resolved_base_url()` instead:

```rust
pub struct OpenAICompatExecutor {
    pub name: String,
    pub format: Format,
    pub global_proxy: Option<String>,
    pub client_pool: Arc<HttpClientPool>,
}

impl ProviderExecutor for OpenAICompatExecutor {
    async fn execute(&self, auth: &AuthRecord, request: ProviderRequest) -> Result<ProviderResponse, ProxyError> {
        let base_url = auth.resolved_base_url();
        // ... rest unchanged ...
    }
}
```

## 7. TranslatorRegistry (crates/translator)

### `crates/translator/src/lib.rs`

```rust
pub fn build_registry() -> TranslatorRegistry {
    let mut reg = TranslatorRegistry::new();

    // 4 translation paths (down from 6)
    reg.register(Format::OpenAI, Format::Claude, ...);           // OpenAI -> Claude
    reg.register(Format::OpenAI, Format::Gemini, ...);           // OpenAI -> Gemini
    reg.register(Format::Gemini, Format::OpenAI, ...);           // Gemini -> OpenAI
    reg.register_request(Format::Gemini, Format::Claude, ...);   // Gemini -> Claude (chained)

    // DELETED:
    // - OpenAI -> OpenAICompat (redundant: from == to passthrough handles it)
    // - Gemini -> OpenAICompat (duplicate of Gemini -> OpenAI)

    reg
}
```

## 8. Dispatch Layer (crates/server)

### `dispatch/features.rs`

```rust
pub(super) fn extract_features(req: &DispatchRequest) -> RouteRequestFeatures {
    let endpoint = match req.source_format {
        Format::Claude => RouteEndpoint::Messages,
        Format::OpenAI => RouteEndpoint::ChatCompletions,
        Format::Gemini => RouteEndpoint::ChatCompletions,
    };
    // ...
}
```

### `dispatch/executor.rs`

```rust
// Line 294: stream usage injection
if req.stream && target_format == Format::OpenAI {
    inject_stream_usage_option(translated_payload)
}
```

RouteAttemptPlan changes:

```rust
pub struct RouteAttemptPlan {
    pub model: String,
    pub provider: Format,           // wire protocol (for executor selection)
    pub provider_name: String,      // NEW: provider identity (for credential lookup)
    pub credential_id: String,
}
```

ExecutionController uses `provider_name` to find credentials and `provider` (Format) to find executor:

```rust
let auth = self.state.router
    .find_credential(&attempt.credential_id)
    .ok_or_else(|| ProxyError::NoCredentials { ... })?;

let executor = self.state.executors
    .get_by_format(attempt.provider)  // still by Format
    .ok_or_else(|| ProxyError::Internal(...))?;
```

### `handler/responses.rs`

```rust
// Line 40: validate format supports Responses API
let target_format = providers
    .iter()
    .find(|f| *f == Format::OpenAI)  // just OpenAI, no OpenAICompat
    .ok_or_else(|| ...)?;
```

## 9. Dashboard API (crates/server/handler/dashboard)

### `providers.rs` — Complete redesign

Provider ID is the `name` field (not `{type}-{index}`):

```
GET    /api/dashboard/providers              → list all
POST   /api/dashboard/providers              → create (body includes name + format)
GET    /api/dashboard/providers/{name}        → get by name
PATCH  /api/dashboard/providers/{name}        → update by name
DELETE /api/dashboard/providers/{name}        → delete by name
```

**list_providers**: Iterate `config.providers`, return each with `name`, `format`, masked key, etc.

**create_provider**: Validate `name` uniqueness and `format` validity. Push to `config.providers`.

**get_provider**: Find by `name` in `config.providers`.

**update_provider**: Find by `name`, update fields in place.

**delete_provider**: Remove by `name` from `config.providers`.

No more `provider_type_to_field()` or type-to-config-section mapping. All operations work directly on one array.

Response shape:

```json
{
  "name": "deepseek",
  "format": "openai",
  "base_url": "https://api.deepseek.com",
  "api_key_masked": "sk-...abc",
  "models": [{"id": "deepseek-chat"}],
  "wire_api": "chat",
  "weight": 1,
  "disabled": false
}
```

### `routing.rs`

PreviewRequest: `source_format` field maps to Format (3 variants, no "openai-compat").

### `config_ops.rs`

Provider counts:
```json
{
  "providers": 6,
  "providers_by_format": {"openai": 3, "claude": 2, "gemini": 1}
}
```

### `system.rs`

System health iterates `config.providers` grouped by format.

### `admin.rs`

```json
{
  "provider_count": 6,
  "providers_by_format": {"openai": 3, "claude": 2, "gemini": 1}
}
```

## 10. Frontend (web/)

### `types/index.ts`

```typescript
export type ProviderFormat = 'openai' | 'claude' | 'gemini';

export interface Provider {
  name: string;                          // unique identifier
  format: ProviderFormat;                // wire protocol
  base_url: string | null;
  proxy_url: string | null;
  api_key_masked: string;
  api_key?: string;
  prefix: string | null;
  disabled: boolean;
  models: ModelMapping[];
  models_count: number;
  excluded_models: string[];
  headers?: Record<string, string>;
  wire_api: 'chat' | 'responses';
  weight: number;
  region: string | null;
}

export interface ProviderCreateRequest {
  name: string;                          // required
  format: ProviderFormat;                // required
  api_key: string;
  base_url?: string;
  // ... rest unchanged
}
```

### `pages/Providers.tsx`

```typescript
const FORMAT_OPTIONS = [
  { value: 'openai', label: 'OpenAI Protocol' },
  { value: 'claude', label: 'Claude Protocol' },
  { value: 'gemini', label: 'Gemini Protocol' },
];

const DEFAULT_BASE_URLS: Record<ProviderFormat, string> = {
  openai: 'https://api.openai.com',
  claude: 'https://api.anthropic.com',
  gemini: 'https://generativelanguage.googleapis.com',
};
```

Form changes:
- **Name** field: always visible, required, text input (replaces auto-generated `{type}-{index}`)
- **Format** dropdown: 3 options (replaces Provider Type with 4 options)
- **Base URL**: always visible with format-specific default as placeholder
- **Wire API**: shown when `format === 'openai'` (not just "openai-compat")
- Provider type dropdown **editable** during edit (changing format is valid since it's explicit now; old "lock type" was because type determined config section — no longer an issue)

Table columns:
- Name (primary identifier, replaces ID)
- Format (badge: OpenAI / Claude / Gemini)
- Base URL
- Models
- Status
- Actions

### `pages/Routing.tsx`

RoutePreview shows provider `name` (not format string).
Profile editor `order` and `weights` reference provider names.

### `pages/RequestLogs.tsx`

Provider filter uses provider names from logs.
Provider badge shows name (e.g., "deepseek") not format.

### `pages/Dashboard.tsx`

Provider distribution chart uses provider names for labels.

## 11. Route Planner (crates/core/routing)

### `planner.rs`

ProviderEntry already has `name: String` and `format: Format`. No structural change needed. But the provider pin matching now matches against user-defined names:

```rust
// collect_candidates
if pinned_providers
    .as_ref()
    .is_some_and(|pins| !pins.iter().any(|p| glob_match(p, &provider.name)))
{
    // provider.name is now "deepseek", "openai", etc. instead of "openai-compat"
    continue;
}
```

RouteAttemptPlan gains `provider_name`:

```rust
pub struct RouteAttemptPlan {
    pub model: String,
    pub provider: Format,
    pub provider_name: String,   // NEW
    pub credential_id: String,
}
```

### `types.rs`

InventorySnapshot and ProviderEntry — no structural changes (already have `name: String` and `format: Format`).

## 12. Test Changes

### Tests to delete

- `test_registry_openai_compat_passthrough` (translator/lib.rs)
- `test_roundtrip_openai_compat_passthrough` (translator/tests/roundtrip_tests.rs)
- `test_create_openai_compat_provider` (server/tests/dashboard_tests.rs)
- `test_has_response_translator` assertion for OpenAICompat (translator/lib.rs)
- `test_build_registry_has_all_paths` — update counts (6→4 requests, 5→3 responses)
- TypeScript test `passes openai-compat provider type directly` (web/__tests__)

### Tests to update

**Provider routing tests** (`crates/provider/src/routing.rs`):
- `make_auth()` and `setup_router()` — key by provider name instead of Format
- All `pick(Format::OpenAI, ...)` calls → `pick("openai", ...)`
- `test_resolve_providers()` — remove `Format::OpenAICompat` assertion
- All AuthRecord constructions — add `provider_name` field

**Catalog tests** (`crates/provider/src/catalog.rs`):
- `test_record()` — add `provider_name` field
- `test_catalog_snapshot()` — provider name from config, not format.as_str()

**Planner tests** (`crates/core/src/routing/planner.rs`):
- `test_features()` helper — no change (already uses Format::OpenAI)
- `test_inventory()` — provider names are user-defined strings
- `test_plan_provider_pin_excludes` — pin uses provider name

**Config tests** (`crates/core/src/config.rs`):
- ProviderKeyEntry construction — add `name` and `format` fields
- Delete tests that reference 4 separate arrays

**Dashboard tests** (`crates/server/tests/dashboard_tests.rs`):
- Provider CRUD tests — use `name` as identifier, `format` in create body
- Update assertions for new JSON response shape

**E2E tests** (`tests/e2e/`):
- `helpers.rs` — `build_*_config()` functions use `Config { providers: vec![...] }`
- `config.e2e.yaml` — rewrite to `providers:` format

### Tests to add

- Config validation: duplicate provider name rejected
- Config validation: missing name/format rejected
- Provider CRUD with OpenAI-format provider at custom base_url (replaces openai-compat test)
- Routing with multiple providers sharing same format but different names
- Provider pin with glob matching on provider names
- Format::from_str rejects "openai-compat"

## 13. Documentation Changes

All references to `openai-compat`, `openai_compat`, `openai_compatibility`, `Format::OpenAICompat` in:

- `CLAUDE.md` / `AGENTS.md` — update Provider Matrix, Format table
- `docs/reference/types/enums.md` — remove OpenAICompat
- `docs/reference/types/provider.md` — remove openai-compat executor, update translation matrix
- `docs/reference/types/config.md` — new `providers` field, remove 4 old fields
- `docs/reference/api-surface.md` — update endpoint descriptions
- `docs/playbooks/add-provider.md` — simplified (just add entry to `providers` array)
- `config.example.yaml` — rewrite entirely
- Completed specs (SPEC-001, SPEC-004, etc.) — update where referenced

## 14. Files Changed Summary

| File | Change Type |
|------|-------------|
| `crates/types/src/format.rs` | Modify (delete OpenAICompat, add default_base_url) |
| `crates/core/src/config.rs` | Major (unified providers, validation) |
| `crates/core/src/provider.rs` | Modify (add provider_name to AuthRecord) |
| `crates/core/src/routing/planner.rs` | Modify (RouteAttemptPlan gains provider_name) |
| `crates/core/src/routing/types.rs` | Modify (RouteAttemptPlan) |
| `crates/core/src/routing/explain.rs` | Modify (test data) |
| `crates/core/src/routing/match_engine.rs` | Modify (test data) |
| `crates/core/src/routing/model_resolver.rs` | No change |
| `crates/provider/src/lib.rs` | Modify (delete openai-compat executor) |
| `crates/provider/src/openai.rs` | Delete |
| `crates/provider/src/openai_compat.rs` | Modify (remove default_base_url field) |
| `crates/provider/src/routing.rs` | Major (key by provider name) |
| `crates/provider/src/catalog.rs` | Modify (name from config) |
| `crates/translator/src/lib.rs` | Modify (delete 2 translation paths) |
| `crates/translator/tests/roundtrip_tests.rs` | Modify (delete openai-compat test) |
| `crates/server/src/dispatch/features.rs` | Modify (delete OpenAICompat branch) |
| `crates/server/src/dispatch/executor.rs` | Modify (use provider_name for credential lookup) |
| `crates/server/src/handler/dashboard/providers.rs` | Major (name-based CRUD) |
| `crates/server/src/handler/dashboard/routing.rs` | Modify (delete openai-compat) |
| `crates/server/src/handler/dashboard/config_ops.rs` | Modify (new summary format) |
| `crates/server/src/handler/dashboard/system.rs` | Modify (iterate providers) |
| `crates/server/src/handler/admin.rs` | Modify (new summary format) |
| `crates/server/src/handler/provider_scoped.rs` | Modify (minor) |
| `crates/server/src/app.rs` | Modify (logging, init) |
| `crates/server/tests/dashboard_tests.rs` | Major (rewrite provider tests) |
| `tests/e2e/helpers.rs` | Modify (new config builders) |
| `tests/e2e-docker/config.e2e.yaml` | Rewrite |
| `web/src/types/index.ts` | Modify |
| `web/src/pages/Providers.tsx` | Major (form + table redesign) |
| `web/src/pages/Routing.tsx` | Minor |
| `web/src/pages/RequestLogs.tsx` | Minor (display) |
| `web/src/pages/Dashboard.tsx` | Minor (chart labels) |
| `web/src/__tests__/services/api.test.ts` | Modify |
| `config.example.yaml` | Rewrite |
| `docs/` (10+ files) | Update references |
| `CLAUDE.md` | Update |

## 15. Implementation Order

Execute in dependency order to keep the codebase compiling at each step:

1. **Types** — `format.rs`: delete OpenAICompat, add `default_base_url()`. (Breaks compilation everywhere — proceed immediately to step 2.)

2. **Core types** — `provider.rs`: add `provider_name` to AuthRecord. `config.rs`: add `providers` field + `format`/`name` to ProviderKeyEntry, delete old 4 fields.

3. **Provider layer** — `routing.rs`: key by name. `catalog.rs`: name from config. `lib.rs`: delete openai-compat executor, delete `openai.rs`. `openai_compat.rs`: remove default_base_url.

4. **Translator** — delete 2 redundant translation paths, update test counts.

5. **Server/dispatch** — update features.rs, executor.rs, responses handler. Use provider_name in attempt plans.

6. **Dashboard handlers** — rewrite providers.rs (name-based CRUD), update routing.rs, config_ops.rs, system.rs, admin.rs.

7. **Tests** — update all test helpers, delete OpenAICompat-specific tests, add new tests.

8. **Frontend** — types, Providers page, Routing page, RequestLogs, Dashboard.

9. **Config & docs** — config.example.yaml, config.e2e.yaml, CLAUDE.md, all docs/.

Steps 1-5 can be done as one atomic commit (they're tightly coupled).
Steps 6-7 as a second commit.
Steps 8-9 as a third commit.
