use crate::AppState;
use crate::streaming::build_sse_response;
use ai_proxy_core::config::RetryConfig;
use ai_proxy_core::error::ProxyError;
use ai_proxy_core::provider::{Format, ProviderRequest, ProviderResponse, StreamChunk};
use ai_proxy_translator::TranslateState;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use std::time::{Duration, Instant};

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
#[derive(Clone, Debug)]
pub struct DispatchMeta {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost: Option<f64>,
}

/// Extract token usage from a response payload (any format).
fn extract_usage(payload: &str) -> (Option<u64>, Option<u64>) {
    let val: serde_json::Value = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    // OpenAI/Claude format: usage.prompt_tokens / usage.input_tokens
    if let Some(usage) = val.get("usage") {
        let input = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64()));
        let output = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64()));
        return (input, output);
    }
    // Gemini format: usageMetadata
    if let Some(usage) = val.get("usageMetadata") {
        let input = usage.get("promptTokenCount").and_then(|v| v.as_u64());
        let output = usage.get("candidatesTokenCount").and_then(|v| v.as_u64());
        return (input, output);
    }
    (None, None)
}

/// Inject dispatch metadata into response extensions for request logging.
fn inject_dispatch_meta(
    response: &mut Response,
    debug: &DispatchDebug,
    translated_payload: &str,
    cost_calculator: &ai_proxy_core::cost::CostCalculator,
    metrics: &ai_proxy_core::metrics::Metrics,
) {
    let (input_tokens, output_tokens) = extract_usage(translated_payload);
    let model = debug.model.as_deref();
    let cost = match (model, input_tokens, output_tokens) {
        (Some(m), Some(inp), Some(out)) => cost_calculator.calculate(m, inp, out),
        _ => None,
    };
    // Record tokens and cost in global metrics
    if let (Some(inp), Some(out)) = (input_tokens, output_tokens) {
        metrics.record_tokens(inp, out);
    }
    if let (Some(m), Some(c)) = (model, cost) {
        metrics.record_cost(m, c);
    }
    response.extensions_mut().insert(DispatchMeta {
        provider: debug.provider.clone(),
        model: debug.model.clone(),
        input_tokens,
        output_tokens,
        cost,
    });
}

