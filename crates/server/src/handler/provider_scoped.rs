use crate::AppState;
use crate::dispatch::{DispatchRequest, dispatch};
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::Format;

/// Resolve a provider path segment to a list of credential name patterns.
///
/// Resolution order:
/// 1. If the provider string matches a provider name in the credential map,
///    collect all credential names for that provider.
/// 2. Otherwise, treat it as a credential name pattern (exact match).
pub(crate) fn resolve_provider(
    state: &AppState,
    provider: &str,
) -> Result<Vec<String>, ProxyError> {
    let cred_map = state.router.credential_map();

    // Try matching as a provider name first
    if let Some(entries) = cred_map.get(provider) {
        let names: Vec<String> = entries
            .iter()
            .filter_map(|a| a.credential_name.clone())
            .collect();
        if names.is_empty() {
            return Err(ProxyError::BadRequest(format!(
                "no credentials found for provider '{provider}'"
            )));
        }
        return Ok(names);
    }

    // Otherwise treat it as a credential name
    let exists = cred_map.values().any(|entries| {
        entries
            .iter()
            .any(|a| a.credential_name.as_deref() == Some(provider))
    });
    if !exists {
        return Err(ProxyError::BadRequest(format!(
            "unknown provider '{provider}'"
        )));
    }
    Ok(vec![provider.to_string()])
}

/// Determine the source format from the API path suffix.
fn source_format_for_path(path_suffix: &str) -> Format {
    if path_suffix == "messages" {
        Format::Claude
    } else {
        // chat/completions and responses both use OpenAI format
        Format::OpenAI
    }
}

/// Determine allowed formats from the API path suffix.
fn allowed_formats_for_path(path_suffix: &str) -> Option<Vec<Format>> {
    if path_suffix == "responses" {
        // Responses API only works with OpenAI-format providers
        Some(vec![Format::OpenAI])
    } else {
        None
    }
}

pub(crate) fn matches_scoped_credential(candidate: &str, requested: &str) -> bool {
    candidate == requested || candidate.rsplit('/').next() == Some(requested)
}

/// POST /api/provider/{provider}/v1/chat/completions
pub async fn provider_chat_completions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    provider_dispatch(&state, &ctx, &headers, body, &provider, "chat/completions").await
}

/// POST /api/provider/{provider}/v1/messages
pub async fn provider_messages(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    provider_dispatch(&state, &ctx, &headers, body, &provider, "messages").await
}

/// POST /api/provider/{provider}/v1/responses
pub async fn provider_responses(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    provider_dispatch(&state, &ctx, &headers, body, &provider, "responses").await
}

async fn provider_dispatch(
    state: &AppState,
    ctx: &RequestContext,
    headers: &HeaderMap,
    body: Bytes,
    provider: &str,
    path_suffix: &str,
) -> Result<Response, ProxyError> {
    let provider_credentials = resolve_provider(state, provider)?;

    let parsed = super::parse_request(headers, &body)?;

    // Merge provider-scoped credentials with any auth-key-level restrictions
    let mut allowed_credentials = provider_credentials;
    if let Some(ref auth_key) = ctx.auth_key
        && !auth_key.allowed_credentials.is_empty()
    {
        // Intersect: keep only credentials allowed by both the provider scope
        // and the auth key restrictions
        let auth_key_creds = &auth_key.allowed_credentials;
        allowed_credentials.retain(|name| {
            auth_key_creds
                .iter()
                .any(|pattern| prism_core::glob::glob_match(pattern, name))
        });
        if allowed_credentials.is_empty() {
            return Err(ProxyError::BadRequest(format!(
                "no accessible credentials for provider '{provider}' with current API key"
            )));
        }
    }

    if let Some(ref requested) = parsed.auth_profile {
        allowed_credentials.retain(|candidate| matches_scoped_credential(candidate, requested));
        if allowed_credentials.is_empty() {
            return Err(ProxyError::BadRequest(format!(
                "unknown auth profile '{requested}' for provider '{provider}'"
            )));
        }
    }

    let source_format = source_format_for_path(path_suffix);
    let allowed_formats = allowed_formats_for_path(path_suffix);
    let responses_passthrough = path_suffix == "responses";

    dispatch(
        state,
        DispatchRequest {
            source_format,
            model: parsed.model,
            models: parsed.models,
            stream: parsed.stream,
            body,
            allowed_formats,
            user_agent: parsed.user_agent,
            debug: parsed.debug,
            api_key: ctx.auth_key.as_ref().map(|e| e.key.clone()),
            client_region: ctx.client_region.clone(),
            request_id: Some(ctx.request_id.clone()),
            api_key_id: ctx.api_key_id.clone(),
            tenant_id: ctx.tenant_id.clone(),
            allowed_credentials,
            responses_passthrough,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_format_for_path() {
        assert_eq!(source_format_for_path("chat/completions"), Format::OpenAI);
        assert_eq!(source_format_for_path("messages"), Format::Claude);
        assert_eq!(source_format_for_path("responses"), Format::OpenAI);
    }

    #[test]
    fn test_allowed_formats_for_path() {
        assert!(allowed_formats_for_path("chat/completions").is_none());
        assert!(allowed_formats_for_path("messages").is_none());
        assert_eq!(
            allowed_formats_for_path("responses"),
            Some(vec![Format::OpenAI])
        );
    }
}
