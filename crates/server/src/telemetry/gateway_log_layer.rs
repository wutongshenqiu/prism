use super::span_data::RequestSpanData;
use super::visitors::{AttemptSpanVisitor, RequestSpanVisitor};
use prism_core::request_log::LogStore;
use prism_core::request_record::AttemptSummary;
use std::sync::Arc;
use tracing::Subscriber;
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

const REQUEST_SPAN_NAME: &str = "gateway.request";
const ATTEMPT_SPAN_NAME: &str = "gateway.attempt";

/// Custom tracing Layer that collects data from `gateway.request` and `gateway.attempt`
/// spans, assembles `RequestRecord`s, and writes them to the log store.
pub struct GatewayLogLayer {
    log_store: Arc<dyn LogStore>,
    /// Cached tokio runtime handle to avoid per-request TLS lookup.
    runtime_handle: Option<tokio::runtime::Handle>,
}

impl GatewayLogLayer {
    pub fn new(log_store: Arc<dyn LogStore>) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current().ok();
        Self {
            log_store,
            runtime_handle,
        }
    }
}

impl<S> Layer<S> for GatewayLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let name = attrs.metadata().name();
        if name == REQUEST_SPAN_NAME {
            let mut data = RequestSpanData::default();
            attrs.record(&mut RequestSpanVisitor::new(&mut data));
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(data);
            }
        } else if name == ATTEMPT_SPAN_NAME {
            let mut data = AttemptSummary::default();
            attrs.record(&mut AttemptSpanVisitor::new(&mut data));
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(data);
            }
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let name = span.metadata().name();
            // Only acquire write lock for our gateway spans
            if name == REQUEST_SPAN_NAME {
                let mut extensions = span.extensions_mut();
                if let Some(data) = extensions.get_mut::<RequestSpanData>() {
                    values.record(&mut RequestSpanVisitor::new(data));
                }
            } else if name == ATTEMPT_SPAN_NAME {
                let mut extensions = span.extensions_mut();
                if let Some(data) = extensions.get_mut::<AttemptSummary>() {
                    values.record(&mut AttemptSpanVisitor::new(data));
                }
            }
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        let name = span.metadata().name();

        if name == ATTEMPT_SPAN_NAME {
            // When an attempt span closes, push its data to the parent request span
            let attempt_data = span.extensions_mut().remove::<AttemptSummary>();
            if let Some(data) = attempt_data
                && let Some(parent) = span.parent()
                && parent.metadata().name() == REQUEST_SPAN_NAME
            {
                let mut parent_ext = parent.extensions_mut();
                if let Some(req_data) = parent_ext.get_mut::<RequestSpanData>() {
                    req_data.attempts.push(data);
                }
            }
        } else if name == REQUEST_SPAN_NAME {
            // When a request span closes, assemble the RequestRecord and write it
            let data = span.extensions_mut().remove::<RequestSpanData>();
            if let Some(data) = data {
                let record = data.into_request_record();
                let store = self.log_store.clone();
                // Use cached handle; fall back to try_current for robustness
                let handle = self
                    .runtime_handle
                    .as_ref()
                    .cloned()
                    .or_else(|| tokio::runtime::Handle::try_current().ok());
                if let Some(handle) = handle {
                    handle.spawn(async move {
                        store.push(record).await;
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::memory_log_store::InMemoryLogStore;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn test_gateway_log_layer_creation() {
        let logs: Arc<dyn LogStore> = Arc::new(InMemoryLogStore::new(100, None));
        let _layer = GatewayLogLayer::new(logs);
    }

    #[tokio::test]
    async fn test_request_span_writes_to_store() {
        let logs: Arc<dyn LogStore> = Arc::new(InMemoryLogStore::new(100, None));
        let layer = GatewayLogLayer::new(logs.clone());

        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        {
            let span = tracing::info_span!(
                "gateway.request",
                request_id = "test-req-1",
                method = "POST",
                path = "/v1/chat/completions",
                stream = false,
                requested_model = "gpt-4",
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
                api_key_id = tracing::field::Empty,
                tenant_id = tracing::field::Empty,
                client_ip = "1.2.3.4",
                client_region = tracing::field::Empty,
            );
            let _enter = span.enter();

            span.record("provider", "openai");
            span.record("model", "gpt-4");
            span.record("status", 200u64);
            span.record("latency_ms", 150u64);
            span.record("total_attempts", 1u64);
            span.record("usage_input", 100u64);
            span.record("usage_output", 50u64);
        }

        // Allow the async push to complete
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let page = logs
            .query(&prism_core::request_log::LogQuery::default())
            .await;
        assert_eq!(page.total, 1);
        let record = &page.data[0];
        assert_eq!(record.request_id, "test-req-1");
        assert_eq!(record.provider.as_deref(), Some("openai"));
        assert_eq!(record.model.as_deref(), Some("gpt-4"));
        assert_eq!(record.status, 200);
        assert_eq!(record.latency_ms, 150);
        assert_eq!(record.total_attempts, 1);
        assert_eq!(record.client_ip.as_deref(), Some("1.2.3.4"));
        let usage = record.usage.as_ref().unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
    }

    #[tokio::test]
    async fn test_attempt_spans_collected_into_request() {
        let logs: Arc<dyn LogStore> = Arc::new(InMemoryLogStore::new(100, None));
        let layer = GatewayLogLayer::new(logs.clone());

        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        {
            let request_span = tracing::info_span!(
                "gateway.request",
                request_id = "test-req-2",
                method = "POST",
                path = "/v1/chat/completions",
                stream = false,
                requested_model = "gpt-4",
                request_body = tracing::field::Empty,
                upstream_request_body = tracing::field::Empty,
                provider = tracing::field::Empty,
                model = tracing::field::Empty,
                credential_name = tracing::field::Empty,
                total_attempts = 2u64,
                status = 200u64,
                latency_ms = 300u64,
                response_body = tracing::field::Empty,
                stream_content_preview = tracing::field::Empty,
                usage_input = tracing::field::Empty,
                usage_output = tracing::field::Empty,
                usage_cache_read = tracing::field::Empty,
                usage_cache_creation = tracing::field::Empty,
                cost = tracing::field::Empty,
                error = tracing::field::Empty,
                error_type = tracing::field::Empty,
                api_key_id = tracing::field::Empty,
                tenant_id = tracing::field::Empty,
                client_ip = tracing::field::Empty,
                client_region = tracing::field::Empty,
            );
            let _request_enter = request_span.enter();

            // First attempt (fails)
            {
                let attempt_span = tracing::info_span!(
                    parent: &request_span,
                    "gateway.attempt",
                    attempt_index = 0u64,
                    provider = "openai",
                    model = "gpt-4",
                    credential_name = "key-1",
                    status = 429u64,
                    latency_ms = 50u64,
                    error = "rate limited",
                    error_type = "rate_limited",
                );
                let _attempt_enter = attempt_span.enter();
            }

            // Second attempt (succeeds)
            {
                let attempt_span = tracing::info_span!(
                    parent: &request_span,
                    "gateway.attempt",
                    attempt_index = 1u64,
                    provider = "openai",
                    model = "gpt-4",
                    credential_name = "key-2",
                    status = 200u64,
                    latency_ms = 250u64,
                    error = tracing::field::Empty,
                    error_type = tracing::field::Empty,
                );
                let _attempt_enter = attempt_span.enter();
            }

            request_span.record("provider", "openai");
            request_span.record("model", "gpt-4");
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let page = logs
            .query(&prism_core::request_log::LogQuery::default())
            .await;
        assert_eq!(page.total, 1);
        let record = &page.data[0];
        assert_eq!(record.request_id, "test-req-2");
        assert_eq!(record.attempts.len(), 2);
        assert_eq!(record.attempts[0].attempt_index, 0);
        assert_eq!(record.attempts[0].status, Some(429));
        assert_eq!(record.attempts[0].error.as_deref(), Some("rate limited"));
        assert_eq!(record.attempts[1].attempt_index, 1);
        assert_eq!(record.attempts[1].status, Some(200));
        assert!(record.attempts[1].error.is_none());
    }
}
