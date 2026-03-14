use prism_domain::content::{
    ContentBlock, Conversation, ImageSource, Message, Role, SystemContent,
};
use prism_domain::event::CanonicalEvent;
use prism_domain::operation::Endpoint;
use prism_domain::request::{CanonicalRequest, RequestLimits};
use prism_domain::response::Usage;
use prism_domain::response::{CanonicalResponse, StopReason};
use prism_domain::tool::{ResponseFormat, ToolChoice, ToolSpec};
use prism_types::types::gemini::*;

// ─── Ingress: Gemini → Canonical ────────────────────────────────────────────

/// Parse a GeminiRequest into a CanonicalRequest.
pub fn ingress_generate(req: &GeminiRequest, model: &str, endpoint: Endpoint) -> CanonicalRequest {
    let system = convert_system(&req.system_instruction);
    let messages = convert_contents(&req.contents);
    let tools = convert_tools(&req.tools);
    let (response_format, limits) = convert_generation_config(&req.generation_config);

    CanonicalRequest {
        ingress_protocol: prism_domain::operation::IngressProtocol::Gemini,
        operation: endpoint.operation(),
        endpoint,
        model: model.to_string(),
        stream: endpoint == Endpoint::StreamGenerateContent,
        input: Conversation { system, messages },
        tools,
        tool_choice: ToolChoice::Auto,
        response_format,
        reasoning: None,
        limits,
        tenant_id: None,
        api_key_id: None,
        region: None,
        raw_body: None,
    }
}

fn convert_system(sys: &Option<GeminiContent>) -> Option<SystemContent> {
    sys.as_ref().map(|c| {
        let text = c
            .parts
            .iter()
            .filter_map(|p| match p {
                GeminiPart::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        SystemContent::Text(text)
    })
}

fn convert_contents(contents: &[GeminiContent]) -> Vec<Message> {
    contents
        .iter()
        .map(|c| {
            let role = match c.role.as_deref() {
                Some("model") => Role::Assistant,
                _ => Role::User,
            };
            let content = c.parts.iter().map(convert_part).collect();
            Message {
                role,
                content,
                name: None,
            }
        })
        .collect()
}

fn convert_part(part: &GeminiPart) -> ContentBlock {
    match part {
        GeminiPart::Text(t) => ContentBlock::Text { text: t.clone() },
        GeminiPart::InlineData { mime_type, data } => ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: mime_type.clone(),
                data: data.clone(),
            },
        },
        GeminiPart::FunctionCall { name, args } => ContentBlock::ToolUse {
            id: format!("call_{}", name),
            name: name.clone(),
            input: args.clone(),
        },
        GeminiPart::FunctionResponse { name, response } => ContentBlock::ToolResult {
            tool_use_id: format!("call_{}", name),
            content: vec![ContentBlock::Text {
                text: serde_json::to_string(response).unwrap_or_default(),
            }],
            is_error: false,
        },
    }
}

