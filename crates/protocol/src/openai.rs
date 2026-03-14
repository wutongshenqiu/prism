use prism_domain::content::{
    ContentBlock, Conversation, ImageSource, Message, Role, SystemContent,
};
use prism_domain::event::CanonicalEvent;
use prism_domain::operation::Endpoint;
use prism_domain::request::{CanonicalRequest, ReasoningConfig, RequestLimits};
use prism_domain::response::{CanonicalResponse, StopReason, Usage};
use prism_domain::tool::{ResponseFormat, ToolChoice, ToolSpec};
use prism_types::types::openai::*;

// ─── Ingress: OpenAI → Canonical ────────────────────────────────────────────

/// Parse an OpenAI ChatCompletionRequest into a CanonicalRequest.
pub fn ingress_chat(req: &ChatCompletionRequest, endpoint: Endpoint) -> CanonicalRequest {
    let system = extract_system(&req.messages);
    let messages = convert_messages(&req.messages);
    let tools = convert_tools(&req.tools);
    let tool_choice = convert_tool_choice(&req.tool_choice);
    let response_format = convert_response_format(&req.response_format);
    let reasoning = extract_reasoning(req);
    let stop = match &req.stop {
        Some(StopSequence::Single(s)) => vec![s.clone()],
        Some(StopSequence::Multiple(v)) => v.clone(),
        None => vec![],
    };

    CanonicalRequest {
        ingress_protocol: prism_domain::operation::IngressProtocol::OpenAi,
        operation: endpoint.operation(),
        endpoint,
        model: req.model.clone(),
        stream: req.stream.unwrap_or(false),
        input: Conversation { system, messages },
        tools,
        tool_choice,
        response_format,
        reasoning,
        limits: RequestLimits {
            max_tokens: req.max_completion_tokens.or(req.max_tokens),
            temperature: req.temperature,
            top_p: req.top_p,
            top_k: None,
            stop,
        },
        tenant_id: None,
        api_key_id: None,
        region: None,
        raw_body: None,
    }
}

fn extract_system(messages: &[ChatMessage]) -> Option<SystemContent> {
    messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| match &m.content {
            Some(MessageContent::Text(t)) => SystemContent::Text(t.clone()),
            _ => SystemContent::Text(String::new()),
        })
}

fn convert_messages(messages: &[ChatMessage]) -> Vec<Message> {
    messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            let role = match m.role.as_str() {
                "assistant" => Role::Assistant,
                "tool" | "function" => Role::Tool,
                _ => Role::User,
            };

            let content = match &m.content {
                Some(MessageContent::Text(t)) => vec![ContentBlock::Text { text: t.clone() }],
                Some(MessageContent::Parts(parts)) => parts.iter().map(convert_part).collect(),
                None => vec![],
            };

            // Add tool calls if present
            let mut blocks = content;
            if let Some(tool_calls) = &m.tool_calls {
                for tc in tool_calls {
                    let input: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                    blocks.push(ContentBlock::ToolUse {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        input,
                    });
                }
            }

            // Tool result
            if role == Role::Tool
                && let Some(ref tool_call_id) = m.tool_call_id
            {
                let inner = std::mem::take(&mut blocks);
                blocks = vec![ContentBlock::ToolResult {
                    tool_use_id: tool_call_id.clone(),
                    content: inner,
                    is_error: false,
                }];
            }

            Message {
                role,
                content: blocks,
                name: m.name.clone(),
            }
        })
        .collect()
}

fn convert_part(part: &ContentPart) -> ContentBlock {
    match part {
        ContentPart::Text { text } => ContentBlock::Text { text: text.clone() },
        ContentPart::ImageUrl { image_url } => ContentBlock::Image {
            source: ImageSource::Url {
                url: image_url.url.clone(),
            },
        },
    }
}

