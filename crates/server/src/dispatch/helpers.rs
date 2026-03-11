use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use prism_core::error::ProxyError;
use prism_core::request_record::TokenUsage;

use super::DispatchDebug;
use super::DispatchMeta;

/// Extract token usage from a response payload (any format), including cache tokens.
pub(super) fn extract_usage(payload: &str) -> Option<TokenUsage> {
    // Quick string check to avoid JSON parsing on chunks without usage data
    if !payload.contains("usage") && !payload.contains("usageMetadata") {
        return None;
    }
    let val: serde_json::Value = serde_json::from_str(payload).ok()?;

    // OpenAI format: usage.prompt_tokens / usage.completion_tokens
    if let Some(usage) = val.get("usage") {
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

/// Inject dispatch metadata into response extensions for request logging.
///
/// `upstream_payload` is the raw upstream response (before translation) — used for
/// token extraction so that provider-specific fields (e.g. Claude's cache tokens)
/// are not lost during format translation.
pub(super) fn inject_dispatch_meta(
    response: &mut Response,
    debug: &DispatchDebug,
    upstream_payload: &[u8],
    cost_calculator: &prism_core::cost::CostCalculator,
    metrics: &prism_core::metrics::Metrics,
    requested_model: &str,
    total_attempts: u32,
) {
    let upstream_str = std::str::from_utf8(upstream_payload).unwrap_or("");
    let usage = extract_usage(upstream_str);
    let model = debug.model.as_deref();
    let cost = match (model, &usage) {
        (Some(m), Some(u)) => cost_calculator.calculate(m, u),
        _ => None,
    };
    // Record tokens and cost in global metrics
    if let Some(ref u) = usage {
        metrics.record_tokens(u.total_input(), u.output_tokens);
    }
    if let (Some(m), Some(c)) = (model, cost) {
        metrics.record_cost(m, c);
    }
    response.extensions_mut().insert(DispatchMeta {
        provider: debug.provider.clone(),
        model: debug.model.clone(),
        requested_model: Some(requested_model.to_string()),
        credential_name: debug.credential_name.clone(),
        stream: false,
        retry_count: total_attempts.saturating_sub(1),
        usage,
        cost,
        error_detail: None,
    });
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

/// Inject debug headers into a response if debug mode is enabled.
pub(super) fn inject_debug_headers(response: &mut Response, debug: &DispatchDebug) {
    let headers = response.headers_mut();
    if let Some(ref provider) = debug.provider {
        headers.insert("x-debug-provider", provider.parse().unwrap());
    }
    if let Some(ref model) = debug.model {
        headers.insert("x-debug-model", model.parse().unwrap());
    }
    if let Some(ref name) = debug.credential_name {
        headers.insert("x-debug-credential", name.parse().unwrap());
    }
    if !debug.attempts.is_empty() {
        headers.insert(
            "x-debug-attempts",
            debug.attempts.join(", ").parse().unwrap(),
        );
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
