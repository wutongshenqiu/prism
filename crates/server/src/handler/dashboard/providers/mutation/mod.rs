mod entry;
mod request;

use super::{
    config_tx_error_response, is_valid_format, normalize_auth_profiles, parse_upstream_kind,
    seed_runtime_oauth_states, strip_runtime_oauth_data, validate_auth_shape,
    validate_provider_auth_profiles, validation_error,
};
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use self::entry::{apply_provider_update, create_provider_entry, prepare_provider_update};
pub use self::request::{CreateProviderRequest, UpdateProviderRequest};

/// POST /api/dashboard/providers
pub async fn create_provider(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "validation_failed", "message": "name is required"})),
        );
    }
    if !is_valid_format(&body.format) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"error": "validation_failed", "message": "Invalid format. Must be one of: openai, claude, gemini"}),
            ),
        );
    }
    let format: prism_core::provider::Format = body
        .format
        .parse()
        .unwrap_or(prism_core::provider::Format::OpenAI);
    let upstream = match parse_upstream_kind(format, body.upstream.as_deref()) {
        Ok(value) => value,
        Err(response) => return response,
    };

    let auth_profiles = match normalize_auth_profiles(&body.auth_profiles) {
        Ok(profiles) => profiles,
        Err(response) => return response,
    };
    if let Err(response) = validate_auth_shape(body.api_key.as_deref(), &auth_profiles) {
        return response;
    }
    if let Err(response) =
        validate_provider_auth_profiles(format, upstream, body.base_url.as_deref(), &auth_profiles)
    {
        return response;
    }

    {
        let config = state.config.load();
        if config.providers.iter().any(|entry| entry.name == body.name) {
            return (
                StatusCode::CONFLICT,
                Json(
                    json!({"error": "duplicate_name", "message": format!("Provider name '{}' already exists", body.name)}),
                ),
            );
        }
    }

    let provider_name = body.name.clone();
    let (auth_profiles, runtime_oauth_states) = strip_runtime_oauth_data(auth_profiles);
    let new_entry = create_provider_entry(&body, format, upstream, auth_profiles);

    if let Err(message) = new_entry.validate_shape() {
        return validation_error(message);
    }

    match update_config_file(&state, |config| {
        config.providers.push(new_entry.clone());
    })
    .await
    {
        Ok(()) => {
            if let Err(err) =
                seed_runtime_oauth_states(&state, &provider_name, &runtime_oauth_states)
            {
                tracing::error!(
                    name = %provider_name,
                    error = %err,
                    "Provider created but runtime oauth seeding failed"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "runtime_auth_seed_failed", "message": err})),
                );
            }
            tracing::info!(
                name = %provider_name,
                format = %body.format,
                "Provider created via dashboard"
            );
            (
                StatusCode::CREATED,
                Json(json!({"message": "Provider created successfully"})),
            )
        }
        Err(error) => {
            tracing::error!(
                name = %provider_name,
                error = ?error,
                "Failed to create provider"
            );
            config_tx_error_response(error)
        }
    }
}

/// PATCH /api/dashboard/providers/:name
pub async fn update_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<UpdateProviderRequest>,
) -> impl IntoResponse {
    let existing_entry = {
        let config = state.config.load();
        match config.providers.iter().find(|entry| entry.name == name) {
            Some(entry) => entry.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "not_found", "message": "Provider not found"})),
                );
            }
        }
    };

    let auth_profiles = match body
        .auth_profiles
        .as_ref()
        .map(|profiles| normalize_auth_profiles(profiles))
        .transpose()
    {
        Ok(profiles) => profiles,
        Err(response) => return response,
    };
    if let Some(ref profiles) = auth_profiles
        && let Err(response) = validate_auth_shape(body.api_key.as_deref(), profiles)
    {
        return response;
    }
    let upstream = match body.upstream.as_ref() {
        Some(upstream) => match parse_upstream_kind(existing_entry.format, upstream.as_deref()) {
            Ok(value) => value,
            Err(response) => return response,
        },
        None => existing_entry.upstream_kind(),
    };

    let prepared = prepare_provider_update(
        &existing_entry,
        &body,
        upstream,
        auth_profiles,
        strip_runtime_oauth_data,
    );
    if let Err(response) = validate_auth_shape(
        Some(prepared.candidate_entry.api_key.as_str()),
        &prepared.candidate_entry.auth_profiles,
    ) {
        return response;
    }
    if let Err(response) = validate_provider_auth_profiles(
        prepared.candidate_entry.format,
        prepared.candidate_entry.upstream_kind(),
        prepared.candidate_entry.base_url.as_deref(),
        &prepared.candidate_entry.auth_profiles,
    ) {
        return response;
    }
    if let Err(message) = prepared.candidate_entry.validate_shape() {
        return validation_error(message);
    }

    let name_for_log = name.clone();
    let body_for_write = body.clone();
    let auth_profiles_for_write = prepared.auth_profiles_for_write.clone();

    match update_config_file(&state, move |config| {
        if let Some(entry) = config.providers.iter_mut().find(|entry| entry.name == name) {
            apply_provider_update(entry, &body_for_write, auth_profiles_for_write.as_ref());
        }
    })
    .await
    {
        Ok(()) => {
            if let Err(err) =
                seed_runtime_oauth_states(&state, &name_for_log, &prepared.runtime_oauth_states)
            {
                tracing::error!(
                    provider = %name_for_log,
                    error = %err,
                    "Provider updated but runtime oauth seeding failed"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "runtime_auth_seed_failed", "message": err})),
                );
            }
            tracing::info!(provider = %name_for_log, "Provider updated via dashboard");
            (
                StatusCode::OK,
                Json(json!({"message": "Provider updated successfully"})),
            )
        }
        Err(error) => {
            tracing::error!(
                provider = %name_for_log,
                error = ?error,
                "Failed to update provider"
            );
            config_tx_error_response(error)
        }
    }
}

/// DELETE /api/dashboard/providers/:name
pub async fn delete_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    {
        let config = state.config.load();
        if !config.providers.iter().any(|entry| entry.name == name) {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Provider not found"})),
            );
        }
    }

    let name_for_log = name.clone();
    match update_config_file(&state, move |config| {
        config.providers.retain(|entry| entry.name != name);
    })
    .await
    {
        Ok(()) => {
            tracing::info!(provider = %name_for_log, "Provider deleted via dashboard");
            (
                StatusCode::OK,
                Json(json!({"message": "Provider deleted successfully"})),
            )
        }
        Err(error) => {
            tracing::error!(
                provider = %name_for_log,
                error = ?error,
                "Failed to delete provider"
            );
            config_tx_error_response(error)
        }
    }
}

async fn update_config_file(
    state: &AppState,
    mutate: impl FnOnce(&mut prism_core::config::Config),
) -> Result<(), super::super::config_tx::ConfigTxError> {
    super::super::config_tx::update_config_versioned(state, None, mutate)
        .await
        .map(|_| ())
}
