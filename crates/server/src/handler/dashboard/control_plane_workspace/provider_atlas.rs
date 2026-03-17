use axum::{
    Json,
    extract::{Query, State},
};
use chrono::{DateTime, Utc};
use prism_core::{
    auth_profile::{AuthMode, AuthProfileEntry},
    config::{Config, ProviderKeyEntry},
};

use crate::{
    AppState,
    handler::dashboard::providers::{ProbeStatus, cached_probe_result},
};

use super::{
    shared::{title_case, wire_api_label},
    types::{
        FactRow, InspectorRow, InspectorSection, ProviderAtlasResponse, ProviderAtlasRow,
        WorkspaceInspector, WorkspaceQuery,
    },
};

pub async fn provider_atlas(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<ProviderAtlasResponse> {
    let config = state.config.load();
    let rows = config
        .providers
        .iter()
        .map(|provider| provider_row(&state, provider))
        .collect::<Vec<_>>();

    let healthy = rows.iter().filter(|row| row.status == "Healthy").count();
    let managed = config
        .providers
        .iter()
        .flat_map(|provider| provider.expanded_auth_profiles())
        .filter(|profile| profile.mode.is_managed())
        .count();
    let stream_ready = config
        .providers
        .iter()
        .filter(|provider| {
            cached_probe_result(&state, &provider.name)
                .map(|probe| probe.capability_status("stream") == ProbeStatus::Verified)
                .unwrap_or(false)
        })
        .count();
    let coverage = vec![
        FactRow {
            label: "Healthy providers".to_string(),
            value: healthy.to_string(),
        },
        FactRow {
            label: "Managed auth profiles".to_string(),
            value: managed.to_string(),
        },
        FactRow {
            label: "Verified stream surfaces".to_string(),
            value: stream_ready.to_string(),
        },
        FactRow {
            label: "Source mode".to_string(),
            value: query.source_mode.clone(),
        },
    ];
    let inspector = rows
        .first()
        .map(|row| provider_inspector(row, &config))
        .unwrap_or_else(default_provider_inspector);

    Json(ProviderAtlasResponse {
        providers: rows,
        coverage,
        inspector,
    })
}

fn provider_row(state: &AppState, provider: &ProviderKeyEntry) -> ProviderAtlasRow {
    let profiles = provider
        .expanded_auth_profiles()
        .into_iter()
        .map(|profile| {
            state
                .auth_runtime
                .apply_runtime_state(&provider.name, &profile)
                .unwrap_or(profile)
        })
        .collect::<Vec<_>>();
    let auth = profiles
        .first()
        .map(auth_mode_label)
        .unwrap_or_else(|| "Static api_key".to_string());
    let rotation = provider_rotation_summary(&profiles);
    let (status, tone, _) = provider_runtime_status(state, provider);

    ProviderAtlasRow {
        provider: provider.name.clone(),
        format: provider.format.as_str().to_string(),
        auth,
        status: title_case(status),
        status_tone: tone.to_string(),
        rotation,
        region: provider
            .region
            .clone()
            .unwrap_or_else(|| "global".to_string()),
        wire_api: wire_api_label(provider.wire_api).to_string(),
        model_count: provider.models.len(),
    }
}

fn auth_mode_label(profile: &AuthProfileEntry) -> String {
    match profile.mode {
        AuthMode::ApiKey => "API key".to_string(),
        AuthMode::BearerToken => "Bearer token".to_string(),
        AuthMode::CodexOAuth => "Codex OAuth".to_string(),
        AuthMode::AnthropicClaudeSubscription => "Claude subscription".to_string(),
    }
}

fn provider_rotation_summary(profiles: &[AuthProfileEntry]) -> String {
    if profiles.is_empty() {
        return "No auth profile".to_string();
    }

    for profile in profiles {
        if profile.mode.is_managed() {
            if profile
                .refresh_token
                .as_deref()
                .unwrap_or_default()
                .is_empty()
                && profile
                    .access_token
                    .as_deref()
                    .unwrap_or_default()
                    .is_empty()
            {
                return "Disconnected".to_string();
            }
            if let Some(expires_at) = &profile.expires_at
                && let Ok(expiry) = DateTime::parse_from_rfc3339(expires_at)
            {
                let remaining = expiry.with_timezone(&Utc) - Utc::now();
                if remaining.num_seconds() <= 0 {
                    return "Expired".to_string();
                }
                if remaining.num_days() < 1 {
                    return format!("Renews in {}h", remaining.num_hours());
                }
                return format!("Renews in {}d", remaining.num_days());
            }
            return "Managed".to_string();
        }
    }

    "Static".to_string()
}

pub(crate) fn provider_runtime_status(
    state: &AppState,
    provider: &ProviderKeyEntry,
) -> (&'static str, &'static str, String) {
    if provider.disabled {
        return (
            "disabled",
            "neutral",
            "Provider is disabled in config.".to_string(),
        );
    }

    let profiles = provider
        .expanded_auth_profiles()
        .into_iter()
        .map(|profile| {
            state
                .auth_runtime
                .apply_runtime_state(&provider.name, &profile)
                .unwrap_or(profile)
        })
        .collect::<Vec<_>>();

    let disconnected = profiles.iter().any(|profile| {
        profile.mode.is_managed()
            && profile
                .refresh_token
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            && profile
                .access_token
                .as_deref()
                .unwrap_or_default()
                .is_empty()
    });
    if disconnected {
        return (
            "degraded",
            "warning",
            "Managed auth is configured but not currently connected.".to_string(),
        );
    }

    if let Some(probe) = cached_probe_result(state, &provider.name) {
        match probe.status.as_str() {
            "failed" => {
                return (
                    "degraded",
                    "warning",
                    "Latest live capability probe failed.".to_string(),
                );
            }
            "verified" => {
                return (
                    "healthy",
                    "success",
                    "Latest live capability probe passed.".to_string(),
                );
            }
            _ => {
                return (
                    "watch",
                    "info",
                    "No successful live probe has been recorded yet.".to_string(),
                );
            }
        }
    }

    ("watch", "info", "No probe result recorded yet.".to_string())
}

fn provider_inspector(row: &ProviderAtlasRow, config: &Config) -> WorkspaceInspector {
    let linked_routes = config
        .routing
        .profiles
        .values()
        .filter(|profile| {
            profile
                .provider_policy
                .order
                .iter()
                .any(|entry| entry == &row.provider)
                || profile.provider_policy.weights.contains_key(&row.provider)
        })
        .count();

    WorkspaceInspector {
        eyebrow: "PROVIDER / PRIMARY".to_string(),
        title: row.provider.clone(),
        summary: format!("{} / {} / {}", row.auth, row.status, row.rotation),
        sections: vec![
            InspectorSection {
                title: "Identity".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Format".to_string(),
                        value: row.format.clone(),
                    },
                    InspectorRow {
                        label: "Region".to_string(),
                        value: row.region.clone(),
                    },
                    InspectorRow {
                        label: "Wire".to_string(),
                        value: row.wire_api.clone(),
                    },
                ],
            },
            InspectorSection {
                title: "Impact".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Models".to_string(),
                        value: row.model_count.to_string(),
                    },
                    InspectorRow {
                        label: "Linked route profiles".to_string(),
                        value: linked_routes.to_string(),
                    },
                ],
            },
        ],
        actions: vec![
            "Open provider config".to_string(),
            "Run live health check".to_string(),
            "Inspect auth profile".to_string(),
        ],
    }
}

fn default_provider_inspector() -> WorkspaceInspector {
    WorkspaceInspector {
        eyebrow: "PROVIDER / EMPTY".to_string(),
        title: "No providers configured".to_string(),
        summary: "Add at least one provider before using runtime control-plane workflows."
            .to_string(),
        sections: vec![],
        actions: vec!["Create provider".to_string()],
    }
}
