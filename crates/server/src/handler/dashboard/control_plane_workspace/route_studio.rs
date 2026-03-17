use std::collections::HashSet;

use axum::{
    Json,
    extract::{Query, State},
};
use prism_core::{
    config::Config,
    routing::{explain::explain, planner::RoutePlanner, types::RouteRequestFeatures},
};

use crate::AppState;

use super::{
    shared::{
        endpoint_from_path, fallback_route_requests, query_recent_logs, range_start_timestamp,
        route_endpoint_label, source_format_from_path, total_model_resolution_steps,
    },
    types::{
        FactRow, InspectorRow, InspectorSection, RouteScenarioRow, RouteStudioResponse,
        WorkspaceInspector, WorkspaceQuery,
    },
};

pub async fn route_studio(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<RouteStudioResponse> {
    let config = state.config.load();
    let scenarios = build_route_scenarios(&state, &config, &query.range).await;
    let routable = scenarios
        .iter()
        .filter(|scenario| scenario.decision != "Blocked")
        .count();
    let summary_facts = vec![
        FactRow {
            label: "Default profile".to_string(),
            value: config.routing.default_profile.clone(),
        },
        FactRow {
            label: "Profiles".to_string(),
            value: config.routing.profiles.len().to_string(),
        },
        FactRow {
            label: "Rules".to_string(),
            value: config.routing.rules.len().to_string(),
        },
        FactRow {
            label: "Model transforms".to_string(),
            value: total_model_resolution_steps(&config).to_string(),
        },
    ];
    let explain_facts = vec![
        FactRow {
            label: "Sampled scenarios".to_string(),
            value: scenarios.len().to_string(),
        },
        FactRow {
            label: "Routable".to_string(),
            value: routable.to_string(),
        },
        FactRow {
            label: "Blocked".to_string(),
            value: scenarios.len().saturating_sub(routable).to_string(),
        },
        FactRow {
            label: "Window".to_string(),
            value: query.range.clone(),
        },
    ];
    let inspector = route_inspector(&config, scenarios.first());

    Json(RouteStudioResponse {
        summary_facts,
        explain_facts,
        scenarios,
        inspector,
    })
}

async fn build_route_scenarios(
    state: &AppState,
    config: &Config,
    range: &str,
) -> Vec<RouteScenarioRow> {
    let from = range_start_timestamp(range);
    let recent = query_recent_logs(state, from, 8).await;
    let mut requests = recent
        .iter()
        .filter_map(|record| {
            let model = record
                .requested_model
                .clone()
                .or_else(|| record.model.clone())?;
            Some(RouteRequestFeatures {
                requested_model: model,
                endpoint: endpoint_from_path(&record.path),
                source_format: source_format_from_path(&record.path),
                tenant_id: record.tenant_id.clone(),
                api_key_id: record.api_key_id.clone(),
                region: record.client_region.clone(),
                stream: record.stream,
                headers: Default::default(),
                allowed_credentials: Vec::new(),
                required_capabilities: None,
            })
        })
        .collect::<Vec<_>>();

    if requests.is_empty() {
        requests = fallback_route_requests(config);
    }

    let inventory = state.catalog.snapshot();
    let health = state.health_manager.snapshot();
    let mut seen = HashSet::new();
    let mut rows = Vec::new();

    for request in requests {
        let key = format!(
            "{}:{}:{}",
            request
                .tenant_id
                .clone()
                .unwrap_or_else(|| "gateway".to_string()),
            request.requested_model,
            request.stream
        );
        if !seen.insert(key) {
            continue;
        }
        let explanation = explain(&RoutePlanner::plan(
            &request,
            &config.routing,
            &inventory,
            &health,
        ));
        let winner = explanation
            .selected
            .as_ref()
            .map(|selected| selected.provider.clone())
            .unwrap_or_else(|| "none".to_string());
        let blocked = explanation.selected.is_none();
        let decision = if blocked {
            "Blocked"
        } else if !explanation.rejections.is_empty() {
            "Fallback-ready"
        } else {
            "Routable"
        };
        let decision_tone = if blocked {
            "danger"
        } else if !explanation.rejections.is_empty() {
            "warning"
        } else {
            "success"
        };
        let delta = explanation
            .matched_rule
            .clone()
            .unwrap_or_else(|| format!("profile {}", explanation.profile));

        rows.push(RouteScenarioRow {
            scenario: format!(
                "{} / {}",
                request
                    .tenant_id
                    .clone()
                    .unwrap_or_else(|| "gateway".to_string()),
                request.requested_model
            ),
            winner,
            delta,
            decision: decision.to_string(),
            decision_tone: decision_tone.to_string(),
            endpoint: route_endpoint_label(&request.endpoint).to_string(),
            source_format: request.source_format.as_str().to_string(),
            stream: request.stream,
            model: request.requested_model.clone(),
            tenant_id: request.tenant_id.clone(),
            api_key_id: request.api_key_id.clone(),
            region: request.region.clone(),
        });

        if rows.len() >= 6 {
            break;
        }
    }

    rows
}

fn route_inspector(
    config: &Config,
    first_scenario: Option<&RouteScenarioRow>,
) -> WorkspaceInspector {
    let profile_names = config
        .routing
        .profiles
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    WorkspaceInspector {
        eyebrow: "ROUTE / CURRENT".to_string(),
        title: config.routing.default_profile.clone(),
        summary: format!("Profiles: {}", profile_names),
        sections: vec![
            InspectorSection {
                title: "Routing scope".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Default profile".to_string(),
                        value: config.routing.default_profile.clone(),
                    },
                    InspectorRow {
                        label: "Rules".to_string(),
                        value: config.routing.rules.len().to_string(),
                    },
                ],
            },
            InspectorSection {
                title: "Current sample".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Scenario".to_string(),
                        value: first_scenario
                            .map(|scenario| scenario.scenario.clone())
                            .unwrap_or_else(|| "none".to_string()),
                    },
                    InspectorRow {
                        label: "Decision".to_string(),
                        value: first_scenario
                            .map(|scenario| scenario.decision.clone())
                            .unwrap_or_else(|| "n/a".to_string()),
                    },
                ],
            },
        ],
        actions: vec![
            "Explain route".to_string(),
            "Open routing config".to_string(),
            "Patch profiles".to_string(),
        ],
    }
}
