use bytes::Bytes;
use prism_core::error::ProxyError;
use prism_core::provider::{Format, ProviderResponse, StreamChunk};
use prism_core::request_record::{LogDetailLevel, TokenUsage, truncate_body};
use prism_translator::TranslateState;
use std::sync::Arc;
use std::time::Duration;

use super::helpers::extract_usage;

/// Maximum characters to capture for stream content preview.
const STREAM_PREVIEW_MAX_CHARS: usize = 500;

type ProviderResult = Result<ProviderResponse, ProxyError>;

/// Translate a stream of provider-specific chunks into the target format.
pub(super) fn translate_stream(
    upstream: std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>,
    >,
    translators: std::sync::Arc<prism_translator::TranslatorRegistry>,
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

/// Build a chunked response body that sends periodic whitespace while waiting
/// for the upstream response. Leading whitespace is valid JSON and is ignored
/// by parsers, so the client receives ` ` ` ` `{"choices":[...]}`.
pub(super) fn build_keepalive_body(
    result_rx: std::pin::Pin<Box<tokio::sync::oneshot::Receiver<ProviderResult>>>,
    interval_secs: u64,
    translators: std::sync::Arc<prism_translator::TranslatorRegistry>,
    source_format: Format,
    target_format: Format,
    model: String,
    original_body: Bytes,
) -> axum::body::Body {
    struct KeepaliveState {
        rx: Option<std::pin::Pin<Box<tokio::sync::oneshot::Receiver<ProviderResult>>>>,
        interval_secs: u64,
        translators: std::sync::Arc<prism_translator::TranslatorRegistry>,
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

pub(super) fn keepalive_error_json(msg: &str) -> String {
    serde_json::json!({
        "error": {"message": msg, "type": "server_error"}
    })
    .to_string()
}

/// Callback context for updating metrics after a stream completes.
pub(super) struct StreamDoneContext {
    pub model: Option<String>,
    pub cost_calculator: Arc<prism_core::cost::CostCalculator>,
    pub metrics: Arc<prism_core::metrics::Metrics>,
    pub rate_limiter: Arc<prism_core::rate_limit::CompositeRateLimiter>,
    pub api_key: Option<String>,
}

/// Wrap an upstream `StreamChunk` stream to capture token usage from SSE events.
///
/// Each chunk's `data` is inspected for usage fields (supports OpenAI, Claude, and Gemini
/// response formats). When the stream is dropped (either after natural completion or due to
/// client disconnect), the captured usage is recorded on the `request_span` and written
/// back to metrics. The span's delayed close triggers GatewayLogLayer::on_close.
pub(super) fn with_usage_capture(
    stream: std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>,
    >,
    ctx: StreamDoneContext,
    request_span: tracing::Span,
    detail_level: LogDetailLevel,
    max_body_bytes: usize,
) -> std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>> {
    struct State {
        inner: std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>,
        >,
        usage: Option<TokenUsage>,
        ctx: Option<StreamDoneContext>,
        request_span: tracing::Span,
        content_preview: String,
        /// Accumulated raw SSE data for full response body logging.
        /// `None` when detail_level < Full.
        response_body: Option<String>,
        max_body_bytes: usize,
    }

    impl Drop for State {
        fn drop(&mut self) {
            if let Some(ctx) = self.ctx.take() {
                if let Some(ref usage) = self.usage {
                    let cost = ctx
                        .model
                        .as_deref()
                        .and_then(|m| ctx.cost_calculator.calculate(m, usage));
                    ctx.metrics
                        .record_tokens(usage.total_input(), usage.output_tokens);
                    if let (Some(m), Some(c)) = (ctx.model.as_deref(), cost) {
                        ctx.metrics.record_cost(m, c);
                    }
                    // Record tokens and cost in rate limiter
                    let total_tokens = usage.total_input() + usage.output_tokens;
                    ctx.rate_limiter
                        .record_tokens(ctx.api_key.as_deref(), total_tokens);
                    if let Some(c) = cost {
                        ctx.rate_limiter.record_cost(ctx.api_key.as_deref(), c);
                    }
                    // Record usage on the request span (for GatewayLogLayer)
                    super::record_usage_on_span(&self.request_span, Some(usage), cost);
                }
                if !self.content_preview.is_empty() {
                    self.request_span
                        .record("stream_content_preview", self.content_preview.as_str());
                }
                // Record full response body for streaming when detail level is Full
                if let Some(ref body) = self.response_body
                    && !body.is_empty()
                {
                    self.request_span.record(
                        "response_body",
                        truncate_body(body, self.max_body_bytes).as_ref(),
                    );
                }
                // Span drops here → GatewayLogLayer::on_close fires
            }
        }
    }

    let capture_body = detail_level >= LogDetailLevel::Full;
    let state = State {
        inner: stream,
        usage: None,
        ctx: Some(ctx),
        request_span,
        content_preview: String::with_capacity(STREAM_PREVIEW_MAX_CHARS),
        response_body: if capture_body {
            Some(String::new())
        } else {
            None
        },
        max_body_bytes,
    };

    Box::pin(futures::stream::unfold(state, |mut state| async move {
        use tokio_stream::StreamExt;
        match state.inner.next().await {
            Some(result) => {
                if let Ok(ref chunk) = result {
                    if let Some(u) = extract_usage(&chunk.data) {
                        match state.usage.as_mut() {
                            Some(existing) => existing.merge(&u),
                            None => state.usage = Some(u),
                        }
                    }
                    // Capture content preview from SSE data (reuse parsed JSON if possible)
                    if state.content_preview.len() < STREAM_PREVIEW_MAX_CHARS
                        && let Some(text) = extract_content_text(&chunk.data)
                    {
                        let remaining = STREAM_PREVIEW_MAX_CHARS - state.content_preview.len();
                        let truncated = truncate_body(&text, remaining);
                        state.content_preview.push_str(&truncated);
                    }
                    // Accumulate raw SSE data for full response body logging
                    // Cap at max_body_bytes to avoid unbounded memory growth
                    if let Some(ref mut body) = state.response_body
                        && body.len() < state.max_body_bytes
                    {
                        if !body.is_empty() {
                            body.push('\n');
                        }
                        let remaining = state.max_body_bytes.saturating_sub(body.len());
                        if chunk.data.len() <= remaining {
                            body.push_str(&chunk.data);
                        } else {
                            let end = truncate_body(&chunk.data, remaining);
                            body.push_str(&end);
                        }
                    }
                }
                Some((result, state))
            }
            None => {
                // Stream ended naturally. State will be dropped here,
                // and Drop impl handles the cleanup.
                None
            }
        }
    }))
}

/// Extract content text from an SSE chunk data string.
/// Supports OpenAI (choices[0].delta.content) and Claude (delta.text) formats.
fn extract_content_text(data: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(data).ok()?;

    // OpenAI format: choices[0].delta.content
    if let Some(content) = val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
    {
        return Some(content.to_string());
    }

    // Claude format: delta.text
    if let Some(text) = val
        .get("delta")
        .and_then(|d| d.get("text"))
        .and_then(|t| t.as_str())
    {
        return Some(text.to_string());
    }

    None
}
