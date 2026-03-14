use prism_domain::content::{
    ContentBlock, Conversation, ImageSource, Message, Role, SystemContent,
};
use prism_domain::event::CanonicalEvent;
use prism_domain::operation::Endpoint;
use prism_domain::request::{CanonicalRequest, ReasoningConfig, RequestLimits};
use prism_domain::response::{CanonicalResponse, StopReason};
use prism_domain::tool::{ResponseFormat, ToolChoice, ToolSpec};
use prism_types::types::claude::*;

// ─── Ingress: Claude → Canonical ────────────────────────────────────────────

/// Parse a ClaudeMessagesRequest into a CanonicalRequest.
pub fn ingress_messages(req: &ClaudeMessagesRequest, endpoint: Endpoint) -> CanonicalRequest {
    let system = convert_system(&req.system);
    let messages = convert_messages(&req.messages);
    let tools = convert_tools(&req.tools);
    let tool_choice = convert_tool_choice(&req.tool_choice);
    let reasoning = extract_reasoning(req);

    CanonicalRequest {
        ingress_protocol: prism_domain::operation::IngressProtocol::Claude,
        operation: endpoint.operation(),
        endpoint,
        model: req.model.clone(),
        stream: req.stream.unwrap_or(false),
        input: Conversation { system, messages },
        tools,
        tool_choice,
        response_format: ResponseFormat::Text,
        reasoning,
        limits: RequestLimits {
            max_tokens: Some(req.max_tokens),
            temperature: req.temperature,
            top_p: req.top_p,
            top_k: req.top_k,
            stop: req.stop_sequences.clone().unwrap_or_default(),
        },
        tenant_id: None,
        api_key_id: None,
        region: None,
        raw_body: None,
    }
}

fn convert_system(system: &Option<ClaudeSystem>) -> Option<SystemContent> {
    system.as_ref().map(|s| match s {
        ClaudeSystem::Text(t) => SystemContent::Text(t.clone()),
        ClaudeSystem::Blocks(blocks) => {
            SystemContent::Blocks(blocks.iter().map(convert_claude_content).collect())
        }
    })
}

fn convert_messages(messages: &[ClaudeMessage]) -> Vec<Message> {
    messages
        .iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "assistant" => Role::Assistant,
                _ => Role::User,
            };

            let content = match &m.content {
                ClaudeMessageContent::Text(t) => vec![ContentBlock::Text { text: t.clone() }],
                ClaudeMessageContent::Blocks(blocks) => {
                    blocks.iter().map(convert_claude_content).collect()
                }
            };

            Message {
                role,
                content,
                name: None,
            }
        })
        .collect()
}

fn convert_claude_content(block: &ClaudeContent) -> ContentBlock {
    match block {
        ClaudeContent::Text { text, .. } => ContentBlock::Text { text: text.clone() },
        ClaudeContent::Image { source } => ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: source.media_type.clone(),
                data: source.data.clone(),
            },
        },
        ClaudeContent::ToolUse { id, name, input } => ContentBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        ClaudeContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let inner = content
                .as_ref()
                .map(|c| match c {
                    ClaudeMessageContent::Text(t) => {
                        vec![ContentBlock::Text { text: t.clone() }]
                    }
                    ClaudeMessageContent::Blocks(blocks) => {
                        blocks.iter().map(convert_claude_content).collect()
                    }
                })
                .unwrap_or_default();
            ContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: inner,
                is_error: is_error.unwrap_or(false),
            }
        }
        ClaudeContent::Thinking {
            thinking,
            signature,
        } => ContentBlock::Thinking {
            thinking: thinking.clone(),
            signature: signature.clone(),
        },
        ClaudeContent::RedactedThinking { data } => {
            ContentBlock::RedactedThinking { data: data.clone() }
        }
    }
}

