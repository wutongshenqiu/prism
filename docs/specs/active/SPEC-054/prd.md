# PRD: Extended Thinking Cross-Provider Translation

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-054       |
| Title     | Extended Thinking Cross-Provider Translation |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Three related problems:
1. Claude thinking blocks in responses are **dropped** during OpenAI format translation (`claude_to_openai.rs` skips thinking events)
2. Different providers use different thinking parameter formats with no unified configuration
3. No support for `model(budget)` suffix syntax for thinking budget

## Goals

- Claude thinking → OpenAI `reasoning_content` bidirectional translation (request + response, streaming + non-streaming)
- Unified thinking mode configuration: budget / level / auto / none
- OpenAI `reasoning_effort` (low/medium/high) → Claude `budget_tokens` mapping
- Thinking suffix parsing: `claude-sonnet-4-5(10000)` → model name + budget injection
- Gemini `thinkingConfig.thinkingBudget` translation

## Non-Goals

- Cross-provider thinking content semantic conversion (format translation only)
- Thinking model capability detection

## User Stories

- As a developer using OpenAI SDK, I want to see Claude's thinking output as `reasoning_content`, so I can use thinking features through the proxy.
- As a developer, I want to specify thinking budget via model suffix `model(budget)`, so I can control thinking cost per-request.
- As a developer, I want `reasoning_effort` to work across providers, so I have a unified thinking interface.

## Success Metrics

- Thinking content preserved in all translation paths
- Model suffix parsing works for all model names
- reasoning_effort correctly maps to provider-specific parameters

## Constraints

- Must be backward-compatible with existing non-thinking translations
- Streaming thinking content must maintain SSE chunk ordering

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| reasoning_effort mapping | Linear, tiered, configurable | Tiered (low=1024, medium=4096, high=80% max) | Matches OpenAI's intent, simple to understand |
| Suffix parsing location | translator, dispatch, middleware | dispatch (before translation) | Can affect routing and translation config |
