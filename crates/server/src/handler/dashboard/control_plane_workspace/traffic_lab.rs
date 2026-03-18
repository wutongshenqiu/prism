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
        TrafficSessionItem, UiText, WorkspaceAction, WorkspaceActionEffect, WorkspaceInspector,
        WorkspaceQuery, raw_text,
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
                label: UiText::new("trafficLab.trace.empty.label"),
                tone: "neutral".to_string(),
                title: UiText::new("trafficLab.trace.empty.title"),
                detail: UiText::new("trafficLab.trace.empty.detail"),
            }]
        });
    let compare_facts = vec![
        FactRow {
            label: UiText::new("common.window"),
            value: query.range.clone(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("trafficLab.fact.entries"),
            value: stats.total_entries.to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("trafficLab.fact.errors"),
            value: stats.error_count.to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("trafficLab.fact.avgLatency"),
            value: format!("{} ms", stats.avg_latency_ms),
            value_text: None,
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
        UiText::with_values(
            "trafficLab.session.decision.fallback",
            [("attempts", record.total_attempts)],
        )
    } else if let Some(provider) = &record.provider {
        UiText::with_values(
            "trafficLab.session.decision.primary",
            [("provider", provider.clone())],
        )
    } else {
        UiText::new("trafficLab.session.decision.unresolved")
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

fn request_result(record: &RequestRecord) -> (UiText, &'static str) {
    if record.status >= 500 || record.error.is_some() {
        return (UiText::new("trafficLab.result.failed"), "danger");
    }
    if record.total_attempts > 1 {
        return (UiText::new("trafficLab.result.recovered"), "warning");
    }
    (UiText::new("trafficLab.result.success"), "success")
}

fn build_traffic_trace(state: &AppState, record: &RequestRecord) -> Vec<TimelineStep> {
    let mut steps = Vec::new();
    steps.push(TimelineStep {
        label: UiText::new("trafficLab.trace.ingress.label"),
        tone: "info".to_string(),
        title: raw_text(format!(
            "{} {}",
            record.method,
            record
                .requested_model
                .clone()
                .or_else(|| record.model.clone())
                .unwrap_or_else(|| "unknown-model".to_string())
        )),
        detail: raw_text(format!(
            "{} request entered {}{}",
            record.path,
            record
                .tenant_id
                .as_deref()
                .map(|tenant| format!("tenant {}", tenant))
                .unwrap_or_else(|| "gateway scope".to_string()),
            if record.stream { " as stream" } else { "" }
        )),
    });

    if let Some(explanation) = explain_record(state, record)
        && let Some(selected) = explanation.selected
    {
        let rejection_count = explanation.rejections.len();
        steps.push(TimelineStep {
            label: UiText::new("trafficLab.trace.routeExplain.label"),
            tone: if rejection_count > 0 {
                "warning"
            } else {
                "success"
            }
            .to_string(),
            title: UiText::with_values(
                "trafficLab.trace.routeExplain.title",
                [
                    ("profile", explanation.profile.clone()),
                    ("provider", selected.provider.clone()),
                ],
            ),
            detail: if rejection_count > 0 {
                UiText::with_values(
                    "trafficLab.trace.routeExplain.detail.rejections",
                    [("count", rejection_count)],
                )
            } else {
                UiText::new("trafficLab.trace.routeExplain.detail.clean")
            },
        });
    }

    if record.attempts.is_empty() {
        steps.push(TimelineStep {
            label: UiText::new("trafficLab.trace.execution.label"),
            tone: request_result(record).1.to_string(),
            title: raw_text(
                record
                    .provider
                    .clone()
                    .unwrap_or_else(|| "No upstream attempt captured".to_string()),
            ),
            detail: raw_text(format!(
                "Finished with HTTP {} in {} ms.",
                record.status, record.latency_ms
            )),
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
        label: UiText::with_values(
            "trafficLab.trace.attempt.label",
            [("index", attempt.attempt_index + 1)],
        ),
        tone: tone.to_string(),
        title: raw_text(format!("{} / {}", attempt.provider, attempt.model)),
        detail: raw_text(detail),
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
            eyebrow: UiText::new("trafficLab.inspector.selected.eyebrow"),
            title: raw_text(record.request_id.clone()),
            summary: record.error.clone().map(raw_text).unwrap_or_else(|| {
                UiText::with_values(
                    "trafficLab.inspector.selected.summary",
                    [(
                        "provider",
                        record
                            .provider
                            .clone()
                            .unwrap_or_else(|| "unknown-provider".to_string()),
                    )],
                )
            }),
            sections: vec![
                InspectorSection {
                    title: UiText::new("trafficLab.inspector.execution"),
                    rows: vec![
                        InspectorRow {
                            label: UiText::new("trafficLab.inspector.outcome"),
                            value: outcome.key.clone(),
                            value_text: Some(outcome.clone()),
                        },
                        InspectorRow {
                            label: UiText::new("common.latency"),
                            value: format!("{} ms", record.latency_ms),
                            value_text: None,
                        },
                        InspectorRow {
                            label: UiText::new("trafficLab.inspector.attempts"),
                            value: record.total_attempts.to_string(),
                            value_text: None,
                        },
                    ],
                },
                InspectorSection {
                    title: UiText::new("trafficLab.inspector.context"),
                    rows: vec![
                        InspectorRow {
                            label: UiText::new("common.provider"),
                            value: record
                                .provider
                                .clone()
                                .unwrap_or_else(|| "unknown".to_string()),
                            value_text: None,
                        },
                        InspectorRow {
                            label: UiText::new("common.tenant"),
                            value: record
                                .tenant_id
                                .clone()
                                .unwrap_or_else(|| "unscoped".to_string()),
                            value_text: None,
                        },
                        InspectorRow {
                            label: UiText::new("common.source"),
                            value: query.source_mode.clone(),
                            value_text: Some(UiText::new(format!("common.{}", query.source_mode))),
                        },
                    ],
                },
            ],
            actions: vec![
                WorkspaceAction {
                    id: "open-raw-log".to_string(),
                    label: UiText::new("trafficLab.action.openRawLog"),
                    effect: WorkspaceActionEffect::Navigate,
                    target_workspace: Some("traffic-lab".to_string()),
                },
                WorkspaceAction {
                    id: "explain-route".to_string(),
                    label: UiText::new("trafficLab.action.explainRoute"),
                    effect: WorkspaceActionEffect::Navigate,
                    target_workspace: Some("route-studio".to_string()),
                },
                WorkspaceAction {
                    id: "compare-window".to_string(),
                    label: UiText::new("trafficLab.action.compareWindow"),
                    effect: WorkspaceActionEffect::Navigate,
                    target_workspace: Some("traffic-lab".to_string()),
                },
            ],
        };
    }

    WorkspaceInspector {
        eyebrow: UiText::new("trafficLab.inspector.empty.eyebrow"),
        title: UiText::new("trafficLab.inspector.empty.title"),
        summary: UiText::new("trafficLab.inspector.empty.summary"),
        sections: vec![],
        actions: vec![WorkspaceAction {
            id: "refresh-workspace".to_string(),
            label: UiText::new("shell.action.refreshWorkspace"),
            effect: WorkspaceActionEffect::Reload,
            target_workspace: None,
        }],
    }
}
