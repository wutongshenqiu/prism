pub mod claude_to_openai;
pub mod claude_to_openai_request;
pub mod common;
pub mod gemini_to_openai;
pub mod gemini_to_openai_request;
pub mod openai_to_claude;
pub mod openai_to_claude_response;
pub mod openai_to_gemini;
pub mod openai_to_gemini_response;

use prism_types::error::ProxyError;
use prism_types::format::Format;
use std::collections::HashMap;

/// State accumulated during stream translation.
#[derive(Debug, Default)]
pub struct TranslateState {
    pub response_id: String,
    pub model: String,
    pub created: i64,
    pub current_tool_call_index: Option<usize>,
    pub current_content_index: Option<usize>,
    pub sent_role: bool,
    pub input_tokens: u64,
}

impl TranslateState {
    /// Increment the tool call index (starts at 0 on first call).
    pub fn next_tool_call_index(&mut self) -> usize {
        let next = self.current_tool_call_index.map(|i| i + 1).unwrap_or(0);
        self.current_tool_call_index = Some(next);
        next
    }

    /// Increment the content index (starts at 0 on first call).
    pub fn next_content_index(&mut self) -> usize {
        let next = self.current_content_index.map(|i| i + 1).unwrap_or(0);
        self.current_content_index = Some(next);
        next
    }

    /// Get the current tool call index for streaming deltas.
    pub fn tool_call_index(&self) -> i32 {
        self.current_tool_call_index.map(|i| i as i32).unwrap_or(0)
    }
}

pub type RequestTransformFn =
    fn(model: &str, raw_json: &[u8], stream: bool) -> Result<Vec<u8>, ProxyError>;