fn convert_tools(tools: &Option<Vec<ClaudeTool>>) -> Vec<ToolSpec> {
    tools
        .as_ref()
        .map(|ts| {
            ts.iter()
                .map(|t| ToolSpec {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn convert_tool_choice(tc: &Option<serde_json::Value>) -> ToolChoice {
    match tc {
        None => ToolChoice::Auto,
        Some(v) => match v.get("type").and_then(|t| t.as_str()) {
            Some("none") => ToolChoice::None,
            Some("any") => ToolChoice::Required,
            Some("tool") => {
                if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                    ToolChoice::Tool {
                        name: name.to_string(),
                    }
                } else {
                    ToolChoice::Required
                }
            }
            _ => ToolChoice::Auto,
        },
    }
}

fn extract_reasoning(req: &ClaudeMessagesRequest) -> Option<ReasoningConfig> {
    req.extra.get("thinking").map(|v| {
        let enabled = v.get("type").and_then(|t| t.as_str()) == Some("enabled");
        let budget = v.get("budget_tokens").and_then(|b| b.as_u64());
        ReasoningConfig {
            enabled,
            budget_tokens: budget,
            effort: None,
        }
    })
}

fn stop_reason_to_claude(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::EndTurn => "end_turn",
        StopReason::StopSequence => "stop_sequence",
        StopReason::MaxTokens => "max_tokens",
        StopReason::ToolUse => "tool_use",
    }
}

fn claude_to_stop_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("end_turn") => StopReason::EndTurn,
        Some("stop_sequence") => StopReason::StopSequence,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("tool_use") => StopReason::ToolUse,
        _ => StopReason::EndTurn,
    }
}

// ─── Egress: Canonical → Claude ─────────────────────────────────────────────

/// Convert a CanonicalResponse into a ClaudeMessagesResponse.
pub fn egress_response(resp: &CanonicalResponse) -> ClaudeMessagesResponse {
    let content: Vec<ClaudeContent> = resp
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(ClaudeContent::Text {
                text: text.clone(),
                extra: Default::default(),
            }),
            ContentBlock::ToolUse { id, name, input } => Some(ClaudeContent::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            ContentBlock::Thinking {
                thinking,
                signature,
            } => Some(ClaudeContent::Thinking {
                thinking: thinking.clone(),
                signature: signature.clone(),
            }),
            ContentBlock::RedactedThinking { data } => {
                Some(ClaudeContent::RedactedThinking { data: data.clone() })
            }
            _ => None,
        })
        .collect();

    ClaudeMessagesResponse {
        id: resp.id.clone(),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        model: resp.model.clone(),
        content,
        stop_reason: Some(stop_reason_to_claude(&resp.stop_reason).to_string()),
        stop_sequence: None,
        usage: ClaudeUsage {
            input_tokens: resp.usage.input_tokens,
            output_tokens: resp.usage.output_tokens,
            cache_creation_input_tokens: resp.usage.cache_creation_tokens,
            cache_read_input_tokens: resp.usage.cache_read_tokens,
        },
    }
}

/// Convert a CanonicalEvent into Claude SSE event (event_type, data_json).
pub fn egress_event(event: &CanonicalEvent) -> Vec<(String, String)> {
    match event {
        CanonicalEvent::StreamStart { id, model } => {
            let msg = serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": [],
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": { "input_tokens": 0, "output_tokens": 0 }
                }
            });
            vec![(
                "message_start".into(),
                serde_json::to_string(&msg).unwrap_or_default(),
            )]
        }
        CanonicalEvent::ContentBlockStart { index, block } => {
            let cb = match block {
                ContentBlock::Text { text } => serde_json::json!({
                    "type": "text",
                    "text": text
                }),
                ContentBlock::ToolUse { id, name, .. } => serde_json::json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": {}
                }),
                ContentBlock::Thinking { .. } => serde_json::json!({
                    "type": "thinking",
                    "thinking": ""
                }),
                _ => serde_json::json!({}),
            };
            let data = serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": cb
            });
            vec![(
                "content_block_start".into(),
                serde_json::to_string(&data).unwrap_or_default(),
            )]
        }
        CanonicalEvent::TextDelta { index, text } => {
            let data = serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": { "type": "text_delta", "text": text }
            });
            vec![(
                "content_block_delta".into(),
                serde_json::to_string(&data).unwrap_or_default(),
            )]
        }
        CanonicalEvent::ThinkingDelta { index, thinking } => {
            let data = serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": { "type": "thinking_delta", "thinking": thinking }
            });
            vec![(
                "content_block_delta".into(),
                serde_json::to_string(&data).unwrap_or_default(),
            )]
        }
        CanonicalEvent::ToolInputDelta {
            index,
            partial_json,
        } => {
            let data = serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": { "type": "input_json_delta", "partial_json": partial_json }
            });
            vec![(
                "content_block_delta".into(),
                serde_json::to_string(&data).unwrap_or_default(),
            )]
        }
        CanonicalEvent::ContentBlockStop { index } => {
            let data = serde_json::json!({
                "type": "content_block_stop",
                "index": index
            });
            vec![(
                "content_block_stop".into(),
                serde_json::to_string(&data).unwrap_or_default(),
            )]
        }
        CanonicalEvent::StreamEnd { stop_reason, usage } => {
            let delta = serde_json::json!({
                "type": "message_delta",
                "delta": { "stop_reason": stop_reason_to_claude(stop_reason) },
                "usage": { "output_tokens": usage.output_tokens }
            });
            let stop = serde_json::json!({ "type": "message_stop" });
            vec![
                (
                    "message_delta".into(),
                    serde_json::to_string(&delta).unwrap_or_default(),
                ),
                (
                    "message_stop".into(),
                    serde_json::to_string(&stop).unwrap_or_default(),
                ),
            ]
        }
        CanonicalEvent::Ping => {
            vec![("ping".into(), r#"{"type":"ping"}"#.into())]
        }
    }
}

