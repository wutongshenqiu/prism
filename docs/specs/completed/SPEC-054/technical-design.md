# Technical Design: Extended Thinking Cross-Provider Translation

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-054       |
| Title     | Extended Thinking Cross-Provider Translation |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Adds bidirectional thinking/reasoning translation between Claude, OpenAI, and Gemini formats. Includes model suffix parsing for thinking budget specification and unified reasoning_effort mapping.

## API Design

No new endpoints. Affects existing translation behavior for:
- `POST /v1/chat/completions` — reasoning_content in responses, reasoning_effort in requests
- `POST /v1/messages` — thinking blocks preserved

## Backend Implementation

### Claude → OpenAI Response Translation

**Non-streaming** (`claude_to_openai.rs`):
- Extract `thinking` blocks from `content` array
- Map to `reasoning_content` field on the message
- Concatenate multiple thinking blocks with newlines

**Streaming** (`claude_to_openai.rs`):
- `content_block_start` with `type: "thinking"` → emit `reasoning_content` delta
- `content_block_delta` with `type: "thinking_delta"` → append to `reasoning_content`

### OpenAI → Claude Request Translation

**`openai_to_claude.rs`**:
- `reasoning_content` in assistant messages → `thinking` content block
- `reasoning_effort` → `thinking.budget_tokens`:
  - `low` → 1024
  - `medium` → 4096
  - `high` → max_tokens * 0.8 (minimum 8192)

### Model Suffix Parsing

In `crates/server/src/dispatch/`:
- Regex: `^(.+)\((\d+)\)$`
- Extract model name and budget
- Strip suffix before routing
- Inject `thinking.budget_tokens` into request body

### Gemini Translation

**`openai_to_gemini.rs`**:
- `reasoning_effort` → `generationConfig.thinkingConfig.thinkingBudget`
- Map: low=1024, medium=4096, high=max_tokens*0.8

## Configuration Changes

No new configuration fields. Thinking behavior is driven by request parameters.

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| Claude   | Yes       | Native thinking blocks, signature support |
| OpenAI   | Yes       | reasoning_content field, reasoning_effort parameter |
| Gemini   | Yes       | thinkingConfig.thinkingBudget |

## Task Breakdown

- [ ] Add reasoning_content to claude_to_openai non-stream translation
- [ ] Add reasoning_content to claude_to_openai stream translation
- [ ] Add reasoning_content → thinking block in openai_to_claude request translation
- [ ] Add reasoning_effort → budget_tokens mapping in openai_to_claude
- [ ] Add model suffix parsing in dispatch
- [ ] Add thinkingConfig translation in openai_to_gemini
- [ ] Add unit tests for all translation paths
- [ ] Add streaming tests for thinking content

## Test Strategy

- **Unit tests:** Each translation function with thinking content, suffix parsing regex
- **Integration tests:** Full request/response cycle with thinking enabled
- **Manual verification:** Test with Claude thinking model, verify reasoning_content in OpenAI response

## Rollout Plan

1. Implement non-streaming thinking translation
2. Add streaming support
3. Add model suffix parsing
4. Add Gemini support
