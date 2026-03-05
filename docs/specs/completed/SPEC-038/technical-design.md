# SPEC-038: Technical Design — Unify Provider Request Building

## 1. New Helper in `common.rs`

File: `crates/provider/src/common.rs`

```rust
pub fn build_provider_request(
    client: &reqwest::Client,
    url: &str,
    auth: &AuthRecord,
    payload: &[u8],
    auth_header: AuthHeaderStyle,
) -> reqwest::RequestBuilder {
    let mut req = client
        .post(url)
        .header("content-type", "application/json")
        .body(payload.to_vec());

    req = match auth_header {
        AuthHeaderStyle::Bearer => req.header("authorization", format!("Bearer {}", auth.api_key)),
        AuthHeaderStyle::XApiKey => req.header("x-api-key", &auth.api_key),
        AuthHeaderStyle::XGoogApiKey => req.header("x-goog-api-key", &auth.api_key),
    };

    req = apply_headers(req, &auth.headers);
    req
}

pub enum AuthHeaderStyle {
    Bearer,
    XApiKey,
    XGoogApiKey,
}
```

## 2. Executor Updates

- **Claude** (`claude.rs`): Use `build_provider_request(client, url, auth, payload, AuthHeaderStyle::XApiKey)`
- **Gemini** (`gemini.rs`): Use `build_provider_request(client, url, auth, payload, AuthHeaderStyle::XGoogApiKey)`
- **OpenAICompat** (`openai_compat.rs`): Use `build_provider_request(client, url, auth, payload, AuthHeaderStyle::Bearer)` in both `execute()` and `execute_stream()`

## 3. apply_headers stays in common.rs

Already exists, just ensure all executors use it consistently.