// ─── Provider-facing: Canonical → Claude request ────────────────────────────

/// Convert a CanonicalRequest into a ClaudeMessagesRequest (to send TO Claude).
pub fn egress_request(canonical: &CanonicalRequest) -> ClaudeMessagesRequest {
    let system = canonical.input.system.as_ref().map(|sys| match sys {
        SystemContent::Text(t) => ClaudeSystem::Text(t.clone()),
        SystemContent::Blocks(blocks) => ClaudeSystem::Blocks(
            blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(ClaudeContent::Text {
                        text: text.clone(),
                        extra: Default::default(),
                    }),
                    _ => None,
                })
                .collect(),
        ),
    });

    let messages: Vec<ClaudeMessage> = canonical
        .input
        .messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                Role::User | Role::Tool => "user",
                Role::Assistant => "assistant",
            };

            let content =
                if msg.content.len() == 1 && matches!(&msg.content[0], ContentBlock::Text { .. }) {
                    if let ContentBlock::Text { text } = &msg.content[0] {
                        ClaudeMessageContent::Text(text.clone())
                    } else {
                        unreachable!()
                    }
                } else {
                    ClaudeMessageContent::Blocks(
                        msg.content.iter().map(canonical_block_to_claude).collect(),
                    )
                };

            ClaudeMessage {
                role: role.to_string(),
                content,
            }
        })
        .collect();

    let tools = if canonical.tools.is_empty() {
        None
    } else {
        Some(
            canonical
                .tools
                .iter()
                .map(|t| ClaudeTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.parameters.clone(),
                })
                .collect(),
        )
    };

    let tool_choice = match &canonical.tool_choice {
        ToolChoice::None => Some(serde_json::json!({"type": "none"})),
        ToolChoice::Required => Some(serde_json::json!({"type": "any"})),
        ToolChoice::Tool { name } => Some(serde_json::json!({"type": "tool", "name": name})),
        ToolChoice::Auto => None,
    };

    ClaudeMessagesRequest {
        model: canonical.model.clone(),
        messages,
        max_tokens: canonical.limits.max_tokens.unwrap_or(4096),
        system,
        temperature: canonical.limits.temperature,
        top_p: canonical.limits.top_p,
        top_k: canonical.limits.top_k,
        stop_sequences: if canonical.limits.stop.is_empty() {
            None
        } else {
            Some(canonical.limits.stop.clone())
        },
        stream: Some(canonical.stream),
        tools,
        tool_choice,
        metadata: None,
        extra: Default::default(),
    }
}

fn canonical_block_to_claude(block: &ContentBlock) -> ClaudeContent {
    match block {
        ContentBlock::Text { text } => ClaudeContent::Text {
            text: text.clone(),
            extra: Default::default(),
        },
        ContentBlock::Image { source } => match source {
            ImageSource::Base64 { media_type, data } => ClaudeContent::Image {
                source: prism_types::types::claude::ImageSource {
                    source_type: "base64".to_string(),
                    media_type: media_type.clone(),
                    data: data.clone(),
                },
            },
            // Claude API only supports base64 images; URL images cannot be converted
            ImageSource::Url { .. } => ClaudeContent::Text {
                text: "[image: unsupported URL source]".to_string(),
                extra: Default::default(),
            },
        },
        ContentBlock::ToolUse { id, name, input } => ClaudeContent::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => ClaudeContent::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: Some(ClaudeMessageContent::Blocks(
                content.iter().map(canonical_block_to_claude).collect(),
            )),
            is_error: Some(*is_error),
        },
        ContentBlock::Thinking {
            thinking,
            signature,
        } => ClaudeContent::Thinking {
            thinking: thinking.clone(),
            signature: signature.clone(),
        },
        ContentBlock::RedactedThinking { data } => {
            ClaudeContent::RedactedThinking { data: data.clone() }
        }
        _ => ClaudeContent::Text {
            text: String::new(),
            extra: Default::default(),
        },
    }
}

// ─── Provider-facing: Claude response → Canonical ───────────────────────────

