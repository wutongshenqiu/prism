use axum::{
    Json,
    extract::{Query, State},
};
use prism_core::{config::Config, request_log::StatsQuery, request_record::RequestRecord};

use crate::{AppState, handler::dashboard::config_tx};

use super::{
    provider_atlas::provider_runtime_status,
    shared::{latest_freshness, percentage, query_recent_logs, range_start_timestamp},
    types::{
        CommandCenterResponse, FactRow, InspectorRow, InspectorSection, KpiMetric, SignalItem,
        WorkspaceInspector, WorkspaceQuery,
    },
};

pub async fn command_center(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<CommandCenterResponse> {
    let from = range_start_timestamp(&query.range);
    let stats = state
        .log_store
        .stats(&StatsQuery {
            from: Some(from),
            ..Default::default()
        })
        .await;
    let recent = query_recent_logs(&state, from, 24).await;
    let config = state.config.load();
    let active_providers = config
        .providers
        .iter()
        .filter(|provider| !provider.disabled)
        .count();
    let degraded_providers = config
        .providers
        .iter()
        .filter(|provider| provider_runtime_status(&state, provider).0 != "healthy")
        .count();
    let fallback_count = recent
        .iter()
        .filter(|record| record.total_attempts > 1)
        .count();
    let fallback_rate = percentage(fallback_count, recent.len());
    let freshness = latest_freshness(&recent)
        .map(|seconds| format!("{seconds}s"))
        .unwrap_or_else(|| "n/a".to_string());

    let mut signals = build_signal_items(&state, &config, &recent, &stats);
    if signals.is_empty() {
        signals.push(SignalItem {
            id: "signal-runtime-stable".to_string(),
            title: "Runtime posture is stable".to_string(),
            detail: "No degraded providers or recent request errors were detected in the selected window.".to_string(),
            severity: "healthy".to_string(),
            severity_tone: "success".to_string(),
            target_workspace: "Command Center".to_string(),
        });
    }

    let inspector = inspector_from_signal(&signals[0], &query, &recent);
    let kpis = vec![
        KpiMetric {
            label: "Signals".to_string(),
            value: signals.len().to_string(),
            delta: format!("{degraded_providers} providers require follow-up"),
        },
        KpiMetric {
            label: "Fallback rate".to_string(),
            value: fallback_rate,
            delta: format!(
                "{fallback_count} of {} recent requests retried",
                recent.len()
            ),
        },
        KpiMetric {
            label: "Ingest freshness".to_string(),
            value: freshness,
            delta: format!("source mode {}", query.source_mode),
        },
        KpiMetric {
            label: "Active providers".to_string(),
            value: active_providers.to_string(),
            delta: format!("{} configured total", config.providers.len()),
        },
    ];

    let pressure_map = vec![
        FactRow {
            label: "Providers under watch".to_string(),
            value: degraded_providers.to_string(),
        },
        FactRow {
            label: "Recent request errors".to_string(),
            value: stats.error_count.to_string(),
        },
        FactRow {
            label: "Tracked tenants".to_string(),
            value: state
                .metrics
                .tenant_snapshot()
                .as_object()
                .map(|value| value.len())
                .unwrap_or_default()
                .to_string(),
        },
        FactRow {
            label: "Auth profiles".to_string(),
            value: config
                .providers
                .iter()
                .map(|provider| provider.expanded_auth_profiles().len())
                .sum::<usize>()
                .to_string(),
        },
    ];

    let config_version = config_tx::read_config_versioned(&state)
        .map(|(_, version)| version)
        .unwrap_or_else(|_| "unavailable".to_string());
    let top_error = stats
        .top_errors
        .first()
        .map(|entry| format!("{} ({})", entry.error_type, entry.count))
        .unwrap_or_else(|| "none".to_string());
    let latest_request = recent
        .first()
        .map(|entry| entry.request_id.clone())
        .unwrap_or_else(|| "none".to_string());
    let watch_windows = vec![
        FactRow {
            label: "Config version".to_string(),
            value: config_version,
        },
        FactRow {
            label: "Top error".to_string(),
            value: top_error,
        },
        FactRow {
            label: "Latest request".to_string(),
            value: latest_request,
        },
        FactRow {
            label: "Window".to_string(),
            value: query.range.clone(),
        },
    ];

    Json(CommandCenterResponse {
        kpis,
        signals,
        pressure_map,
        watch_windows,
        inspector,
    })
}

fn build_signal_items(
    state: &AppState,
    config: &Config,
    recent: &[RequestRecord],
    stats: &prism_core::request_log::LogStats,
) -> Vec<SignalItem> {
    let mut signals = Vec::new();

    for provider in &config.providers {
        let (status, tone, detail) = provider_runtime_status(state, provider);
        if status != "healthy" {
            signals.push(SignalItem {
                id: format!("provider-{}", provider.name),
                title: format!("Provider {} is {}", provider.name, status),
                detail,
                severity: status.to_string(),
                severity_tone: tone.to_string(),
                target_workspace: "Provider Atlas".to_string(),
            });
        }
    }

    if let Some(top_error) = stats.top_errors.first() {
        signals.push(SignalItem {
            id: format!("error-{}", top_error.error_type),
            title: format!(
                "Recent {} requests need investigation",
                top_error.error_type
            ),
            detail: format!(
                "{} requests in the current window hit this error type.",
                top_error.count
            ),
            severity: "watch".to_string(),
            severity_tone: "warning".to_string(),
            target_workspace: "Traffic Lab".to_string(),
        });
    }

    let fallback_sessions = recent
        .iter()
        .filter(|record| record.total_attempts > 1)
        .count();
    if fallback_sessions > 0 {
        signals.push(SignalItem {
            id: "fallback-surge".to_string(),
            title: "Fallback traffic is above zero".to_string(),
            detail: format!(
                "{} recent request sessions retried across providers.",
                fallback_sessions
            ),
            severity: "watch".to_string(),
            severity_tone: "info".to_string(),
            target_workspace: "Route Studio".to_string(),
        });
    }

    signals.truncate(6);
    signals
}

fn inspector_from_signal(
    signal: &SignalItem,
    query: &WorkspaceQuery,
    recent: &[RequestRecord],
) -> WorkspaceInspector {
    WorkspaceInspector {
        eyebrow: "SIGNAL / ACTIVE".to_string(),
        title: signal.title.clone(),
        summary: signal.detail.clone(),
        sections: vec![
            InspectorSection {
                title: "Posture".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Severity".to_string(),
                        value: signal.severity.clone(),
                    },
                    InspectorRow {
                        label: "Target".to_string(),
                        value: signal.target_workspace.clone(),
                    },
                    InspectorRow {
                        label: "Source".to_string(),
                        value: query.source_mode.clone(),
                    },
                ],
            },
            InspectorSection {
                title: "Runtime".to_string(),
                rows: vec![
                    InspectorRow {
                        label: "Latest request".to_string(),
                        value: recent
                            .first()
                            .map(|record| record.request_id.clone())
                            .unwrap_or_else(|| "none".to_string()),
                    },
                    InspectorRow {
                        label: "Window".to_string(),
                        value: query.range.clone(),
                    },
                ],
            },
        ],
        actions: vec![
            "Open investigation".to_string(),
            "Jump to workspace".to_string(),
            "Refresh signal queue".to_string(),
        ],
    }
}
