# SPEC-036: Add RequestContext to ProviderExecutor Trait

## Problem

`ProviderExecutor::execute()` and `execute_stream()` do not receive `RequestContext`. This makes request tracking, tenant billing, and per-request auditing impossible at the executor level. Currently, context is only available in the server crate's dispatch logic, creating an information gap.

## Requirements

1. Add `RequestContext` parameter to `ProviderExecutor::execute()` and `execute_stream()` trait methods
2. All 4 executor implementations (Claude, OpenAI, Gemini, OpenAICompat) must accept and forward context
3. Dispatch must pass context when calling executors
4. No breaking change to external behavior — purely internal interface enhancement

## Non-Goals

- Using context to modify request behavior (future work)
- Adding tracing spans in executors (future work)

## Success Criteria

- All executor implementations accept `RequestContext`
- All existing tests pass
- No change to API behavior