/// Parse a Claude API response into a CanonicalResponse.
pub fn parse_response(
    data: &[u8],
    provider: &str,
    credential: &str,
) -> Result<CanonicalResponse, String> {
    let resp: ClaudeMessagesResponse = serde_json::from_slice(data)
        .map_err(|e| format!("failed to parse Claude response: {e}"))?;

    let content: Vec<ContentBlock> = resp
        .content
        .iter()
        .filter_map(|block| match block {
            ClaudeContent::Text { text, .. } => Some(ContentBlock::Text { text: text.clone() }),
            ClaudeContent::ToolUse { id, name, input } => Some(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            ClaudeContent::Thinking {
                thinking,
                signature,
            } => Some(ContentBlock::Thinking {
                thinking: thinking.clone(),
                signature: signature.clone(),
            }),
            ClaudeContent::RedactedThinking { data } => {
                Some(ContentBlock::RedactedThinking { data: data.clone() })
            }
            _ => None,
        })
        .collect();

    let stop_reason = claude_to_stop_reason(resp.stop_reason.as_deref());

    let usage = prism_domain::response::Usage {
        input_tokens: resp.usage.input_tokens,
        output_tokens: resp.usage.output_tokens,
        cache_creation_tokens: resp.usage.cache_creation_input_tokens,
        cache_read_tokens: resp.usage.cache_read_input_tokens,
        reasoning_tokens: 0,
    };

    Ok(CanonicalResponse {
        id: resp.id,
        model: resp.model,
        content,
        stop_reason,
        usage,
        execution_mode: prism_domain::operation::ExecutionMode::Native,
        provider: provider.to_string(),
        credential: credential.to_string(),
    })
}

/// Parse a Claude SSE event into a CanonicalEvent.
pub fn parse_event(event_type: &str, data: &str) -> Option<CanonicalEvent> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;

    match event_type {
        "message_start" => {
            let msg = v.get("message")?;
            let id = msg.get("id")?.as_str()?.to_string();
            let model = msg.get("model")?.as_str()?.to_string();
            Some(CanonicalEvent::StreamStart { id, model })
        }
        "content_block_start" => {
            let index = v.get("index")?.as_u64()? as u32;
            let cb = v.get("content_block")?;
            let block = match cb.get("type")?.as_str()? {
                "text" => ContentBlock::Text {
                    text: String::new(),
                },
                "tool_use" => ContentBlock::ToolUse {
                    id: cb.get("id")?.as_str()?.to_string(),
                    name: cb.get("name")?.as_str()?.to_string(),
                    input: serde_json::Value::Null,
                },
                "thinking" => ContentBlock::Thinking {
                    thinking: String::new(),
                    signature: None,
                },
                _ => return None,
            };
            Some(CanonicalEvent::ContentBlockStart { index, block })
        }
        "content_block_delta" => {
            let index = v.get("index")?.as_u64()? as u32;
            let delta = v.get("delta")?;
            match delta.get("type")?.as_str()? {
                "text_delta" => Some(CanonicalEvent::TextDelta {
                    index,
                    text: delta.get("text")?.as_str()?.to_string(),
                }),
                "thinking_delta" => Some(CanonicalEvent::ThinkingDelta {
                    index,
                    thinking: delta.get("thinking")?.as_str()?.to_string(),
                }),
                "input_json_delta" => Some(CanonicalEvent::ToolInputDelta {
                    index,
                    partial_json: delta.get("partial_json")?.as_str()?.to_string(),
                }),
                _ => None,
            }
        }
        "content_block_stop" => {
            let index = v.get("index")?.as_u64()? as u32;
            Some(CanonicalEvent::ContentBlockStop { index })
        }
        "message_delta" => {
            let delta = v.get("delta")?;
            let stop_reason = claude_to_stop_reason(delta.get("stop_reason")?.as_str());
            let usage = v
                .get("usage")
                .map(|u| prism_domain::response::Usage {
                    input_tokens: u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0),
                    output_tokens: u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0),
                    ..Default::default()
                })
                .unwrap_or_default();
            Some(CanonicalEvent::StreamEnd { stop_reason, usage })
        }
        "ping" => Some(CanonicalEvent::Ping),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_domain::response::Usage;

    fn minimal_claude_request() -> ClaudeMessagesRequest {
        ClaudeMessagesRequest {
            model: "claude-sonnet-4-5".to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: ClaudeMessageContent::Text("Hello".to_string()),
            }],
            max_tokens: 1024,
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: None,
            tools: None,
            tool_choice: None,
            metadata: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_ingress_basic() {
        let req = minimal_claude_request();
        let canonical = ingress_messages(&req, Endpoint::Messages);
        assert_eq!(canonical.model, "claude-sonnet-4-5");
        assert!(!canonical.stream);
        assert_eq!(canonical.limits.max_tokens, Some(1024));
        assert_eq!(canonical.input.messages.len(), 1);
    }

    #[test]
    fn test_ingress_with_system() {
        let req = ClaudeMessagesRequest {
            system: Some(ClaudeSystem::Text("You are helpful".to_string())),
            ..minimal_claude_request()
        };
        let canonical = ingress_messages(&req, Endpoint::Messages);
        assert!(canonical.input.system.is_some());
    }

    #[test]
    fn test_ingress_thinking() {
        let mut req = minimal_claude_request();
        req.messages = vec![ClaudeMessage {
            role: "assistant".to_string(),
            content: ClaudeMessageContent::Blocks(vec![
                ClaudeContent::Thinking {
                    thinking: "Let me think...".to_string(),
                    signature: Some("sig123".to_string()),
                },
                ClaudeContent::Text {
                    text: "Answer".to_string(),
                    extra: Default::default(),
                },
            ]),
        }];
        let canonical = ingress_messages(&req, Endpoint::Messages);
        assert_eq!(canonical.input.messages[0].content.len(), 2);
        assert!(matches!(
            &canonical.input.messages[0].content[0],
            ContentBlock::Thinking { .. }
        ));
    }

    #[test]
    fn test_egress_response() {
        let canonical = CanonicalResponse {
            id: "msg-1".to_string(),
            model: "claude-sonnet-4-5".to_string(),
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
            provider: "anthropic".to_string(),
            credential: "cred-1".to_string(),
        };
        let resp = egress_response(&canonical);
        assert_eq!(resp.id, "msg-1");
        assert_eq!(resp.stop_reason, Some("end_turn".to_string()));
        assert_eq!(resp.content.len(), 1);
    }

    #[test]
    fn test_egress_stream_events() {
        let events = egress_event(&CanonicalEvent::TextDelta {
            index: 0,
            text: "Hi".to_string(),
        });
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "content_block_delta");
        let json: serde_json::Value = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(json["delta"]["text"].as_str().unwrap(), "Hi");
    }

    // ── Provider-facing tests ──

    #[test]
    fn test_egress_request_basic() {
        let canonical = ingress_messages(&minimal_claude_request(), Endpoint::Messages);
        let wire = egress_request(&canonical);
        assert_eq!(wire.model, "claude-sonnet-4-5");
        assert_eq!(wire.max_tokens, 1024);
        assert_eq!(wire.messages.len(), 1);
    }

    #[test]
    fn test_egress_request_with_tools() {
        let mut req = minimal_claude_request();
        req.tools = Some(vec![ClaudeTool {
            name: "get_weather".to_string(),
            description: Some("Get weather info".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
        }]);
        let canonical = ingress_messages(&req, Endpoint::Messages);
        let wire = egress_request(&canonical);
        assert_eq!(wire.tools.as_ref().unwrap().len(), 1);
        assert_eq!(wire.tools.as_ref().unwrap()[0].name, "get_weather");
    }

    #[test]
    fn test_parse_response_basic() {
        let wire_resp = serde_json::json!({
            "id": "msg-123",
            "type": "message",
            "role": "assistant",
            "model": "claude-sonnet-4-5",
            "content": [{"type": "text", "text": "Hello!"}],
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 0
            }
        });
        let data = serde_json::to_vec(&wire_resp).unwrap();
        let canonical = parse_response(&data, "anthropic", "cred-1").unwrap();
        assert_eq!(canonical.id, "msg-123");
        assert_eq!(canonical.model, "claude-sonnet-4-5");
        assert_eq!(canonical.content.len(), 1);
        assert_eq!(canonical.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn test_parse_event_text_delta() {
        let data =
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#;
        let event = parse_event("content_block_delta", data).unwrap();
        assert!(matches!(event, CanonicalEvent::TextDelta { text, .. } if text == "Hi"));
    }

    #[test]
    fn test_parse_event_message_start() {
        let data = r#"{"type":"message_start","message":{"id":"msg-1","type":"message","role":"assistant","model":"claude-sonnet-4-5","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":0}}}"#;
        let event = parse_event("message_start", data).unwrap();
        if let CanonicalEvent::StreamStart { id, model } = event {
            assert_eq!(id, "msg-1");
            assert_eq!(model, "claude-sonnet-4-5");
        } else {
            panic!("expected StreamStart");
        }
    }
}
