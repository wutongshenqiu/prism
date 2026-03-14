use crate::AppState;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use prism_core::error::ProxyError;
use prism_core::provider::{Format, ProviderRequest, ProviderResponse};
use prism_core::request_record::{LogDetailLevel, truncate_body};
use prism_core::routing::config::FailoverConfig;
use prism_core::routing::types::{RouteAttemptPlan, RouteFallbackEvent, RoutePlan, RouteTrace};
use std::time::{Duration, Instant};

use super::helpers::{
    build_json_response, extract_usage, inject_stream_usage_option_value, rewrite_model_in_body,
};
use super::streaming::{
    StreamDoneContext, build_keepalive_body, translate_stream, with_usage_capture,
};
use super::{
    DispatchRequest, record_attempt_failure, record_attempt_success, record_usage_on_span,
};

/// Result of executing a route plan.
pub(super) struct ExecutionResult {
    pub response: Response,
    #[allow(dead_code)]
    pub trace: RouteTrace,
    pub total_attempts: u32,
    /// Provider format of the successful attempt (for span recording).
    pub provider: Option<String>,
    /// Model used in the successful attempt.
    pub model: Option<String>,
    /// Credential name of the successful attempt.
    pub credential_name: Option<String>,
}

/// Executes a pre-computed `RoutePlan` with stage-aware failover.
///
/// Walks through attempts grouped by model → provider → credential,
/// each with independent attempt limits from `FailoverConfig`.
pub(super) struct ExecutionController<'a> {
    state: &'a AppState,
}