fn convert_tools(tools: &Option<Vec<Tool>>) -> Vec<ToolSpec> {
    tools
        .as_ref()
        .map(|ts| {
            ts.iter()
                .map(|t| ToolSpec {
                    name: t.function.name.clone(),
                    description: t.function.description.clone(),
                    parameters: t.function.parameters.clone().unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn convert_tool_choice(tc: &Option<serde_json::Value>) -> ToolChoice {
    match tc {
        None => ToolChoice::Auto,
        Some(v) if v.is_string() => match v.as_str().unwrap_or("auto") {
            "none" => ToolChoice::None,
            "required" => ToolChoice::Required,
            _ => ToolChoice::Auto,
        },
        Some(v) => {
            if let Some(name) = v
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                ToolChoice::Tool {
                    name: name.to_string(),
                }
            } else {
                ToolChoice::Auto
            }
        }
    }
}

fn convert_response_format(rf: &Option<serde_json::Value>) -> ResponseFormat {
    match rf {
        None => ResponseFormat::Text,
        Some(v) => match v.get("type").and_then(|t| t.as_str()) {
            Some("json_schema") => {
                let schema = v.get("json_schema").and_then(|s| s.get("schema")).cloned();
                let name = v
                    .get("json_schema")
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from);
                let strict = v
                    .get("json_schema")
                    .and_then(|s| s.get("strict"))
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                ResponseFormat::JsonSchema {
                    schema,
                    name,
                    strict,
                }
            }
            Some("json_object") => ResponseFormat::JsonObject,
            _ => ResponseFormat::Text,
        },
    }
}

fn extract_reasoning(req: &ChatCompletionRequest) -> Option<ReasoningConfig> {
    req.extra
        .get("reasoning_effort")
        .map(|effort| ReasoningConfig {
            enabled: true,
            budget_tokens: None,
            effort: effort.as_str().map(String::from),
        })
}

fn stop_reason_to_openai(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::EndTurn | StopReason::StopSequence => "stop",
        StopReason::MaxTokens => "length",
        StopReason::ToolUse => "tool_calls",
    }
}

fn openai_to_stop_reason(s: Option<&str>) -> StopReason {
    match s {
        Some("stop") => StopReason::EndTurn,
        Some("length") => StopReason::MaxTokens,
        Some("tool_calls") => StopReason::ToolUse,
        _ => StopReason::EndTurn,
    }
}

// ─── Egress: Canonical → OpenAI ─────────────────────────────────────────────

/// Convert a CanonicalResponse into an OpenAI ChatCompletionResponse.
pub fn egress_response(resp: &CanonicalResponse) -> ChatCompletionResponse {
    let mut message_content = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for block in &resp.content {
        match block {
            ContentBlock::Text { text } => {
                if !message_content.is_empty() {
                    message_content.push('\n');
                }
                message_content.push_str(text);
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id: id.clone(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: name.clone(),
                        arguments: serde_json::to_string(input).unwrap_or_default(),
                    },
                });
            }
            _ => {}
        }
    }

    let finish_reason = stop_reason_to_openai(&resp.stop_reason).to_string();

    ChatCompletionResponse {
        id: resp.id.clone(),
        object: "chat.completion".to_string(),
        created: chrono_now_secs(),
        model: resp.model.clone(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: if message_content.is_empty() {
                    None
                } else {
                    Some(MessageContent::Text(message_content))
                },
                name: None,
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
                tool_call_id: None,
                extra: Default::default(),
            },
            finish_reason: Some(finish_reason),
        }],
        usage: Some(openai_usage(&resp.usage)),
        system_fingerprint: None,
    }
}

/// Convert a CanonicalEvent into OpenAI SSE chunk JSON lines.
pub fn egress_event(event: &CanonicalEvent, model: &str) -> Vec<String> {
    match event {
        CanonicalEvent::StreamStart { id, .. } => {
            let chunk = serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": chrono_now_secs(),
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": { "role": "assistant", "content": "" },
                    "finish_reason": null
                }]
            });
            vec![serde_json::to_string(&chunk).unwrap_or_default()]
        }
        CanonicalEvent::TextDelta { text, .. } => {
            let chunk = serde_json::json!({
                "id": "",
                "object": "chat.completion.chunk",
                "created": chrono_now_secs(),
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": { "content": text },
                    "finish_reason": null
                }]
            });
            vec![serde_json::to_string(&chunk).unwrap_or_default()]
        }
        CanonicalEvent::ToolInputDelta {
            index,
            partial_json,
        } => {
            let chunk = serde_json::json!({
                "id": "",
                "object": "chat.completion.chunk",
                "created": chrono_now_secs(),
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [{
                            "index": index,
                            "function": { "arguments": partial_json }
                        }]
                    },
                    "finish_reason": null
                }]
            });
            vec![serde_json::to_string(&chunk).unwrap_or_default()]
        }
        CanonicalEvent::ContentBlockStart { index, block } => {
            if let ContentBlock::ToolUse { id, name, .. } = block {
                let chunk = serde_json::json!({
                    "id": "",
                    "object": "chat.completion.chunk",
                    "created": chrono_now_secs(),
                    "model": model,
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "tool_calls": [{
                                "index": index,
                                "id": id,
                                "type": "function",
                                "function": { "name": name, "arguments": "" }
                            }]
                        },
                        "finish_reason": null
                    }]
                });
                vec![serde_json::to_string(&chunk).unwrap_or_default()]
            } else {
                vec![]
            }
        }
        CanonicalEvent::StreamEnd { stop_reason, usage } => {
            let finish_reason = stop_reason_to_openai(stop_reason);
            let chunk = serde_json::json!({
                "id": "",
                "object": "chat.completion.chunk",
                "created": chrono_now_secs(),
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": finish_reason
                }],
                "usage": {
                    "prompt_tokens": usage.input_tokens,
                    "completion_tokens": usage.output_tokens,
                    "total_tokens": usage.input_tokens + usage.output_tokens
                }
            });
            vec![serde_json::to_string(&chunk).unwrap_or_default()]
        }
        CanonicalEvent::Ping | CanonicalEvent::ContentBlockStop { .. } => vec![],
        CanonicalEvent::ThinkingDelta { .. } => vec![],
    }
}

