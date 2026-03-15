use crate::AppState;
use crate::dispatch::{DispatchRequest, dispatch};
use crate::handler::merge_requested_credential;
use crate::handler::provider_scoped::{matches_scoped_credential, resolve_provider};
use axum::Extension;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::{Format, UpstreamKind};
use prism_provider::sse::parse_sse_stream;
use serde_json::{Value, json};
use tokio_stream::StreamExt;

#[derive(Default)]
struct ResponsesWsSession {
    last_request: Option<Value>,
    last_response_output: Value,
    pinned_credential: Option<String>,
    pinned_upstream: Option<UpstreamKind>,
    request_index: u64,
}

pub async fn responses_ws(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ProxyError> {
    let allowed_credentials = merge_requested_credential(
        ctx.auth_key
            .as_ref()
            .map(|entry| entry.allowed_credentials.clone())
            .unwrap_or_default(),
        header_auth_profile(&headers).as_deref(),
    )?;
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state, ctx, headers, allowed_credentials)))
}

pub async fn provider_responses_ws(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ProxyError> {
    let mut allowed_credentials = resolve_provider(&state, &provider)?;
    if let Some(ref auth_key) = ctx.auth_key
        && !auth_key.allowed_credentials.is_empty()
    {
        let patterns = &auth_key.allowed_credentials;
        allowed_credentials.retain(|name| {
            patterns
                .iter()
                .any(|pattern| prism_core::glob::glob_match(pattern, name))
        });
        if allowed_credentials.is_empty() {
            return Err(ProxyError::BadRequest(format!(
                "no accessible credentials for provider '{provider}' with current API key"
            )));
        }
    }

    if let Some(requested) = header_auth_profile(&headers) {
        allowed_credentials.retain(|candidate| matches_scoped_credential(candidate, &requested));
        if allowed_credentials.is_empty() {
            return Err(ProxyError::BadRequest(format!(
                "unknown auth profile '{requested}' for provider '{provider}'"
            )));
        }
    }

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state, ctx, headers, allowed_credentials)))
}

async fn handle_ws(
    mut socket: WebSocket,
    state: AppState,
    ctx: RequestContext,
    headers: HeaderMap,
    base_allowed_credentials: Vec<String>,
) {
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let mut session = ResponsesWsSession {
        last_response_output: Value::Array(Vec::new()),
        ..Default::default()
    };

    while let Some(message) = socket.recv().await {
        let payload = match message {
            Ok(Message::Text(text)) => text.to_string(),
            Ok(Message::Binary(bytes)) => match String::from_utf8(bytes.to_vec()) {
                Ok(text) => text,
                Err(_) => {
                    if send_ws_error(&mut socket, "websocket payload must be valid UTF-8")
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
            },
            Ok(Message::Close(_)) | Err(_) => return,
            _ => continue,
        };

        let allow_incremental_previous_response_id =
            session.pinned_upstream == Some(UpstreamKind::Codex);
        let normalized = match normalize_ws_request(
            &payload,
            session.last_request.as_ref(),
            &session.last_response_output,
            allow_incremental_previous_response_id,
        ) {
            Ok(request) => request,
            Err(message) => {
                if send_ws_error(&mut socket, &message).await.is_err() {
                    return;
                }
                continue;
            }
        };

        let model = normalized
            .get("model")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let request_body = match serde_json::to_vec(&normalized) {
            Ok(bytes) => Bytes::from(bytes),
            Err(err) => {
                if send_ws_error(&mut socket, &format!("failed to serialize request: {err}"))
                    .await
                    .is_err()
                {
                    return;
                }
                continue;
            }
        };

        session.request_index += 1;
        let request_id = format!("{}:ws:{}", ctx.request_id, session.request_index);
        let allowed_credentials = session
            .pinned_credential
            .clone()
            .map(|credential| vec![credential])
            .unwrap_or_else(|| base_allowed_credentials.clone());
        let dispatch_result = dispatch(
            &state,
            DispatchRequest {
                source_format: Format::OpenAI,
                model,
                models: None,
                stream: true,
                body: request_body,
                allowed_formats: Some(vec![Format::OpenAI]),
                user_agent: user_agent.clone(),
                debug: true,
                api_key: ctx.auth_key.as_ref().map(|entry| entry.key.clone()),
                client_region: ctx.client_region.clone(),
                request_id: Some(request_id),
                api_key_id: ctx.api_key_id.clone(),
                tenant_id: ctx.tenant_id.clone(),
                allowed_credentials,
                responses_passthrough: true,
            },
        )
        .await;

        let response = match dispatch_result {
            Ok(response) => response,
            Err(err) => {
                if send_ws_error(&mut socket, &err.to_string()).await.is_err() {
                    return;
                }
                continue;
            }
        };

        if session.pinned_credential.is_none()
            && let Some(credential) = response
                .headers()
                .get("x-prism-route-credential")
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        {
            session.pinned_upstream =
                find_upstream_for_credential_name(&state, credential.as_str());
            session.pinned_credential = Some(credential);
        }
        session.last_request = Some(normalized);

        let mut sse_stream = parse_sse_stream(response.into_body().into_data_stream());
        while let Some(event) = sse_stream.next().await {
            let event = match event {
                Ok(event) => event,
                Err(err) => {
                    let _ = send_ws_error(&mut socket, &err.to_string()).await;
                    return;
                }
            };
            if event.data != "[DONE]"
                && let Ok(value) = serde_json::from_str::<Value>(&event.data)
            {
                session.last_response_output = extract_response_output(&value);
            }
            if socket.send(Message::Text(event.data.into())).await.is_err() {
                return;
            }
        }
    }
}

fn header_auth_profile(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-prism-auth-profile")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_response_output(event: &Value) -> Value {
    if event.get("type").and_then(|value| value.as_str()) != Some("response.completed") {
        return Value::Array(Vec::new());
    }
    event
        .get("response")
        .and_then(|response| response.get("output"))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

fn find_upstream_for_credential_name(
    state: &AppState,
    credential_name: &str,
) -> Option<UpstreamKind> {
    state
        .router
        .credential_map()
        .into_values()
        .flatten()
        .find(|auth| auth.name() == Some(credential_name))
        .map(|auth| auth.upstream)
}

fn normalize_ws_request(
    raw: &str,
    last_request: Option<&Value>,
    last_response_output: &Value,
    allow_incremental_previous_response_id: bool,
) -> Result<Value, String> {
    let mut value: Value =
        serde_json::from_str(raw).map_err(|e| format!("invalid websocket JSON: {e}"))?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "expected websocket request object".to_string())?;
    let request_type = obj
        .remove("type")
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .ok_or_else(|| "websocket request requires type".to_string())?;
    match (request_type.as_str(), last_request) {
        ("response.create", None) => normalize_ws_create(value),
        ("response.create" | "response.append", Some(previous)) => normalize_ws_append(
            value,
            previous,
            last_response_output,
            allow_incremental_previous_response_id,
        ),
        ("response.append", None) => {
            Err("response.append requires an existing websocket session".to_string())
        }
        _ => Err(format!(
            "unsupported websocket request type: {request_type}"
        )),
    }
}

fn normalize_ws_create(mut value: Value) -> Result<Value, String> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "expected websocket request object".to_string())?;
    obj.insert("stream".into(), Value::Bool(true));
    if !obj.contains_key("input") {
        obj.insert("input".into(), Value::Array(Vec::new()));
    }
    if obj
        .get("model")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("response.create requires model".to_string());
    }
    Ok(value)
}

