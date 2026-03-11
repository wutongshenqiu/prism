mod helpers;
mod retry;
mod streaming;

use crate::AppState;
use crate::streaming::build_sse_response;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use helpers::{
    build_json_response, inject_debug_headers, inject_dispatch_meta, rewrite_model_in_body,
};
use prism_core::error::ProxyError;
use prism_core::provider::{Format, ProviderRequest, ProviderResponse};
use retry::handle_retry_error;
use std::time::{Duration, Instant};
use streaming::{StreamDoneContext, build_keepalive_body, translate_stream, with_usage_capture};

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
}

/// Debug information collected during dispatch for x-debug response headers.
#[derive(Debug, Default)]
struct DispatchDebug {
    provider: Option<String>,
    model: Option<String>,
    credential_name: Option<String>,
    attempts: Vec<String>,
}

/// Metadata about a dispatched request, stored in response extensions
/// so the logging middleware can populate log entries.
#[derive(Clone, Debug, Default)]
pub struct DispatchMeta {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub requested_model: Option<String>,
    pub credential_name: Option<String>,
    pub stream: bool,
    pub retry_count: u32,
    pub usage: Option<prism_core::request_record::TokenUsage>,
    pub cost: Option<f64>,
    pub error_detail: Option<String>,
}

/// Unified dispatch: resolves providers, picks credentials, translates, executes, retries.
///
/// Supports model fallback chains via `req.models` and debug mode via `req.debug`.
/// The retry loop iterates across all provider formats on each attempt, ensuring that
/// quota exhaustion (429) on one provider automatically falls through to the next (5B).
pub async fn dispatch(state: &AppState, req: DispatchRequest) -> Result<Response, ProxyError> {
    let start = Instant::now();
    let config = state.config.load();

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

    // ── Cache lookup (non-stream, temperature=0) ──
    if !req.stream
        && let Some(ref cache) = state.response_cache
        && let Ok(body_val) = serde_json::from_slice::<serde_json::Value>(&req.body)
        && let Some(cache_key) = prism_core::cache::CacheKey::build(&req.model, &body_val)
    {
        if let Some(cached) = cache.get(&cache_key).await {
            state.metrics.record_cache_hit();
            let mut resp = axum::http::Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .header("x-cache", "HIT")
                .body(axum::body::Body::from(cached.payload))
                .map_err(|e| ProxyError::Internal(format!("failed to build response: {e}")))?
                .into_response();
            resp.extensions_mut().insert(DispatchMeta {
                provider: Some(cached.provider),
                model: Some(cached.model),
                requested_model: Some(req.model.clone()),
                stream: false,
                ..Default::default()
            });
            return Ok(resp);
        }
        state.metrics.record_cache_miss();
    }

    // Build the model fallback chain
    let model_chain: Vec<String> = if let Some(ref models) = req.models {
        if models.is_empty() {
            vec![req.model.clone()]
        } else {
            models.clone()
        }
    } else {
        vec![req.model.clone()]
    };

    let mut debug_info = DispatchDebug::default();
    let mut last_error: Option<ProxyError> = None;
    let mut total_attempts: u32 = 0;

    // Outer loop: try each model in the fallback chain
    for current_model in &model_chain {
        // Enforce model prefix requirement
        if config.force_model_prefix && !state.router.model_has_prefix(current_model) {
            debug_info
                .attempts
                .push(format!("{current_model}: prefix_required"));
            continue;
        }

        let providers = match req.allowed_formats {
            Some(ref formats) => formats.clone(),
            None => state.router.resolve_providers(current_model),
        };

        if providers.is_empty() {
            debug_info
                .attempts
                .push(format!("{current_model}: no_provider"));
            continue;
        }

        let retry_cfg = &config.retry;
        let max_retries = retry_cfg.max_retries;
        let max_backoff_secs = retry_cfg.max_backoff_secs;
        let bootstrap_limit = config.streaming.bootstrap_retries;
        let keepalive_secs = config.non_stream_keepalive_secs;

        let mut tried: Vec<String> = Vec::new();
        let mut bootstrap_attempts = 0u32;

        // Rewrite request body to use current_model (for fallback)
        let body = if current_model != &req.model {
            rewrite_model_in_body(&req.body, current_model)
        } else {
            req.body.clone()
        };

        for attempt in 0..max_retries {
            for &target_format in &providers {
                let auth = match state.router.pick(
                    target_format,
                    current_model,
                    &tried,
                    req.client_region.as_deref(),
                ) {
                    Some(a) => a,
                    None => continue,
                };

                let actual_model = auth.resolve_model_id(current_model);

                let executor = match state.executors.get_by_format(target_format) {
                    Some(e) => e,
                    None => continue,
                };

                debug_info
                    .attempts
                    .push(format!("{}@{}", actual_model, target_format.as_str()));

                total_attempts += 1;

                // Record metrics
                state
                    .metrics
                    .record_request(&actual_model, target_format.as_str());

                // Translate request (source → target format)
                let translated_payload = state.translators.translate_request(
                    req.source_format,
                    target_format,
                    &actual_model,
                    &body,
                    req.stream,
                )?;

                // Apply payload manipulation rules
                let translated_payload = {
                    let mut payload_value: serde_json::Value =
                        serde_json::from_slice(&translated_payload)
                            .unwrap_or(serde_json::Value::Null);
                    if payload_value.is_object() {
                        prism_core::payload::apply_payload_rules(
                            &mut payload_value,
                            &config.payload,
                            &actual_model,
                            Some(target_format.as_str()),
                        );
                        serde_json::to_vec(&payload_value).unwrap_or(translated_payload)
                    } else {
                        translated_payload
                    }
                };

                // Apply cloaking for Claude targets
                let translated_payload = if target_format == Format::Claude {
                    if let Some(ref cloak_cfg) = auth.cloak {
                        if prism_core::cloak::should_cloak(cloak_cfg, req.user_agent.as_deref()) {
                            let mut val: serde_json::Value =
                                serde_json::from_slice(&translated_payload)
                                    .unwrap_or(serde_json::Value::Null);
                            if val.is_object() {
                                prism_core::cloak::apply_cloak(&mut val, cloak_cfg, &auth.api_key);
                                serde_json::to_vec(&val).unwrap_or(translated_payload)
                            } else {
                                translated_payload
                            }
                        } else {
                            translated_payload
                        }
                    } else {
                        translated_payload
                    }
                } else {
                    translated_payload
                };

                // Build request headers — inject claude-header-defaults when cloaking
                let mut request_headers: std::collections::HashMap<String, String> =
                    Default::default();
                if target_format == Format::Claude
                    && let Some(ref cloak_cfg) = auth.cloak
                    && prism_core::cloak::should_cloak(cloak_cfg, req.user_agent.as_deref())
                {
                    for (k, v) in &config.claude_header_defaults {
                        request_headers.insert(k.clone(), v.clone());
                    }
                }

                let provider_request = ProviderRequest {
                    model: actual_model.clone(),
                    payload: Bytes::from(translated_payload),
                    source_format: req.source_format,
                    stream: req.stream,
                    headers: request_headers,
                    original_request: Some(body.clone()),
                };

                // Update debug info for successful routing
                debug_info.provider = Some(target_format.as_str().to_string());
                debug_info.model = Some(actual_model.clone());
                debug_info.credential_name = auth.name().map(|s| s.to_string());

                if req.stream {
                    // ── Streaming path with bootstrap retry limit (4D) ──
                    match executor.execute_stream(&auth, provider_request).await {
                        Ok(stream_result) => {
                            let latency_ms = start.elapsed().as_millis();
                            state.metrics.record_latency_ms(latency_ms);
                            state.router.record_success(&auth.id);
                            state.router.record_latency(&auth.id, latency_ms as f64);

                            let need_translate = state
                                .translators
                                .has_response_translator(req.source_format, target_format);

                            let keepalive = config.streaming.keepalive_seconds;

                            // Wrap upstream stream to capture token usage from SSE events.
                            // When the stream ends, captured usage is written back to the
                            // request log entry created by the logging middleware.
                            let captured_stream = if let Some(ref rid) = req.request_id {
                                with_usage_capture(
                                    stream_result.stream,
                                    StreamDoneContext {
                                        request_id: rid.clone(),
                                        model: debug_info.model.clone(),
                                        request_logs: state.request_logs.clone(),
                                        cost_calculator: state.cost_calculator.clone(),
                                        metrics: state.metrics.clone(),
                                    },
                                )
                            } else {
                                stream_result.stream
                            };

                            let dispatch_meta = DispatchMeta {
                                provider: debug_info.provider.clone(),
                                model: debug_info.model.clone(),
                                requested_model: Some(req.model.clone()),
                                credential_name: debug_info.credential_name.clone(),
                                stream: true,
                                retry_count: total_attempts.saturating_sub(1),
                                ..Default::default()
                            };

                            if !need_translate {
                                if req.source_format == Format::Claude {
                                    let data_stream =
                                        tokio_stream::StreamExt::map(captured_stream, |result| {
                                            result.map(|chunk| {
                                                if let Some(ref event_type) = chunk.event_type {
                                                    format!(
                                                        "event: {event_type}\ndata: {}",
                                                        chunk.data
                                                    )
                                                } else {
                                                    chunk.data
                                                }
                                            })
                                        });
                                    let mut resp =
                                        build_sse_response(data_stream, keepalive).into_response();
                                    resp.extensions_mut().insert(dispatch_meta);
                                    if req.debug {
                                        inject_debug_headers(&mut resp, &debug_info);
                                    }
                                    return Ok(resp);
                                }
                                let data_stream =
                                    tokio_stream::StreamExt::map(captured_stream, |result| {
                                        result.map(|chunk| chunk.data)
                                    });
                                let mut resp =
                                    build_sse_response(data_stream, keepalive).into_response();
                                resp.extensions_mut().insert(dispatch_meta);
                                if req.debug {
                                    inject_debug_headers(&mut resp, &debug_info);
                                }
                                return Ok(resp);
                            }

                            let translated_stream = translate_stream(
                                captured_stream,
                                state.translators.clone(),
                                req.source_format,
                                target_format,
                                actual_model.clone(),
                                body.clone(),
                            );

                            let mut resp =
                                build_sse_response(translated_stream, keepalive).into_response();
                            resp.extensions_mut().insert(dispatch_meta);
                            if req.debug {
                                inject_debug_headers(&mut resp, &debug_info);
                            }
                            return Ok(resp);
                        }
                        Err(e) => {
                            bootstrap_attempts += 1;
                            tried.push(auth.id.clone());
                            handle_retry_error(state, &auth.id, &e, retry_cfg);

                            if bootstrap_attempts > bootstrap_limit {
                                tracing::warn!(
                                    "Streaming bootstrap retry limit reached ({bootstrap_limit}), giving up"
                                );
                                state.metrics.record_error();
                                state.metrics.record_latency_ms(start.elapsed().as_millis());
                                // For fallback: continue to next model instead of returning error
                                last_error = Some(e);
                                break;
                            }
                            last_error = Some(e);
                        }
                    }
                } else if keepalive_secs > 0 {
                    // ── Non-stream with keepalive (5A) ──
                    let (result_tx, result_rx) =
                        tokio::sync::oneshot::channel::<Result<ProviderResponse, ProxyError>>();
                    let exec = executor.clone();
                    let auth_clone = auth.clone();
                    tokio::spawn(async move {
                        let result = exec.execute(&auth_clone, provider_request).await;
                        let _ = result_tx.send(result);
                    });

                    let mut result_rx = Box::pin(result_rx);

                    tokio::select! {
                        result = &mut result_rx => {
                            match result {
                                Ok(Ok(response)) => {
                                    let latency_ms = start.elapsed().as_millis();
                                    state.metrics.record_latency_ms(latency_ms);
                                    state.router.record_success(&auth.id);
                                    state.router.record_latency(&auth.id, latency_ms as f64);

                                    let translated = state.translators.translate_non_stream(
                                        req.source_format,
                                        target_format,
                                        &actual_model,
                                        &body,
                                        &response.payload,
                                    )?;

                                    let mut resp = build_json_response(
                                        &translated,
                                        &config.passthrough_headers,
                                        &response.headers,
                                    )?;
                                    inject_dispatch_meta(
                                        &mut resp,
                                        &debug_info,
                                        &response.payload,
                                        &state.cost_calculator,
                                        &state.metrics,
                                        &req.model,
                                        total_attempts,
                                    );
                                    if req.debug {
                                        inject_debug_headers(&mut resp, &debug_info);
                                    }
                                    return Ok(resp);
                                }
                                Ok(Err(e)) => {
                                    tried.push(auth.id.clone());
                                    handle_retry_error(state, &auth.id, &e, retry_cfg);
                                    last_error = Some(e);
                                }
                                Err(_) => {
                                    tried.push(auth.id.clone());
                                    last_error = Some(ProxyError::Internal(
                                        "upstream execute task failed".into(),
                                    ));
                                }
                            }
                        }
                        _ = tokio::time::sleep(Duration::from_secs(keepalive_secs)) => {
                            tracing::debug!(
                                "Non-stream request exceeded {keepalive_secs}s, enabling keepalive"
                            );
                            state.metrics.record_latency_ms(start.elapsed().as_millis());

                            let keepalive_body = build_keepalive_body(
                                result_rx,
                                keepalive_secs,
                                state.translators.clone(),
                                req.source_format,
                                target_format,
                                actual_model.clone(),
                                body.clone(),
                            );

                            let mut resp = axum::http::Response::builder()
                                .header(axum::http::header::CONTENT_TYPE, "application/json")
                                .body(keepalive_body)
                                .map_err(|e| ProxyError::Internal(format!("failed to build response: {e}")))?
                                .into_response();
                            if req.debug {
                                inject_debug_headers(&mut resp, &debug_info);
                            }
                            return Ok(resp);
                        }
                    }
                } else {
                    // ── Non-stream without keepalive (standard path) ──
                    match executor.execute(&auth, provider_request).await {
                        Ok(response) => {
                            let latency_ms = start.elapsed().as_millis();
                            state.metrics.record_latency_ms(latency_ms);
                            state.router.record_success(&auth.id);
                            state.router.record_latency(&auth.id, latency_ms as f64);

                            let translated = state.translators.translate_non_stream(
                                req.source_format,
                                target_format,
                                &actual_model,
                                &body,
                                &response.payload,
                            )?;

                            // Write to cache for non-stream, temperature=0 requests
                            if let Some(ref cache) = state.response_cache
                                && let Ok(body_val) =
                                    serde_json::from_slice::<serde_json::Value>(&req.body)
                                && let Some(cache_key) =
                                    prism_core::cache::CacheKey::build(&req.model, &body_val)
                            {
                                let cached = prism_core::cache::CachedResponse {
                                    payload: Bytes::from(translated.clone()),
                                    provider: target_format.as_str().to_string(),
                                    model: actual_model.clone(),
                                    input_tokens: 0,
                                    output_tokens: 0,
                                };
                                cache.insert(cache_key, cached).await;
                            }

                            let mut resp = build_json_response(
                                &translated,
                                &config.passthrough_headers,
                                &response.headers,
                            )?;
                            inject_dispatch_meta(
                                &mut resp,
                                &debug_info,
                                &response.payload,
                                &state.cost_calculator,
                                &state.metrics,
                                &req.model,
                                total_attempts,
                            );
                            if req.debug {
                                inject_debug_headers(&mut resp, &debug_info);
                            }
                            return Ok(resp);
                        }
                        Err(e) => {
                            tried.push(auth.id.clone());
                            handle_retry_error(state, &auth.id, &e, retry_cfg);
                            last_error = Some(e);
                        }
                    }
                }
            }

            // Exponential backoff with configurable jitter between retry rounds
            if attempt + 1 < max_retries {
                let cap = std::cmp::min(1u64 << attempt, max_backoff_secs) as f64;
                let jitter_factor = retry_cfg.jitter_factor.clamp(0.0, 1.0);
                let base = cap * (1.0 - jitter_factor);
                let jittered = base + rand::random::<f64>() * cap * jitter_factor;
                tokio::time::sleep(Duration::from_secs_f64(jittered)).await;
            }
        }
    }

    state.metrics.record_error();
    state.metrics.record_latency_ms(start.elapsed().as_millis());

    Err(last_error.unwrap_or_else(|| ProxyError::NoCredentials {
        provider: "all".to_string(),
        model: model_chain.join(","),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use helpers::*;
    use streaming::keepalive_error_json;

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

    // === inject_debug_headers ===

    #[test]
    fn test_inject_debug_headers_full() {
        let mut response = axum::http::Response::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        let debug = DispatchDebug {
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            credential_name: Some("my-key".to_string()),
            attempts: vec!["attempt1".to_string(), "attempt2".to_string()],
        };

        inject_debug_headers(&mut response, &debug);

        assert_eq!(
            response.headers().get("x-debug-provider").unwrap(),
            "openai"
        );
        assert_eq!(response.headers().get("x-debug-model").unwrap(), "gpt-4");
        assert_eq!(
            response.headers().get("x-debug-credential").unwrap(),
            "my-key"
        );
        assert_eq!(
            response.headers().get("x-debug-attempts").unwrap(),
            "attempt1, attempt2"
        );
    }

    #[test]
    fn test_inject_debug_headers_empty() {
        let mut response = axum::http::Response::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        let debug = DispatchDebug::default();

        inject_debug_headers(&mut response, &debug);

        assert!(response.headers().get("x-debug-provider").is_none());
        assert!(response.headers().get("x-debug-model").is_none());
        assert!(response.headers().get("x-debug-credential").is_none());
        assert!(response.headers().get("x-debug-attempts").is_none());
    }

    #[test]
    fn test_inject_debug_headers_partial() {
        let mut response = axum::http::Response::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        let debug = DispatchDebug {
            provider: Some("claude".to_string()),
            model: None,
            credential_name: None,
            attempts: vec![],
        };

        inject_debug_headers(&mut response, &debug);

        assert_eq!(
            response.headers().get("x-debug-provider").unwrap(),
            "claude"
        );
        assert!(response.headers().get("x-debug-model").is_none());
    }

    // === DispatchRequest model chain ===

    #[test]
    fn test_model_chain_from_models() {
        let req = DispatchRequest {
            source_format: Format::OpenAI,
            model: "gpt-4".to_string(),
            models: Some(vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()]),
            stream: false,
            body: Bytes::new(),
            allowed_formats: None,
            user_agent: None,
            debug: false,
            api_key: None,
            client_region: None,
            request_id: None,
        };

        let chain: Vec<String> = if let Some(ref models) = req.models {
            if models.is_empty() {
                vec![req.model.clone()]
            } else {
                models.clone()
            }
        } else {
            vec![req.model.clone()]
        };

        assert_eq!(chain, vec!["gpt-4", "gpt-3.5-turbo"]);
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
        // Returns original body on parse failure
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

    #[test]
    fn test_model_chain_single() {
        let model_chain: Vec<String> = {
            let models: Option<Vec<String>> = None;
            if let Some(ref models) = models {
                if models.is_empty() {
                    vec!["gpt-4".to_string()]
                } else {
                    models.clone()
                }
            } else {
                vec!["gpt-4".to_string()]
            }
        };

        assert_eq!(model_chain, vec!["gpt-4"]);
    }
}
