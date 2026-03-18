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
        FactRow, InspectorRow, InspectorSection, ProviderAtlasResponse, ProviderAtlasRow, UiText,
        WorkspaceAction, WorkspaceActionEffect, WorkspaceInspector, WorkspaceQuery, raw_text,
    },
};

pub async fn provider_atlas(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<ProviderAtlasResponse> {
    let config = state.config.load();
    let healthy = config
        .providers
        .iter()
        .filter(|provider| provider_runtime_status(&state, provider).0 == "healthy")
        .count();
    let rows = config
        .providers
        .iter()
        .map(|provider| provider_row(&state, provider))
        .collect::<Vec<_>>();
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
            label: UiText::new("providerAtlas.coverage.healthyProviders"),
            value: healthy.to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("providerAtlas.coverage.managedAuthProfiles"),
            value: managed.to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("providerAtlas.coverage.verifiedStreamSurfaces"),
            value: stream_ready.to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("providerAtlas.coverage.sourceMode"),
            value: query.source_mode.clone(),
            value_text: Some(UiText::new(format!("common.{}", query.source_mode))),
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
        .unwrap_or_else(|| UiText::new("providerAtlas.auth.staticApiKey"));
    let rotation = provider_rotation_summary(&profiles);
    let (status, tone, _) = provider_runtime_status(state, provider);

    ProviderAtlasRow {
        provider: provider.name.clone(),
        format: provider.format.as_str().to_string(),
        auth,
        status: provider_status_label(status),
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

fn provider_status_label(status: &str) -> UiText {
    match status {
        "healthy" => UiText::new("common.healthy"),
        "disabled" => UiText::new("common.disabled"),
        "degraded" => UiText::new("common.degraded"),
        "watch" => UiText::new("common.watch"),
        other => raw_text(title_case(other)),
    }
}

fn auth_mode_label(profile: &AuthProfileEntry) -> UiText {
    match profile.mode {
        AuthMode::ApiKey => UiText::new("providerAtlas.auth.apiKey"),
        AuthMode::BearerToken => UiText::new("providerAtlas.auth.bearerToken"),
        AuthMode::CodexOAuth => UiText::new("providerAtlas.auth.codexOauth"),
        AuthMode::AnthropicClaudeSubscription => {
            UiText::new("providerAtlas.auth.claudeSubscription")
        }
    }
}

fn provider_rotation_summary(profiles: &[AuthProfileEntry]) -> UiText {
    if profiles.is_empty() {
        return UiText::new("providerAtlas.rotation.none");
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
                return UiText::new("providerAtlas.rotation.disconnected");
            }
            if let Some(expires_at) = &profile.expires_at
                && let Ok(expiry) = DateTime::parse_from_rfc3339(expires_at)
            {
                let remaining = expiry.with_timezone(&Utc) - Utc::now();
                if remaining.num_seconds() <= 0 {
                    return UiText::new("providerAtlas.rotation.expired");
                }
                if remaining.num_days() < 1 {
                    return UiText::with_values(
                        "providerAtlas.rotation.renewsHours",
                        [("hours", remaining.num_hours())],
                    );
                }
                return UiText::with_values(
                    "providerAtlas.rotation.renewsDays",
                    [("days", remaining.num_days())],
                );
            }
            return UiText::new("providerAtlas.rotation.managed");
        }
    }

    UiText::new("providerAtlas.rotation.static")
}

pub(crate) fn provider_runtime_status(
    state: &AppState,
    provider: &ProviderKeyEntry,
) -> (&'static str, &'static str, UiText) {
    if provider.disabled {
        return (
            "disabled",
            "neutral",
            UiText::new("providerAtlas.runtime.disabled"),
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
            UiText::new("providerAtlas.runtime.disconnected"),
        );
    }

    if let Some(probe) = cached_probe_result(state, &provider.name) {
        match probe.status.as_str() {
            "error" | "failed" => {
                return (
                    "degraded",
                    "warning",
                    UiText::new("providerAtlas.runtime.probeFailed"),
                );
            }
            "ok" | "verified" => {
                return (
                    "healthy",
                    "success",
                    UiText::new("providerAtlas.runtime.probePassed"),
                );
            }
            "warning" => {
                return (
                    "watch",
                    "info",
                    UiText::new("providerAtlas.runtime.probePending"),
                );
            }
            _ => {
                return (
                    "watch",
                    "info",
                    UiText::new("providerAtlas.runtime.probePending"),
                );
            }
        }
    }

    (
        "watch",
        "info",
        UiText::new("providerAtlas.runtime.noProbe"),
    )
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
        eyebrow: UiText::new("providerAtlas.inspector.primary.eyebrow"),
        title: raw_text(row.provider.clone()),
        summary: UiText::new("providerAtlas.inspector.primary.summary"),
        sections: vec![
            InspectorSection {
                title: UiText::new("providerAtlas.inspector.identity"),
                rows: vec![
                    InspectorRow {
                        label: UiText::new("common.format"),
                        value: row.format.clone(),
                        value_text: None,
                    },
                    InspectorRow {
                        label: UiText::new("common.region"),
                        value: row.region.clone(),
                        value_text: None,
                    },
                    InspectorRow {
                        label: UiText::new("providerAtlas.inspector.wire"),
                        value: row.wire_api.clone(),
                        value_text: None,
                    },
                ],
            },
            InspectorSection {
                title: UiText::new("providerAtlas.inspector.impact"),
                rows: vec![
                    InspectorRow {
                        label: UiText::new("common.models"),
                        value: row.model_count.to_string(),
                        value_text: None,
                    },
                    InspectorRow {
                        label: UiText::new("providerAtlas.inspector.linkedRouteProfiles"),
                        value: linked_routes.to_string(),
                        value_text: None,
                    },
                ],
            },
        ],
        actions: vec![
            WorkspaceAction {
                id: "open-provider-config".to_string(),
                label: UiText::new("providerAtlas.action.openProviderConfig"),
                effect: WorkspaceActionEffect::Navigate,
                target_workspace: Some("provider-atlas".to_string()),
            },
            WorkspaceAction {
                id: "run-live-health-check".to_string(),
                label: UiText::new("providerAtlas.action.runHealthCheck"),
                effect: WorkspaceActionEffect::Navigate,
                target_workspace: Some("provider-atlas".to_string()),
            },
            WorkspaceAction {
                id: "inspect-auth-profile".to_string(),
                label: UiText::new("providerAtlas.action.inspectAuthProfile"),
                effect: WorkspaceActionEffect::Navigate,
                target_workspace: Some("provider-atlas".to_string()),
            },
        ],
    }
}

fn default_provider_inspector() -> WorkspaceInspector {
    WorkspaceInspector {
        eyebrow: UiText::new("providerAtlas.inspector.empty.eyebrow"),
        title: UiText::new("providerAtlas.inspector.empty.title"),
        summary: UiText::new("providerAtlas.inspector.empty.summary"),
        sections: vec![],
        actions: vec![WorkspaceAction {
            id: "create-provider".to_string(),
            label: UiText::new("providerAtlas.action.createProvider"),
            effect: WorkspaceActionEffect::Navigate,
            target_workspace: Some("provider-atlas".to_string()),
        }],
    }
}
