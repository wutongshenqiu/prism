# SPEC-038: Technical Design — Unify Provider Request Building

## 1. Shared Helpers in `common.rs`

File: `crates/provider/src/common.rs`

Existing helpers used by all executors:
- `build_client(auth, global_proxy)` — Creates `reqwest::Client` with optional proxy
- `apply_headers(req, headers, auth)` — Applies custom headers and auth header based on provider format

## 2. Executor Updates

- **Gemini** (`gemini.rs`): Changed from manual header loop to `common::apply_headers(req, &request.headers, auth)` for consistency
- **OpenAICompat** (`openai_compat.rs`): Extracted `build_request()` method to deduplicate request building between `execute()` and `execute_stream()`:

```rust
fn build_request(
    &self,
    auth: &AuthRecord,
    url: &str,
    body: &[u8],
    request_headers: &HashMap<String, String>,
) -> Result<reqwest::RequestBuilder, ProxyError>
```

## 3. Design Decision

Instead of creating a single `build_provider_request()` with an `AuthHeaderStyle` enum (as originally proposed), the implementation kept each executor's existing auth pattern (`apply_headers` handles this) and focused on deduplicating within each executor. This is simpler and avoids an unnecessary abstraction layer.
