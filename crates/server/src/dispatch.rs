mod executor;
mod features;
mod helpers;
mod streaming;

use crate::AppState;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use executor::ExecutionController;
use features::extract_features;
use helpers::{inject_route_headers, rewrite_model_in_body};
use prism_core::error::ProxyError;
use prism_core::provider::Format;
use prism_core::request_record::{LogDetailLevel, classify_error, truncate_body};
use prism_core::routing::planner::RoutePlanner;
use std::time::Instant;

/// A dispatch request encapsulating all information needed to route and execute an API call.
pub struct DispatchRequest {
    /// The API format of the incoming request (e.g., OpenAI, Claude).
    pub source_format: Format,
    /// The requested model name (may include prefix/alias).
    pub model: String,
    /// Fallback model chain: try models in order until one succeeds.
    pub models: Option<Vec<String>>,
    /// Whether the client requested streaming.
    pub stream: bool,
    /// The raw request body.
    pub body: Bytes,
    /// Restrict to specific provider formats. `None` means auto-resolve from model.
    pub allowed_formats: Option<Vec<Format>>,
    /// Client User-Agent header (used for cloak auto-mode detection).
    pub user_agent: Option<String>,
    /// Debug mode: return routing details in response headers.
    pub debug: bool,
    /// API key (for per-key rate limiting post-check).
    pub api_key: Option<String>,
    /// Client region (for geo-aware routing).
    pub client_region: Option<String>,
    /// Request ID for correlating streaming usage updates with log entries.
    pub request_id: Option<String>,
    /// Masked API key ID for logging.
    pub api_key_id: Option<String>,
    /// Tenant ID for logging.
    pub tenant_id: Option<String>,
    /// Restrict to specific credentials by name (glob patterns).
    pub allowed_credentials: Vec<String>,
}