fn convert_tools(tools: &Option<Vec<GeminiToolDeclaration>>) -> Vec<ToolSpec> {
    tools
        .as_ref()
        .map(|ts| {
            ts.iter()
                .flat_map(|td| {
                    td.function_declarations.iter().map(|f| ToolSpec {
                        name: f.name.clone(),
                        description: Some(f.description.clone()),
                        parameters: f.parameters.clone().unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn convert_generation_config(gc: &Option<GenerationConfig>) -> (ResponseFormat, RequestLimits) {
    let gc = match gc {
        Some(c) => c,
        None => {
            return (ResponseFormat::Text, RequestLimits::default());
        }
    };

    let response_format = match gc.response_mime_type.as_deref() {
        Some("application/json") => ResponseFormat::JsonObject,
        _ => ResponseFormat::Text,
    };

    let limits = RequestLimits {
        max_tokens: gc.max_output_tokens,
        temperature: gc.temperature,
        top_p: gc.top_p,
        top_k: gc.top_k,
        stop: gc.stop_sequences.clone().unwrap_or_default(),
    };

    (response_format, limits)
}

fn stop_reason_to_gemini(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::EndTurn | StopReason::StopSequence | StopReason::ToolUse => "STOP",
        StopReason::MaxTokens => "MAX_TOKENS",
    }
}

fn gemini_to_stop_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("STOP") => StopReason::EndTurn,
        Some("MAX_TOKENS") => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    }
}

// ─── Egress: Canonical → Gemini ─────────────────────────────────────────────

/// Convert a CanonicalResponse into a GeminiResponse.
pub fn egress_response(resp: &CanonicalResponse) -> GeminiResponse {
    let parts: Vec<GeminiPart> = resp
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(GeminiPart::Text(text.clone())),
            ContentBlock::ToolUse { name, input, .. } => Some(GeminiPart::FunctionCall {
                name: name.clone(),
                args: input.clone(),
            }),
            _ => None,
        })
        .collect();

    let finish_reason = stop_reason_to_gemini(&resp.stop_reason).to_string();

    GeminiResponse {
        candidates: Some(vec![GeminiCandidate {
            content: Some(GeminiContent {
                role: Some("model".to_string()),
                parts,
            }),
            finish_reason: Some(finish_reason),
            safety_ratings: None,
            index: Some(0),
        }]),
        usage_metadata: Some(GeminiUsageMetadata {
            prompt_token_count: resp.usage.input_tokens,
            candidates_token_count: resp.usage.output_tokens,
            total_token_count: resp.usage.input_tokens + resp.usage.output_tokens,
        }),
        model_version: None,
    }
}

/// Convert a CanonicalEvent into Gemini streaming JSON.
/// Gemini streams by emitting GeminiResponse objects line by line.
pub fn egress_event(event: &CanonicalEvent, model: &str) -> Vec<String> {
    match event {
        CanonicalEvent::TextDelta { text, .. } => {
            let resp = serde_json::json!({
                "candidates": [{
                    "content": {
                        "role": "model",
                        "parts": [{ "text": text }]
                    },
                    "finishReason": null,
                    "index": 0
                }]
            });
            vec![serde_json::to_string(&resp).unwrap_or_default()]
        }
        CanonicalEvent::ContentBlockStart { block, .. } => {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                let resp = serde_json::json!({
                    "candidates": [{
                        "content": {
                            "role": "model",
                            "parts": [{
                                "functionCall": { "name": name, "args": input }
                            }]
                        },
                        "index": 0
                    }]
                });
                vec![serde_json::to_string(&resp).unwrap_or_default()]
            } else {
                vec![]
            }
        }
        CanonicalEvent::StreamEnd { stop_reason, usage } => {
            let fr = stop_reason_to_gemini(stop_reason);
            let resp = serde_json::json!({
                "candidates": [{
                    "content": { "role": "model", "parts": [] },
                    "finishReason": fr,
                    "index": 0
                }],
                "usageMetadata": {
                    "promptTokenCount": usage.input_tokens,
                    "candidatesTokenCount": usage.output_tokens,
                    "totalTokenCount": usage.input_tokens + usage.output_tokens
                },
                "modelVersion": model
            });
            vec![serde_json::to_string(&resp).unwrap_or_default()]
        }
        _ => vec![],
    }
}

// ─── Provider-facing: Canonical → Gemini request ────────────────────────────

/// Convert a CanonicalRequest into a GeminiRequest (to send TO a Gemini provider).
pub fn egress_request(canonical: &CanonicalRequest) -> GeminiRequest {
    let system_instruction = canonical.input.system.as_ref().map(|sys| {
        let text = match sys {
            SystemContent::Text(t) => t.clone(),
            SystemContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text { text } = b {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };
        GeminiContent {
            role: None,
            parts: vec![GeminiPart::Text(text)],
        }
    });

    let contents: Vec<GeminiContent> = canonical
        .input
        .messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                Role::User | Role::Tool => Some("user".to_string()),
                Role::Assistant => Some("model".to_string()),
            };
            let parts = msg
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(GeminiPart::Text(text.clone())),
                    ContentBlock::Image { source } => {
                        if let ImageSource::Base64 { media_type, data } = source {
                            Some(GeminiPart::InlineData {
                                mime_type: media_type.clone(),
                                data: data.clone(),
                            })
                        } else {
                            None
                        }
                    }
                    ContentBlock::ToolUse { name, input, .. } => Some(GeminiPart::FunctionCall {
                        name: name.clone(),
                        args: input.clone(),
                    }),
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => {
                        let response = content
                            .iter()
                            .find_map(|c| {
                                if let ContentBlock::Text { text } = c {
                                    serde_json::from_str(text).ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(serde_json::Value::Null);
                        // Extract function name from tool_use_id (format: "call_{name}")
                        let name = tool_use_id
                            .strip_prefix("call_")
                            .unwrap_or(tool_use_id)
                            .to_string();
                        Some(GeminiPart::FunctionResponse { name, response })
                    }
                    _ => None,
                })
                .collect();
            GeminiContent { role, parts }
        })
        .collect();

    let tools = if canonical.tools.is_empty() {
        None
    } else {
        Some(vec![GeminiToolDeclaration {
            function_declarations: canonical
                .tools
                .iter()
                .map(|t| GeminiFunctionDeclaration {
                    name: t.name.clone(),
                    description: t.description.clone().unwrap_or_default(),
                    parameters: if t.parameters.is_null() {
                        None
                    } else {
                        Some(t.parameters.clone())
                    },
                })
                .collect(),
        }])
    };

    let generation_config = Some(GenerationConfig {
        temperature: canonical.limits.temperature,
        top_p: canonical.limits.top_p,
        top_k: canonical.limits.top_k,
        max_output_tokens: canonical.limits.max_tokens,
        stop_sequences: if canonical.limits.stop.is_empty() {
            None
        } else {
            Some(canonical.limits.stop.clone())
        },
        candidate_count: None,
        response_mime_type: match &canonical.response_format {
            ResponseFormat::JsonObject | ResponseFormat::JsonSchema { .. } => {
                Some("application/json".to_string())
            }
            _ => None,
        },
    });

    GeminiRequest {
        contents,
        system_instruction,
        generation_config,
        tools,
        safety_settings: None,
    }
}

