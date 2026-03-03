# Technical Design: Request Cloaking & Payload Rules

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-007                           |
| Title     | Request Cloaking & Payload Rules   |
| Author    | Prism Team                      |
| Status    | Completed                          |
| Created   | 2026-02-27                         |
| Updated   | 2026-02-27                         |

## Overview

Request cloaking and payload rules provide two complementary request modification systems. Cloaking disguises third-party Claude API requests as Claude Code requests (injecting system prompts, user IDs, and obfuscating sensitive words). Payload rules allow operators to set defaults, force overrides, and filter fields on a per-model basis with glob matching. Both systems operate on the translated JSON payload before it is sent upstream. See PRD (SPEC-007) for requirements.

## Backend Implementation

### Module Structure

```
crates/core/src/cloak.rs        -- CloakConfig, CloakMode, should_cloak, apply_cloak, obfuscation
crates/core/src/payload.rs      -- PayloadConfig, PayloadRule, FilterRule, ModelMatcher, apply_payload_rules
crates/core/src/glob.rs         -- glob_match utility (used by ModelMatcher)
crates/server/src/dispatch.rs   -- Integration: payload rules applied first, then cloaking
```

### Execution Order in dispatch()

```
1. Translate request (source format -> target format)
2. Apply payload rules (default -> override -> filter)
3. Apply cloaking (if target_format == Claude && should_cloak)
4. Send to upstream
```

---

## Part 1: Request Cloaking

### CloakConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct CloakConfig {
    pub mode: CloakMode,              // Auto | Always | Never (default: Never)
    pub strict_mode: bool,            // default: false
    pub sensitive_words: Vec<String>,  // default: []
    pub cache_user_id: bool,          // default: false
}
```

CloakConfig is defined per `ProviderKeyEntry`, not globally. Each Claude API key can have its own cloaking settings.

### CloakMode

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CloakMode {
    Auto,    // cloak unless client is Claude CLI/Code
    Always,  // always cloak
    Never,   // never cloak (default)
}
```

### should_cloak()

```rust
pub fn should_cloak(cloak_cfg: &CloakConfig, user_agent: Option<&str>) -> bool {
    match cloak_cfg.mode {
        CloakMode::Always => true,
        CloakMode::Never => false,
        CloakMode::Auto => {
            // Skip cloaking for native Claude CLI/Code clients
            !user_agent
                .map(|ua| ua.starts_with("claude-cli") || ua.starts_with("claude-code"))
                .unwrap_or(false)
        }
    }
}
```

**Auto mode logic:** If User-Agent starts with `"claude-cli"` or `"claude-code"`, the request is assumed to be from a native Claude client and cloaking is skipped. All other User-Agents (or absent User-Agent) trigger cloaking.

### apply_cloak()

```rust
pub fn apply_cloak(body: &mut serde_json::Value, cloak_cfg: &CloakConfig, api_key: &str)
```

Performs three modifications to the Claude Messages API request body:

#### 1. System Prompt Injection

Uses a constant `CLOAK_SYSTEM_PROMPT`:
```
"You are Claude Code, Anthropic's official CLI for Claude. You are an interactive agent
specialized in software engineering tasks. You help users with coding, debugging, and
software development."
```

- **strict_mode = false (default):** Prepends the cloak prompt to the existing system prompt, separated by `\n\n`. If no existing system prompt, uses the cloak prompt alone.
- **strict_mode = true:** Replaces the entire system prompt with the cloak prompt, discarding the user's original system prompt.

#### 2. Fake user_id Generation

```rust
pub fn generate_user_id(api_key: &str, cache: bool) -> String
```

Generates a user ID in the format: `user_{64_hex_chars}_account__session_{uuid}`

- **cache_user_id = false:** Generates a new random user_id on every request
- **cache_user_id = true:** Caches the generated user_id per API key in a global `LazyLock<Mutex<HashMap>>`. Same API key always gets the same user_id.

The user_id is injected into `metadata.user_id` in the request body, creating the `metadata` object if it does not exist.

#### 3. Sensitive Word Obfuscation

```rust
fn obfuscate_sensitive_words(body: &mut serde_json::Value, words: &[String])
```

- Builds a single case-insensitive regex from all sensitive words: `(?i)(word1|word2|...)`
- Walks through `messages` and `system` fields recursively
- In object values, only processes `text` and `content` keys (not structural keys)
- For each match, inserts a Unicode zero-width space (`\u{200B}`) after the first character: `"API"` becomes `"A\u{200B}PI"`
- This breaks exact string matching by providers while preserving readability for the model

### Claude Header Defaults

When cloaking is active, `claude_header_defaults` from config are injected into the upstream request headers:

```rust
// In dispatch.rs
if should_cloak(cloak_cfg, user_agent) {
    for (k, v) in &config.claude_header_defaults {
        request_headers.insert(k.clone(), v.clone());
    }
}
```

---

## Part 2: Payload Rules

### PayloadConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PayloadConfig {
    pub default: Vec<PayloadRule>,    // set field if missing
    pub r#override: Vec<PayloadRule>, // always set field
    pub filter: Vec<FilterRule>,      // remove fields
}
```

### PayloadRule

```rust
pub struct PayloadRule {
    pub models: Vec<ModelMatcher>,                    // which models this rule applies to
    pub params: serde_json::Map<String, Value>,       // dot-separated path -> value
}
```

### FilterRule

```rust
pub struct FilterRule {
    pub models: Vec<ModelMatcher>,   // which models this rule applies to
    pub params: Vec<String>,         // dot-separated paths to remove
}
```

### ModelMatcher

```rust
pub struct ModelMatcher {
    pub name: String,              // glob pattern (e.g., "gemini-*", "gpt-4*", "*")
    pub protocol: Option<String>,  // optional protocol filter (e.g., "openai", "claude", "gemini")
}
```

**Matching logic:**

```rust
fn matches_rule(matchers: &[ModelMatcher], model: &str, protocol: Option<&str>) -> bool {
    matchers.iter().any(|m| {
        let name_match = glob_match(&m.name, model);
        let protocol_match = m.protocol.as_ref().map_or(true, |p| {
            protocol.map_or(false, |actual| actual.eq_ignore_ascii_case(p))
        });
        name_match && protocol_match
    })
}
```

- `name`: Uses glob matching (`*` wildcard). `"gemini-*"` matches `"gemini-2.5-pro"`, `"*"` matches all models.
- `protocol`: Optional filter. If set, the rule only applies when the target wire protocol matches (case-insensitive). If not set, the rule applies to all protocols.

### apply_payload_rules()

```rust
pub fn apply_payload_rules(
    body: &mut Value,
    config: &PayloadConfig,
    model: &str,
    protocol: Option<&str>,
)
```

Execution order is fixed:

#### Step 1: Defaults (set if missing)

```rust
for rule in &config.default {
    if matches_rule(&rule.models, model, protocol) {
        for (path, value) in &rule.params {
            set_nested(body, path, value.clone(), /* only_if_missing */ true);
        }
    }
}
```

Sets a value at a dot-separated path only if the field does not already exist. Creates intermediate objects as needed.

#### Step 2: Overrides (always set)

```rust
for rule in &config.r#override {
    if matches_rule(&rule.models, model, protocol) {
        for (path, value) in &rule.params {
            set_nested(body, path, value.clone(), /* only_if_missing */ false);
        }
    }
}
```

Always sets the value, overwriting any existing value at that path.

#### Step 3: Filters (remove fields)

```rust
for rule in &config.filter {
    if matches_rule(&rule.models, model, protocol) {
        for path in &rule.params {
            remove_nested(body, path);
        }
    }
}
```

Removes the field at the dot-separated path. If intermediate objects become empty, they are left in place (not cleaned up).

### Dot-Separated Path Operations

#### set_nested(root, path, value, only_if_missing)

Traverses a dot-separated path (e.g., `"generationConfig.thinkingConfig.thinkingBudget"`), creating intermediate JSON objects as needed, and sets the value at the final key.

#### remove_nested(root, path)

Traverses a dot-separated path and removes the final key from its parent object.

### Integration in dispatch()

```rust
// In dispatch.rs, after request translation:
let translated_payload = {
    let mut payload_value: serde_json::Value = serde_json::from_slice(&translated_payload)
        .unwrap_or_else(|_| serde_json::Value::Null);
    if payload_value.is_object() {
        prism_core::payload::apply_payload_rules(
            &mut payload_value,
            &config.payload,
            &actual_model,
            Some(target_format.as_str()),
        );
        serde_json::to_vec(&payload_value).unwrap_or(translated_payload)
    } else {
        translated_payload
    }
};
```

- Payload rules are applied after format translation but before cloaking
- Only applied if the payload deserializes to a JSON object
- Falls back to the original payload on deserialization failure

## Configuration Changes

```yaml
# Cloaking (per Claude API key entry)
claude-api-key:
  - api-key: "sk-ant-xxx"
    cloak:
      mode: auto             # auto | always | never
      strict-mode: false
      sensitive-words:
        - "CompanyName"
        - "internal-project"
      cache-user-id: true

# Claude header defaults (injected during cloaking)
claude-header-defaults:
  anthropic-beta: "some-beta-feature"

# Payload rules (global)
payload:
  default:
    - models:
        - name: "gemini-*"
      params:
        generationConfig.thinkingConfig.thinkingBudget: 32768
  override:
    - models:
        - name: "gpt-*"
          protocol: "openai"
      params:
        reasoning.effort: "high"
  filter:
    - models:
        - name: "gemini-*"
      params:
        - "generationConfig.responseJsonSchema"
```

## Provider Compatibility

| Provider | Cloaking | Payload Rules | Notes |
|----------|----------|---------------|-------|
| OpenAI   | N/A      | Yes           | Override/filter params for GPT models |
| Claude   | Yes      | Yes           | Cloaking is Claude-specific; payload rules also apply |
| Gemini   | N/A      | Yes           | Default thinking budget, filter unsupported fields |
| Compat   | N/A      | Yes           | Same payload rule engine |

## Test Strategy

- **Unit tests:**
  - `test_should_cloak_auto` -- Auto mode skips claude-cli, cloaks other UAs
  - `test_should_cloak_always_never` -- Always/Never modes
  - `test_generate_user_id_format` -- user_id format validation
  - `test_generate_user_id_caching` -- cached IDs are consistent
  - `test_apply_cloak_system_prompt` -- prepend mode
  - `test_apply_cloak_strict_mode` -- replace mode
  - `test_obfuscate_sensitive_words` -- zero-width space insertion
  - `test_user_id_in_metadata` -- metadata.user_id injection
  - `test_default_sets_missing` -- default rule sets absent field
  - `test_default_does_not_overwrite` -- default rule skips existing field
  - `test_override_always_sets` -- override rule overwrites existing
  - `test_filter_removes_fields` -- filter removes nested field
  - `test_protocol_filter` -- protocol matching in rules
- **Manual verification:** Enable cloaking, send request via non-Claude client, verify system prompt and user_id in upstream request logs