fn normalize_ws_append(
    mut value: Value,
    last_request: &Value,
    last_response_output: &Value,
    allow_incremental_previous_response_id: bool,
) -> Result<Value, String> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "expected websocket request object".to_string())?;
    let next_input = obj
        .get("input")
        .and_then(|value| value.as_array())
        .cloned()
        .ok_or_else(|| "websocket request requires array field: input".to_string())?;

    let has_previous_response_id = obj
        .get("previous_response_id")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    if allow_incremental_previous_response_id && has_previous_response_id {
        inherit_request_fields(obj, last_request);
        obj.insert("stream".into(), Value::Bool(true));
        return Ok(value);
    }

    obj.remove("previous_response_id");
    let mut merged_input = last_request
        .get("input")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if let Some(output_items) = last_response_output.as_array() {
        merged_input.extend(output_items.iter().cloned());
    }
    merged_input.extend(next_input);
    obj.insert("input".into(), Value::Array(merged_input));
    inherit_request_fields(obj, last_request);
    obj.insert("stream".into(), Value::Bool(true));
    Ok(value)
}

fn inherit_request_fields(obj: &mut serde_json::Map<String, Value>, last_request: &Value) {
    if !obj.contains_key("model")
        && let Some(model) = last_request.get("model").cloned()
    {
        obj.insert("model".into(), model);
    }
    if !obj.contains_key("instructions")
        && let Some(instructions) = last_request.get("instructions").cloned()
    {
        obj.insert("instructions".into(), instructions);
    }
}

async fn send_ws_error(socket: &mut WebSocket, message: &str) -> Result<(), axum::Error> {
    socket
        .send(Message::Text(
            json!({
                "type": "error",
                "error": {
                    "message": message,
                },
            })
            .to_string()
            .into(),
        ))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_create_sets_stream() {
        let value = normalize_ws_request(
            r#"{"type":"response.create","model":"gpt-5","input":[]}"#,
            None,
            &Value::Array(Vec::new()),
            false,
        )
        .unwrap();
        assert_eq!(value.get("stream"), Some(&Value::Bool(true)));
    }

    #[test]
    fn normalize_append_merges_history() {
        let previous = json!({
            "model": "gpt-5",
            "instructions": "be terse",
            "input": [{"role":"user","content":[{"type":"input_text","text":"hi"}]}]
        });
        let output = json!([
            {"type":"message","role":"assistant","content":[{"type":"output_text","text":"hello"}]}
        ]);
        let value = normalize_ws_request(
            r#"{"type":"response.append","input":[{"role":"user","content":[{"type":"input_text","text":"next"}]}]}"#,
            Some(&previous),
            &output,
            false,
        )
        .unwrap();
        assert_eq!(value.get("model").and_then(|v| v.as_str()), Some("gpt-5"));
        assert_eq!(
            value.get("input").and_then(|v| v.as_array()).map(Vec::len),
            Some(3)
        );
    }

    #[test]
    fn normalize_append_preserves_previous_response_id_for_incremental_mode() {
        let previous = json!({
            "model": "gpt-5",
            "instructions": "be terse",
            "input": []
        });
        let value = normalize_ws_request(
            r#"{"type":"response.append","previous_response_id":"resp_123","input":[]}"#,
            Some(&previous),
            &Value::Array(Vec::new()),
            true,
        )
        .unwrap();
        assert_eq!(
            value.get("previous_response_id").and_then(|v| v.as_str()),
            Some("resp_123")
        );
    }
}
