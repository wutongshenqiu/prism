use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize)]
struct ProviderSummary {
    id: String,
    provider_type: String,
    name: Option<String>,
    api_key_masked: String,
    base_url: Option<String>,
    models_count: usize,
    disabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub provider_type: String,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub models: Vec<ai_proxy_core::config::ModelMapping>,
    #[serde(default)]
    pub excluded_models: Vec<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<Option<String>>,
    #[serde(default)]
    pub name: Option<Option<String>>,
    #[serde(default)]
    pub prefix: Option<Option<String>>,
    #[serde(default)]
    pub models: Option<Vec<ai_proxy_core::config::ModelMapping>>,
    #[serde(default)]
    pub excluded_models: Option<Vec<String>>,
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub disabled: Option<bool>,
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}****{}", &key[..4], &key[key.len() - 4..])
}

fn provider_type_to_field(pt: &str) -> Option<&'static str> {
    match pt {
        "claude" => Some("claude-api-key"),
        "openai" => Some("openai-api-key"),
        "gemini" => Some("gemini-api-key"),
        "openai-compat" => Some("openai-compatibility"),
        _ => None,
    }
}

fn get_entries_by_type(
    config: &ai_proxy_core::config::Config,
    provider_type: &str,
) -> Vec<ai_proxy_core::config::ProviderKeyEntry> {
    match provider_type {
        "claude" => config.claude_api_key.clone(),
        "openai" => config.openai_api_key.clone(),
        "gemini" => config.gemini_api_key.clone(),
        "openai-compat" => config.openai_compatibility.clone(),
        _ => vec![],
    }
}

/// GET /api/dashboard/providers
pub async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let mut providers = Vec::new();

    let types = [
        ("claude", &config.claude_api_key),
        ("openai", &config.openai_api_key),
        ("gemini", &config.gemini_api_key),
        ("openai-compat", &config.openai_compatibility),
    ];

    for (ptype, entries) in &types {
        for (i, entry) in entries.iter().enumerate() {
            providers.push(ProviderSummary {
                id: format!("{}-{}", ptype, i),
                provider_type: ptype.to_string(),
                name: entry.name.clone(),
                api_key_masked: mask_key(&entry.api_key),
                base_url: entry.base_url.clone(),
                models_count: entry.models.len(),
                disabled: entry.disabled,
            });
        }
    }

    (StatusCode::OK, Json(json!({ "providers": providers })))
}

/// GET /api/dashboard/providers/:id
pub async fn get_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let config = state.config.load();
    let (ptype, idx) = match parse_provider_id(&id) {
        Some(v) => v,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Provider not found"})),
            );
        }
    };

    let entries = get_entries_by_type(&config, ptype);
    match entries.get(idx) {
        Some(entry) => {
            let detail = json!({
                "id": id,
                "provider_type": ptype,
                "name": entry.name,
                "api_key_masked": mask_key(&entry.api_key),
                "base_url": entry.base_url,
                "prefix": entry.prefix,
                "models": entry.models,
                "excluded_models": entry.excluded_models,
                "headers": entry.headers,
                "disabled": entry.disabled,
            });
            (StatusCode::OK, Json(detail))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Provider not found"})),
        ),
    }
}

/// POST /api/dashboard/providers
pub async fn create_provider(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    if provider_type_to_field(&body.provider_type).is_none() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"error": "validation_failed", "message": "Invalid provider_type. Must be one of: claude, openai, gemini, openai-compat"}),
            ),
        );
    }
    if body.api_key.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "validation_failed", "message": "api_key is required"})),
        );
    }

    let new_entry = ai_proxy_core::config::ProviderKeyEntry {
        api_key: body.api_key,
        base_url: body.base_url,
        proxy_url: None,
        prefix: body.prefix,
        models: body.models,
        excluded_models: body.excluded_models,
        headers: body.headers,
        disabled: body.disabled,
        name: body.name,
        cloak: Default::default(),
        wire_api: Default::default(),
        weight: 1,
    };

    match update_config_file(&state, |config| match body.provider_type.as_str() {
        "claude" => config.claude_api_key.push(new_entry.clone()),
        "openai" => config.openai_api_key.push(new_entry.clone()),
        "gemini" => config.gemini_api_key.push(new_entry.clone()),
        "openai-compat" => config.openai_compatibility.push(new_entry.clone()),
        _ => {}
    })
    .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({"message": "Provider created successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": e})),
        ),
    }
}

