use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use futures::stream::StreamExt;
use prism_core::error::ProxyError;
use std::convert::Infallible;
use std::time::Duration;

/// Build an SSE response from a stream of data strings.
///
/// Each string in the stream can be:
/// - Plain JSON data (will be wrapped in `data: ...\n\n`)
/// - `"[DONE]"` sentinel (emitted as `data: [DONE]\n\n`)
/// - Multi-line with `event:` prefix for Claude SSE (e.g. `"event: message_start\ndata: {...}"`)
/// - Empty string (skipped)
pub fn build_sse_response(
    data_stream: impl Stream<Item = Result<String, ProxyError>> + Send + 'static,
    keepalive_seconds: u64,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = data_stream
        .filter_map(|result| async move {
            match result {
                Ok(data) if data.is_empty() => None,
                Ok(data) => Some(Ok(data)),
                Err(e) => Some(Err(e)),
            }
        })
        .flat_map(|result| {
            let items: Vec<Result<Event, Infallible>> = match result {
                Ok(data) => {
                    // Split multi-line output into individual SSE events
                    // Each line might be a JSON chunk or "[DONE]" or "event: ...\ndata: ..."
                    let mut events = Vec::new();
                    for line in data.split('\n') {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if line == "[DONE]" {
                            events.push(Ok(Event::default().data("[DONE]")));
                        } else if let Some(rest) = line.strip_prefix("event: ") {
                            // This is an SSE event type line - create event with the type
                            // The next line should be the data
                            events.push(Ok(Event::default().event(rest)));
                        } else if let Some(rest) = line.strip_prefix("data: ") {
                            events.push(Ok(Event::default().data(rest)));
                        } else {
                            // Raw JSON data
                            events.push(Ok(Event::default().data(line)));
                        }
                    }
                    events
                }
                Err(e) => {
                    let error_json = serde_json::json!({"error": {"message": e.to_string()}});
                    vec![Ok(Event::default().data(error_json.to_string()))]
                }
            };
            futures::stream::iter(items)
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(keepalive_seconds))
            .text(""),
    )
}
