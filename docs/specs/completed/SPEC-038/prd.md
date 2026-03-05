# SPEC-038: Unify Provider Request Building

## Problem

Request building code is duplicated across executor implementations:
- `openai_compat.rs` duplicates ~90 lines between `execute()` and `execute_stream()`
- `gemini.rs` manually loops headers instead of using `common::apply_headers()`
- Inconsistent header application patterns across Claude, Gemini, OpenAICompat

## Requirements

1. Extract `common::build_provider_request()` helper
2. All executors use the shared helper for request construction
3. Per-provider auth differences (x-api-key vs Bearer vs x-goog-api-key) handled via parameter
4. Zero behavior change — pure refactor

## Success Criteria

- No duplicated request building code in executors
- All executors use `common::build_provider_request()` or `common::apply_headers()`
- All tests pass
