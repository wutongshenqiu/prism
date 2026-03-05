# SPEC-036: Technical Design — RequestContext in ProviderExecutor

## 1. Trait Signature Change

File: `crates/core/src/provider.rs`

```rust
#[async_trait]
pub trait ProviderExecutor: Send + Sync {
    async fn execute(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
        ctx: &RequestContext,  // NEW
    ) -> Result<ProviderResponse, ProxyError>;

    async fn execute_stream(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
        ctx: &RequestContext,  // NEW
    ) -> Result<StreamResult, ProxyError>;

    // unchanged
    fn identifier(&self) -> &str;
    fn native_format(&self) -> Format;
    fn default_base_url(&self) -> &str;
    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo>;
}
```

## 2. Executor Adaptations

All executors receive `ctx` parameter but initially just accept it (no behavioral change):

- `crates/provider/src/claude.rs` — Add `_ctx: &RequestContext`
- `crates/provider/src/gemini.rs` — Add `_ctx: &RequestContext`
- `crates/provider/src/openai_compat.rs` — Add `_ctx: &RequestContext`

## 3. Dispatch Update

File: `crates/server/src/dispatch.rs`

Pass `RequestContext` from dispatch to executor calls. The context is already available in dispatch via the middleware chain.

## 4. Migration

Single atomic change — update trait + all implementations + all call sites in one commit.
