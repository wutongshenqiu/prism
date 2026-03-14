//! Golden tests for protocol adapters.
//!
//! These tests load JSON fixtures and verify that ingress/egress adapters produce
//! correct canonical types and protocol-native output. Adding a new test case is
//! as simple as adding a fixture file and a test function using the helpers.

use prism_domain::content::Role;
use prism_domain::event::CanonicalEvent;
use prism_domain::operation::Endpoint;
use prism_domain::response::StopReason;
use prism_types::types::{claude, gemini, openai};
use serde_json::Value;
use std::path::PathBuf;

fn fixture_path(protocol: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(protocol)
        .join(name)
}

fn load_fixture(protocol: &str, name: &str) -> Value {
    let path = fixture_path(protocol, name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()))
}

// ─── OpenAI Golden Tests ──────────────────────────────────────────────────────

#[test]
fn golden_openai_ingress_basic() {
    let fixture = load_fixture("openai", "chat_basic_request.json");
    let req: openai::ChatCompletionRequest = serde_json::from_value(fixture).unwrap();
    let canonical = prism_protocol::openai::ingress_chat(&req, Endpoint::ChatCompletions);

    assert_eq!(canonical.model, "gpt-4");
    assert!(!canonical.stream);
    assert!(canonical.input.system.is_some());
    assert_eq!(canonical.input.messages.len(), 1);
    assert_eq!(canonical.input.messages[0].role, Role::User);
    assert_eq!(canonical.limits.max_tokens, Some(100));
    assert_eq!(canonical.limits.temperature, Some(0.7));
}

#[test]
fn golden_openai_ingress_with_tools() {
    let fixture = load_fixture("openai", "chat_with_tools_request.json");
    let req: openai::ChatCompletionRequest = serde_json::from_value(fixture).unwrap();
    let canonical = prism_protocol::openai::ingress_chat(&req, Endpoint::ChatCompletions);

    assert_eq!(canonical.tools.len(), 1);
    assert_eq!(canonical.tools[0].name, "get_weather");
    assert_eq!(
        canonical.tools[0].description,
        Some("Get weather for a city".into())
    );
}

