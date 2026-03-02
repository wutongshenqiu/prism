pub mod claude_to_openai;
pub mod gemini_to_openai;
pub mod openai_to_claude;
pub mod openai_to_gemini;

use ai_proxy_core::error::ProxyError;
use ai_proxy_core::provider::Format;
use std::collections::HashMap;

/// State accumulated during stream translation.
#[derive(Debug, Default)]
pub struct TranslateState {
    pub response_id: String,
    pub model: String,
    pub created: i64,
    pub current_tool_call_index: i32,
    pub current_content_index: i32,
    pub sent_role: bool,
    pub input_tokens: u64,
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

    // OpenAI -> OpenAICompat passthrough (only replace model name, pass responses as-is)
    reg.register(
        Format::OpenAI,
        Format::OpenAICompat,
        |model, raw_json, _stream| replace_model_in_payload(raw_json, model),
        ResponseTransform {
            stream: |_model, _orig_req, _event_type, data, _state| {
                Ok(vec![String::from_utf8_lossy(data).to_string()])
            },
            non_stream: |_model, _orig_req, data| Ok(String::from_utf8_lossy(data).to_string()),
        },
    );

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

    #[test]
    fn test_registry_openai_compat_passthrough() {
        let reg = build_registry();
        let payload = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 0.7
        });
        let raw = serde_json::to_vec(&payload).unwrap();

        let result = reg
            .translate_request(
                Format::OpenAI,
                Format::OpenAICompat,
                "deepseek-chat",
                &raw,
                false,
            )
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        // Model replaced, everything else preserved
        assert_eq!(val["model"], "deepseek-chat");
        assert_eq!(val["temperature"], 0.7);
        assert_eq!(val["messages"][0]["content"], "Hello");
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
        assert!(reg.has_response_translator(Format::OpenAI, Format::OpenAICompat));
        // Same format should return false
        assert!(!reg.has_response_translator(Format::OpenAI, Format::OpenAI));
        // Unregistered pair should return false
        assert!(!reg.has_response_translator(Format::Claude, Format::Gemini));
    }

    // === build_registry ===

    #[test]
    fn test_build_registry_has_all_paths() {
        let reg = build_registry();
        // Should have 3 request translators
        assert_eq!(reg.requests.len(), 3);
        // Should have 3 response translators
        assert_eq!(reg.responses.len(), 3);
    }
}
