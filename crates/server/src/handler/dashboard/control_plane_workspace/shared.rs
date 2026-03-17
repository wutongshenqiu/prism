use chrono::{Duration, Utc};
use prism_core::{
    config::Config,
    provider::{Format, WireApi},
    request_log::{LogQuery, SortField, SortOrder},
    request_record::RequestRecord,
    routing::types::{RouteEndpoint, RouteRequestFeatures},
};

use crate::AppState;

pub(crate) fn range_start_timestamp(range: &str) -> i64 {
    let now = Utc::now();
    let from = match range {
        "15m" => now - Duration::minutes(15),
        "6h" => now - Duration::hours(6),
        "24h" => now - Duration::hours(24),
        _ => now - Duration::hours(1),
    };
    from.timestamp_millis()
}

pub(crate) async fn query_recent_logs(
    state: &AppState,
    from: i64,
    limit: usize,
) -> Vec<RequestRecord> {
    state
        .log_store
        .query(&LogQuery {
            page: Some(1),
            page_size: Some(limit),
            from: Some(from),
            sort_by: Some(SortField::Timestamp),
            sort_order: Some(SortOrder::Desc),
            ..Default::default()
        })
        .await
        .data
}

pub(crate) fn percentage(numerator: usize, denominator: usize) -> String {
    if denominator == 0 {
        return "0%".to_string();
    }
    format!("{:.1}%", (numerator as f64 / denominator as f64) * 100.0)
}

pub(crate) fn latest_freshness(records: &[RequestRecord]) -> Option<i64> {
    records
        .first()
        .map(|record| (Utc::now() - record.timestamp).num_seconds().max(0))
}

pub(crate) fn endpoint_from_path(path: &str) -> RouteEndpoint {
    match path {
        "/v1/messages" => RouteEndpoint::Messages,
        "/v1/responses" | "/v1/responses/ws" => RouteEndpoint::Responses,
        value if value.contains(":generateContent") => RouteEndpoint::GenerateContent,
        value if value.contains(":streamGenerateContent") => RouteEndpoint::StreamGenerateContent,
        _ => RouteEndpoint::ChatCompletions,
    }
}

pub(crate) fn source_format_from_path(path: &str) -> Format {
    match endpoint_from_path(path) {
        RouteEndpoint::Messages => Format::Claude,
        RouteEndpoint::GenerateContent | RouteEndpoint::StreamGenerateContent => Format::Gemini,
        _ => Format::OpenAI,
    }
}

pub(crate) fn route_endpoint_label(endpoint: &RouteEndpoint) -> &'static str {
    match endpoint {
        RouteEndpoint::ChatCompletions => "chat-completions",
        RouteEndpoint::Messages => "messages",
        RouteEndpoint::Responses => "responses",
        RouteEndpoint::GenerateContent => "generate-content",
        RouteEndpoint::StreamGenerateContent => "stream-generate-content",
        RouteEndpoint::Models => "models",
    }
}

pub(crate) fn fallback_route_requests(config: &Config) -> Vec<RouteRequestFeatures> {
    config
        .providers
        .iter()
        .flat_map(|provider| {
            provider
                .models
                .iter()
                .take(2)
                .map(|model| RouteRequestFeatures {
                    requested_model: model.alias.clone().unwrap_or_else(|| model.id.clone()),
                    endpoint: RouteEndpoint::ChatCompletions,
                    source_format: Format::OpenAI,
                    tenant_id: None,
                    api_key_id: None,
                    region: provider.region.clone(),
                    stream: false,
                    headers: Default::default(),
                    allowed_credentials: Vec::new(),
                    required_capabilities: None,
                })
        })
        .collect()
}

pub(crate) fn wire_api_label(wire_api: WireApi) -> &'static str {
    match wire_api {
        WireApi::Chat => "chat",
        WireApi::Responses => "responses",
    }
}

pub(crate) fn total_model_resolution_steps(config: &Config) -> usize {
    config.routing.model_resolution.aliases.len()
        + config.routing.model_resolution.rewrites.len()
        + config.routing.model_resolution.fallbacks.len()
        + config.routing.model_resolution.provider_pins.len()
}

pub(crate) fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}