/// PATCH /api/dashboard/providers/:id
pub async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProviderRequest>,
) -> impl IntoResponse {
    let (ptype, idx) = match parse_provider_id(&id) {
        Some(v) => v,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Provider not found"})),
            );
        }
    };

    let ptype = ptype.to_string();
    match update_config_file(&state, move |config| {
        let entries = match ptype.as_str() {
            "claude" => &mut config.claude_api_key,
            "openai" => &mut config.openai_api_key,
            "gemini" => &mut config.gemini_api_key,
            "openai-compat" => &mut config.openai_compatibility,
            _ => return,
        };
        if let Some(entry) = entries.get_mut(idx) {
            if let Some(ref key) = body.api_key {
                entry.api_key = key.clone();
            }
            if let Some(ref url) = body.base_url {
                entry.base_url = url.clone();
            }
            if let Some(ref name) = body.name {
                entry.name = name.clone();
            }
            if let Some(ref prefix) = body.prefix {
                entry.prefix = prefix.clone();
            }
            if let Some(ref models) = body.models {
                entry.models = models.clone();
            }
            if let Some(ref excluded) = body.excluded_models {
                entry.excluded_models = excluded.clone();
            }
            if let Some(ref headers) = body.headers {
                entry.headers = headers.clone();
            }
            if let Some(disabled) = body.disabled {
                entry.disabled = disabled;
            }
        }
    })
    .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"message": "Provider updated successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": e})),
        ),
    }
}

/// DELETE /api/dashboard/providers/:id
pub async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (ptype, idx) = match parse_provider_id(&id) {
        Some(v) => v,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Provider not found"})),
            );
        }
    };

    let ptype = ptype.to_string();
    match update_config_file(&state, move |config| {
        let entries = match ptype.as_str() {
            "claude" => &mut config.claude_api_key,
            "openai" => &mut config.openai_api_key,
            "gemini" => &mut config.gemini_api_key,
            "openai-compat" => &mut config.openai_compatibility,
            _ => return,
        };
        if idx < entries.len() {
            entries.remove(idx);
        }
    })
    .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"message": "Provider deleted successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": e})),
        ),
    }
}

fn parse_provider_id(id: &str) -> Option<(&str, usize)> {
    let (ptype, idx_str) = id.rsplit_once('-')?;
    let idx = idx_str.parse::<usize>().ok()?;
    // Validate provider type
    if !["claude", "openai", "gemini", "openai-compat"].contains(&ptype) {
        return None;
    }
    Some((ptype, idx))
}

/// Read current config from file, apply mutation, write back atomically.
/// Public wrapper for use by sibling modules.
pub async fn update_config_file_public(
    state: &AppState,
    mutate: impl FnOnce(&mut ai_proxy_core::config::Config),
) -> Result<(), String> {
    update_config_file(state, mutate).await
}

async fn update_config_file(
    state: &AppState,
    mutate: impl FnOnce(&mut ai_proxy_core::config::Config),
) -> Result<(), String> {
    let config_path = state
        .config_path
        .lock()
        .map_err(|e| format!("Failed to lock config path: {e}"))?
        .clone();

    let contents =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Failed to read config: {e}"))?;
    let mut config: ai_proxy_core::config::Config =
        serde_yml::from_str(&contents).map_err(|e| format!("Failed to parse config: {e}"))?;

    mutate(&mut config);

    // Rebuild derived fields
    config.api_keys_set = config.api_keys.iter().cloned().collect();

    let yaml =
        serde_yml::to_string(&config).map_err(|e| format!("Failed to serialize config: {e}"))?;

    // Atomic write: write to temp file then rename
    let dir = std::path::Path::new(&config_path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let tmp_path = dir.join(".config.yaml.tmp");
    std::fs::write(&tmp_path, &yaml).map_err(|e| format!("Failed to write temp file: {e}"))?;
    std::fs::rename(&tmp_path, &config_path)
        .map_err(|e| format!("Failed to rename config file: {e}"))?;

    // Reload in-memory config so changes take effect immediately
    state.config.store(std::sync::Arc::new(config));

    Ok(())
}
