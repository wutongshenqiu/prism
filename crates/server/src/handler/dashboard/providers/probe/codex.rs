use std::time::Instant;

use prism_provider::sse::parse_sse_stream;
use serde_json::json;
use tokio_stream::StreamExt;

use super::common::probe_check;
use super::super::{ProbeStatus, ProviderProbeResult};

const CODEX_HEALTH_USER_AGENT: &str =
    "codex_cli_rs/0.101.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464";
const RED_DOT_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

fn build_codex_probe_request(
    client: &reqwest::Client,
    auth: &prism_core::provider::AuthRecord,
    body: &serde_json::Value,
) -> reqwest::RequestBuilder {
    let body = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    let mut req = client
        .post(format!("{}/responses", auth.resolved_base_url()))
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("authorization", format!("Bearer {}", auth.current_secret()))
        .header("version", "0.101.0")
        .header("session_id", uuid::Uuid::new_v4().to_string())
        .header("originator", "codex_cli_rs")
        .body(body);
    if let Some(account_id) = auth.current_account_id()
        && !account_id.trim().is_empty()
    {
        req = req.header("chatgpt-account-id", account_id);
    }
    if !auth
        .headers
        .keys()
        .any(|key| key.eq_ignore_ascii_case("user-agent"))
    {
        req = req.header("user-agent", CODEX_HEALTH_USER_AGENT);
    }
    for (k, v) in &auth.headers {
        if prism_core::presentation::protected::is_protected(k) {
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }
    req
}

fn codex_probe_model(auth: &prism_core::provider::AuthRecord) -> String {
    auth.models
        .first()
        .map(|entry| entry.id.clone())
        .unwrap_or_else(|| "gpt-5".to_string())
}

async fn collect_codex_probe_response(
    resp: reqwest::Response,
) -> Result<(bool, serde_json::Value), String> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("upstream returned {status}: {body}"));
    }

    let mut saw_delta = false;
    let mut sse_stream = parse_sse_stream(resp.bytes_stream());
    while let Some(event) = sse_stream.next().await {
        let event = event.map_err(|e| e.to_string())?;
        if event.data == "[DONE]" {
            continue;
        }
        let payload: serde_json::Value =
            serde_json::from_str(&event.data).map_err(|e| format!("invalid SSE payload: {e}"))?;
        let event_type = payload
            .get("type")
            .and_then(|value| value.as_str())
            .or(event.event.as_deref())
            .unwrap_or("");
        if event_type == "response.output_text.delta" {
            saw_delta = true;
        }
        if event_type == "response.completed" {
            let response = payload
                .get("response")
                .cloned()
                .ok_or_else(|| "response.completed missing response payload".to_string())?;
            return Ok((saw_delta, response));
        }
    }

    Err("stream ended before response.completed".to_string())
}

fn extract_response_text(payload: &serde_json::Value) -> String {
    payload
        .get("output")
        .and_then(|value| value.as_array())
        .map(|items| {
            let mut text = String::new();
            for item in items {
                if item.get("type").and_then(|value| value.as_str()) == Some("message")
                    && let Some(content) = item.get("content").and_then(|value| value.as_array())
                {
                    for part in content {
                        if part.get("type").and_then(|value| value.as_str()) == Some("output_text")
                            && let Some(value) = part.get("text").and_then(|value| value.as_str())
                        {
                            text.push_str(value);
                        }
                    }
                }
            }
            text
        })
        .unwrap_or_default()
}

