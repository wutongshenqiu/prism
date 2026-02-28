use crate::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::sync::broadcast;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// GET /ws/dashboard — WebSocket endpoint for real-time updates.
pub async fn ws_handler(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Validate JWT from query param
    let config = state.config.load();
    if let Some(secret) = config.dashboard.resolve_jwt_secret() {
        let token = match query.token {
            Some(t) => t,
            None => {
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    "Missing token query parameter",
                )
                    .into_response();
            }
        };
        let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
        if jsonwebtoken::decode::<crate::middleware::dashboard_auth::Claims>(
            &token,
            &key,
            &jsonwebtoken::Validation::default(),
        )
        .is_err()
        {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                "Invalid or expired token",
            )
                .into_response();
        }
    }

    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut subscribed_metrics = true;
    let mut subscribed_logs = true;

    let mut log_rx: broadcast::Receiver<ai_proxy_core::request_log::RequestLogEntry> =
        state.request_logs.subscribe();

    let mut metrics_interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            // Send metrics snapshot every second
            _ = metrics_interval.tick(), if subscribed_metrics => {
                let snapshot = state.metrics.snapshot();
                let msg = json!({
                    "type": "metrics",
                    "data": snapshot,
                });
                if socket.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }

            // Forward new log entries
            Ok(entry) = log_rx.recv(), if subscribed_logs => {
                let msg = json!({
                    "type": "request_log",
                    "data": entry,
                });
                if socket.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }

            // Handle incoming messages (subscribe/unsubscribe)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text)
                            && cmd.get("type").and_then(|t| t.as_str()) == Some("subscribe")
                            && let Some(channels) = cmd.get("channels").and_then(|c| c.as_array())
                        {
                            let names: Vec<&str> = channels.iter().filter_map(|c| c.as_str()).collect();
                            subscribed_metrics = names.contains(&"metrics");
                            subscribed_logs = names.contains(&"request_log");
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
