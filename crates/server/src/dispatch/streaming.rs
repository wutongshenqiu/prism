use bytes::Bytes;
use prism_core::error::ProxyError;
use prism_core::provider::{Format, ProviderResponse, StreamChunk};
use prism_translator::TranslateState;
use std::time::Duration;

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
