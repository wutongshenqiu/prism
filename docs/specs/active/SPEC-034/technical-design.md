# SPEC-034: Technical Design — Translator & Server Refactoring

## 1a. Extract `crates/translator/src/common.rs`

Shared helpers for building OpenAI-format responses:

- `map_claude_finish_reason(reason) -> &str`
- `map_gemini_finish_reason(reason) -> &str`
- `build_openai_chunk(id, created, model, delta, finish_reason) -> Value`
- `build_openai_response(id, created, model, message, finish_reason, usage) -> Value`
- `build_tool_call(id, name, arguments, index) -> Value`
- `build_tool_call_delta(index, id, name, arguments) -> Value`
- `build_assistant_message(content, tool_calls) -> Value`

Files: `common.rs` (new), `lib.rs`, `claude_to_openai.rs`, `gemini_to_openai.rs`

## 1b. Remove duplicate AppState field

Remove `credential_router` from `AppState`. Change `state.credential_router` → `state.router`.

Files: `server/src/lib.rs`, `config_ops.rs`, `src/app.rs`, `dashboard_tests.rs`

## 1c. Deduplicate handler modules

Extract `dispatch_api_request()` in `handler/mod.rs`. Reduce `chat_completions.rs` and `messages.rs` to thin wrappers.

## 1d. Reduce dispatch.rs repetition

Extract `build_json_response()` to deduplicate the non-stream response building pattern repeated at two locations.