impl<'a> ExecutionController<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// Execute the route plan, trying attempts in order with stage-aware limits.
    pub async fn execute(
        &self,
        plan: &RoutePlan,
        req: &DispatchRequest,
        failover: &FailoverConfig,
        request_span: &tracing::Span,
        detail_level: LogDetailLevel,
        max_body_bytes: usize,
    ) -> Result<ExecutionResult, ProxyError> {
        let mut trace = plan.trace.clone();
        let mut total_attempts: u32 = 0;
        let mut last_error: Option<ProxyError> = None;

        // Group attempts by model, then by provider within each model
        let model_groups = group_attempts_by_model(&plan.model_chain, &plan.attempts);

        for (model_idx, (model, provider_groups)) in model_groups.iter().enumerate() {
            if model_idx >= failover.model_attempts as usize {
                break;
            }

            for (provider_idx, (provider, attempts)) in provider_groups.iter().enumerate() {
                if provider_idx >= failover.provider_attempts as usize {
                    break;
                }

                for (cred_idx, attempt) in attempts.iter().enumerate() {
                    if cred_idx >= failover.credential_attempts as usize {
                        break;
                    }

                    total_attempts += 1;

                    match self
                        .execute_single_attempt(
                            attempt,
                            model,
                            *provider,
                            req,
                            request_span,
                            detail_level,
                            max_body_bytes,
                            total_attempts,
                        )
                        .await
                    {
                        Ok(response) => {
                            return Ok(ExecutionResult {
                                response,
                                trace,
                                total_attempts,
                                provider: Some(provider.as_str().to_string()),
                                model: Some(attempt.model.clone()),
                                credential_name: Some(attempt.credential_name.clone()),
                            });
                        }
                        Err(err) => {
                            trace.fallback_events.push(RouteFallbackEvent {
                                from_model: model.clone(),
                                to_model: model.clone(),
                                reason: format!("{err}"),
                            });
                            last_error = Some(err);
                        }
                    }
                }
            }

            // Record model fallback event
            if model_idx + 1 < model_groups.len() {
                let next_model = &model_groups[model_idx + 1].0;
                trace.fallback_events.push(RouteFallbackEvent {
                    from_model: model.clone(),
                    to_model: next_model.clone(),
                    reason: "all_providers_exhausted".into(),
                });
            }
        }

        Err(last_error.unwrap_or_else(|| ProxyError::NoCredentials {
            provider: "all".to_string(),
            model: plan.model_chain.join(","),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_single_attempt(
        &self,
        attempt: &RouteAttemptPlan,
        _model: &str,
        target_format: Format,
        req: &DispatchRequest,
        request_span: &tracing::Span,
        detail_level: LogDetailLevel,
        max_body_bytes: usize,
        attempt_number: u32,
    ) -> Result<Response, ProxyError> {
        let config = self.state.config.load();
        let start = Instant::now();

        // Find the actual credential from the router
        let auth = self
            .state
            .router
            .find_credential(&attempt.credential_id)
            .ok_or_else(|| ProxyError::NoCredentials {
                provider: target_format.as_str().to_string(),
                model: attempt.model.clone(),
            })?;

        let actual_model = auth.resolve_model_id(&attempt.model);

        let executor = self
            .state
            .executors
            .get_by_format(target_format)
            .ok_or_else(|| {
                ProxyError::Internal(format!("no executor for format {}", target_format.as_str()))
            })?;

        let attempt_start = Instant::now();

        // Create attempt span
        let attempt_span = tracing::info_span!(
            parent: request_span,
            "gateway.attempt",
            attempt_index = attempt_number.saturating_sub(1) as u64,
            provider = target_format.as_str(),
            model = actual_model.as_str(),
            credential_name = auth.name().unwrap_or("-"),
            status = tracing::field::Empty,
            latency_ms = tracing::field::Empty,
            error = tracing::field::Empty,
            error_type = tracing::field::Empty,
        );

        // Record metrics
        self.state
            .metrics
            .record_request(&actual_model, target_format.as_str());

        // Rewrite body if model changed (for fallback chain)
        let body = if attempt.model != req.model {
            rewrite_model_in_body(&req.body, &attempt.model)
        } else {
            req.body.clone()
        };

        // Translate request
        let translated_payload = self.state.translators.translate_request(
            req.source_format,
            target_format,
            &actual_model,
            &body,
            req.stream,
        )?;

        // Parse payload into mutable Value for manipulation pipeline
        let mut payload_value: serde_json::Value =
            serde_json::from_slice(&translated_payload).unwrap_or(serde_json::Value::Null);

        // Apply payload manipulation rules
        if payload_value.is_object() {
            prism_core::payload::apply_payload_rules(
                &mut payload_value,
                &config.payload,
                &actual_model,
                Some(target_format.as_str()),
            );
        }

        // Apply upstream presentation (unified headers + body mutations)
        let presentation_ctx = prism_core::presentation::PresentationContext {
            target_format,
            model: &actual_model,
            user_agent: req.user_agent.as_deref(),
            api_key: &auth.api_key,
        };
        let presentation_result = prism_core::presentation::apply(
            &auth.upstream_presentation,
            &presentation_ctx,
            &mut payload_value,
        );

        // Inject cached thinking signatures for Claude targets
        if target_format == Format::Claude
            && let Some(ref thinking_cache) = self.state.thinking_cache
        {
            let tenant_id = req.tenant_id.as_deref().unwrap_or("");
            let injected = thinking_cache
                .inject_into_request(tenant_id, &actual_model, &mut payload_value)
                .await;
            if injected > 0 {
                tracing::debug!(
                    injected,
                    model = actual_model.as_str(),
                    "Injected cached thinking signatures"
                );
            }
        }

        // Inject stream_options.include_usage for OpenAI-format streaming
        if req.stream && target_format == Format::OpenAI {
            inject_stream_usage_option_value(&mut payload_value);
        }

        // Serialize final payload
        let final_payload =
            serde_json::to_vec(&payload_value).unwrap_or_else(|_| translated_payload.clone());

        // Record upstream request body on span
        if detail_level >= LogDetailLevel::Standard
            && let Ok(upstream_str) = std::str::from_utf8(&final_payload)
        {
            request_span.record(
                "upstream_request_body",
                truncate_body(upstream_str, max_body_bytes).as_ref(),
            );
        }

        let provider_request = ProviderRequest {
            model: actual_model.clone(),
            payload: Bytes::from(final_payload),
            source_format: req.source_format,
            stream: req.stream,
            headers: presentation_result.headers,
            original_request: Some(body.clone()),
        };

        // Debug info for headers
        let debug_provider = target_format.as_str().to_string();
        let debug_model = actual_model.clone();
        let debug_credential = auth.name().map(|s| s.to_string());

        let keepalive_secs = config.non_stream_keepalive_secs;

        if req.stream {
            // ── Streaming path ──
            match executor.execute_stream(&auth, provider_request).await {
                Ok(stream_result) => {
                    let latency_ms = start.elapsed().as_millis();
                    self.state.metrics.record_latency_ms(latency_ms);
                    self.state.router.record_success(&auth.id);
                    self.state
                        .router
                        .record_latency(&auth.id, latency_ms as f64);

                    record_attempt_success(
                        attempt_span,
                        attempt_start.elapsed().as_millis() as u64,
                    );

                    // Record request-level fields
                    request_span.record("provider", debug_provider.as_str());
                    request_span.record("model", debug_model.as_str());
                    request_span.record(
                        "credential_name",
                        debug_credential.as_deref().unwrap_or("-"),
                    );
                    request_span.record("status", 200u64);
                    request_span.record("latency_ms", latency_ms as u64);

                    let need_translate = self
                        .state
                        .translators
                        .has_response_translator(req.source_format, target_format);

                    let keepalive = config.streaming.keepalive_seconds;

                    let captured_stream = with_usage_capture(
                        stream_result.stream,
                        StreamDoneContext {
                            model: Some(debug_model.clone()),
                            cost_calculator: self.state.cost_calculator.clone(),
                            metrics: self.state.metrics.clone(),
                            rate_limiter: self.state.rate_limiter.clone(),
                            api_key: req.api_key.clone(),
                        },
                        request_span.clone(),
                        detail_level,
                        max_body_bytes,
                    );

                    if !need_translate {
                        if req.source_format == Format::Claude {
                            let data_stream =
                                tokio_stream::StreamExt::map(captured_stream, |result| {
                                    result.map(|chunk| {
                                        if let Some(ref event_type) = chunk.event_type {
                                            format!("event: {event_type}\ndata: {}", chunk.data)
                                        } else {
                                            chunk.data
                                        }
                                    })
                                });
                            let resp = crate::streaming::build_sse_response(data_stream, keepalive)
                                .into_response();
                            return Ok(resp);
                        }
                        let data_stream = tokio_stream::StreamExt::map(captured_stream, |result| {
                            result.map(|chunk| chunk.data)
                        });
                        let resp = crate::streaming::build_sse_response(data_stream, keepalive)
                            .into_response();
                        return Ok(resp);
                    }

                    let translated_stream = translate_stream(
                        captured_stream,
                        self.state.translators.clone(),
                        req.source_format,
                        target_format,
                        actual_model.clone(),
                        body.clone(),
                    );

                    let resp = crate::streaming::build_sse_response(translated_stream, keepalive)
                        .into_response();
                    Ok(resp)
                }
                Err(e) => {
                    record_attempt_failure(
                        &attempt_span,
                        &e,
                        attempt_start.elapsed().as_millis() as u64,
                    );
                    drop(attempt_span);

                    self.handle_attempt_error(&auth.id, &e);

                    Err(e)
                }
            }
        } else if keepalive_secs > 0 {
            // ── Non-stream with keepalive ──
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
                            self.state.metrics.record_latency_ms(latency_ms);
                            self.state.router.record_success(&auth.id);
                            self.state.router.record_latency(&auth.id, latency_ms as f64);

                            // Extract thinking signatures from Claude responses
                            if target_format == Format::Claude
                                && let Some(ref tc) = self.state.thinking_cache
                            {
                                let tenant_id = req.tenant_id.as_deref().unwrap_or("");
                                tc.extract_from_response(tenant_id, &actual_model, &response.payload)
                                    .await;
                            }

                            let translated = self.state.translators.translate_non_stream(
                                req.source_format,
                                target_format,
                                &actual_model,
                                &body,
                                &response.payload,
                            )?;

                            record_attempt_success(attempt_span, attempt_start.elapsed().as_millis() as u64);

                            // Record usage and metrics
                            self.record_non_stream_success(
                                request_span,
                                &debug_provider,
                                &debug_model,
                                debug_credential.as_deref(),
                                &response.payload,
                                req,
                                start,
                            );

                            if detail_level >= LogDetailLevel::Standard {
                                request_span.record("response_body",
                                    truncate_body(&translated, max_body_bytes).as_ref());
                            }

                            // Write to cache
                            self.try_cache_write(req, &auth, target_format, &actual_model, &translated).await;

                            let resp = build_json_response(
                                &translated,
                                &config.passthrough_headers,
                                &response.headers,
                            )?;
                            Ok(resp)
                        }
                        Ok(Err(e)) => {
                            record_attempt_failure(&attempt_span, &e, attempt_start.elapsed().as_millis() as u64);
                            drop(attempt_span);
                            self.handle_attempt_error(&auth.id, &e);
                            Err(e)
                        }
                        Err(_) => {
                            let join_err = ProxyError::Internal("upstream execute task failed".into());
                            record_attempt_failure(&attempt_span, &join_err, attempt_start.elapsed().as_millis() as u64);
                            drop(attempt_span);
                            Err(join_err)
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(keepalive_secs)) => {
                    tracing::debug!(
                        "Non-stream request exceeded {keepalive_secs}s, enabling keepalive"
                    );
                    self.state.metrics.record_latency_ms(start.elapsed().as_millis());

                    request_span.record("provider", debug_provider.as_str());
                    request_span.record("model", debug_model.as_str());

                    let keepalive_body = build_keepalive_body(
                        result_rx,
                        keepalive_secs,
                        self.state.translators.clone(),
                        req.source_format,
                        target_format,
                        actual_model.clone(),
                        body.clone(),
                    );

                    let resp = axum::http::Response::builder()
                        .header(axum::http::header::CONTENT_TYPE, "application/json")
                        .body(keepalive_body)
                        .map_err(|e| ProxyError::Internal(format!("failed to build response: {e}")))?
                        .into_response();
                    Ok(resp)
                }
            }
        } else {
            // ── Non-stream standard path ──
            match executor.execute(&auth, provider_request).await {
                Ok(response) => {
                    let latency_ms = start.elapsed().as_millis();
                    self.state.metrics.record_latency_ms(latency_ms);
                    self.state.router.record_success(&auth.id);
                    self.state
                        .router
                        .record_latency(&auth.id, latency_ms as f64);

                    // Extract thinking signatures from Claude responses
                    if target_format == Format::Claude
                        && let Some(ref tc) = self.state.thinking_cache
                    {
                        let tenant_id = req.tenant_id.as_deref().unwrap_or("");
                        tc.extract_from_response(tenant_id, &actual_model, &response.payload)
                            .await;
                    }

                    let translated = self.state.translators.translate_non_stream(
                        req.source_format,
                        target_format,
                        &actual_model,
                        &body,
                        &response.payload,
                    )?;

                    // Write to cache
                    self.try_cache_write(req, &auth, target_format, &actual_model, &translated)
                        .await;

                    record_attempt_success(
                        attempt_span,
                        attempt_start.elapsed().as_millis() as u64,
                    );

                    self.record_non_stream_success(
                        request_span,
                        &debug_provider,
                        &debug_model,
                        debug_credential.as_deref(),
                        &response.payload,
                        req,
                        start,
                    );

                    if detail_level >= LogDetailLevel::Standard {
                        request_span.record(
                            "response_body",
                            truncate_body(&translated, max_body_bytes).as_ref(),
                        );
                    }

                    let resp = build_json_response(
                        &translated,
                        &config.passthrough_headers,
                        &response.headers,
                    )?;
                    Ok(resp)
                }
                Err(e) => {
                    record_attempt_failure(
                        &attempt_span,
                        &e,
                        attempt_start.elapsed().as_millis() as u64,
                    );
                    drop(attempt_span);
                    self.handle_attempt_error(&auth.id, &e);
                    Err(e)
                }
            }
        }
    }

    fn handle_attempt_error(&self, auth_id: &str, error: &ProxyError) {
        self.state.metrics.record_error();
        match error {
            ProxyError::Upstream {
                status: 429,
                retry_after_secs,
                ..
            } => {
                self.state.router.record_failure(auth_id);
                let config = self.state.config.load();
                let cooldown_secs = retry_after_secs.unwrap_or(config.quota_cooldown_default_secs);
                self.state
                    .router
                    .set_quota_cooldown(auth_id, Duration::from_secs(cooldown_secs));
            }
            ProxyError::RateLimited {
                retry_after_secs, ..
            } => {
                self.state.router.record_failure(auth_id);
                self.state
                    .router
                    .set_quota_cooldown(auth_id, Duration::from_secs(*retry_after_secs));
            }
            ProxyError::Upstream {
                status: 500..=599, ..
            }
            | ProxyError::Network(_) => {
                self.state.router.record_failure(auth_id);
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_non_stream_success(
        &self,
        request_span: &tracing::Span,
        provider: &str,
        model: &str,
        credential_name: Option<&str>,
        upstream_payload: &[u8],
        req: &DispatchRequest,
        start: Instant,
    ) {
        let upstream_str = std::str::from_utf8(upstream_payload).unwrap_or("");
        let usage = extract_usage(upstream_str);
        let cost = match &usage {
            Some(u) => self.state.cost_calculator.calculate(model, u),
            _ => None,
        };

        if let Some(ref u) = usage {
            self.state
                .metrics
                .record_tokens(u.total_input(), u.output_tokens);
            self.state
                .rate_limiter
                .record_tokens(req.api_key.as_deref(), u.total_input() + u.output_tokens);
        }
        if let Some(c) = cost {
            self.state.metrics.record_cost(model, c);
            self.state
                .rate_limiter
                .record_cost(req.api_key.as_deref(), c);
        }

        request_span.record("provider", provider);
        request_span.record("model", model);
        request_span.record("credential_name", credential_name.unwrap_or(""));
        request_span.record("status", 200u64);
        request_span.record("latency_ms", start.elapsed().as_millis() as u64);
        record_usage_on_span(request_span, usage.as_ref(), cost);
    }

    async fn try_cache_write(
        &self,
        req: &DispatchRequest,
        auth: &prism_core::provider::AuthRecord,
        target_format: Format,
        actual_model: &str,
        translated: &str,
    ) {
        if let Some(ref cache) = self.state.response_cache
            && let Ok(body_val) = serde_json::from_slice::<serde_json::Value>(&req.body)
            && let Some(cache_key) = prism_core::cache::CacheKey::build_with_context(
                &req.model,
                &body_val,
                req.tenant_id.as_deref(),
                req.api_key_id.as_deref(),
                auth.name(),
            )
        {
            let cached = prism_core::cache::CachedResponse {
                payload: Bytes::from(translated.to_string()),
                provider: target_format.as_str().to_string(),
                model: actual_model.to_string(),
                input_tokens: 0,
                output_tokens: 0,
            };
            cache.insert(cache_key, cached).await;
        }
    }
}

type ModelProviderGroups<'a> = Vec<(String, Vec<(Format, Vec<&'a RouteAttemptPlan>)>)>;

/// Group attempts by model, then by provider within each model.
fn group_attempts_by_model<'a>(
    model_chain: &[String],
    attempts: &'a [RouteAttemptPlan],
) -> ModelProviderGroups<'a> {
    let mut result = Vec::new();

    for model in model_chain {
        let model_attempts: Vec<&RouteAttemptPlan> =
            attempts.iter().filter(|a| &a.model == model).collect();

        if model_attempts.is_empty() {
            continue;
        }

        // Group by provider (Format), preserving order
        let mut provider_groups: Vec<(Format, Vec<&RouteAttemptPlan>)> = Vec::new();
        for attempt in model_attempts {
            if let Some(group) = provider_groups
                .iter_mut()
                .find(|(f, _)| *f == attempt.provider)
            {
                group.1.push(attempt);
            } else {
                provider_groups.push((attempt.provider, vec![attempt]));
            }
        }

        result.push((model.clone(), provider_groups));
    }

    result
}
