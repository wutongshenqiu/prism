use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use tokio_stream::StreamExt;

/// Maximum SSE buffer size (16 MB). Prevents unbounded memory growth from
/// malicious or misbehaving upstream providers.
const MAX_SSE_BUFFER_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Parse a byte stream into SSE events.
/// Handles `event:` and `data:` prefixes, multi-line data, and `[DONE]` sentinel.
pub fn parse_sse_stream(
    byte_stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> Pin<Box<dyn Stream<Item = Result<SseEvent, prism_core::error::ProxyError>> + Send>> {
    let stream = async_stream(byte_stream);
    Box::pin(stream)
}

struct SseState {
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    buffer: String,
}

fn async_stream(
    byte_stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<SseEvent, prism_core::error::ProxyError>> + Send {
    futures::stream::unfold(
        SseState {
            stream: Box::pin(byte_stream),
            buffer: String::new(),
        },
        |mut state| async move {
            loop {
                // Check if we have a complete event block in the buffer (double newline)
                if let Some(pos) = find_event_boundary(&state.buffer) {
                    let block = state.buffer[..pos].to_string();
                    // Skip past the double newline
                    let skip = if state.buffer[pos..].starts_with("\r\n\r\n") {
                        4
                    } else {
                        2
                    };
                    drop(state.buffer.drain(..pos + skip));

                    if let Some(event) = parse_event_block(&block) {
                        return Some((Ok(event), state));
                    }
                    // Empty event block, continue looking
                    continue;
                }

                // Need more data
                match state.stream.next().await {
                    Some(Ok(bytes)) => match std::str::from_utf8(&bytes) {
                        Ok(text) => {
                            if state.buffer.len() + text.len() > MAX_SSE_BUFFER_SIZE {
                                return Some((
                                    Err(prism_core::error::ProxyError::Internal(
                                        "SSE buffer exceeded maximum size".to_string(),
                                    )),
                                    state,
                                ));
                            }
                            state.buffer.push_str(text);
                        }
                        Err(e) => {
                            return Some((
                                Err(prism_core::error::ProxyError::Internal(format!(
                                    "invalid UTF-8 in SSE stream: {e}"
                                ))),
                                state,
                            ));
                        }
                    },
                    Some(Err(e)) => {
                        return Some((
                            Err(prism_core::error::ProxyError::Network(e.to_string())),
                            state,
                        ));
                    }
                    None => {
                        // Stream ended. Process any remaining data in the buffer.
                        if !state.buffer.trim().is_empty() {
                            let block = std::mem::take(&mut state.buffer);
                            if let Some(event) = parse_event_block(&block) {
                                return Some((Ok(event), state));
                            }
                        }
                        return None;
                    }
                }
            }
        },
    )
}

/// Find the position of a double-newline event boundary.
fn find_event_boundary(s: &str) -> Option<usize> {
    if let Some(pos) = s.find("\n\n") {
        return Some(pos);
    }
    if let Some(pos) = s.find("\r\n\r\n") {
        return Some(pos);
    }
    None
}

/// Parse a single SSE event block into an SseEvent.
/// Returns None for empty/comment-only blocks and [DONE] sentinels.
fn parse_event_block(block: &str) -> Option<SseEvent> {
    let mut event_type: Option<String> = None;
    let mut data_lines: Vec<String> = Vec::new();

    for line in block.lines() {
        let line = line.trim_start_matches('\r');
        if line.starts_with(':') {
            // Comment line, skip
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event_type = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            let value = value.trim_start();
            data_lines.push(value.to_string());
        } else if line.starts_with("id:") || line.starts_with("retry:") {
            // Ignore id and retry fields
        }
    }

    if data_lines.is_empty() {
        return None;
    }

    let data = data_lines.join("\n");

    Some(SseEvent {
        event: event_type,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_event_block_basic() {
        let block = "data: {\"hello\": \"world\"}";
        let event = parse_event_block(block).unwrap();
        assert!(event.event.is_none());
        assert_eq!(event.data, "{\"hello\": \"world\"}");
    }

    #[test]
    fn test_parse_event_block_with_event_type() {
        let block = "event: message_start\ndata: {\"type\": \"message_start\"}";
        let event = parse_event_block(block).unwrap();
        assert_eq!(event.event.as_deref(), Some("message_start"));
        assert_eq!(event.data, "{\"type\": \"message_start\"}");
    }

    #[test]
    fn test_parse_event_block_done() {
        let block = "data: [DONE]";
        let event = parse_event_block(block).unwrap();
        assert_eq!(event.data, "[DONE]");
    }

    #[test]
    fn test_parse_event_block_multiline_data() {
        let block = "data: line1\ndata: line2";
        let event = parse_event_block(block).unwrap();
        assert_eq!(event.data, "line1\nline2");
    }

    #[test]
    fn test_parse_event_block_comment() {
        let block = ": this is a comment";
        assert!(parse_event_block(block).is_none());
    }

    #[test]
    fn test_parse_event_block_mixed_comment_and_data() {
        let block = ": comment\ndata: hello";
        let event = parse_event_block(block).unwrap();
        assert_eq!(event.data, "hello");
    }

    #[test]
    fn test_parse_event_block_id_and_retry_ignored() {
        let block = "id: 123\nretry: 5000\ndata: payload";
        let event = parse_event_block(block).unwrap();
        assert_eq!(event.data, "payload");
    }

    #[test]
    fn test_parse_event_block_empty_data() {
        let block = "event: ping";
        assert!(parse_event_block(block).is_none());
    }

    #[test]
    fn test_find_event_boundary_lf() {
        assert_eq!(find_event_boundary("data: x\n\ndata: y"), Some(7));
    }

    #[test]
    fn test_find_event_boundary_crlf() {
        assert_eq!(find_event_boundary("data: x\r\n\r\ndata: y"), Some(7));
    }

    #[test]
    fn test_find_event_boundary_none() {
        assert_eq!(find_event_boundary("data: x\ndata: y"), None);
    }

    #[test]
    fn test_parse_event_block_data_with_whitespace() {
        // data: field value should trim leading space
        let block = "data:  spaced";
        let event = parse_event_block(block).unwrap();
        assert_eq!(event.data, "spaced");
    }

    #[tokio::test]
    async fn test_parse_sse_stream_basic() {
        let raw = "data: hello\n\ndata: world\n\n";
        let stream = futures::stream::once(async move { Ok(Bytes::from(raw)) });
        let mut sse_stream = parse_sse_stream(stream);

        let event1 = StreamExt::next(&mut sse_stream).await.unwrap().unwrap();
        assert_eq!(event1.data, "hello");

        let event2 = StreamExt::next(&mut sse_stream).await.unwrap().unwrap();
        assert_eq!(event2.data, "world");

        assert!(StreamExt::next(&mut sse_stream).await.is_none());
    }

    #[tokio::test]
    async fn test_parse_sse_stream_chunked() {
        // Data arrives in multiple chunks
        let stream = futures::stream::iter(vec![
            Ok(Bytes::from("data: hel")),
            Ok(Bytes::from("lo\n\n")),
        ]);
        let mut sse_stream = parse_sse_stream(stream);

        let event = StreamExt::next(&mut sse_stream).await.unwrap().unwrap();
        assert_eq!(event.data, "hello");
    }

    #[tokio::test]
    async fn test_parse_sse_stream_with_event_type() {
        let raw = "event: message_start\ndata: {}\n\n";
        let stream = futures::stream::once(async move { Ok(Bytes::from(raw)) });
        let mut sse_stream = parse_sse_stream(stream);

        let event = StreamExt::next(&mut sse_stream).await.unwrap().unwrap();
        assert_eq!(event.event.as_deref(), Some("message_start"));
        assert_eq!(event.data, "{}");
    }

    #[tokio::test]
    async fn test_parse_sse_stream_done_sentinel() {
        let raw = "data: [DONE]\n\n";
        let stream = futures::stream::once(async move { Ok(Bytes::from(raw)) });
        let mut sse_stream = parse_sse_stream(stream);

        let event = StreamExt::next(&mut sse_stream).await.unwrap().unwrap();
        assert_eq!(event.data, "[DONE]");
    }

    #[tokio::test]
    async fn test_parse_sse_stream_remaining_data_on_eof() {
        // Data without trailing double newline (stream ends abruptly)
        let raw = "data: final";
        let stream = futures::stream::once(async move { Ok(Bytes::from(raw)) });
        let mut sse_stream = parse_sse_stream(stream);

        let event = StreamExt::next(&mut sse_stream).await.unwrap().unwrap();
        assert_eq!(event.data, "final");
    }
}