/// Unified dispatch: plans route via RoutePlanner, then executes via ExecutionController.
///
/// Creates `gateway.request` and `gateway.attempt` tracing spans that are collected by
/// `GatewayLogLayer` to produce structured request records.
///
/// Flow: extract features → plan route → cache check → execute plan → debug headers → log.
pub async fn dispatch(state: &AppState, mut req: DispatchRequest) -> Result<Response, ProxyError> {
    let start = Instant::now();
    let config = state.config.load();
    let detail_level = config.log_store.detail_level;
    let max_body_bytes = config.log_store.max_body_bytes;

    let request_id = req.request_id.clone().unwrap_or_else(|| "-".to_string());

    // Create the gateway.request span — GatewayLogLayer collects this on close
    let request_span = tracing::info_span!(
        "gateway.request",
        request_id = %request_id,
        method = "POST",
        path = tracing::field::Empty,
        stream = req.stream,
        requested_model = %req.model,
        request_body = tracing::field::Empty,
        upstream_request_body = tracing::field::Empty,
        provider = tracing::field::Empty,
        model = tracing::field::Empty,
        credential_name = tracing::field::Empty,
        total_attempts = tracing::field::Empty,
        status = tracing::field::Empty,
        latency_ms = tracing::field::Empty,
        response_body = tracing::field::Empty,
        stream_content_preview = tracing::field::Empty,
        usage_input = tracing::field::Empty,
        usage_output = tracing::field::Empty,
        usage_cache_read = tracing::field::Empty,
        usage_cache_creation = tracing::field::Empty,
        cost = tracing::field::Empty,
        error = tracing::field::Empty,
        error_type = tracing::field::Empty,
        api_key_id = req.api_key_id.as_deref().unwrap_or(""),
        tenant_id = req.tenant_id.as_deref().unwrap_or(""),
        client_ip = tracing::field::Empty,
        client_region = req.client_region.as_deref().unwrap_or(""),
    );

    // Record client request body if detail level allows
    if detail_level >= LogDetailLevel::Standard
        && let Ok(body_str) = std::str::from_utf8(&req.body)
    {
        request_span.record(
            "request_body",
            truncate_body(body_str, max_body_bytes).as_ref(),
        );
    }

    // ── Model suffix parsing: "model(budget)" → model + thinking budget injection ──
    if let Some((base_model, budget)) = parse_model_thinking_suffix(&req.model) {
        req.model = base_model.clone();
        req.body = inject_thinking_budget(&req.body, budget);
        req.body = rewrite_model_in_body(&req.body, &base_model);
    }

    // ── Model ACL check ──
    if let Some(ctx) = req
        .api_key
        .as_ref()
        .and_then(|k| config.auth_key_store.lookup(k))
        && !prism_core::auth_key::AuthKeyStore::check_model_access(ctx, &req.model)
    {
        return Err(ProxyError::ModelNotAllowed(format!(
            "model '{}' not allowed for this API key",
            req.model
        )));
    }

    // ── Model rewrite (aliases + glob rewrites) ──
    if let Some(rewritten) = config.routing.resolve_model_rewrite(&req.model) {
        let rewritten = rewritten.to_string();
        req.body = rewrite_model_in_body(&req.body, &rewritten);
        req.model = rewritten;
    }

    // ── Cache lookup (non-stream, temperature=0) ──
    if !req.stream
        && let Some(ref cache) = state.response_cache
        && let Ok(body_val) = serde_json::from_slice::<serde_json::Value>(&req.body)
        && let Some(cache_key) = prism_core::cache::CacheKey::build_with_context(
            &req.model,
            &body_val,
            req.tenant_id.as_deref(),
            req.api_key_id.as_deref(),
            None,
        )
    {
        if let Some(cached) = cache.get(&cache_key).await {
            state.metrics.record_cache_hit();
            request_span.record("provider", cached.provider.as_str());
            request_span.record("model", cached.model.as_str());
            request_span.record("status", 200u64);
            request_span.record("latency_ms", start.elapsed().as_millis() as u64);
            request_span.record("total_attempts", 0u64);
            let resp = axum::http::Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .header("x-cache", "HIT")
                .body(axum::body::Body::from(cached.payload))
                .map_err(|e| ProxyError::Internal(format!("failed to build response: {e}")))?
                .into_response();
            return Ok(resp);
        }
        state.metrics.record_cache_miss();
    }

    // ── Extract features and plan route ──
    let features = extract_features(&req);

    // Merge client-provided model chain with planner's model resolution
    let catalog = state.catalog.snapshot();
    let health_snapshot = state.health_manager.snapshot();
    let plan = RoutePlanner::plan(&features, &config.routing, &catalog, &health_snapshot);

    // Override model chain with client-provided models if present
    let mut plan = plan;
    if let Some(ref models) = req.models
        && !models.is_empty()
    {
        // Client-provided model chain takes precedence, append planner fallbacks
        let mut chain = models.clone();
        for m in &plan.model_chain {
            if !chain.contains(m) {
                chain.push(m.clone());
            }
        }
        plan.model_chain = chain;
    }

    // Resolve failover config from the matched profile
    let profile_name = &plan.profile;
    let failover = config
        .routing
        .profiles
        .get(profile_name)
        .map(|p| p.failover.clone())
        .unwrap_or_default();

    if plan.attempts.is_empty() {
        state.metrics.record_error();
        state.metrics.record_latency_ms(start.elapsed().as_millis());
        let err = ProxyError::NoCredentials {
            provider: "all".to_string(),
            model: plan.model_chain.join(","),
        };
        request_span.record("total_attempts", 0u64);
        request_span.record("status", err.status_code_u16() as u64);
        request_span.record("latency_ms", start.elapsed().as_millis() as u64);
        request_span.record("error", err.to_string());
        request_span.record("error_type", classify_error(&err));
        return Err(err);
    }

    // ── Execute plan ──
    let controller = ExecutionController::new(state);
    match controller
        .execute(
            &plan,
            &req,
            &failover,
            &request_span,
            detail_level,
            max_body_bytes,
        )
        .await
    {
        Ok(result) => {
            request_span.record("total_attempts", result.total_attempts as u64);

            let mut resp = result.response;
            if req.debug {
                inject_route_headers(
                    &mut resp,
                    &plan.profile,
                    result.provider.as_deref(),
                    result.credential_name.as_deref(),
                    result.model.as_deref(),
                    result.total_attempts,
                );
            }
            Ok(resp)
        }
        Err(err) => {
            state.metrics.record_error();
            state.metrics.record_latency_ms(start.elapsed().as_millis());

            request_span.record("total_attempts", plan.attempts.len() as u64);
            request_span.record("status", err.status_code_u16() as u64);
            request_span.record("latency_ms", start.elapsed().as_millis() as u64);
            request_span.record("error", err.to_string());
            request_span.record("error_type", classify_error(&err));

            Err(err)
        }
    }
}