#[test]
fn golden_openai_parse_response() {
    let fixture = load_fixture("openai", "chat_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let response = prism_protocol::openai::parse_response(&data, "openai", "cred-1").unwrap();

    assert_eq!(response.id, "chatcmpl-golden-1");
    assert_eq!(response.model, "gpt-4");
    assert_eq!(response.content.len(), 1);
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(response.usage.input_tokens, 20);
    assert_eq!(response.usage.output_tokens, 8);
}

#[test]
fn golden_openai_parse_tool_call_response() {
    let fixture = load_fixture("openai", "chat_tool_call_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let response = prism_protocol::openai::parse_response(&data, "openai", "cred-1").unwrap();

    assert_eq!(response.stop_reason, StopReason::ToolUse);
    let tool_blocks: Vec<_> = response
        .content
        .iter()
        .filter(|b| matches!(b, prism_domain::content::ContentBlock::ToolUse { .. }))
        .collect();
    assert_eq!(tool_blocks.len(), 1);
}

#[test]
fn golden_openai_egress_response_roundtrip() {
    let fixture = load_fixture("openai", "chat_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let canonical = prism_protocol::openai::parse_response(&data, "openai", "cred-1").unwrap();
    let egress = prism_protocol::openai::egress_response(&canonical);

    // Serialize to JSON for verification
    let egress_val = serde_json::to_value(&egress).unwrap();
    assert_eq!(egress_val["object"], "chat.completion");
    assert_eq!(egress_val["model"], "gpt-4");
    assert_eq!(egress_val["choices"][0]["finish_reason"], "stop");
    assert_eq!(egress_val["choices"][0]["message"]["role"], "assistant");
}

#[test]
fn golden_openai_stream_events() {
    let fixture = load_fixture("openai", "stream_events.json");
    let events: Vec<Value> = serde_json::from_value(fixture).unwrap();

    let mut parsed_events = Vec::new();
    for event_val in &events {
        let data_str = serde_json::to_string(event_val).unwrap();
        if let Some(event) = prism_protocol::openai::parse_event(&data_str) {
            parsed_events.push(event);
        }
    }

    // Should parse at least some events
    assert!(!parsed_events.is_empty());
    // Should contain text deltas
    let text_deltas: Vec<_> = parsed_events
        .iter()
        .filter(|e| matches!(e, CanonicalEvent::TextDelta { .. }))
        .collect();
    assert!(!text_deltas.is_empty());
    // Last should be StreamEnd
    assert!(matches!(
        parsed_events.last().unwrap(),
        CanonicalEvent::StreamEnd { .. }
    ));
}

// ─── Claude Golden Tests ──────────────────────────────────────────────────────

#[test]
fn golden_claude_ingress_basic() {
    let fixture = load_fixture("claude", "messages_basic_request.json");
    let req: claude::ClaudeMessagesRequest = serde_json::from_value(fixture).unwrap();
    let canonical = prism_protocol::claude::ingress_messages(&req, Endpoint::Messages);

    assert_eq!(canonical.model, "claude-3-5-sonnet-20241022");
    assert!(!canonical.stream);
    assert!(canonical.input.system.is_some());
    assert_eq!(canonical.input.messages.len(), 1);
    assert_eq!(canonical.input.messages[0].role, Role::User);
    assert_eq!(canonical.limits.max_tokens, Some(100));
}

#[test]
fn golden_claude_ingress_with_tools() {
    let fixture = load_fixture("claude", "messages_with_tools_request.json");
    let req: claude::ClaudeMessagesRequest = serde_json::from_value(fixture).unwrap();
    let canonical = prism_protocol::claude::ingress_messages(&req, Endpoint::Messages);

    assert_eq!(canonical.tools.len(), 1);
    assert_eq!(canonical.tools[0].name, "get_weather");
}

#[test]
fn golden_claude_parse_response() {
    let fixture = load_fixture("claude", "messages_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let response = prism_protocol::claude::parse_response(&data, "anthropic", "cred-1").unwrap();

    assert_eq!(response.id, "msg_golden_1");
    assert_eq!(response.model, "claude-3-5-sonnet-20241022");
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(response.usage.input_tokens, 20);
    assert_eq!(response.usage.output_tokens, 8);
}

#[test]
fn golden_claude_egress_response_roundtrip() {
    let fixture = load_fixture("claude", "messages_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let canonical = prism_protocol::claude::parse_response(&data, "anthropic", "cred-1").unwrap();
    let egress = prism_protocol::claude::egress_response(&canonical);

    let egress_val = serde_json::to_value(&egress).unwrap();
    assert_eq!(egress_val["type"], "message");
    assert_eq!(egress_val["role"], "assistant");
    assert_eq!(egress_val["model"], "claude-3-5-sonnet-20241022");
    assert_eq!(egress_val["stop_reason"], "end_turn");
}

#[test]
fn golden_claude_stream_events() {
    let fixture = load_fixture("claude", "stream_events.json");
    let events: Vec<Value> = serde_json::from_value(fixture).unwrap();

    let mut parsed_events = Vec::new();
    for event_val in &events {
        let event_type = event_val["event"].as_str().unwrap_or("");
        let data_str = serde_json::to_string(&event_val["data"]).unwrap();
        if let Some(event) = prism_protocol::claude::parse_event(event_type, &data_str) {
            parsed_events.push(event);
        }
    }

    assert!(!parsed_events.is_empty());
    let text_deltas: Vec<_> = parsed_events
        .iter()
        .filter(|e| matches!(e, CanonicalEvent::TextDelta { .. }))
        .collect();
    assert!(!text_deltas.is_empty());
    assert!(matches!(
        parsed_events.last().unwrap(),
        CanonicalEvent::StreamEnd { .. }
    ));
}

// ─── Gemini Golden Tests ──────────────────────────────────────────────────────

#[test]
fn golden_gemini_ingress_basic() {
    let fixture = load_fixture("gemini", "generate_basic_request.json");
    let req: gemini::GeminiRequest = serde_json::from_value(fixture).unwrap();
    let canonical =
        prism_protocol::gemini::ingress_generate(&req, "gemini-1.5-pro", Endpoint::GenerateContent);

    assert_eq!(canonical.model, "gemini-1.5-pro");
    assert!(!canonical.stream);
    assert!(canonical.input.system.is_some());
    assert_eq!(canonical.input.messages.len(), 1);
    assert_eq!(canonical.input.messages[0].role, Role::User);
}

#[test]
fn golden_gemini_parse_response() {
    let fixture = load_fixture("gemini", "generate_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let response = prism_protocol::gemini::parse_response(&data, "gemini", "cred-1").unwrap();

    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(response.usage.input_tokens, 15);
    assert_eq!(response.usage.output_tokens, 8);
}

#[test]
fn golden_gemini_egress_response_roundtrip() {
    let fixture = load_fixture("gemini", "generate_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let canonical = prism_protocol::gemini::parse_response(&data, "gemini", "cred-1").unwrap();
    let egress = prism_protocol::gemini::egress_response(&canonical);

    let egress_val = serde_json::to_value(&egress).unwrap();
    assert!(
        egress_val["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .contains("4")
    );
    assert_eq!(egress_val["candidates"][0]["finishReason"], "STOP");
}

#[test]
fn golden_gemini_stream_events() {
    let fixture = load_fixture("gemini", "stream_events.json");
    let events: Vec<Value> = serde_json::from_value(fixture).unwrap();

    let mut parsed_events = Vec::new();
    for event_val in &events {
        let data_str = serde_json::to_string(event_val).unwrap();
        if let Some(event) = prism_protocol::gemini::parse_event(&data_str) {
            parsed_events.push(event);
        }
    }

    let text_deltas: Vec<_> = parsed_events
        .iter()
        .filter(|e| matches!(e, CanonicalEvent::TextDelta { .. }))
        .collect();
    assert!(!text_deltas.is_empty());
}

// ─── Cross-Protocol Egress Tests ──────────────────────────────────────────────

#[test]
fn golden_cross_protocol_openai_to_claude_egress() {
    let fixture = load_fixture("openai", "chat_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let canonical = prism_protocol::openai::parse_response(&data, "openai", "cred-1").unwrap();
    let claude_egress = prism_protocol::claude::egress_response(&canonical);

    let val = serde_json::to_value(&claude_egress).unwrap();
    assert_eq!(val["type"], "message");
    assert_eq!(val["role"], "assistant");
    assert_eq!(val["stop_reason"], "end_turn");
    assert_eq!(val["content"][0]["type"], "text");
}

#[test]
fn golden_cross_protocol_claude_to_openai_egress() {
    let fixture = load_fixture("claude", "messages_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let canonical = prism_protocol::claude::parse_response(&data, "anthropic", "cred-1").unwrap();
    let openai_egress = prism_protocol::openai::egress_response(&canonical);

    let val = serde_json::to_value(&openai_egress).unwrap();
    assert_eq!(val["object"], "chat.completion");
    assert_eq!(val["choices"][0]["finish_reason"], "stop");
    assert_eq!(val["choices"][0]["message"]["role"], "assistant");
}

#[test]
fn golden_cross_protocol_gemini_to_openai_egress() {
    let fixture = load_fixture("gemini", "generate_basic_response.json");
    let data = serde_json::to_vec(&fixture).unwrap();
    let canonical = prism_protocol::gemini::parse_response(&data, "gemini", "cred-1").unwrap();
    let openai_egress = prism_protocol::openai::egress_response(&canonical);

    let val = serde_json::to_value(&openai_egress).unwrap();
    assert_eq!(val["object"], "chat.completion");
    assert_eq!(val["choices"][0]["finish_reason"], "stop");
}