// ─── Provider-facing: Gemini response → Canonical ───────────────────────────

/// Parse a Gemini API response into a CanonicalResponse.
pub fn parse_response(
    data: &[u8],
    provider: &str,
    credential: &str,
) -> Result<CanonicalResponse, String> {
    let resp: GeminiResponse = serde_json::from_slice(data)
        .map_err(|e| format!("failed to parse Gemini response: {e}"))?;

    let candidate = resp.candidates.as_ref().and_then(|c| c.first());

    let content: Vec<ContentBlock> = candidate
        .and_then(|c| c.content.as_ref())
        .map(|c| {
            c.parts
                .iter()
                .filter_map(|part| match part {
                    GeminiPart::Text(text) => Some(ContentBlock::Text { text: text.clone() }),
                    GeminiPart::FunctionCall { name, args } => Some(ContentBlock::ToolUse {
                        id: format!("call_{name}"),
                        name: name.clone(),
                        input: args.clone(),
                    }),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let stop_reason = gemini_to_stop_reason(candidate.and_then(|c| c.finish_reason.as_deref()));

    let usage = resp
        .usage_metadata
        .map(|u| Usage {
            input_tokens: u.prompt_token_count,
            output_tokens: u.candidates_token_count,
            ..Default::default()
        })
        .unwrap_or_default();

    let model = resp.model_version.unwrap_or_default();

    Ok(CanonicalResponse {
        id: String::new(),
        model,
        content,
        stop_reason,
        usage,
        execution_mode: prism_domain::operation::ExecutionMode::Native,
        provider: provider.to_string(),
        credential: credential.to_string(),
    })
}

/// Parse a Gemini streaming JSON line into a CanonicalEvent.
pub fn parse_event(data: &str) -> Option<CanonicalEvent> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    let candidates = v.get("candidates")?.as_array()?;
    let candidate = candidates.first()?;

    // Finish event
    if let Some(fr) = candidate.get("finishReason").and_then(|f| f.as_str()) {
        let stop_reason = gemini_to_stop_reason(Some(fr));
        let usage = v
            .get("usageMetadata")
            .map(|u| Usage {
                input_tokens: u
                    .get("promptTokenCount")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0),
                output_tokens: u
                    .get("candidatesTokenCount")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0),
                ..Default::default()
            })
            .unwrap_or_default();
        return Some(CanonicalEvent::StreamEnd { stop_reason, usage });
    }

    // Content delta
    let content = candidate.get("content")?;
    let parts = content.get("parts")?.as_array()?;

    for part in parts {
        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
            return Some(CanonicalEvent::TextDelta {
                index: 0,
                text: text.to_string(),
            });
        }
        if let Some(fc) = part.get("functionCall") {
            let name = fc.get("name")?.as_str()?.to_string();
            let args = fc.get("args").cloned().unwrap_or(serde_json::Value::Null);
            return Some(CanonicalEvent::ContentBlockStart {
                index: 0,
                block: ContentBlock::ToolUse {
                    id: format!("call_{name}"),
                    name,
                    input: args,
                },
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_domain::response::Usage;

    fn minimal_gemini_request() -> GeminiRequest {
        GeminiRequest {
            contents: vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart::Text("Hello".to_string())],
            }],
            system_instruction: None,
            generation_config: None,
            tools: None,
            safety_settings: None,
        }
    }

    #[test]
    fn test_ingress_basic() {
        let req = minimal_gemini_request();
        let canonical = ingress_generate(&req, "gemini-pro", Endpoint::GenerateContent);
        assert_eq!(canonical.model, "gemini-pro");
        assert!(!canonical.stream);
        assert_eq!(canonical.input.messages.len(), 1);
        assert_eq!(canonical.input.messages[0].role, Role::User);
    }

    #[test]
    fn test_ingress_with_system() {
        let req = GeminiRequest {
            system_instruction: Some(GeminiContent {
                role: None,
                parts: vec![GeminiPart::Text("You are helpful".to_string())],
            }),
            ..minimal_gemini_request()
        };
        let canonical = ingress_generate(&req, "gemini-pro", Endpoint::GenerateContent);
        assert!(canonical.input.system.is_some());
    }

    #[test]
    fn test_ingress_stream_endpoint() {
        let req = minimal_gemini_request();
        let canonical = ingress_generate(&req, "gemini-pro", Endpoint::StreamGenerateContent);
        assert!(canonical.stream);
    }

    #[test]
    fn test_ingress_with_tools() {
        let req = GeminiRequest {
            tools: Some(vec![GeminiToolDeclaration {
                function_declarations: vec![GeminiFunctionDeclaration {
                    name: "get_weather".to_string(),
                    description: "Get weather".to_string(),
                    parameters: Some(serde_json::json!({"type": "object"})),
                }],
            }]),
            ..minimal_gemini_request()
        };
        let canonical = ingress_generate(&req, "gemini-pro", Endpoint::GenerateContent);
        assert_eq!(canonical.tools.len(), 1);
        assert_eq!(canonical.tools[0].name, "get_weather");
    }

    #[test]
    fn test_egress_response() {
        let canonical = CanonicalResponse {
            id: "resp-1".to_string(),
            model: "gemini-pro".to_string(),
            content: vec![ContentBlock::Text {
                text: "Hello!".to_string(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
                ..Default::default()
            },
            execution_mode: prism_domain::operation::ExecutionMode::Native,
            provider: "gemini".to_string(),
            credential: "cred-1".to_string(),
        };
        let resp = egress_response(&canonical);
        let candidates = resp.candidates.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].finish_reason, Some("STOP".to_string()));
    }

    #[test]
    fn test_egress_stream_text() {
        let events = egress_event(
            &CanonicalEvent::TextDelta {
                index: 0,
                text: "Hi".to_string(),
            },
            "gemini-pro",
        );
        assert_eq!(events.len(), 1);
        let json: serde_json::Value = serde_json::from_str(&events[0]).unwrap();
        assert_eq!(
            json["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .unwrap(),
            "Hi"
        );
    }

    // ── Provider-facing tests ──

    #[test]
    fn test_egress_request_basic() {
        let canonical = ingress_generate(
            &minimal_gemini_request(),
            "gemini-pro",
            Endpoint::GenerateContent,
        );
        let wire = egress_request(&canonical);
        assert_eq!(wire.contents.len(), 1);
        assert_eq!(wire.contents[0].role.as_deref(), Some("user"));
    }

    #[test]
    fn test_egress_request_with_system() {
        let req = GeminiRequest {
            system_instruction: Some(GeminiContent {
                role: None,
                parts: vec![GeminiPart::Text("Be helpful".to_string())],
            }),
            ..minimal_gemini_request()
        };
        let canonical = ingress_generate(&req, "gemini-pro", Endpoint::GenerateContent);
        let wire = egress_request(&canonical);
        assert!(wire.system_instruction.is_some());
    }

    #[test]
    fn test_parse_response_basic() {
        let wire_resp = serde_json::json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hello!"}]
                },
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        });
        let data = serde_json::to_vec(&wire_resp).unwrap();
        let canonical = parse_response(&data, "gemini", "cred-1").unwrap();
        assert_eq!(canonical.content.len(), 1);
        assert_eq!(canonical.stop_reason, StopReason::EndTurn);
        assert_eq!(canonical.usage.input_tokens, 10);
    }

    #[test]
    fn test_parse_event_text() {
        let data = r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"Hi"}]},"finishReason":null,"index":0}]}"#;
        let event = parse_event(data).unwrap();
        assert!(matches!(event, CanonicalEvent::TextDelta { text, .. } if text == "Hi"));
    }

    #[test]
    fn test_parse_event_finish() {
        let data = r#"{"candidates":[{"content":{"role":"model","parts":[]},"finishReason":"STOP","index":0}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5,"totalTokenCount":15}}"#;
        let event = parse_event(data).unwrap();
        if let CanonicalEvent::StreamEnd { stop_reason, usage } = event {
            assert_eq!(stop_reason, StopReason::EndTurn);
            assert_eq!(usage.input_tokens, 10);
        } else {
            panic!("expected StreamEnd");
        }
    }
}
