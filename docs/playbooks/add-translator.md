# Playbook: Add a New Format Translator

Step-by-step guide for adding a new format translator to convert requests and responses between provider API formats.

## Overview

The proxy accepts requests in OpenAI format and translates them to the target provider's native format. Responses are translated back to OpenAI format. Translators are registered in the `TranslatorRegistry` in `crates/translator/src/lib.rs`.

Each translator pair consists of:

- **Request translator**: Converts incoming requests from one format to another
- **Stream response translator**: Converts SSE stream events back to the source format
- **Non-stream response translator**: Converts complete responses back to the source format

## Architecture

```
Client (OpenAI format)
  |
  v
Request Translator: openai_to_newformat::translate_request
  |
  v
Provider Executor (sends to upstream API)
  |
  v
Response Translator: newformat_to_openai::translate_stream / translate_non_stream
  |
  v
Client (OpenAI format response)
```

## Steps

### 1. Create the Request Translator

Create `crates/translator/src/openai_to_newformat.rs`.

The request translator function signature must match `RequestTransformFn`:

```rust
use prism_core::error::ProxyError;
use serde_json::{json, Value};

pub fn translate_request(
    model: &str,
    raw_json: &[u8],
    stream: bool,
) -> Result<Vec<u8>, ProxyError> {
    let req: Value = serde_json::from_slice(raw_json)?;

    // 1. Extract system messages
    // 2. Convert messages to target format
    // 3. Convert tools if applicable
    // 4. Map parameters (temperature, max_tokens, etc.)
    // 5. Build the target format request

    let new_req = json!({
        "model": model,
        // ... converted fields
    });

    if stream {
        // Add stream-specific fields
    }

    serde_json::to_vec(&new_req)
        .map_err(|e| ProxyError::Translation(e.to_string()))
}
```

Key translation concerns:
- **System messages**: Some APIs use a top-level `system` field (Claude), others inline it in messages (OpenAI), others use a different structure (Gemini `system_instruction`).
- **Message roles**: Map `user`, `assistant`, `system`, `tool` roles to the target format.
- **Tool calls**: Convert between `tool_calls` (OpenAI) and provider-specific formats like `tool_use` blocks (Claude) or `functionCall` (Gemini).
- **Parameters**: Map `max_tokens`/`max_completion_tokens`, `temperature`, `top_p`, `stop`/`stop_sequences`, etc.

### 2. Create the Response Translators

Create `crates/translator/src/newformat_to_openai.rs`.

#### Stream Response Translator

Must match `StreamTransformFn`:

```rust
use crate::TranslateState;
use prism_core::error::ProxyError;
use serde_json::{json, Value};

pub fn translate_stream(
    model: &str,
    original_req: &[u8],
    event_type: Option<&str>,
    data: &[u8],
    state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError> {
    let event: Value = serde_json::from_slice(data)?;

    // Use event_type to determine what kind of SSE event this is.
    // Produce one or more OpenAI-format SSE data lines.

    let mut chunks = Vec::new();

    // Example: convert a content delta to OpenAI chat completion chunk format
    let chunk = json!({
        "id": state.response_id,
        "object": "chat.completion.chunk",
        "created": state.created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "content": "text here" },
            "finish_reason": null,
        }],
    });
    chunks.push(serde_json::to_string(&chunk).unwrap());

    Ok(chunks)
}
```

#### Non-Stream Response Translator

Must match `NonStreamTransformFn`:

```rust
pub fn translate_non_stream(
    _model: &str,
    _original_req: &[u8],
    data: &[u8],
) -> Result<String, ProxyError> {
    let resp: Value = serde_json::from_slice(data)?;

    // Convert the complete response to OpenAI chat completion format
    let openai_resp = json!({
        "id": "chatcmpl-...",
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": resp.get("model").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "extracted text",
            },
            "finish_reason": "stop",
        }],
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0,
        },
    });

    serde_json::to_string(&openai_resp)
        .map_err(|e| ProxyError::Translation(e.to_string()))
}
```

### 3. Handle TranslateState

