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
        UiText, WorkspaceAction, WorkspaceActionEffect, WorkspaceInspector, WorkspaceQuery,
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
            title: UiText::new("commandCenter.signal.runtimeStable.title"),
            detail: UiText::new("commandCenter.signal.runtimeStable.detail"),
            severity: UiText::new("common.healthy"),
            severity_tone: "success".to_string(),
            target_workspace: "command-center".to_string(),
        });
    }

    let inspector = inspector_from_signal(&signals[0], &query, &recent);
    let kpis = vec![
        KpiMetric {
            label: UiText::new("commandCenter.kpi.signals"),
            value: signals.len().to_string(),
            delta: UiText::with_values(
                "commandCenter.kpi.signals.delta",
                [("count", degraded_providers)],
            ),
        },
        KpiMetric {
            label: UiText::new("commandCenter.kpi.fallbackRate"),
            value: fallback_rate,
            delta: UiText::with_values(
                "commandCenter.kpi.fallbackRate.delta",
                [("count", fallback_count), ("total", recent.len())],
            ),
        },
        KpiMetric {
            label: UiText::new("commandCenter.kpi.ingestFreshness"),
            value: freshness,
            delta: UiText::with_values(
                "commandCenter.kpi.ingestFreshness.delta",
                [("sourceMode", query.source_mode.clone())],
            ),
        },
        KpiMetric {
            label: UiText::new("commandCenter.kpi.activeProviders"),
            value: active_providers.to_string(),
            delta: UiText::with_values(
                "commandCenter.kpi.activeProviders.delta",
                [("count", config.providers.len())],
            ),
        },
    ];

    let pressure_map = vec![
        FactRow {
            label: UiText::new("commandCenter.pressure.providersUnderWatch"),
            value: degraded_providers.to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("commandCenter.pressure.recentRequestErrors"),
            value: stats.error_count.to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("commandCenter.pressure.trackedTenants"),
            value: state
                .metrics
                .tenant_snapshot()
                .as_object()
                .map(|value| value.len())
                .unwrap_or_default()
                .to_string(),
            value_text: None,
        },
        FactRow {
            label: UiText::new("commandCenter.pressure.authProfiles"),
            value: config
                .providers
                .iter()
                .map(|provider| provider.expanded_auth_profiles().len())
                .sum::<usize>()
                .to_string(),
            value_text: None,
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
            label: UiText::new("commandCenter.watch.configVersion"),
            value: config_version,
            value_text: None,
        },
        FactRow {
            label: UiText::new("commandCenter.watch.topError"),
            value: top_error,
            value_text: None,
        },
        FactRow {
            label: UiText::new("commandCenter.watch.latestRequest"),
            value: latest_request,
            value_text: None,
        },
        FactRow {
            label: UiText::new("common.window"),
            value: query.range.clone(),
            value_text: None,
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
                title: UiText::with_values(
                    "commandCenter.signal.provider.title",
                    [("provider", provider.name.clone())],
                ),
                detail,
                severity: match status {
                    "disabled" => UiText::new("common.disabled"),
                    "degraded" => UiText::new("common.degraded"),
                    "watch" => UiText::new("common.watch"),
                    _ => UiText::new("common.warning"),
                },
                severity_tone: tone.to_string(),
                target_workspace: "provider-atlas".to_string(),
            });
        }
    }

    if let Some(top_error) = stats.top_errors.first() {
        signals.push(SignalItem {
            id: format!("error-{}", top_error.error_type),
            title: UiText::with_values(
                "commandCenter.signal.error.title",
                [("errorType", top_error.error_type.clone())],
            ),
            detail: UiText::with_values(
                "commandCenter.signal.error.detail",
                [("count", top_error.count)],
            ),
            severity: UiText::new("common.watch"),
            severity_tone: "warning".to_string(),
            target_workspace: "traffic-lab".to_string(),
        });
    }

    let fallback_sessions = recent
        .iter()
        .filter(|record| record.total_attempts > 1)
        .count();
    if fallback_sessions > 0 {
        signals.push(SignalItem {
            id: "fallback-surge".to_string(),
            title: UiText::new("commandCenter.signal.fallback.title"),
            detail: UiText::with_values(
                "commandCenter.signal.fallback.detail",
                [("count", fallback_sessions)],
            ),
            severity: UiText::new("common.watch"),
            severity_tone: "info".to_string(),
            target_workspace: "route-studio".to_string(),
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
        eyebrow: UiText::new("commandCenter.inspector.eyebrow"),
        title: signal.title.clone(),
        summary: signal.detail.clone(),
        sections: vec![
            InspectorSection {
                title: UiText::new("commandCenter.inspector.posture"),
                rows: vec![
                    InspectorRow {
                        label: UiText::new("commandCenter.inspector.severity"),
                        value: signal
                            .severity
                            .values
                            .get("value")
                            .cloned()
                            .unwrap_or_else(|| signal.severity.key.clone()),
                        value_text: Some(signal.severity.clone()),
                    },
                    InspectorRow {
                        label: UiText::new("commandCenter.inspector.target"),
                        value: signal.target_workspace.clone(),
                        value_text: None,
                    },
                    InspectorRow {
                        label: UiText::new("common.source"),
                        value: query.source_mode.clone(),
                        value_text: Some(UiText::new(format!("common.{}", query.source_mode))),
                    },
                ],
            },
            InspectorSection {
                title: UiText::new("commandCenter.inspector.runtime"),
                rows: vec![
                    InspectorRow {
                        label: UiText::new("commandCenter.watch.latestRequest"),
                        value: recent
                            .first()
                            .map(|record| record.request_id.clone())
                            .unwrap_or_else(|| "none".to_string()),
                        value_text: None,
                    },
                    InspectorRow {
                        label: UiText::new("common.window"),
                        value: query.range.clone(),
                        value_text: None,
                    },
                ],
            },
        ],
        actions: vec![
            WorkspaceAction {
                id: "open-investigation".to_string(),
                label: UiText::new("commandCenter.action.openInvestigation"),
                effect: WorkspaceActionEffect::Navigate,
                target_workspace: Some(signal.target_workspace.clone()),
            },
            WorkspaceAction {
                id: "jump-to-workspace".to_string(),
                label: UiText::new("commandCenter.action.jumpToWorkspace"),
                effect: WorkspaceActionEffect::Navigate,
                target_workspace: Some(signal.target_workspace.clone()),
            },
            WorkspaceAction {
                id: "refresh-signal-queue".to_string(),
                label: UiText::new("commandCenter.action.refreshSignalQueue"),
                effect: WorkspaceActionEffect::Reload,
                target_workspace: None,
            },
        ],
    }
}
