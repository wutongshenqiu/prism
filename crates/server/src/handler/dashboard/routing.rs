use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prism_core::routing::config::{
    CredentialPolicy, ModelResolution, ProviderPolicy, ProviderStrategy, RouteProfile, RouteRule,
};
use prism_core::routing::explain::explain;
use prism_core::routing::planner::RoutePlanner;
use prism_core::routing::types::RouteRequestFeatures;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UpdateRoutingRequest {
    pub default_profile: Option<String>,
    pub profiles: Option<HashMap<String, RouteProfile>>,
    pub rules: Option<Vec<RouteRule>>,
    pub model_resolution: Option<ModelResolution>,
}

/// GET /api/dashboard/routing
pub async fn get_routing(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    (StatusCode::OK, Json(json!(&config.routing)))
}

/// PATCH /api/dashboard/routing
pub async fn update_routing(
    State(state): State<AppState>,
    Json(body): Json<UpdateRoutingRequest>,
) -> impl IntoResponse {
    // Validate before applying — pass current config for cross-field consistency
    let current_routing = state.config.load().routing.clone();
    if let Err(errors) = validate_routing_update(&body, &current_routing) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "validation_failed", "details": errors})),
        );
    }

    match super::providers::update_config_file_public(&state, move |config| {
        if let Some(dp) = body.default_profile {
            config.routing.default_profile = dp;
        }
        if let Some(p) = body.profiles {
            config.routing.profiles = p;
        }
        if let Some(r) = body.rules {
            config.routing.rules = r;
        }
        if let Some(mr) = body.model_resolution {
            config.routing.model_resolution = mr;
        }
    })
    .await
    {
        Ok(new_version) => {
            tracing::info!("Routing configuration updated via dashboard");
            (
                StatusCode::OK,
                Json(
                    json!({"message": "Routing configuration updated successfully", "config_version": new_version}),
                ),
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update routing configuration");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "write_failed", "message": e})),
            )
        }
    }
}

/// POST /api/dashboard/routing/preview — lightweight introspection (no scoring detail)
pub async fn preview_route(
    State(state): State<AppState>,
    Json(req): Json<RouteIntrospectionRequest>,
) -> impl IntoResponse {
    let features = req.into_features();
    let config = state.config.load();
    let inventory = state.catalog.snapshot();
    let health = state.health_manager.snapshot();

    let plan = RoutePlanner::plan(&features, &config.routing, &inventory, &health);
    let mut explanation = explain(&plan);
    // Preview omits detailed scoring
    explanation.scoring.clear();

    (StatusCode::OK, Json(json!(explanation)))
}

/// POST /api/dashboard/routing/explain — full introspection with scoring detail
pub async fn explain_route(
    State(state): State<AppState>,
    Json(req): Json<RouteIntrospectionRequest>,
) -> impl IntoResponse {
    let features = req.into_features();
    let config = state.config.load();
    let inventory = state.catalog.snapshot();
    let health = state.health_manager.snapshot();

    let plan = RoutePlanner::plan(&features, &config.routing, &inventory, &health);
    let explanation = explain(&plan);

    (StatusCode::OK, Json(json!(explanation)))
}

/// Canonical route introspection request shared by preview and explain endpoints.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RouteIntrospectionRequest {
    pub model: String,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_source_format")]
    pub source_format: String,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,
    pub region: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
}

fn default_endpoint() -> String {
    "chat-completions".into()
}

fn default_source_format() -> String {
    "openai".to_string()
}

impl RouteIntrospectionRequest {
    pub fn into_features(self) -> RouteRequestFeatures {
        use prism_core::provider::Format;
        use prism_core::routing::types::RouteEndpoint;

        let endpoint = match self.endpoint.as_str() {
            "messages" => RouteEndpoint::Messages,
            "responses" => RouteEndpoint::Responses,
            "generate-content" | "generate_content" => RouteEndpoint::GenerateContent,
            "stream-generate-content" => RouteEndpoint::StreamGenerateContent,
            "models" => RouteEndpoint::Models,
            _ => RouteEndpoint::ChatCompletions,
        };

        let source_format = match self.source_format.as_str() {
            "claude" => Format::Claude,
            "gemini" => Format::Gemini,
            _ => Format::OpenAI,
        };

        RouteRequestFeatures {
            requested_model: self.model,
            endpoint,
            source_format,
            tenant_id: self.tenant_id,
            api_key_id: self.api_key_id,
            region: self.region,
            stream: self.stream,
            headers: self.headers,
            required_capabilities: None,
        }
    }
}

/// Validate a routing update request against the current config.
/// Returns Ok(()) if valid, Err(details) if invalid.
fn validate_routing_update(
    body: &UpdateRoutingRequest,
    current: &prism_core::routing::config::RoutingConfig,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Validate profiles
    if let Some(ref profiles) = body.profiles {
        if profiles.is_empty() {
            errors.push("profiles map must not be empty".to_string());
        }

        for (name, profile) in profiles {
            validate_profile(name, profile, &mut errors);
        }
    }

    // The effective profiles after this update: request profiles override current
    let effective_profiles = body.profiles.as_ref().unwrap_or(&current.profiles);

    // Validate rules reference profiles that will exist after update
    if let Some(rules) = &body.rules {
        for rule in rules {
            if !effective_profiles.contains_key(&rule.use_profile) {
                errors.push(format!(
                    "rule '{}' references non-existent profile '{}'",
                    rule.name, rule.use_profile
                ));
            }
        }
    }

    // Validate default_profile exists in effective profiles
    let effective_dp = body
        .default_profile
        .as_deref()
        .unwrap_or(&current.default_profile);
    if !effective_profiles.contains_key(effective_dp) {
        errors.push(format!(
            "default-profile '{}' does not exist in profiles",
            effective_dp
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_profile(name: &str, profile: &RouteProfile, errors: &mut Vec<String>) {
    validate_provider_policy(name, &profile.provider_policy, errors);
    validate_credential_policy(name, &profile.credential_policy, errors);
}

fn validate_provider_policy(profile_name: &str, policy: &ProviderPolicy, errors: &mut Vec<String>) {
    match policy.strategy {
        ProviderStrategy::OrderedFallback => {
            if policy.order.is_empty() {
                errors.push(format!(
                    "profile '{}': ordered-fallback strategy requires non-empty 'order' list",
                    profile_name
                ));
            }
        }
        ProviderStrategy::WeightedRoundRobin => {
            if policy.weights.is_empty() {
                errors.push(format!(
                    "profile '{}': weighted-round-robin strategy requires non-empty 'weights' map",
                    profile_name
                ));
            }
        }
        _ => {}
    }
}

fn validate_credential_policy(
    _profile_name: &str,
    _policy: &CredentialPolicy,
    _errors: &mut Vec<String>,
) {
    // No additional validation needed for credential policies currently
}
