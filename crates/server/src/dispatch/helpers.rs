use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use prism_core::error::ProxyError;
use prism_core::request_record::TokenUsage;

/// Extract token usage from a response payload (any format), including cache tokens.
pub(super) fn extract_usage(payload: &str) -> Option<TokenUsage> {
    // Quick string check to avoid JSON parsing on chunks without usage data
    if !payload.contains("usage") && !payload.contains("usageMetadata") {
        return None;
    }
    let val: serde_json::Value = serde_json::from_str(payload).ok()?;

    // Claude streaming: message_start has usage nested inside "message"
    // e.g. {"type":"message_start","message":{"usage":{"input_tokens":15}}}
    let usage_obj = val
        .get("usage")
        .or_else(|| val.get("message").and_then(|m| m.get("usage")));

    // OpenAI format: usage.prompt_tokens / usage.completion_tokens
    if let Some(usage) = usage_obj {
        let input = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64()));
        let output = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64()));

        if input.is_none() && output.is_none() {
            return None;
        }

        // Cache tokens: Claude uses top-level fields, OpenAI nests under prompt_tokens_details
        let cache_read = usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                usage
                    .get("prompt_tokens_details")
                    .and_then(|d| d.get("cached_tokens"))
                    .and_then(|v| v.as_u64())
            })
            .unwrap_or(0);

        let cache_creation = usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        return Some(TokenUsage {
            input_tokens: input.unwrap_or(0),
            output_tokens: output.unwrap_or(0),
            cache_read_tokens: cache_read,
            cache_creation_tokens: cache_creation,
        });
    }

    // Gemini format: usageMetadata
    if let Some(usage) = val.get("usageMetadata") {
        let input = usage.get("promptTokenCount").and_then(|v| v.as_u64());
        let output = usage.get("candidatesTokenCount").and_then(|v| v.as_u64());

        if input.is_none() && output.is_none() {
            return None;
        }

        let cache_read = usage
            .get("cachedContentTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        return Some(TokenUsage {
            input_tokens: input.unwrap_or(0),
            output_tokens: output.unwrap_or(0),
            cache_read_tokens: cache_read,
            cache_creation_tokens: 0,
        });
    }

    None
}

/// Build a non-stream JSON response with passthrough headers.
pub(super) fn build_json_response(
    translated: &str,
    passthrough_headers: &[String],
    upstream_headers: &std::collections::HashMap<String, String>,
) -> Result<Response, ProxyError> {
    let mut builder = axum::http::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json");

    for header_name in passthrough_headers {
        if let Some(val) = upstream_headers.get(header_name) {
            builder = builder.header(header_name.as_str(), val.as_str());
        }
    }

    builder
        .body(axum::body::Body::from(translated.to_string()))
        .map_err(|e| ProxyError::Internal(format!("failed to build response: {e}")))
        .map(IntoResponse::into_response)
}

/// Inject route debug headers into a response (x-prism-route-* format).
pub(super) fn inject_route_headers(
    response: &mut Response,
    profile: &str,
    provider: Option<&str>,
    credential_name: Option<&str>,
    model: Option<&str>,
    attempts: u32,
) {
    let headers = response.headers_mut();

    // x-prism-route-id: unique route identifier
    let route_id = uuid::Uuid::new_v4().to_string();
    headers.insert("x-prism-route-id", route_id.parse().unwrap());

    // x-prism-route-summary: human-readable summary
    let mut summary = format!("profile={profile}");
    if let Some(p) = provider {
        summary.push_str(&format!(" provider={p}"));
        headers.insert("x-prism-route-provider", p.parse().unwrap());
    }
    if let Some(c) = credential_name {
        summary.push_str(&format!(" credential={c}"));
        headers.insert("x-prism-route-credential", c.parse().unwrap());
    }
    if let Some(m) = model {
        summary.push_str(&format!(" model={m}"));
        headers.insert("x-prism-route-model", m.parse().unwrap());
    }
    headers.insert("x-prism-route-summary", summary.parse().unwrap());

    // x-prism-route-attempts: total attempt count
    headers.insert(
        "x-prism-route-attempts",
        attempts.to_string().parse().unwrap(),
    );
}

/// Inject `stream_options.include_usage = true` into an OpenAI-format streaming request
/// payload so that the final SSE chunk includes token usage data.
#[cfg(test)]
pub(super) fn inject_stream_usage_option(payload: Vec<u8>) -> Vec<u8> {
    if let Ok(mut val) = serde_json::from_slice::<serde_json::Value>(&payload)
        && let Some(obj) = val.as_object_mut()
    {
        let stream_opts = obj
            .entry("stream_options")
            .or_insert_with(|| serde_json::json!({}));
        if let Some(opts) = stream_opts.as_object_mut() {
            opts.entry("include_usage")
                .or_insert(serde_json::Value::Bool(true));
        }
        if let Ok(bytes) = serde_json::to_vec(&val) {
            return bytes;
        }
    }
    payload
}

/// Inject `stream_options.include_usage = true` into a mutable Value.
pub(super) fn inject_stream_usage_option_value(val: &mut serde_json::Value) {
    if let Some(obj) = val.as_object_mut() {
        let stream_opts = obj
            .entry("stream_options")
            .or_insert_with(|| serde_json::json!({}));
        if let Some(opts) = stream_opts.as_object_mut() {
            opts.entry("include_usage")
                .or_insert(serde_json::Value::Bool(true));
        }
    }
}

/// Rewrite the `model` field in a JSON request body to use a different model name.
pub(super) fn rewrite_model_in_body(body: &Bytes, new_model: &str) -> Bytes {
    if let Ok(mut val) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(obj) = val.as_object_mut()
    {
        obj.insert(
            "model".to_string(),
            serde_json::Value::String(new_model.to_string()),
        );
        if let Ok(bytes) = serde_json::to_vec(&val) {
            return Bytes::from(bytes);
        }
    }
    body.clone()
}