/// Record attempt success fields on an attempt span, then drop it.
fn record_attempt_success(attempt_span: tracing::Span, latency_ms: u64) {
    attempt_span.record("status", 200u64);
    attempt_span.record("latency_ms", latency_ms);
}

/// Record attempt failure fields on an attempt span.
fn record_attempt_failure(attempt_span: &tracing::Span, error: &ProxyError, latency_ms: u64) {
    attempt_span.record("latency_ms", latency_ms);
    if let ProxyError::Upstream { status, .. } = error {
        attempt_span.record("status", *status as u64);
    }
    attempt_span.record("error", error.to_string());
    attempt_span.record("error_type", classify_error(error));
}

/// Record usage and cost fields on a span.
pub(super) fn record_usage_on_span(
    span: &tracing::Span,
    usage: Option<&prism_core::request_record::TokenUsage>,
    cost: Option<f64>,
) {
    if let Some(u) = usage {
        span.record("usage_input", u.input_tokens);
        span.record("usage_output", u.output_tokens);
        span.record("usage_cache_read", u.cache_read_tokens);
        span.record("usage_cache_creation", u.cache_creation_tokens);
    }
    if let Some(c) = cost {
        span.record("cost", c);
    }
}

/// Parse model suffix `model(budget)` → (model_name, budget_tokens).
/// e.g., `claude-sonnet-4-5(10000)` → ("claude-sonnet-4-5", 10000)
fn parse_model_thinking_suffix(model: &str) -> Option<(String, u64)> {
    let model = model.trim();
    if !model.ends_with(')') {
        return None;
    }
    let open = model.rfind('(')?;
    let base = &model[..open];
    let budget_str = &model[open + 1..model.len() - 1];
    let budget = budget_str.parse::<u64>().ok()?;
    if base.is_empty() || budget == 0 {
        return None;
    }
    Some((base.to_string(), budget))
}

/// Inject `thinking.budget_tokens` into a request body if not already present.
fn inject_thinking_budget(body: &Bytes, budget: u64) -> Bytes {
    let Ok(mut val) = serde_json::from_slice::<serde_json::Value>(body) else {
        return body.clone();
    };
    // Don't override existing thinking config
    if val.get("thinking").is_some() {
        return body.clone();
    }
    val["thinking"] = serde_json::json!({
        "type": "enabled",
        "budget_tokens": budget,
    });
    serde_json::to_vec(&val)
        .map(Bytes::from)
        .unwrap_or_else(|_| body.clone())
}

#[cfg(test)]
mod tests {
    use super::helpers::{extract_usage, inject_stream_usage_option};
    use super::streaming::keepalive_error_json;
    use super::*;

    // === extract_usage ===

    #[test]
    fn test_extract_usage_openai_format() {
        let payload = r#"{"usage":{"prompt_tokens":10,"completion_tokens":20}}"#;
        let usage = extract_usage(payload).unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
    }

    #[test]
    fn test_extract_usage_claude_format() {
        let payload = r#"{"usage":{"input_tokens":15,"output_tokens":25}}"#;
        let usage = extract_usage(payload).unwrap();
        assert_eq!(usage.input_tokens, 15);
        assert_eq!(usage.output_tokens, 25);
    }