fn openai_usage(usage: &Usage) -> prism_types::types::openai::Usage {
    prism_types::types::openai::Usage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.input_tokens + usage.output_tokens,
        prompt_tokens_details: None,
        completion_tokens_details: None,
    }
}

fn chrono_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ─── Provider-facing: Canonical → OpenAI request ────────────────────────────

/// Convert a CanonicalRequest into an OpenAI ChatCompletionRequest (to send TO an OpenAI provider).
pub fn egress_request(canonical: &CanonicalRequest) -> ChatCompletionRequest {
    let mut messages = Vec::new();

    // System message
    if let Some(ref sys) = canonical.input.system {
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
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(MessageContent::Text(text)),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            extra: Default::default(),
        });
    }

    for msg in &canonical.input.messages {
        messages.push(canonical_msg_to_openai(msg));
    }

    let tools = if canonical.tools.is_empty() {
        None
    } else {
        Some(
            canonical
                .tools
                .iter()
                .map(|t| Tool {
                    tool_type: "function".to_string(),
                    function: FunctionDef {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: Some(t.parameters.clone()),
                    },
                })
                .collect(),
        )
    };

    let tool_choice = match &canonical.tool_choice {
        ToolChoice::Auto => None,
        ToolChoice::None => Some(serde_json::json!("none")),
        ToolChoice::Required => Some(serde_json::json!("required")),
        ToolChoice::Tool { name } => {
            Some(serde_json::json!({"type": "function", "function": {"name": name}}))
        }
    };

    let stop = if canonical.limits.stop.is_empty() {
        None
    } else if canonical.limits.stop.len() == 1 {
        Some(StopSequence::Single(canonical.limits.stop[0].clone()))
    } else {
        Some(StopSequence::Multiple(canonical.limits.stop.clone()))
    };

    let has_reasoning = canonical.reasoning.as_ref().is_some_and(|r| r.enabled);

    ChatCompletionRequest {
        model: canonical.model.clone(),
        messages,
        temperature: canonical.limits.temperature,
        top_p: canonical.limits.top_p,
        n: None,
        stream: Some(canonical.stream),
        stop,
        max_tokens: if has_reasoning {
            None
        } else {
            canonical.limits.max_tokens
        },
        max_completion_tokens: if has_reasoning {
            canonical.limits.max_tokens
        } else {
            None
        },
        presence_penalty: None,
        frequency_penalty: None,
        user: None,
        tools,
        tool_choice,
        response_format: None,
        extra: Default::default(),
    }
}

