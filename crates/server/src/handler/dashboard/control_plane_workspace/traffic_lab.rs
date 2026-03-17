use axum::{
    Json,
    extract::{Query, State},
};
use prism_core::{
    request_log::StatsQuery,
    request_record::{AttemptSummary, RequestRecord},
    routing::{
        explain::explain,
        planner::RoutePlanner,
        types::{RouteExplanation, RouteRequestFeatures},
    },
};

use crate::AppState;

use super::{
    shared::{
        endpoint_from_path, query_recent_logs, range_start_timestamp, source_format_from_path,
    },
    types::{
        FactRow, InspectorRow, InspectorSection, TimelineStep, TrafficLabResponse,
        TrafficSessionItem, WorkspaceInspector, WorkspaceQuery,
    },
};

pub async fn traffic_lab(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<TrafficLabResponse> {
    let from = range_start_timestamp(&query.range);
    let stats = state
        .log_store
        .stats(&StatsQuery {
            from: Some(from),
            ..Default::default()
        })
        .await;
    let recent = query_recent_logs(&state, from, query.limit.max(1)).await;
    let selected = recent.first().cloned();
    let sessions = recent.iter().map(traffic_session_item).collect::<Vec<_>>();
    let trace = selected
        .as_ref()
        .map(|record| build_traffic_trace(&state, record))
        .unwrap_or_else(|| {
            vec![TimelineStep {
                label: "No traffic".to_string(),
                tone: "neutral".to_string(),
                title: "No request sessions available".to_string(),
                detail: "The selected time range has no request log entries yet.".to_string(),
            }]
        });
    let compare_facts = vec![
        FactRow {
            label: "Window".to_string(),
            value: query.range.clone(),
        },
        FactRow {
            label: "Entries".to_string(),
            value: stats.total_entries.to_string(),
        },
        FactRow {
            label: "Errors".to_string(),
            value: stats.error_count.to_string(),
        },
        FactRow {
            label: "Avg latency".to_string(),
            value: format!("{} ms", stats.avg_latency_ms),
        },
    ];
    let inspector = traffic_inspector(selected.as_ref(), &query);

    Json(TrafficLabResponse {
        selected_request_id: selected.as_ref().map(|record| record.request_id.clone()),
        sessions,
        compare_facts,
        trace,
        inspector,
    })
}

fn traffic_session_item(record: &RequestRecord) -> TrafficSessionItem {
    let decision = if record.total_attempts > 1 {
        format!("Fallback after {} attempts", record.total_attempts)
    } else if let Some(provider) = &record.provider {
        format!("Primary {} served request", provider)
    } else {
        "Provider not resolved".to_string()
    };
    let (result, result_tone) = request_result(record);

    TrafficSessionItem {
        request_id: record.request_id.clone(),
        model: record
            .requested_model
            .clone()
            .or_else(|| record.model.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        decision,
        result,
        result_tone: result_tone.to_string(),
        latency_ms: record.latency_ms,
    }
}

fn request_result(record: &RequestRecord) -> (String, &'static str) {
    if record.status >= 500 || record.error.is_some() {
        return ("Failed".to_string(), "danger");
    }
    if record.total_attempts > 1 {
        return ("Recovered".to_string(), "warning");
    }
    ("Success".to_string(), "success")
}

fn build_traffic_trace(state: &AppState, record: &RequestRecord) -> Vec<TimelineStep> {
    let mut steps = Vec::new();
    steps.push(TimelineStep {
        label: "Ingress".to_string(),
        tone: "info".to_string(),
        title: format!(
            "{} {}",
            record.method,
            record
                .requested_model
                .clone()
                .or_else(|| record.model.clone())
                .unwrap_or_else(|| "unknown-model".to_string())
        ),
        detail: format!(
            "{} request entered {}{}",
            record.path,
            record
                .tenant_id
                .as_deref()
                .map(|tenant| format!("tenant {}", tenant))
                .unwrap_or_else(|| "gateway scope".to_string()),
            if record.stream { " as stream" } else { "" }
        ),
    });

    if let Some(explanation) = explain_record(state, record)
        && let Some(selected) = explanation.selected
    {
        let rejection_count = explanation.rejections.len();
        steps.push(TimelineStep {
            label: "Route explain".to_string(),
            tone: if rejection_count > 0 {
                "warning"
            } else {
                "success"
            }
            .to_string(),
            title: format!("{} selected {}", explanation.profile, selected.provider),
            detail: if rejection_count > 0 {
                format!(
                    "{} rejections observed before final route selection.",
                    rejection_count
                )
            } else {
                "Planner selected a provider without rejections.".to_string()
            },
        });
    }

    if record.attempts.is_empty() {
        steps.push(TimelineStep {
            label: "Execution".to_string(),
            tone: request_result(record).1.to_string(),
            title: record
                .provider
                .clone()
                .unwrap_or_else(|| "No upstream attempt captured".to_string()),
            detail: format!(
                "Finished with HTTP {} in {} ms.",
                record.status, record.latency_ms
            ),
        });
    } else {
        steps.extend(record.attempts.iter().take(4).map(attempt_timeline_step));
    }

    steps
}

fn attempt_timeline_step(attempt: &AttemptSummary) -> TimelineStep {
    let tone = if attempt.status.unwrap_or_default() >= 500 || attempt.error.is_some() {
        "danger"
    } else if attempt.status.unwrap_or_default() >= 400 {
        "warning"
    } else {
        "success"
    };

    let detail = attempt.error.clone().unwrap_or_else(|| {
        format!(
            "status {} in {} ms",
            attempt
                .status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            attempt.latency_ms
        )
    });

    TimelineStep {
        label: format!("Attempt {}", attempt.attempt_index + 1),
        tone: tone.to_string(),
        title: format!("{} / {}", attempt.provider, attempt.model),
        detail,
    }
}

fn explain_record(state: &AppState, record: &RequestRecord) -> Option<RouteExplanation> {
    let requested_model = record
        .requested_model
        .clone()
        .or_else(|| record.model.clone())?;
    let endpoint = endpoint_from_path(&record.path);
    let features = RouteRequestFeatures {
        requested_model,
        endpoint,
        source_format: source_format_from_path(&record.path),
        tenant_id: record.tenant_id.clone(),
        api_key_id: record.api_key_id.clone(),
        region: record.client_region.clone(),
        stream: record.stream,
        headers: Default::default(),
        allowed_credentials: Vec::new(),
        required_capabilities: None,
    };
    let config = state.config.load();
    let inventory = state.catalog.snapshot();
    let health = state.health_manager.snapshot();
    let plan = RoutePlanner::plan(&features, &config.routing, &inventory, &health);
    Some(explain(&plan))
}

fn traffic_inspector(record: Option<&RequestRecord>, query: &WorkspaceQuery) -> WorkspaceInspector {
    if let Some(record) = record {
        let (outcome, _) = request_result(record);
        return WorkspaceInspector {
            eyebrow: "SESSION / SELECTED".to_string(),
            title: record.request_id.clone(),
            summary: record.error.clone().unwrap_or_else(|| {
                format!(
                    "{} via {}",
                    outcome,
                    record
                        .provider
                        .clone()
                        .unwrap_or_else(|| "unknown-provider".to_string())
                )
            }),
            sections: vec![
                InspectorSection {
                    title: "Execution".to_string(),
                    rows: vec![
                        InspectorRow {
                            label: "Outcome".to_string(),
                            value: outcome,
                        },
                        InspectorRow {
                            label: "Latency".to_string(),
                            value: format!("{} ms", record.latency_ms),
                        },
                        InspectorRow {
                            label: "Attempts".to_string(),
                            value: record.total_attempts.to_string(),
                        },
                    ],
                },
                InspectorSection {
                    title: "Context".to_string(),
                    rows: vec![
                        InspectorRow {
                            label: "Provider".to_string(),
                            value: record
                                .provider
                                .clone()
                                .unwrap_or_else(|| "unknown".to_string()),
                        },
                        InspectorRow {
                            label: "Tenant".to_string(),
                            value: record
                                .tenant_id
                                .clone()
                                .unwrap_or_else(|| "unscoped".to_string()),
                        },
                        InspectorRow {
                            label: "Source".to_string(),
                            value: query.source_mode.clone(),
                        },
                    ],
                },
            ],
            actions: vec![
                "Open raw log".to_string(),
                "Explain route".to_string(),
                "Compare current window".to_string(),
            ],
        };
    }

    WorkspaceInspector {
        eyebrow: "SESSION / EMPTY".to_string(),
        title: "No request session selected".to_string(),
        summary: "The selected time range does not currently contain request sessions.".to_string(),
        sections: vec![],
        actions: vec!["Refresh".to_string()],
    }
}