/// Inject debug headers into a response if debug mode is enabled.
fn inject_debug_headers(response: &mut Response, debug: &DispatchDebug) {
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

/// Unified dispatch: resolves providers, picks credentials, translates, executes, retries.
///
/// Supports model fallback chains via `req.models` and debug mode via `req.debug`.
/// The retry loop iterates across all provider formats on each attempt, ensuring that
/// quota exhaustion (429) on one provider automatically falls through to the next (5B).
pub async fn dispatch(state: &AppState, req: DispatchRequest) -> Result<Response, ProxyError> {
    let start = Instant::now();
    let config = state.config.load();

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
                let auth = match state.router.pick(target_format, current_model, &tried) {
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
                        ai_proxy_core::payload::apply_payload_rules(
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
                        if ai_proxy_core::cloak::should_cloak(cloak_cfg, req.user_agent.as_deref())
                        {
                            let mut val: serde_json::Value =
                                serde_json::from_slice(&translated_payload)
                                    .unwrap_or(serde_json::Value::Null);
                            if val.is_object() {
                                ai_proxy_core::cloak::apply_cloak(
                                    &mut val,
                                    cloak_cfg,
                                    &auth.api_key,
                                );
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
                    && ai_proxy_core::cloak::should_cloak(cloak_cfg, req.user_agent.as_deref())
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
                            state.metrics.record_latency_ms(start.elapsed().as_millis());

                            let need_translate = state
                                .translators
                                .has_response_translator(req.source_format, target_format);

                            let keepalive = config.streaming.keepalive_seconds;

                            // For streaming, we can't easily inject headers after the fact.
                            // Debug info is not available for streaming responses.
                            if !need_translate {
                                if req.source_format == Format::Claude {
                                    let data_stream = tokio_stream::StreamExt::map(
                                        stream_result.stream,
                                        |result| {
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
                                        },
                                    );
                                    let mut resp =
                                        build_sse_response(data_stream, keepalive).into_response();
                                    resp.extensions_mut().insert(DispatchMeta {
                                        provider: debug_info.provider.clone(),
                                        model: debug_info.model.clone(),
                                        input_tokens: None,
                                        output_tokens: None,
                                        cost: None,
                                    });
                                    if req.debug {
                                        inject_debug_headers(&mut resp, &debug_info);
                                    }
                                    return Ok(resp);
                                }
                                let data_stream =
                                    tokio_stream::StreamExt::map(stream_result.stream, |result| {
                                        result.map(|chunk| chunk.data)
                                    });
                                let mut resp =
                                    build_sse_response(data_stream, keepalive).into_response();
                                resp.extensions_mut().insert(DispatchMeta {
                                    provider: debug_info.provider.clone(),
                                    model: debug_info.model.clone(),
                                    input_tokens: None,
                                    output_tokens: None,
                                    cost: None,
                                });
                                if req.debug {
                                    inject_debug_headers(&mut resp, &debug_info);
                                }
                                return Ok(resp);
                            }

                            let translated_stream = translate_stream(
                                stream_result.stream,
                                state.translators.clone(),
                                req.source_format,
                                target_format,
                                actual_model.clone(),
                                body.clone(),
                            );

                            let mut resp =
                                build_sse_response(translated_stream, keepalive).into_response();
                            resp.extensions_mut().insert(DispatchMeta {
                                provider: debug_info.provider.clone(),
                                model: debug_info.model.clone(),
                                input_tokens: None,
                                output_tokens: None,
                                cost: None,
                            });
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
                                    state.metrics.record_latency_ms(start.elapsed().as_millis());

                                    let translated = state.translators.translate_non_stream(
                                        req.source_format,
                                        target_format,
                                        &actual_model,
                                        &body,
                                        &response.payload,
                                    )?;

                                    let mut builder = axum::http::Response::builder()
                                        .header(axum::http::header::CONTENT_TYPE, "application/json");

                                    for header_name in &config.passthrough_headers {
                                        if let Some(val) = response.headers.get(header_name) {
                                            builder = builder.header(header_name.as_str(), val.as_str());
                                        }
                                    }

                                    let mut resp = builder
                                        .body(axum::body::Body::from(translated.clone()))
                                        .map_err(|e| ProxyError::Internal(format!("failed to build response: {e}")))?
                                        .into_response();
                                    inject_dispatch_meta(
                                        &mut resp,
                                        &debug_info,
                                        &translated,
                                        &state.cost_calculator,
                                        &state.metrics,
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
                            state.metrics.record_latency_ms(start.elapsed().as_millis());

                            let translated = state.translators.translate_non_stream(
                                req.source_format,
                                target_format,
                                &actual_model,
                                &body,
                                &response.payload,
                            )?;

                            let mut builder = axum::http::Response::builder()
                                .header(axum::http::header::CONTENT_TYPE, "application/json");

                            for header_name in &config.passthrough_headers {
                                if let Some(val) = response.headers.get(header_name) {
                                    builder = builder.header(header_name.as_str(), val.as_str());
                                }
                            }

                            let mut resp = builder
                                .body(axum::body::Body::from(translated.clone()))
                                .map_err(|e| {
                                    ProxyError::Internal(format!("failed to build response: {e}"))
                                })?
                                .into_response();
                            inject_dispatch_meta(
                                &mut resp,
                                &debug_info,
                                &translated,
                                &state.cost_calculator,
                                &state.metrics,
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

            // Exponential backoff with full jitter between retry rounds
            if attempt + 1 < max_retries {
                let cap = std::cmp::min(1u64 << attempt, max_backoff_secs) as f64;
                let jittered = rand::random::<f64>() * cap;
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

// ─── Model rewriting for fallback ──────────────────────────────────────────

/// Rewrite the `model` field in a JSON request body to use a different model name.
fn rewrite_model_in_body(body: &Bytes, new_model: &str) -> Bytes {
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

// ─── Non-stream keepalive body ─────────────────────────────────────────────

type ProviderResult = Result<ProviderResponse, ProxyError>;

/// Build a chunked response body that sends periodic whitespace while waiting
/// for the upstream response. Leading whitespace is valid JSON and is ignored
/// by parsers, so the client receives ` ` ` ` `{"choices":[...]}`.
fn build_keepalive_body(
    result_rx: std::pin::Pin<Box<tokio::sync::oneshot::Receiver<ProviderResult>>>,
    interval_secs: u64,
    translators: std::sync::Arc<ai_proxy_translator::TranslatorRegistry>,
    source_format: Format,
    target_format: Format,
    model: String,
    original_body: Bytes,
) -> axum::body::Body {
    struct KeepaliveState {
        rx: Option<std::pin::Pin<Box<tokio::sync::oneshot::Receiver<ProviderResult>>>>,
        interval_secs: u64,
        translators: std::sync::Arc<ai_proxy_translator::TranslatorRegistry>,
        source_format: Format,
        target_format: Format,
        model: String,
        original_body: Bytes,
    }

    let state = KeepaliveState {
        rx: Some(result_rx),
        interval_secs,
        translators,
        source_format,
        target_format,
        model,
        original_body,
    };

    let stream = futures::stream::unfold(state, |mut state| async move {
        let mut rx = state.rx.take()?;

        tokio::select! {
            result = &mut rx => {
                let data = match result {
                    Ok(Ok(response)) => {
                        match state.translators.translate_non_stream(
                            state.source_format,
                            state.target_format,
                            &state.model,
                            &state.original_body,
                            &response.payload,
                        ) {
                            Ok(translated) => translated,
                            Err(e) => keepalive_error_json(&e.to_string()),
                        }
                    }
                    Ok(Err(e)) => keepalive_error_json(&e.to_string()),
                    Err(_) => keepalive_error_json("internal error"),
                };
                // rx is consumed; stream will end on the next call (rx = None)
                Some((Ok::<Bytes, std::convert::Infallible>(Bytes::from(data)), state))
            }
            _ = tokio::time::sleep(Duration::from_secs(state.interval_secs)) => {
                // Put the receiver back for the next iteration
                state.rx = Some(rx);
                Some((Ok(Bytes::from_static(b" ")), state))
            }
        }
    });

    axum::body::Body::from_stream(stream)
}

fn keepalive_error_json(msg: &str) -> String {
    serde_json::json!({
        "error": {"message": msg, "type": "server_error"}
    })
    .to_string()
}

// ─── Stream translation ────────────────────────────────────────────────────

fn translate_stream(
    upstream: std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>,
    >,
    translators: std::sync::Arc<ai_proxy_translator::TranslatorRegistry>,
    from: Format,
    to: Format,
    model: String,
    orig_req: Bytes,
) -> impl tokio_stream::Stream<Item = Result<String, ProxyError>> + Send {
    futures::stream::unfold(
        (upstream, TranslateState::default(), true),
        move |(mut stream, mut state, active)| {
            let translators = translators.clone();
            let model = model.clone();
            let orig_req = orig_req.clone();
            async move {
                if !active {
                    return None;
                }

                use tokio_stream::StreamExt;
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        match translators.translate_stream(
                            from,
                            to,
                            &model,
                            &orig_req,
                            chunk.event_type.as_deref(),
                            chunk.data.as_bytes(),
                            &mut state,
                        ) {
                            Ok(lines) => {
                                let has_done = lines.iter().any(|l| l == "[DONE]");
                                let combined = lines.join("\n");
                                if combined.is_empty() {
                                    Some((Ok(String::new()), (stream, state, !has_done)))
                                } else {
                                    Some((Ok(combined), (stream, state, !has_done)))
                                }
                            }
                            Err(e) => Some((Err(e), (stream, state, false))),
                        }
                    }
                    Some(Err(e)) => Some((Err(e), (stream, state, false))),
                    None => None,
                }
            }
        },
    )
}

// ─── Retry error handling ──────────────────────────────────────────────────

fn handle_retry_error(
    state: &AppState,
    auth_id: &str,
    error: &ProxyError,
    retry_cfg: &RetryConfig,
) {
    state.metrics.record_error();
    match error {
        ProxyError::Upstream {
            status,
            retry_after_secs,
            ..
        } => match *status {
            429 => {
                // Respect upstream Retry-After header if present, otherwise use config default
                let secs = retry_after_secs.unwrap_or(retry_cfg.cooldown_429_secs);
                let cooldown = Duration::from_secs(secs);
                state.router.mark_unavailable(auth_id, cooldown);
                tracing::warn!("Rate limited (429), cooling down credential for {cooldown:?}");
            }
            s if (500..=599).contains(&s) => {
                let secs = retry_after_secs.unwrap_or(retry_cfg.cooldown_5xx_secs);
                let cooldown = Duration::from_secs(secs);
                state.router.mark_unavailable(auth_id, cooldown);
                tracing::warn!("Upstream error ({s}), cooling down credential for {cooldown:?}");
            }
            _ => {}
        },
        ProxyError::Network(_) => {
            let cooldown = Duration::from_secs(retry_cfg.cooldown_network_secs);
            state.router.mark_unavailable(auth_id, cooldown);
            tracing::warn!("Network error, cooling down credential for {cooldown:?}");
        }
        _ => {}
    }
}