pub(super) async fn run_codex_probe(
    client: &reqwest::Client,
    auth: &prism_core::provider::AuthRecord,
) -> ProviderProbeResult {
    let started = Instant::now();
    let model = codex_probe_model(auth);

    let text_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "Reply with exactly TEXT_PROBE_OK"
            }]
        }],
        "store": false,
        "stream": true,
    });
    let text_check = match build_codex_probe_request(client, auth, &text_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((_saw_delta, payload)) => {
                let text = extract_response_text(&payload);
                if text.contains("TEXT_PROBE_OK") {
                    probe_check("text", ProbeStatus::Verified, None)
                } else {
                    probe_check(
                        "text",
                        ProbeStatus::Failed,
                        Some(format!("unexpected text response: {text}")),
                    )
                }
            }
            Err(err) => probe_check("text", ProbeStatus::Failed, Some(err)),
        },
        Err(err) => probe_check("text", ProbeStatus::Failed, Some(err.to_string())),
    };

    let stream_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "Reply with exactly STREAM_PROBE_OK"
            }]
        }],
        "store": false,
        "stream": true,
    });
    let stream_check = match build_codex_probe_request(client, auth, &stream_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((saw_delta, payload)) => {
                let text = extract_response_text(&payload);
                if saw_delta && text.contains("STREAM_PROBE_OK") {
                    probe_check("stream", ProbeStatus::Verified, None)
                } else if !saw_delta {
                    probe_check(
                        "stream",
                        ProbeStatus::Failed,
                        Some("no output_text delta event observed".to_string()),
                    )
                } else {
                    probe_check(
                        "stream",
                        ProbeStatus::Failed,
                        Some(format!("unexpected stream response: {text}")),
                    )
                }
            }
            Err(err) => probe_check("stream", ProbeStatus::Failed, Some(err)),
        },
        Err(err) => probe_check("stream", ProbeStatus::Failed, Some(err.to_string())),
    };

    let tools_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "Call the probe_tool function exactly once."
            }]
        }],
        "tools": [{
            "type": "function",
            "name": "probe_tool",
            "description": "Health probe tool",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }],
        "tool_choice": "required",
        "store": false,
        "stream": true,
    });
    let tools_check = match build_codex_probe_request(client, auth, &tools_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((_saw_delta, payload)) => {
                let found = payload
                    .get("output")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items.iter().any(|item| {
                            item.get("type")
                                .and_then(|value| value.as_str())
                                .is_some_and(|value| {
                                    value.contains("function_call") || value.contains("tool_call")
                                })
                                || item
                                    .get("name")
                                    .and_then(|value| value.as_str())
                                    .is_some_and(|value| value == "probe_tool")
                        })
                    })
                    .unwrap_or(false);
                if found {
                    probe_check("tools", ProbeStatus::Verified, None)
                } else {
                    probe_check(
                        "tools",
                        ProbeStatus::Failed,
                        Some("response completed without a tool call".to_string()),
                    )
                }
            }
            Err(err) => {
                let status = if err.contains("Unsupported") || err.contains("unknown_parameter") {
                    ProbeStatus::Unsupported
                } else {
                    ProbeStatus::Failed
                };
                probe_check("tools", status, Some(err))
            }
        },
        Err(err) => probe_check("tools", ProbeStatus::Failed, Some(err.to_string())),
    };

    let images_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": "The image is a solid red square. Answer with exactly red."
                },
                {
                    "type": "input_image",
                    "image_url": RED_DOT_PNG_DATA_URL
                }
            ]
        }],
        "store": false,
        "stream": true,
    });
    let images_check = match build_codex_probe_request(client, auth, &images_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((_saw_delta, payload)) => {
                let text = extract_response_text(&payload).to_lowercase();
                if text.contains("red") {
                    probe_check("images", ProbeStatus::Verified, None)
                } else {
                    probe_check(
                        "images",
                        ProbeStatus::Failed,
                        Some(format!("unexpected image response: {text}")),
                    )
                }
            }
            Err(err) => {
                let status = if err.contains("Unsupported") || err.contains("unknown_parameter") {
                    ProbeStatus::Unsupported
                } else {
                    ProbeStatus::Failed
                };
                probe_check("images", status, Some(err))
            }
        },
        Err(err) => probe_check("images", ProbeStatus::Failed, Some(err.to_string())),
    };

    let checks = vec![
        text_check,
        stream_check,
        tools_check,
        images_check,
        probe_check(
            "json_schema",
            ProbeStatus::Unknown,
            Some("no live probe implemented".to_string()),
        ),
        probe_check(
            "reasoning",
            ProbeStatus::Unknown,
            Some("no live probe implemented".to_string()),
        ),
        probe_check(
            "count_tokens",
            ProbeStatus::Unsupported,
            Some("Codex backend does not expose count_tokens".to_string()),
        ),
    ];

    let status = if checks.iter().any(|check| {
        (check.capability == "text" || check.capability == "stream") && !check.status.is_verified()
    }) {
        "error"
    } else if checks.iter().all(|check| {
        matches!(
            check.status,
            ProbeStatus::Verified | ProbeStatus::Unsupported | ProbeStatus::Unknown
        )
    }) && checks.iter().any(|check| {
        matches!(
            check.status,
            ProbeStatus::Unsupported | ProbeStatus::Unknown
        )
    }) {
        "warning"
    } else {
        "ok"
    };

    ProviderProbeResult {
        provider: auth.provider_name.clone(),
        upstream: auth.upstream.to_string(),
        status: status.to_string(),
        checked_at: chrono::Utc::now().to_rfc3339(),
        latency_ms: started.elapsed().as_millis() as u64,
        checks,
    }
}
