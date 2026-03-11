use bytes::Bytes;
use prism_core::error::ProxyError;
use prism_core::provider::{Format, ProviderResponse, StreamChunk};
use prism_core::request_record::TokenUsage;
use prism_translator::TranslateState;
use std::sync::Arc;
use std::time::Duration;

use super::helpers::extract_usage;

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

/// Callback context for updating request logs after a stream completes.
pub(super) struct StreamDoneContext {
    pub request_id: String,
    pub model: Option<String>,
    pub request_logs: Arc<prism_core::request_log::RequestLogStore>,
    pub cost_calculator: Arc<prism_core::cost::CostCalculator>,
    pub metrics: Arc<prism_core::metrics::Metrics>,
}

/// Wrap an upstream `StreamChunk` stream to capture token usage from SSE events.
///
/// Each chunk's `data` is inspected for usage fields (supports OpenAI, Claude, and Gemini
/// response formats). When the stream is dropped (either after natural completion or due to
/// client disconnect), the captured usage is written back to the request log entry and
/// recorded in global metrics via the `Drop` implementation on the internal state.
pub(super) fn with_usage_capture(
    stream: std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>,
    >,
    ctx: StreamDoneContext,
) -> std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>> {
    struct State {
        inner: std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = Result<StreamChunk, ProxyError>> + Send>,
        >,
        usage: Option<TokenUsage>,
        ctx: Option<StreamDoneContext>,
    }

    impl Drop for State {
        fn drop(&mut self) {
            if let Some(ctx) = self.ctx.take()
                && let Some(ref usage) = self.usage
            {
                let cost = ctx
                    .model
                    .as_deref()
                    .and_then(|m| ctx.cost_calculator.calculate(m, usage));
                ctx.metrics
                    .record_tokens(usage.total_input(), usage.output_tokens);
                if let (Some(m), Some(c)) = (ctx.model.as_deref(), cost) {
                    ctx.metrics.record_cost(m, c);
                }
                ctx.request_logs
                    .update_usage(&ctx.request_id, usage.clone(), cost);
            }
        }
    }

    let state = State {
        inner: stream,
        usage: None,
        ctx: Some(ctx),
    };

    Box::pin(futures::stream::unfold(state, |mut state| async move {
        use tokio_stream::StreamExt;
        match state.inner.next().await {
            Some(result) => {
                if let Ok(ref chunk) = result
                    && let Some(u) = extract_usage(&chunk.data)
                {
                    match state.usage.as_mut() {
                        Some(existing) => existing.merge(&u),
                        None => state.usage = Some(u),
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
