use crate::dispatch::DispatchRequest;
use prism_core::provider::Format;
use prism_core::routing::types::{RouteEndpoint, RouteRequestFeatures};
use std::collections::BTreeMap;

/// Extract `RouteRequestFeatures` from a `DispatchRequest` for the route planner.
pub(super) fn extract_features(req: &DispatchRequest) -> RouteRequestFeatures {
    let endpoint = match req.source_format {
        Format::Claude => RouteEndpoint::Messages,
        Format::OpenAI => RouteEndpoint::ChatCompletions,
        Format::Gemini => RouteEndpoint::ChatCompletions,
    };

    RouteRequestFeatures {
        requested_model: req.model.clone(),
        endpoint,
        source_format: req.source_format,
        tenant_id: req.tenant_id.clone(),
        api_key_id: req.api_key_id.clone(),
        region: req.client_region.clone(),
        stream: req.stream,
        headers: BTreeMap::new(),
        required_capabilities: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    fn test_req(format: Format, model: &str) -> DispatchRequest {
        DispatchRequest {
            source_format: format,
            model: model.to_string(),
            models: None,
            stream: false,
            body: Bytes::new(),
            allowed_formats: None,
            user_agent: None,
            debug: false,
            api_key: None,
            client_region: None,
            request_id: None,
            api_key_id: None,
            tenant_id: None,
            allowed_credentials: Vec::new(),
        }
    }

    #[test]
    fn test_extract_features_openai() {
        let req = test_req(Format::OpenAI, "gpt-4");
        let f = extract_features(&req);
        assert_eq!(f.requested_model, "gpt-4");
        assert_eq!(f.endpoint, RouteEndpoint::ChatCompletions);
        assert_eq!(f.source_format, Format::OpenAI);
        assert!(f.tenant_id.is_none());
        assert!(!f.stream);
    }

    #[test]
    fn test_extract_features_claude() {
        let req = test_req(Format::Claude, "claude-3-opus");
        let f = extract_features(&req);
        assert_eq!(f.endpoint, RouteEndpoint::Messages);
    }

    #[test]
    fn test_extract_features_with_optional_fields() {
        let mut req = test_req(Format::OpenAI, "gpt-4");
        req.tenant_id = Some("acme".to_string());
        req.api_key_id = Some("sk-proxy-123".to_string());
        req.client_region = Some("us-east-1".to_string());
        req.stream = true;

        let f = extract_features(&req);
        assert_eq!(f.tenant_id.as_deref(), Some("acme"));
        assert_eq!(f.api_key_id.as_deref(), Some("sk-proxy-123"));
        assert_eq!(f.region.as_deref(), Some("us-east-1"));
        assert!(f.stream);
    }

    #[test]
    fn test_extract_features_missing_optional_fields() {
        let req = test_req(Format::Gemini, "gemini-pro");
        let f = extract_features(&req);
        assert!(f.tenant_id.is_none());
        assert!(f.api_key_id.is_none());
        assert!(f.region.is_none());
        assert!(f.headers.is_empty());
    }
}