pub type StreamTransformFn = fn(
    model: &str,
    original_req: &[u8],
    event_type: Option<&str>,
    data: &[u8],
    state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError>;

pub type NonStreamTransformFn =
    fn(model: &str, original_req: &[u8], data: &[u8]) -> Result<String, ProxyError>;

pub struct ResponseTransform {
    pub stream: StreamTransformFn,
    pub non_stream: NonStreamTransformFn,
}

pub struct TranslatorRegistry {
    requests: HashMap<(Format, Format), RequestTransformFn>,
    responses: HashMap<(Format, Format), ResponseTransform>,
}

impl Default for TranslatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslatorRegistry {
    pub fn new() -> Self {
        Self {
            requests: HashMap::new(),
            responses: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        from: Format,
        to: Format,
        request: RequestTransformFn,
        response: ResponseTransform,
    ) {
        self.requests.insert((from, to), request);
        self.responses.insert((from, to), response);
    }

    pub fn register_request(&mut self, from: Format, to: Format, request: RequestTransformFn) {
        self.requests.insert((from, to), request);
    }

    pub fn translate_request(
        &self,
        from: Format,
        to: Format,
        model: &str,
        raw_json: &[u8],
        stream: bool,
    ) -> Result<Vec<u8>, ProxyError> {
        if from == to {
            // Even for passthrough, replace the model name (alias → actual ID)
            return replace_model_in_payload(raw_json, model);
        }
        match self.requests.get(&(from, to)) {
            Some(f) => f(model, raw_json, stream),
            None => Ok(raw_json.to_vec()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn translate_stream(
        &self,
        from: Format,
        to: Format,
        model: &str,
        orig_req: &[u8],
        event_type: Option<&str>,
        data: &[u8],
        state: &mut TranslateState,
    ) -> Result<Vec<String>, ProxyError> {
        if from == to {
            let line = String::from_utf8_lossy(data).to_string();
            // Pass through [DONE] sentinel and raw data as-is
            return Ok(vec![line]);
        }
        // Skip [DONE] sentinel for translation paths (translators produce their own)
        if data == b"[DONE]" {
            return Ok(vec!["[DONE]".to_string()]);
        }
        match self.responses.get(&(from, to)) {
            Some(rt) => (rt.stream)(model, orig_req, event_type, data, state),
            None => {
                let line = String::from_utf8_lossy(data).to_string();
                Ok(vec![line])
            }
        }
    }

    pub fn translate_non_stream(
        &self,
        from: Format,
        to: Format,
        model: &str,
        orig_req: &[u8],
        data: &[u8],
    ) -> Result<String, ProxyError> {
        if from == to {
            return Ok(String::from_utf8_lossy(data).to_string());
        }
        match self.responses.get(&(from, to)) {
            Some(rt) => (rt.non_stream)(model, orig_req, data),
            None => Ok(String::from_utf8_lossy(data).to_string()),
        }
    }

    pub fn has_response_translator(&self, from: Format, to: Format) -> bool {
        from != to && self.responses.contains_key(&(from, to))
    }
}

/// Replace the "model" field in a JSON payload with the resolved model name.
fn replace_model_in_payload(raw_json: &[u8], model: &str) -> Result<Vec<u8>, ProxyError> {
    let mut val: serde_json::Value = serde_json::from_slice(raw_json)?;
    if let Some(obj) = val.as_object_mut()
        && obj.contains_key("model")
    {
        obj.insert(
            "model".to_string(),
            serde_json::Value::String(model.to_string()),
        );
    }
    serde_json::to_vec(&val).map_err(|e| ProxyError::Translation(e.to_string()))
}

pub fn build_registry() -> TranslatorRegistry {
    let mut reg = TranslatorRegistry::new();

    // OpenAI -> Claude request translation, Claude -> OpenAI response translation
    reg.register(
        Format::OpenAI,
        Format::Claude,
        openai_to_claude::translate_request,
        ResponseTransform {
            stream: claude_to_openai::translate_stream,
            non_stream: claude_to_openai::translate_non_stream,
        },
    );

    // OpenAI -> Gemini request translation, Gemini -> OpenAI response translation
    reg.register(
        Format::OpenAI,
        Format::Gemini,
        openai_to_gemini::translate_request,
        ResponseTransform {
            stream: gemini_to_openai::translate_stream,
            non_stream: gemini_to_openai::translate_non_stream,
        },
    );

    // Gemini -> OpenAI request translation, OpenAI -> Gemini response translation
    reg.register(
        Format::Gemini,
        Format::OpenAI,
        gemini_to_openai_request::translate_request,
        ResponseTransform {
            stream: openai_to_gemini_response::translate_stream,
            non_stream: openai_to_gemini_response::translate_non_stream,
        },
    );

    // Gemini -> Claude (chain: Gemini -> OpenAI -> Claude request only)
    reg.register_request(Format::Gemini, Format::Claude, |model, raw, stream| {
        let openai_payload = gemini_to_openai_request::translate_request(model, raw, stream)?;
        openai_to_claude::translate_request(model, &openai_payload, stream)
    });

    // Claude -> OpenAI request translation, OpenAI -> Claude response translation
    reg.register(
        Format::Claude,
        Format::OpenAI,
        claude_to_openai_request::translate_request,
        ResponseTransform {
            stream: openai_to_claude_response::translate_stream,
            non_stream: openai_to_claude_response::translate_non_stream,
        },
    );

    // Claude -> Gemini (chain: Claude -> OpenAI -> Gemini request only)
    reg.register_request(Format::Claude, Format::Gemini, |model, raw, stream| {
        let openai_payload = claude_to_openai_request::translate_request(model, raw, stream)?;
        openai_to_gemini::translate_request(model, &openai_payload, stream)
    });

    reg
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // === replace_model_in_payload ===

    #[test]
    fn test_replace_model_in_payload() {
        let payload = json!({"model": "gpt-4", "messages": []});
        let raw = serde_json::to_vec(&payload).unwrap();
        let result = replace_model_in_payload(&raw, "actual-model-id").unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["model"], "actual-model-id");
        // Other fields preserved
        assert!(val["messages"].is_array());
    }

    #[test]
    fn test_replace_model_no_model_field() {
        let payload = json!({"messages": []});
        let raw = serde_json::to_vec(&payload).unwrap();
        let result = replace_model_in_payload(&raw, "new-model").unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        // Should not add model field if not present
        assert!(val.get("model").is_none());
    }

    // === TranslatorRegistry ===

    #[test]
    fn test_registry_same_format_passthrough() {
        let reg = build_registry();
        let payload = json!({"model": "gpt-4", "messages": [{"role": "user", "content": "Hi"}]});
        let raw = serde_json::to_vec(&payload).unwrap();

        let result = reg
            .translate_request(Format::OpenAI, Format::OpenAI, "gpt-4o", &raw, false)
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        // Model should be replaced even on passthrough
        assert_eq!(val["model"], "gpt-4o");
    }

    #[test]
    fn test_registry_openai_to_claude_translation() {
        let reg = build_registry();
        let payload = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let raw = serde_json::to_vec(&payload).unwrap();

        let result = reg
            .translate_request(
                Format::OpenAI,
                Format::Claude,
                "claude-3-5-sonnet-20241022",
                &raw,
                false,
            )
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(val["messages"][0]["role"], "user");
    }

    #[test]
    fn test_registry_openai_to_gemini_translation() {
        let reg = build_registry();
        let payload = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let raw = serde_json::to_vec(&payload).unwrap();

        let result = reg
            .translate_request(
                Format::OpenAI,
                Format::Gemini,
                "gemini-1.5-pro",
                &raw,
                false,
            )
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["contents"][0]["role"], "user");
    }

    // === translate_stream ===

    #[test]
    fn test_stream_same_format_passthrough() {
        let reg = build_registry();
        let mut state = TranslateState::default();
        let data = b"{\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}";

        let result = reg
            .translate_stream(
                Format::OpenAI,
                Format::OpenAI,
                "gpt-4",
                b"{}",
                None,
                data,
                &mut state,
            )
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], String::from_utf8_lossy(data));
    }

    #[test]
    fn test_stream_done_sentinel_passthrough() {
        let reg = build_registry();
        let mut state = TranslateState::default();

        let result = reg
            .translate_stream(
                Format::OpenAI,
                Format::Claude,
                "claude-3",
                b"{}",
                None,
                b"[DONE]",
                &mut state,
            )
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "[DONE]");
    }

    #[test]
    fn test_stream_no_translator_fallback() {
        let reg = TranslatorRegistry::new(); // empty registry
        let mut state = TranslateState::default();
        let data = b"some raw data";

        let result = reg
            .translate_stream(
                Format::Claude,
                Format::Gemini,
                "model",
                b"{}",
                None,
                data,
                &mut state,
            )
            .unwrap();
        assert_eq!(result[0], "some raw data");
    }

    // === translate_non_stream ===

    #[test]
    fn test_non_stream_same_format_passthrough() {
        let reg = build_registry();
        let data = b"{\"choices\":[{\"message\":{\"content\":\"Hello\"}}]}";

        let result = reg
            .translate_non_stream(Format::OpenAI, Format::OpenAI, "gpt-4", b"{}", data)
            .unwrap();
        assert_eq!(result, String::from_utf8_lossy(data));
    }

    #[test]
    fn test_non_stream_no_translator_fallback() {
        let reg = TranslatorRegistry::new();
        let data = b"raw response";

        let result = reg
            .translate_non_stream(Format::Claude, Format::Gemini, "model", b"{}", data)
            .unwrap();
        assert_eq!(result, "raw response");
    }

    // === has_response_translator ===

    #[test]
    fn test_has_response_translator() {
        let reg = build_registry();
        assert!(reg.has_response_translator(Format::OpenAI, Format::Claude));
        assert!(reg.has_response_translator(Format::OpenAI, Format::Gemini));
        // Same format should return false
        assert!(!reg.has_response_translator(Format::OpenAI, Format::OpenAI));
        // Unregistered pair should return false
        assert!(!reg.has_response_translator(Format::Claude, Format::Gemini));
    }

    // === Gemini reverse paths ===

    #[test]
    fn test_registry_gemini_to_openai_request() {
        let reg = build_registry();
        let payload = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
        });
        let raw = serde_json::to_vec(&payload).unwrap();
        let result = reg
            .translate_request(Format::Gemini, Format::OpenAI, "gpt-4", &raw, false)
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["model"], "gpt-4");
        assert_eq!(val["messages"][0]["role"], "user");
        assert_eq!(val["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_registry_gemini_to_claude_chained_request() {
        let reg = build_registry();
        let payload = json!({
            "systemInstruction": {"parts": [{"text": "Be helpful"}]},
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });
        let raw = serde_json::to_vec(&payload).unwrap();
        let result = reg
            .translate_request(
                Format::Gemini,
                Format::Claude,
                "claude-sonnet-4-20250514",
                &raw,
                false,
            )
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        // Should produce Claude format (model, system, messages)
        assert_eq!(val["model"], "claude-sonnet-4-20250514");
        assert_eq!(val["messages"][0]["role"], "user");
        // System should be extracted
        let system = val.get("system");
        assert!(system.is_some(), "system prompt should be present");
    }

    #[test]
    fn test_registry_gemini_to_openai_response() {
        let reg = build_registry();
        let openai_resp = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result = reg
            .translate_non_stream(Format::Gemini, Format::OpenAI, "gpt-4", b"{}", &data)
            .unwrap();
        let val: serde_json::Value = serde_json::from_str(&result).unwrap();
        // Should produce Gemini response format
        assert_eq!(
            val["candidates"][0]["content"]["parts"][0]["text"],
            "Hello!"
        );
        assert_eq!(val["candidates"][0]["finishReason"], "STOP");
        assert_eq!(val["usageMetadata"]["promptTokenCount"], 10);
    }

    // === build_registry ===

    #[test]
    fn test_build_registry_has_all_paths() {
        let reg = build_registry();
        // Should have 6 request translators:
        // OpenAI→Claude, OpenAI→Gemini,
        // Gemini→OpenAI, Gemini→Claude,
        // Claude→OpenAI, Claude→Gemini
        assert_eq!(reg.requests.len(), 6);
        // Should have 4 response translators:
        // OpenAI→Claude, OpenAI→Gemini,
        // Gemini→OpenAI,
        // Claude→OpenAI
        assert_eq!(reg.responses.len(), 4);
    }
}
