# SPEC-034: Translator & Server Refactoring

## Problem

Code review identified significant duplication across the translator and server crates:

1. **Translator duplication**: `claude_to_openai.rs` and `gemini_to_openai.rs` duplicate OpenAI chunk/response building patterns 29+ times.
2. **AppState duplicate field**: `router` and `credential_router` point to the same `Arc<CredentialRouter>`.
3. **Handler duplication**: `chat_completions.rs` and `messages.rs` are 95% identical (36 lines each).
4. **Dispatch repetition**: 3 response paths in `dispatch()` repeat `DispatchMeta` injection and debug header insertion.

## Goals

- Extract shared translator helpers into `common.rs` (~130 lines removed, ~80 added)
- Remove duplicate `credential_router` field from `AppState`
- Extract `dispatch_api_request` helper to deduplicate handlers
- Extract `build_json_response` helper in dispatch.rs

## Non-Goals

- Provider routing O(n) optimization (deferred)
- RwLock consolidation in CredentialRouter (deferred)
- dispatch.rs module split (deferred)

## Success Criteria

- All 292 Rust tests pass
- Zero clippy warnings
- Net reduction in lines of code
