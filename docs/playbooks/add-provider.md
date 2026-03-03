# Playbook: Add a New Provider

Step-by-step guide for adding a new AI provider to the Prism.

## Overview

The proxy uses a provider/executor architecture. Each provider has:

- A `Format` enum variant identifying its API format
- A config section for API key entries
- An executor struct implementing the `ProviderExecutor` trait
- Registration in the `ExecutorRegistry`
- Optionally, translators for format conversion

## Steps

### 1. Add a Format Variant

In `crates/core/src/provider.rs`, add a new variant to the `Format` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    OpenAI,
    Claude,
    Gemini,
    OpenAICompat,
    NewProvider, // <-- add here
}
```

Then update the associated methods:

- `Format::as_str()` -- add a match arm returning the kebab-case string (e.g., `"new-provider"`)
- `Format::from_str()` -- add parsing for the string representation
- `Format::fmt()` -- uses `as_str()`, so no change needed

### 2. Add Provider Key Config Field

In `crates/core/src/config.rs`, add a new credential field to the `Config` struct:

```rust
pub struct Config {
    // ... existing fields ...
    pub new_provider_api_key: Vec<ProviderKeyEntry>,
}
```

Update `Default for Config` to initialize it:

```rust
new_provider_api_key: Vec::new(),
```

The `ProviderKeyEntry` struct is shared across all providers and includes fields for `api_key`, `base_url`, `proxy_url`, `prefix`, `models`, `excluded_models`, `headers`, `disabled`, `name`, `cloak`, and `wire_api`.

### 3. Wire Up Config Methods

In `Config::sanitize()`, add:

```rust
sanitize_entries(&mut self.new_provider_api_key);
```

In `Config::all_provider_keys()`, chain the new field:

```rust
pub fn all_provider_keys(&self) -> impl Iterator<Item = &ProviderKeyEntry> {
    self.claude_api_key
        .iter()
        .chain(self.openai_api_key.iter())
        .chain(self.gemini_api_key.iter())
        .chain(self.openai_compatibility.iter())
        .chain(self.new_provider_api_key.iter()) // <-- add
}
```

### 4. Create the Executor

Create `crates/provider/src/new_provider.rs`. Use an existing executor as a reference (e.g., `claude.rs` for a bespoke API, or `openai_compat.rs` for an OpenAI-compatible API).

The executor must implement the `ProviderExecutor` trait:

```rust
use prism_core::error::ProxyError;
use prism_core::provider::*;
use async_trait::async_trait;
use crate::common;

const DEFAULT_BASE_URL: &str = "https://api.newprovider.com";

pub struct NewProviderExecutor {
    pub global_proxy: Option<String>,
}

impl NewProviderExecutor {
    pub fn new(global_proxy: Option<String>) -> Self {
        Self { global_proxy }
    }
}

#[async_trait]
impl ProviderExecutor for NewProviderExecutor {
    fn identifier(&self) -> &str {
        "new-provider"
    }

    fn native_format(&self) -> Format {
        Format::NewProvider
    }

    fn default_base_url(&self) -> &str {
        DEFAULT_BASE_URL
    }

    async fn execute(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<ProviderResponse, ProxyError> {
        let base_url = auth.base_url_or_default(DEFAULT_BASE_URL);
        let url = format!("{base_url}/v1/completions");

        let client = common::build_client(auth, self.global_proxy.as_deref())?;
        let req = client
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", auth.api_key))
            .body(request.payload.to_vec());
        let req = common::apply_headers(req, &request.headers, auth);

        let (body, headers) = common::handle_response(req.send().await?).await?;
        Ok(ProviderResponse { payload: body, headers })
    }

    async fn execute_stream(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<StreamResult, ProxyError> {
        let base_url = auth.base_url_or_default(DEFAULT_BASE_URL);
        let url = format!("{base_url}/v1/completions");

        let client = common::build_client(auth, self.global_proxy.as_deref())?;
        let req = client
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", auth.api_key))
            .body(request.payload.to_vec());
        let req = common::apply_headers(req, &request.headers, auth);

        common::handle_stream_response(req.send().await?).await
    }

    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo> {
        common::supported_models_from_auth(auth, "new-provider", "new-provider-org")
    }
}
```

Key points:

- Use `common::build_client()` to create an HTTP client (handles proxy configuration).
- Use `common::apply_headers()` to apply custom headers from config.
- Use `common::handle_response()` and `common::handle_stream_response()` for response processing.
- Use `common::supported_models_from_auth()` to build the model list from `AuthRecord`.

### 5. Register the Executor

In `crates/provider/src/lib.rs`:

1. Add the module declaration:
   ```rust
   pub mod new_provider;
   ```

2. Register in `build_registry()`:
   ```rust
   let new_provider = new_provider::NewProviderExecutor::new(global_proxy.clone());
   executors.insert("new-provider".to_string(), Arc::new(new_provider));
   ```

### 6. Add Request/Response Types (If Needed)

If the provider uses a unique API format, add request/response types in `crates/core/src/types/`. These are used by translators to convert between formats.

### 7. Add Translators (If Needed)

If the new provider's native format differs from OpenAI (the proxy's canonical incoming format), you need translators. See the [add-translator.md](add-translator.md) playbook for details.

Translators are needed when:
- Clients send OpenAI-format requests to a non-OpenAI provider
- Responses from the provider need to be converted back to OpenAI format

If the provider is OpenAI-compatible (uses the same request/response format), no translator is needed -- use `Format::OpenAICompat` instead.

### 8. Update AuthRecord Building

Ensure the server's credential-building logic (which converts `ProviderKeyEntry` to `AuthRecord`) includes the new provider. The `AuthRecord` is built in `crates/server/` where config entries are mapped to `Format` variants.

### 9. Add Tests

- Unit tests for the executor in `crates/provider/src/new_provider.rs`
- Integration tests if applicable
- Run `make test` to verify everything passes

### 10. Update Documentation

- Create a spec for the new provider (see [create-new-spec.md](create-new-spec.md))
- Update `docs/reference/` with provider-specific details
- Update `AGENTS.md` provider matrix if applicable

## Checklist

- [ ] `Format` enum variant added
- [ ] `Format::as_str()` and `FromStr` updated
- [ ] Config field added to `Config` struct with default
- [ ] `Config::sanitize()` updated
- [ ] `Config::all_provider_keys()` updated
- [ ] Executor struct created implementing `ProviderExecutor`
- [ ] Executor registered in `build_registry()`
- [ ] Translators added (if non-OpenAI format)
- [ ] AuthRecord building updated
- [ ] Tests written and passing
- [ ] Documentation updated
- [ ] `make lint` passes
- [ ] `make test` passes

## Reference: Existing Providers

| Provider       | Format          | Executor File       | Has Translator |
|----------------|-----------------|---------------------|----------------|
| OpenAI         | `OpenAI`        | `openai.rs`         | No (canonical) |
| Claude         | `Claude`        | `claude.rs`         | Yes            |
| Gemini         | `Gemini`        | `gemini.rs`         | Yes            |
| OpenAI-Compat  | `OpenAICompat`  | `openai_compat.rs`  | No             |