The `TranslateState` struct (in `crates/translator/src/lib.rs`) accumulates state across SSE stream events:

```rust
#[derive(Debug, Default)]
pub struct TranslateState {
    pub response_id: String,      // Response ID from the first event
    pub model: String,            // Model name
    pub created: i64,             // Timestamp
    pub current_tool_call_index: i32,  // Track tool call indices for OpenAI format
    pub current_content_index: i32,    // Track content block indices
    pub sent_role: bool,          // Whether the role delta has been sent
    pub input_tokens: u64,        // Token count from usage events
}
```

Use this state to:
- Track the response ID from the initial event and reuse it across all chunks
- Maintain tool call indices (OpenAI requires sequential indices on `tool_calls`)
- Track whether the `role: "assistant"` delta has been sent (only send it once)
- Accumulate token counts from usage events

### 4. Register in the TranslatorRegistry

In `crates/translator/src/lib.rs`:

1. Add the module declarations:
   ```rust
   pub mod openai_to_newformat;
   pub mod newformat_to_openai;
   ```

2. Register in `build_registry()`:
   ```rust
   pub fn build_registry() -> TranslatorRegistry {
       let mut reg = TranslatorRegistry::new();

       // ... existing registrations ...

       // OpenAI -> NewFormat request, NewFormat -> OpenAI response
       reg.register(
           Format::OpenAI,
           Format::NewFormat,
           openai_to_newformat::translate_request,
           ResponseTransform {
               stream: newformat_to_openai::translate_stream,
               non_stream: newformat_to_openai::translate_non_stream,
           },
       );

       reg
   }
   ```

Note the direction convention:
- The request translator key is `(source_format, target_format)` -- e.g., `(OpenAI, Claude)`
- The response transformer registered under the same key handles the reverse direction -- e.g., Claude responses back to OpenAI format

### 5. Add Tests

Create test modules in each translator file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_request_basic() {
        let openai_req = serde_json::json!({
            "model": "test-model",
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "max_tokens": 100,
        });
        let raw = serde_json::to_vec(&openai_req).unwrap();
        let result = translate_request("test-model", &raw, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        // Assert the converted format is correct
        assert_eq!(parsed["model"], "test-model");
    }

    #[test]
    fn test_translate_non_stream() {
        // Test full response translation
    }

    #[test]
    fn test_translate_stream() {
        // Test stream chunk translation with TranslateState
    }
}
```

Test cases to cover:
- Basic text messages
- System messages
- Multi-turn conversations
- Tool calls and tool results
- Image/multimodal content
- Streaming with multiple event types
- Edge cases (empty content, missing fields)

### 6. Run Quality Checks

```sh
make lint   # cargo fmt --check + cargo clippy
make test   # cargo test --workspace
```

## Checklist

- [ ] Request translator function created (`openai_to_newformat.rs`)
- [ ] Stream response translator function created (`newformat_to_openai.rs`)
- [ ] Non-stream response translator function created
- [ ] `TranslateState` usage is correct for stateful stream translation
- [ ] Translator registered in `build_registry()`
- [ ] Module declarations added to `crates/translator/src/lib.rs`
- [ ] Tests cover basic messages, tools, streaming, and edge cases
- [ ] `make lint` passes
- [ ] `make test` passes

## Reference: Existing Translators

| Direction          | Request File            | Response File           |
|--------------------|-------------------------|-------------------------|
| OpenAI <-> Claude  | `openai_to_claude.rs`   | `claude_to_openai.rs`   |
| OpenAI <-> Gemini  | `openai_to_gemini.rs`   | `gemini_to_openai.rs`   |

## Function Signature Reference

```rust
// Request translator
type RequestTransformFn =
    fn(model: &str, raw_json: &[u8], stream: bool) -> Result<Vec<u8>, ProxyError>;

// Stream response translator
type StreamTransformFn = fn(
    model: &str,
    original_req: &[u8],
    event_type: Option<&str>,
    data: &[u8],
    state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError>;

// Non-stream response translator
type NonStreamTransformFn =
    fn(model: &str, original_req: &[u8], data: &[u8]) -> Result<String, ProxyError>;
```
