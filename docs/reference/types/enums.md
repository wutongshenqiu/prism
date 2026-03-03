# Enums Reference

All public enums used throughout the ai-proxy codebase.

---

## Format

**Source:** `crates/core/src/provider.rs`

Identifies the API format / provider type for request routing and translation.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    OpenAI,
    Claude,
    Gemini,
    OpenAICompat,
}
```

### Serde serialized values

| Variant | Serialized value |
|---------|-----------------|
| `OpenAI` | `"openai"` |
| `Claude` | `"claude"` |
| `Gemini` | `"gemini"` |
| `OpenAICompat` | `"openai-compat"` |

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `as_str` | `fn as_str(&self) -> &'static str` | Returns the kebab-case string. Maps variants to `"openai"`, `"claude"`, `"gemini"`, `"openai-compat"`. |
| `Display` | `impl Display for Format` | Delegates to `as_str()`. |
| `FromStr` | `impl FromStr for Format` | Parses from string. Accepts `"openai-compat"` and `"openai_compat"` for `OpenAICompat`. Returns `Err(String)` for unknown values. |

### Usage context

- `ProviderRequest.source_format` -- the format of the incoming client request
- `CredentialRouter` credential map keys
- `TranslatorRegistry` translation pair keys `(Format, Format)`
- `ProviderExecutor.native_format()` -- the native format a provider executor handles

---

## WireApi

**Source:** `crates/core/src/provider.rs`

Selects the wire API format for OpenAI-compatible providers. Controls whether the provider uses the Chat Completions or Responses endpoint.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireApi {
    #[default]
    Chat,
    Responses,
}
```

### Serde serialized values

| Variant | Serialized value | Note |
|---------|-----------------|------|
| `Chat` | `"chat"` | Default |
| `Responses` | `"responses"` | |

### Usage context

- `ProviderKeyEntry.wire_api` -- per-credential config field
- `AuthRecord.wire_api` -- runtime credential field

---

## RoutingStrategy

**Source:** `crates/core/src/config.rs`

Controls how credentials are selected when multiple credentials can serve a request.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingStrategy {
    RoundRobin,
    FillFirst,
    LatencyAware,
    GeoAware,
}
```

### Serde serialized values

| Variant | Serialized value | Behavior |
|---------|-----------------|----------|
| `RoundRobin` | `"round-robin"` | Distributes requests across credentials using `AtomicUsize` counters per `"provider:model"` key. Default strategy. |
| `FillFirst` | `"fill-first"` | Always picks the first available credential in the list. |
| `LatencyAware` | `"latency-aware"` | Picks the credential with the lowest EWMA latency. Uses `ewma_alpha` smoothing factor from `RoutingConfig`. |
| `GeoAware` | `"geo-aware"` | Prefers credentials whose `region` matches the client's region. Falls back to `default_region` in `RoutingConfig`. |

### YAML example

```yaml
routing:
  strategy: round-robin   # or fill-first, latency-aware, geo-aware
  fallback-enabled: true
  ewma-alpha: 0.3          # for latency-aware
  default-region: us-east   # for geo-aware
```

### Usage context

- `RoutingConfig.strategy` -- configured via YAML
- `CredentialRouter.pick()` -- evaluated at routing time

---

## CloakMode

**Source:** `crates/core/src/cloak.rs`

Controls when cloaking is applied to Claude API requests (system prompt injection, user_id generation, sensitive word obfuscation).

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CloakMode {
    Auto,
    Always,
    Never,
}
```

### Serde serialized values

| Variant | Serialized value | Behavior |
|---------|-----------------|----------|
| `Auto` | `"auto"` | Cloak unless `User-Agent` starts with `"claude-cli"` or `"claude-code"`. |
| `Always` | `"always"` | Always apply cloaking. |
| `Never` | `"never"` | Never apply cloaking. Default. |

### YAML example

```yaml
claude-api-key:
  - api-key: "sk-ant-xxx"
    cloak:
      mode: auto
      strict-mode: false
      sensitive-words: ["API", "proxy"]
      cache-user-id: true
```

### Usage context

- `CloakConfig.mode` -- per-credential cloak config
- `should_cloak()` function evaluates this against `User-Agent`
- `apply_cloak()` injects system prompt, `metadata.user_id`, and obfuscates sensitive words

---

## CircuitState

**Source:** `crates/core/src/circuit_breaker.rs`

The state of a credential's circuit breaker.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}
```

| Variant | Serialized value | Meaning |
|---------|-----------------|---------|
| `Closed` | `"closed"` | Healthy -- requests flow normally. |
| `Open` | `"open"` | Tripped -- requests blocked until cooldown expires. |
| `HalfOpen` | `"half-open"` | Probing -- limited requests allowed to test recovery. |

### Usage context

- `CircuitBreakerPolicy.state()` -- current state of a credential's circuit breaker
- `AuthRecord.circuit_state()` -- convenience accessor

---

## BudgetPeriod

**Source:** `crates/core/src/auth_key.rs`

Budget reset period for per-key cost budgets.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetPeriod {
    Daily,
    Monthly,
}
```

| Variant | Serialized value |
|---------|-----------------|
| `Daily` | `"daily"` |
| `Monthly` | `"monthly"` |

### Usage context

- `BudgetConfig.period` -- in `AuthKeyEntry` budget configuration
