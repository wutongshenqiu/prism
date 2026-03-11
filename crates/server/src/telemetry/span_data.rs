use prism_core::request_record::{AttemptSummary, RequestRecord, TokenUsage};

/// Data collected from a `gateway.request` span during its lifetime.
#[derive(Debug, Default)]
pub struct RequestSpanData {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub stream: bool,
    pub requested_model: Option<String>,
    pub request_body: Option<String>,
    pub upstream_request_body: Option<String>,

    pub provider: Option<String>,
    pub model: Option<String>,
    pub credential_name: Option<String>,
    pub total_attempts: u32,

    pub status: u16,
    pub latency_ms: u64,
    pub response_body: Option<String>,
    pub stream_content_preview: Option<String>,

    pub usage_input: Option<u64>,
    pub usage_output: Option<u64>,
    pub usage_cache_read: Option<u64>,
    pub usage_cache_creation: Option<u64>,
    pub cost: Option<f64>,

    pub error: Option<String>,
    pub error_type: Option<String>,

    pub api_key_id: Option<String>,
    pub tenant_id: Option<String>,
    pub client_ip: Option<String>,
    pub client_region: Option<String>,

    pub attempts: Vec<AttemptSummary>,
}

impl RequestSpanData {
    pub fn into_request_record(self) -> RequestRecord {
        let usage = if self.usage_input.is_some() || self.usage_output.is_some() {
            Some(TokenUsage {
                input_tokens: self.usage_input.unwrap_or(0),
                output_tokens: self.usage_output.unwrap_or(0),
                cache_read_tokens: self.usage_cache_read.unwrap_or(0),
                cache_creation_tokens: self.usage_cache_creation.unwrap_or(0),
            })
        } else {
            None
        };

        RequestRecord {
            request_id: self.request_id,
            timestamp: chrono::Utc::now(),
            method: self.method,
            path: self.path,
            stream: self.stream,
            requested_model: self.requested_model,
            request_body: self.request_body,
            upstream_request_body: self.upstream_request_body,
            provider: self.provider,
            model: self.model,
            credential_name: self.credential_name,
            total_attempts: self.total_attempts,
            status: self.status,
            latency_ms: self.latency_ms,
            response_body: self.response_body,
            stream_content_preview: self.stream_content_preview,
            usage,
            cost: self.cost,
            error: self.error,
            error_type: self.error_type,
            api_key_id: self.api_key_id,
            tenant_id: self.tenant_id,
            client_ip: self.client_ip,
            client_region: self.client_region,
            attempts: self.attempts,
        }
    }
}
