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
        ChangeStudioResponse, FactRow, InspectorRow, InspectorSection, RegistryRow, UiText,
        WorkspaceAction, WorkspaceActionEffect, WorkspaceInspector, WorkspaceQuery,
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
            label: UiText::new("changeStudio.fact.configVersion"),
            value: config_version.clone(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("changeStudio.fact.configPath"),
            value: state
                .config_path
                .lock()
                .map(|path| path.clone())
                .unwrap_or_else(|_| "unavailable".to_string()),
            value_text: None,
        },
        FactRow {
            label: UiText::new("common.providers"),
            value: config.providers.len().to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("changeStudio.fact.selectedWindow"),
            value: query.range.clone(),
            value_text: None,
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
            family_label: UiText::new("changeStudio.family.providers"),
            record: format!("{} providers", config.providers.len()),
            state: if config.providers.is_empty() {
                UiText::new("common.empty")
            } else {
                UiText::new("common.configured")
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
            family_label: UiText::new("changeStudio.family.authProfiles"),
            record: format!("{explicit_auth_profiles} explicit profiles"),
            state: if explicit_auth_profiles == 0 {
                UiText::new("changeStudio.state.implicitOnly")
            } else {
                UiText::new("common.configured")
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
            family_label: UiText::new("changeStudio.family.authKeys"),
            record: format!("{} auth keys", config.auth_keys.len()),
            state: if config.auth_keys.is_empty() {
                UiText::new("common.empty")
            } else {
                UiText::new("common.configured")
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
            family_label: UiText::new("changeStudio.family.routeProfiles"),
            record: format!("{} profiles", config.routing.profiles.len()),
            state: UiText::new("changeStudio.state.live"),
            state_tone: "success".to_string(),
            dependents: format!("{} rules", config.routing.rules.len()),
        },
        RegistryRow {
            family: "model-resolution".to_string(),
            family_label: UiText::new("changeStudio.family.modelResolution"),
            record: format!("{} transforms", total_model_resolution_steps(config)),
            state: if total_model_resolution_steps(config) == 0 {
                UiText::new("changeStudio.state.baseline")
            } else {
                UiText::new("changeStudio.state.customized")
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
        eyebrow: UiText::new("changeStudio.inspector.eyebrow"),
        title: UiText::with_values(
            "changeStudio.inspector.title",
            [("configVersion", config_version)],
        ),
        summary: UiText::new("changeStudio.inspector.summary"),
        sections: vec![
            InspectorSection {
                title: UiText::new("changeStudio.inspector.currentShape"),
                rows: vec![
                    InspectorRow {
                        label: UiText::new("common.providers"),
                        value: config.providers.len().to_string(),
                        value_text: None,
                    },
                    InspectorRow {
                        label: UiText::new("changeStudio.inspector.authKeys"),
                        value: config.auth_keys.len().to_string(),
                        value_text: None,
                    },
                    InspectorRow {
                        label: UiText::new("changeStudio.inspector.routeRules"),
                        value: config.routing.rules.len().to_string(),
                        value_text: None,
                    },
                ],
            },
            InspectorSection {
                title: UiText::new("changeStudio.inspector.transactionPath"),
                rows: vec![
                    InspectorRow {
                        label: UiText::new("changeStudio.inspector.validate"),
                        value: "available".to_string(),
                        value_text: Some(UiText::new("common.available")),
                    },
                    InspectorRow {
                        label: UiText::new("changeStudio.inspector.apply"),
                        value: "available".to_string(),
                        value_text: Some(UiText::new("common.available")),
                    },
                    InspectorRow {
                        label: UiText::new("changeStudio.inspector.reload"),
                        value: "available".to_string(),
                        value_text: Some(UiText::new("common.available")),
                    },
                ],
            },
        ],
        actions: vec![
            WorkspaceAction {
                id: "open-raw-yaml".to_string(),
                label: UiText::new("changeStudio.action.openRawYaml"),
                effect: WorkspaceActionEffect::Navigate,
                target_workspace: Some("change-studio".to_string()),
            },
            WorkspaceAction {
                id: "validate-current-config".to_string(),
                label: UiText::new("changeStudio.action.validateCurrentConfig"),
                effect: WorkspaceActionEffect::Navigate,
                target_workspace: Some("change-studio".to_string()),
            },
            WorkspaceAction {
                id: "reload-runtime".to_string(),
                label: UiText::new("changeStudio.action.reloadRuntime"),
                effect: WorkspaceActionEffect::Reload,
                target_workspace: None,
            },
        ],
    }
}