fn canonical_msg_to_openai(msg: &Message) -> ChatMessage {
    let role = match msg.role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };

    let mut text_parts = Vec::new();
    let mut tool_calls_vec = Vec::new();
    let mut tool_call_id = None;
    let mut has_image = false;
    let mut content_parts = Vec::new();

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
                content_parts.push(ContentPart::Text { text: text.clone() });
            }
            ContentBlock::Image { source } => {
                has_image = true;
                let url = match source {
                    ImageSource::Url { url } => url.clone(),
                    ImageSource::Base64 {
                        media_type, data, ..
                    } => format!("data:{media_type};base64,{data}"),
                };
                content_parts.push(ContentPart::ImageUrl {
                    image_url: ImageUrl { url, detail: None },
                });
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls_vec.push(ToolCall {
                    id: id.clone(),
                    call_type: "function".to_string(),
                    function: FunctionCall {
                        name: name.clone(),
                        arguments: serde_json::to_string(input).unwrap_or_default(),
                    },
                });
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                tool_call_id = Some(tool_use_id.clone());
                for c in content {
                    if let ContentBlock::Text { text } = c {
                        text_parts.push(text.clone());
                    }
                }
            }
            _ => {}
        }
    }

    let content = if has_image {
        Some(MessageContent::Parts(content_parts))
    } else if !text_parts.is_empty() {
        Some(MessageContent::Text(text_parts.join("")))
    } else {
        None
    };

    ChatMessage {
        role: role.to_string(),
        content,
        name: msg.name.clone(),
        tool_calls: if tool_calls_vec.is_empty() {
            None
        } else {
            Some(tool_calls_vec)
        },
        tool_call_id,
        extra: Default::default(),
    }
}

// ─── Provider-facing: OpenAI response → Canonical ───────────────────────────

/// Parse an OpenAI ChatCompletionResponse into a CanonicalResponse.
pub fn parse_response(
    data: &[u8],
    provider: &str,
    credential: &str,
) -> Result<CanonicalResponse, String> {
    let resp: ChatCompletionResponse = serde_json::from_slice(data)
        .map_err(|e| format!("failed to parse OpenAI response: {e}"))?;

    let choice = resp.choices.first();
    let mut content = Vec::new();

    if let Some(choice) = choice {
        if let Some(MessageContent::Text(ref text)) = choice.message.content
            && !text.is_empty()
        {
            content.push(ContentBlock::Text { text: text.clone() });
        }
        if let Some(ref tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                let input = serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                content.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input,
                });
            }
        }
    }

    let stop_reason = openai_to_stop_reason(choice.and_then(|c| c.finish_reason.as_deref()));

    let usage = resp
        .usage
        .map(|u| Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            ..Default::default()
        })
        .unwrap_or_default();

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

