# Technical Design: Structured Output Translation

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-059       |
| Title     | Structured Output Translation |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Translates OpenAI's structured output parameters (response_format with json_schema or json_object) into equivalent constructs for Claude and Gemini.

## Backend Implementation

### OpenAI → Claude (json_schema)

In `openai_to_claude.rs`:
1. Extract `response_format.json_schema` from request
2. Create synthetic tool with schema as input_schema
3. Add `tool_choice: {"type": "tool", "name": "__structured_output"}`
4. In response (`claude_to_openai.rs`): unwrap tool result as message content

### OpenAI → Gemini (json_schema)

In `openai_to_gemini.rs`:
1. Extract schema
2. Add `generationConfig.responseMimeType: "application/json"`
3. Add `generationConfig.responseSchema: <schema>`

### OpenAI → Claude (json_object)

1. Inject system prompt: "Respond with valid JSON only."
2. No tool injection needed

### OpenAI → Gemini (json_object)

1. Add `generationConfig.responseMimeType: "application/json"`

## Task Breakdown

- [ ] Implement json_schema → synthetic tool (openai_to_claude)
- [ ] Implement synthetic tool unwrap (claude_to_openai)
- [ ] Implement json_schema → responseSchema (openai_to_gemini)
- [ ] Implement json_object handling for both providers
- [ ] Unit tests for each path
- [ ] Integration tests

## Test Strategy

- **Unit tests:** Schema extraction, tool injection, response unwrapping
- **Integration tests:** Full structured output request cycle
