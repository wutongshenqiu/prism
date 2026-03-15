use crate::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde_json::json;
use std::time::Duration;
use tokio::sync::broadcast;

/// GET /ws/dashboard — WebSocket endpoint for real-time updates.
///
/// Authentication is handled by the `dashboard_auth_middleware` layer
/// (supports `Authorization: Bearer` headers and same-site session cookies).
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut subscribed_metrics = true;
    let mut subscribed_logs = true;

    let mut log_rx: broadcast::Receiver<prism_core::request_record::RequestRecord> =
        state.log_store.subscribe();

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
