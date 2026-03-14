use crate::AppState;
use crate::dispatch::{DispatchRequest, dispatch};
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Json, Response};
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::Format;

/// Parse a Gemini-style path segment like "gemini-2.0-flash:generateContent"
/// into (model, action).
fn parse_model_action(path: &str) -> Result<(&str, &str), ProxyError> {
    let (model, action) = path.rsplit_once(':').ok_or_else(|| {
        ProxyError::BadRequest(format!(
            "invalid Gemini path: expected '{{model}}:{{action}}', got '{path}'"
        ))
    })?;
    if model.is_empty() || action.is_empty() {
        return Err(ProxyError::BadRequest(format!(
            "invalid Gemini path: model and action must not be empty, got '{path}'"
        )));
    }
    Ok((model, action))
}

/// POST /v1beta/models/{model_action} — unified entry point for Gemini model actions.
///
/// Handles both `generateContent` and `streamGenerateContent` by parsing
/// the `{model}:{action}` path segment.
pub async fn gemini_model_action(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(model_action): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let (model, action) = parse_model_action(&model_action)?;

    match action {
        "generateContent" => dispatch_gemini(&state, &ctx, &headers, body, model, false).await,
        "streamGenerateContent" => dispatch_gemini(&state, &ctx, &headers, body, model, true).await,
        _ => Err(ProxyError::BadRequest(format!(
            "unsupported Gemini action: '{action}'"
        ))),
    }
}

async fn dispatch_gemini(
    state: &AppState,
    ctx: &RequestContext,
    headers: &HeaderMap,
    body: Bytes,
    model: &str,
    stream: bool,
) -> Result<Response, ProxyError> {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let debug = headers
        .get("x-debug")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "true" || v == "1");

    let allowed_credentials = ctx
        .auth_key
        .as_ref()
        .map(|e| e.allowed_credentials.clone())
        .unwrap_or_default();

    dispatch(
        state,
        DispatchRequest {
            source_format: Format::Gemini,
            model: model.to_string(),
            models: None,
            stream,
            body,
            allowed_formats: None,
            user_agent,
            debug,
            api_key: ctx.auth_key.as_ref().map(|e| e.key.clone()),
            client_region: ctx.client_region.clone(),
            request_id: Some(ctx.request_id.clone()),
            api_key_id: ctx.api_key_id.clone(),
            tenant_id: ctx.tenant_id.clone(),
            allowed_credentials,
            responses_passthrough: false,
        },
    )
    .await
}

/// GET /v1beta/models — list models in Gemini format
pub async fn list_models(State(state): State<AppState>) -> Result<impl IntoResponse, ProxyError> {
    let models = state.router.all_models();

    let gemini_models: Vec<serde_json::Value> = models
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "name": format!("models/{}", m.id),
                "displayName": m.id,
                "supportedGenerationMethods": ["generateContent", "streamGenerateContent"],
            })
        })
        .collect();

    let response = serde_json::json!({
        "models": gemini_models,
    });

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_action_generate_content() {
        let (model, action) = parse_model_action("gemini-2.0-flash:generateContent").unwrap();
        assert_eq!(model, "gemini-2.0-flash");
        assert_eq!(action, "generateContent");
    }

    #[test]
    fn test_parse_model_action_stream() {
        let (model, action) = parse_model_action("gemini-1.5-pro:streamGenerateContent").unwrap();
        assert_eq!(model, "gemini-1.5-pro");
        assert_eq!(action, "streamGenerateContent");
    }

    #[test]
    fn test_parse_model_action_no_colon() {
        let err = parse_model_action("gemini-2.0-flash").unwrap_err();
        assert!(err.to_string().contains("invalid Gemini path"));
    }

    #[test]
    fn test_parse_model_action_empty_model() {
        let err = parse_model_action(":generateContent").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_parse_model_action_empty_action() {
        let err = parse_model_action("gemini-2.0-flash:").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_parse_model_action_with_version() {
        let (model, action) =
            parse_model_action("gemini-2.5-flash-preview:generateContent").unwrap();
        assert_eq!(model, "gemini-2.5-flash-preview");
        assert_eq!(action, "generateContent");
    }
}