    #[test]
    fn test_extract_usage_claude_with_cache() {
        let payload = r#"{"usage":{"input_tokens":15,"output_tokens":25,"cache_read_input_tokens":100,"cache_creation_input_tokens":50}}"#;
        let usage = extract_usage(payload).unwrap();
        assert_eq!(usage.input_tokens, 15);
        assert_eq!(usage.output_tokens, 25);
        assert_eq!(usage.cache_read_tokens, 100);
        assert_eq!(usage.cache_creation_tokens, 50);
    }

    #[test]
    fn test_extract_usage_openai_with_cached_tokens() {
        let payload = r#"{"usage":{"prompt_tokens":10,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":5}}}"#;
        let usage = extract_usage(payload).unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.cache_read_tokens, 5);
    }

    #[test]
    fn test_extract_usage_gemini_format() {
        let payload = r#"{"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":8}}"#;
        let usage = extract_usage(payload).unwrap();
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 8);
    }

    #[test]
    fn test_extract_usage_no_usage() {
        let payload = r#"{"choices":[{"message":{"content":"hi"}}]}"#;
        assert!(extract_usage(payload).is_none());
    }

    #[test]
    fn test_extract_usage_invalid_json() {
        let payload = "not json";
        assert!(extract_usage(payload).is_none());
    }

    #[test]
    fn test_extract_usage_empty_usage() {
        let payload = r#"{"usage":{}}"#;
        assert!(extract_usage(payload).is_none());
    }

    // === inject_route_headers ===

    #[test]
    fn test_inject_route_headers_full() {
        let mut response = axum::http::Response::builder()
            .body(axum::body::Body::empty())
            .unwrap()
            .into_response();

        inject_route_headers(
            &mut response,
            "balanced",
            Some("openai"),
            Some("my-key"),
            Some("gpt-4"),
            2,
        );

        let summary = response
            .headers()
            .get("x-prism-route-summary")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(summary.contains("profile=balanced"));
        assert!(summary.contains("provider=openai"));
        assert!(summary.contains("credential=my-key"));
        assert!(summary.contains("model=gpt-4"));

        assert_eq!(
            response
                .headers()
                .get("x-prism-route-attempts")
                .unwrap()
                .to_str()
                .unwrap(),
            "2"
        );

        assert!(response.headers().get("x-prism-route-id").is_some());
    }

    #[test]
    fn test_inject_route_headers_minimal() {
        let mut response = axum::http::Response::builder()
            .body(axum::body::Body::empty())
            .unwrap()
            .into_response();

        inject_route_headers(&mut response, "stable", None, None, None, 1);

        let summary = response
            .headers()
            .get("x-prism-route-summary")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(summary.contains("profile=stable"));
        assert!(response.headers().get("x-prism-route-id").is_some());
    }

    // === rewrite_model_in_body ===

    #[test]
    fn test_rewrite_model_in_body() {
        let body = Bytes::from(r#"{"model":"gpt-4","messages":[]}"#);
        let result = rewrite_model_in_body(&body, "claude-3-sonnet");
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["model"], "claude-3-sonnet");
        assert!(val["messages"].is_array());
    }

    #[test]
    fn test_rewrite_model_in_body_no_model() {
        let body = Bytes::from(r#"{"messages":[]}"#);
        let result = rewrite_model_in_body(&body, "new-model");
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["model"], "new-model");
    }

    #[test]
    fn test_rewrite_model_in_body_invalid_json() {
        let body = Bytes::from("not json");
        let result = rewrite_model_in_body(&body, "new-model");
        assert_eq!(result, body);
    }

    // === keepalive_error_json ===

    #[test]
    fn test_keepalive_error_json() {
        let result = keepalive_error_json("something went wrong");
        let val: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(val["error"]["message"], "something went wrong");
        assert_eq!(val["error"]["type"], "server_error");
    }

    // === extract_usage: Claude streaming nested message.usage ===

    #[test]
    fn test_extract_usage_claude_message_start() {
        let payload = r#"{"type":"message_start","message":{"id":"msg_1","model":"claude-3","usage":{"input_tokens":25,"cache_read_input_tokens":100,"cache_creation_input_tokens":10}}}"#;
        let usage = extract_usage(payload).unwrap();
        assert_eq!(usage.input_tokens, 25);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_read_tokens, 100);
        assert_eq!(usage.cache_creation_tokens, 10);
    }

    #[test]
    fn test_extract_usage_claude_message_delta() {
        let payload = r#"{"type":"message_delta","usage":{"output_tokens":47}}"#;
        let usage = extract_usage(payload).unwrap();
        assert_eq!(usage.output_tokens, 47);
    }

    // === inject_stream_usage_option ===

    #[test]
    fn test_inject_stream_usage_option_adds_option() {
        let payload =
            serde_json::to_vec(&serde_json::json!({"model": "gpt-4", "stream": true})).unwrap();
        let result = inject_stream_usage_option(payload);
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["stream_options"]["include_usage"], true);
    }

    #[test]
    fn test_inject_stream_usage_option_preserves_existing() {
        let payload = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-4",
            "stream_options": {"include_usage": false, "other": 1}
        }))
        .unwrap();
        let result = inject_stream_usage_option(payload);
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["stream_options"]["include_usage"], false);
        assert_eq!(val["stream_options"]["other"], 1);
    }

    #[test]
    fn test_inject_stream_usage_option_invalid_json() {
        let payload = b"not json".to_vec();
        let result = inject_stream_usage_option(payload.clone());
        assert_eq!(result, payload);
    }

    // === parse_model_thinking_suffix ===

    #[test]
    fn test_parse_model_suffix_basic() {
        let (model, budget) = parse_model_thinking_suffix("claude-sonnet-4-5(10000)").unwrap();
        assert_eq!(model, "claude-sonnet-4-5");
        assert_eq!(budget, 10000);
    }

    #[test]
    fn test_parse_model_suffix_large_budget() {
        let (model, budget) = parse_model_thinking_suffix("gemini-2.5-flash(50000)").unwrap();
        assert_eq!(model, "gemini-2.5-flash");
        assert_eq!(budget, 50000);
    }

    #[test]
    fn test_parse_model_suffix_no_suffix() {
        assert!(parse_model_thinking_suffix("claude-3-5-sonnet").is_none());
    }

    #[test]
    fn test_parse_model_suffix_empty_budget() {
        assert!(parse_model_thinking_suffix("model()").is_none());
    }

    #[test]
    fn test_parse_model_suffix_zero_budget() {
        assert!(parse_model_thinking_suffix("model(0)").is_none());
    }

    #[test]
    fn test_parse_model_suffix_non_numeric() {
        assert!(parse_model_thinking_suffix("model(abc)").is_none());
    }

    #[test]
    fn test_parse_model_suffix_empty_model_name() {
        assert!(parse_model_thinking_suffix("(10000)").is_none());
    }

    // === inject_thinking_budget ===

    #[test]
    fn test_inject_thinking_budget_basic() {
        let body = Bytes::from(r#"{"model":"claude-sonnet-4-5","messages":[]}"#);
        let result = inject_thinking_budget(&body, 10000);
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(val["thinking"]["type"], "enabled");
        assert_eq!(val["thinking"]["budget_tokens"], 10000);
    }

    #[test]
    fn test_inject_thinking_budget_no_override() {
        let body = Bytes::from(
            r#"{"model":"claude-sonnet-4-5","thinking":{"type":"enabled","budget_tokens":5000}}"#,
        );
        let result = inject_thinking_budget(&body, 10000);
        let val: serde_json::Value = serde_json::from_slice(&result).unwrap();
        // Should not override existing thinking config
        assert_eq!(val["thinking"]["budget_tokens"], 5000);
    }

    #[test]
    fn test_inject_thinking_budget_invalid_json() {
        let body = Bytes::from("not json");
        let result = inject_thinking_budget(&body, 10000);
        assert_eq!(result, body);
    }
}