/// Parse an OpenAI SSE data line into a CanonicalEvent.
pub fn parse_event(data: &str) -> Option<CanonicalEvent> {
    if data == "[DONE]" {
        return None;
    }

    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    let choices = v.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let delta = choice.get("delta")?;
    let finish = choice.get("finish_reason");

    // Finish event
    if let Some(fr) = finish.and_then(|f| f.as_str()) {
        let stop_reason = openai_to_stop_reason(Some(fr));
        let usage = v
            .get("usage")
            .map(|u| Usage {
                input_tokens: u.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0),
                output_tokens: u
                    .get("completion_tokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0),
                ..Default::default()
            })
            .unwrap_or_default();
        return Some(CanonicalEvent::StreamEnd { stop_reason, usage });
    }

    // Role delta = stream start
    if delta.get("role").is_some() {
        let id = v
            .get("id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        let model = v
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        return Some(CanonicalEvent::StreamStart { id, model });
    }

    // Content delta
    if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
        return Some(CanonicalEvent::TextDelta {
            index: 0,
            text: text.to_string(),
        });
    }

    // Tool call delta
    if let Some(tool_calls) = delta.get("tool_calls").and_then(|tc| tc.as_array())
        && let Some(tc) = tool_calls.first()
    {
        let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
        if let Some(func) = tc.get("function") {
            if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                let id = tc
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                return Some(CanonicalEvent::ContentBlockStart {
                    index,
                    block: ContentBlock::ToolUse {
                        id,
                        name: name.to_string(),
                        input: serde_json::Value::Null,
                    },
                });
            }
            if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                return Some(CanonicalEvent::ToolInputDelta {
                    index,
                    partial_json: args.to_string(),
                });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_chat_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                extra: Default::default(),
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: Some(false),
            stop: None,
            max_tokens: Some(100),
            max_completion_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            user: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn test_ingress_basic() {
        let req = minimal_chat_request();
        let canonical = ingress_chat(&req, Endpoint::ChatCompletions);
        assert_eq!(canonical.model, "gpt-4");
        assert!(!canonical.stream);
        assert_eq!(canonical.limits.max_tokens, Some(100));
        assert_eq!(canonical.input.messages.len(), 1);
        assert_eq!(canonical.input.messages[0].role, Role::User);
    }

    #[test]
    fn test_ingress_with_system() {
        let req = ChatCompletionRequest {
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Some(MessageContent::Text("You are helpful".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    extra: Default::default(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Some(MessageContent::Text("Hi".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    extra: Default::default(),
                },
            ],
            ..minimal_chat_request()
        };
        let canonical = ingress_chat(&req, Endpoint::ChatCompletions);
        assert!(canonical.input.system.is_some());
        // System messages are filtered from the messages list
        assert_eq!(canonical.input.messages.len(), 1);
    }

    #[test]
    fn test_ingress_with_tools() {
        let req = ChatCompletionRequest {
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: "get_weather".to_string(),
                    description: Some("Get weather".to_string()),
                    parameters: Some(serde_json::json!({"type": "object"})),
                },
            }]),
            ..minimal_chat_request()
        };
        let canonical = ingress_chat(&req, Endpoint::ChatCompletions);
        assert_eq!(canonical.tools.len(), 1);
        assert_eq!(canonical.tools[0].name, "get_weather");
    }

    #[test]
    fn test_egress_response() {
        let canonical = CanonicalResponse {
            id: "resp-1".to_string(),
            model: "gpt-4".to_string(),
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
            provider: "openai".to_string(),
            credential: "cred-1".to_string(),
        };
        let resp = egress_response(&canonical);
        assert_eq!(resp.id, "resp-1");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].finish_reason, Some("stop".to_string()));
    }

    #[test]
    fn test_egress_stream_events() {
        let events = egress_event(
            &CanonicalEvent::TextDelta {
                index: 0,
                text: "Hi".to_string(),
            },
            "gpt-4",
        );
        assert_eq!(events.len(), 1);
        let json: serde_json::Value = serde_json::from_str(&events[0]).unwrap();
        assert_eq!(
            json["choices"][0]["delta"]["content"].as_str().unwrap(),
            "Hi"
        );
    }

    // ── Provider-facing tests ──

    #[test]
    fn test_egress_request_basic() {
        let canonical = ingress_chat(&minimal_chat_request(), Endpoint::ChatCompletions);
        let wire = egress_request(&canonical);
        assert_eq!(wire.model, "gpt-4");
        assert_eq!(wire.messages.len(), 1);
        assert_eq!(wire.messages[0].role, "user");
    }

    #[test]
    fn test_egress_request_with_system() {
        let mut req = minimal_chat_request();
        req.messages.insert(
            0,
            ChatMessage {
                role: "system".to_string(),
                content: Some(MessageContent::Text("You are helpful".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                extra: Default::default(),
            },
        );
        let canonical = ingress_chat(&req, Endpoint::ChatCompletions);
        let wire = egress_request(&canonical);
        // System is preserved as first message
        assert_eq!(wire.messages[0].role, "system");
        assert_eq!(wire.messages.len(), 2);
    }

    #[test]
    fn test_parse_response_basic() {
        let wire_resp = serde_json::json!({
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "created": 1700000000i64,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });
        let data = serde_json::to_vec(&wire_resp).unwrap();
        let canonical = parse_response(&data, "openai", "cred-1").unwrap();
        assert_eq!(canonical.id, "chatcmpl-1");
        assert_eq!(canonical.model, "gpt-4");
        assert_eq!(canonical.content.len(), 1);
        assert_eq!(canonical.stop_reason, StopReason::EndTurn);
        assert_eq!(canonical.usage.input_tokens, 10);
        assert_eq!(canonical.usage.output_tokens, 5);
    }

    #[test]
    fn test_parse_event_text_delta() {
        let data = r#"{"id":"x","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#;
        let event = parse_event(data).unwrap();
        assert!(matches!(event, CanonicalEvent::TextDelta { text, .. } if text == "Hi"));
    }

    #[test]
    fn test_parse_event_done() {
        assert!(parse_event("[DONE]").is_none());
    }

    #[test]
    fn test_parse_event_finish() {
        let data = r#"{"id":"x","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let event = parse_event(data).unwrap();
        if let CanonicalEvent::StreamEnd { stop_reason, usage } = event {
            assert_eq!(stop_reason, StopReason::EndTurn);
            assert_eq!(usage.input_tokens, 10);
        } else {
            panic!("expected StreamEnd");
        }
    }
}
