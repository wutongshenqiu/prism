use std::collections::BTreeSet;

use axum::{
    Json,
    extract::{Query, State},
};
use prism_core::config::Config;

use crate::{AppState, handler::dashboard::config_tx};

use super::{
    shared::total_model_resolution_steps,
    types::{
        ChangeStudioResponse, FactRow, InspectorRow, InspectorSection, RegistryRow,
        WorkspaceInspector, WorkspaceQuery,
    },
};

pub async fn change_studio(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<ChangeStudioResponse> {
    let config = state.config.load();
    let config_version = config_tx::read_config_versioned(&state)
        .map(|(_, version)| version)
        .unwrap_or_else(|_| "unavailable".to_string());
    let registry = build_registry_rows(&config);
    let publish_facts = vec![
        FactRow {
            label: "Config version".to_string(),
            value: config_version.clone(),
        },
        FactRow {
            label: "Config path".to_string(),
            value: state
                .config_path
                .lock()
                .map(|path| path.clone())
                .unwrap_or_else(|_| "unavailable".to_string()),
        },
        FactRow {
            label: "Providers".to_string(),
            value: config.providers.len().to_string(),
        },
        FactRow {
            label: "Selected window".to_string(),
            value: query.range.clone(),
        },
    ];
    let inspector = change_inspector(&config, config_version);

    Json(ChangeStudioResponse {
        registry,
        publish_facts,
        inspector,
    })
}

fn build_registry_rows(config: &Config) -> Vec<RegistryRow> {
    let explicit_auth_profiles = config
        .providers
        .iter()
        .map(|provider| provider.auth_profiles.len())
        .sum::<usize>();
    let tenants = config
        .auth_keys
        .iter()
        .filter_map(|entry| entry.tenant_id.clone())
        .collect::<BTreeSet<_>>();

    vec![
        RegistryRow {
            family: "providers".to_string(),
            record: format!("{} providers", config.providers.len()),
            state: if config.providers.is_empty() {
                "empty".to_string()
            } else {
                "configured".to_string()
            },
            state_tone: if config.providers.is_empty() {
                "warning".to_string()
            } else {
                "success".to_string()
            },
            dependents: format!("{} route profiles", config.routing.profiles.len()),
        },
        RegistryRow {
            family: "auth-profiles".to_string(),
            record: format!("{explicit_auth_profiles} explicit profiles"),
            state: if explicit_auth_profiles == 0 {
                "implicit-only".to_string()
            } else {
                "configured".to_string()
            },
            state_tone: if explicit_auth_profiles == 0 {
                "info".to_string()
            } else {
                "success".to_string()
            },
            dependents: format!("{} providers", config.providers.len()),
        },
        RegistryRow {
            family: "auth-keys".to_string(),
            record: format!("{} auth keys", config.auth_keys.len()),
            state: if config.auth_keys.is_empty() {
                "empty".to_string()
            } else {
                "configured".to_string()
            },
            state_tone: if config.auth_keys.is_empty() {
                "warning".to_string()
            } else {
                "success".to_string()
            },
            dependents: format!("{} tenants", tenants.len()),
        },
        RegistryRow {
            family: "route-profiles".to_string(),
            record: format!("{} profiles", config.routing.profiles.len()),
            state: "live".to_string(),
            state_tone: "success".to_string(),
            dependents: format!("{} rules", config.routing.rules.len()),
        },
        RegistryRow {
            family: "model-resolution".to_string(),
            record: format!("{} transforms", total_model_resolution_steps(config)),
            state: if total_model_resolution_steps(config) == 0 {
                "baseline".to_string()
            } else {
                "customized".to_string()
            },
            state_tone: if total_model_resolution_steps(config) == 0 {
                "info".to_string()
            } else {
                "success".to_string()
            },
            dependents: config.routing.default_profile.clone(),
        },
    ]
}

fn change_inspector(config: &Config, config_version: String) -> WorkspaceInspector {
    WorkspaceInspector {
        eyebrow: "CHANGE / CONFIG".to_string(),
        title: config_version,
        summary: "Change Studio currently uses the config transaction path as the runtime truth until structured change objects land.".to_string(),
        sections: vec![
            InspectorSection {
                title: "Current shape".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Providers".to_string(),
                        value: config.providers.len().to_string(),
                    },
                    InspectorRow {
                        label: "Auth keys".to_string(),
                        value: config.auth_keys.len().to_string(),
                    },
                    InspectorRow {
                        label: "Route rules".to_string(),
                        value: config.routing.rules.len().to_string(),
                    },
                ],
            },
            InspectorSection {
                title: "Transaction path".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Validate".to_string(),
                        value: "available".to_string(),
                    },
                    InspectorRow {
                        label: "Apply".to_string(),
                        value: "available".to_string(),
                    },
                    InspectorRow {
                        label: "Reload".to_string(),
                        value: "available".to_string(),
                    },
                ],
            },
        ],
        actions: vec![
            "Open raw YAML".to_string(),
            "Validate current config".to_string(),
            "Reload runtime".to_string(),
        ],
    }
}
