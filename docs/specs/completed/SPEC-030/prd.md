# PRD: Provider & Dispatch Unit Tests

| Field     | Value                            |
|-----------|----------------------------------|
| Spec ID   | SPEC-030                         |
| Title     | Provider & Dispatch Unit Tests   |
| Author    | Claude                           |
| Status    | Active                           |
| Created   | 2026-03-03                       |
| Updated   | 2026-03-03                       |

## Problem Statement

The provider crate's CredentialRouter and the server's dispatch orchestrator have zero unit tests for their core routing logic, SSE parsing edge cases, and OpenAI-compatible format conversions. These are critical paths that handle credential selection, retry logic, and request/response transformation.

## Goals

- Add unit tests for CredentialRouter routing strategies (FillFirst, RoundRobin, LatencyAware, GeoAware)
- Extend SSE parsing tests for streaming edge cases
- Add unit tests for OpenAI-compatible format conversions (chat_to_responses, responses_to_chat)
- Add unit tests for dispatch pure functions (extract_usage, inject_debug_headers)

## Success Metrics

- ~45 new tests covering routing, SSE, dispatch helpers, and format conversions
- All tests pass with `make test`
- Zero clippy warnings
