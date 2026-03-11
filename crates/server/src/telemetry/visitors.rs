use super::span_data::RequestSpanData;
use prism_core::request_record::AttemptSummary;
use tracing::field::{Field, Visit};

/// Visitor for recording fields from a `gateway.request` span into `RequestSpanData`.
pub struct RequestSpanVisitor<'a> {
    pub data: &'a mut RequestSpanData,
}

impl<'a> RequestSpanVisitor<'a> {
    pub fn new(data: &'a mut RequestSpanData) -> Self {
        Self { data }
    }
}

impl Visit for RequestSpanVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "request_id" => self.data.request_id = value.to_string(),
            "method" => self.data.method = value.to_string(),
            "path" => self.data.path = value.to_string(),
            "requested_model" => self.data.requested_model = Some(value.to_string()),
            "request_body" => self.data.request_body = Some(value.to_string()),
            "upstream_request_body" => self.data.upstream_request_body = Some(value.to_string()),
            "provider" => self.data.provider = Some(value.to_string()),
            "model" => self.data.model = Some(value.to_string()),
            "credential_name" => self.data.credential_name = Some(value.to_string()),
            "response_body" => self.data.response_body = Some(value.to_string()),
            "stream_content_preview" => {
                self.data.stream_content_preview = Some(value.to_string());
            }
            "error" => self.data.error = Some(value.to_string()),
            "error_type" => self.data.error_type = Some(value.to_string()),
            "api_key_id" => self.data.api_key_id = Some(value.to_string()),
            "tenant_id" => self.data.tenant_id = Some(value.to_string()),
            "client_ip" => self.data.client_ip = Some(value.to_string()),
            "client_region" => self.data.client_region = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "status" => self.data.status = value as u16,
            "latency_ms" => self.data.latency_ms = value,
            "total_attempts" => self.data.total_attempts = value as u32,
            "usage_input" => self.data.usage_input = Some(value),
            "usage_output" => self.data.usage_output = Some(value),
            "usage_cache_read" => self.data.usage_cache_read = Some(value),
            "usage_cache_creation" => self.data.usage_cache_creation = Some(value),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        // tracing may pass integers as i64
        self.record_u64(field, value as u64);
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_u64(field, value as u64);
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_u64(field, value as u64);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() == "cost" {
            self.data.cost = Some(value);
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "stream" {
            self.data.stream = value;
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {
        // Ignored — we handle specific types above
    }
}

/// Visitor for recording fields from a `gateway.attempt` span into `AttemptSummary`.
pub struct AttemptSpanVisitor<'a> {
    pub data: &'a mut AttemptSummary,
}

impl<'a> AttemptSpanVisitor<'a> {
    pub fn new(data: &'a mut AttemptSummary) -> Self {
        Self { data }
    }
}

impl Visit for AttemptSpanVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "provider" => self.data.provider = value.to_string(),
            "model" => self.data.model = value.to_string(),
            "credential_name" => self.data.credential_name = Some(value.to_string()),
            "error" => self.data.error = Some(value.to_string()),
            "error_type" => self.data.error_type = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "attempt_index" => self.data.attempt_index = value as u32,
            "status" => self.data.status = Some(value as u16),
            "latency_ms" => self.data.latency_ms = value,
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_u64(field, value as u64);
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_u64(field, value as u64);
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_u64(field, value as u64);
    }

    fn record_bool(&mut self, _field: &Field, _value: bool) {}
    fn record_f64(&mut self, _field: &Field, _value: f64) {}

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}
