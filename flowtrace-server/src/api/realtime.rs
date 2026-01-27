// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::convert::Infallible;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Extension,
};
use flowtrace_core::{AgentFlowEdge, SpanType};
use futures::{stream::Stream, SinkExt, StreamExt as FuturesStreamExt};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::{api::AppState, auth::AuthContext};

/// WebSocket endpoint that streams newly ingested traces in real time.
pub async fn ws_traces(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> impl IntoResponse {
    info!("WebSocket upgrade requested for tenant {}", auth.tenant_id);
    ws.on_upgrade(move |socket| handle_trace_stream(socket, state, auth))
}

async fn handle_trace_stream(socket: WebSocket, state: AppState, auth: AuthContext) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.trace_broadcaster.subscribe();

    // Heartbeat: ping every 30 seconds, timeout after 60 seconds
    let mut ping_interval = interval(Duration::from_secs(30));
    let mut last_pong = Instant::now();

    if sender
        .send(Message::Text(
            serde_json::to_string(&ServerMessage::Connected {
                timestamp: current_timestamp_us(),
            })
            .unwrap_or_else(|_| "{\"type\":\"Connected\"}".to_string()),
        ))
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            // Server-side ping/pong keepalive
            _ = ping_interval.tick() => {
                // Check if client is unresponsive
                if last_pong.elapsed() > Duration::from_secs(60) {
                    warn!("WebSocket client unresponsive for tenant {}, closing connection", auth.tenant_id);
                    break;
                }

                // Send ping
                if sender.send(Message::Ping(vec![])).await.is_err() {
                    info!("Failed to send ping, client disconnected (tenant {})", auth.tenant_id);
                    break;
                }
            }

            next = FuturesStreamExt::next(&mut receiver) => {
                match next {
                    Some(Ok(Message::Close(_))) | None => {
                        info!("WebSocket closed by client (tenant {})", auth.tenant_id);
                        break;
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = sender.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Update last pong timestamp
                        last_pong = Instant::now();
                    }
                    Some(Ok(Message::Text(text))) => {
                        debug!("Ignoring client text message: {}", text);
                    }
                    Some(Ok(Message::Binary(_))) => {
                        debug!("Ignoring binary message from client");
                    }
                    Some(Err(err)) => {
                        error!("WebSocket receive error: {}", err);
                        break;
                    }
                }
            }
            event = rx.recv() => {
                match event {
                    Ok(edge) => {
                        if edge.tenant_id != auth.tenant_id {
                            continue;
                        }

                        let payload = match serde_json::to_string(&TraceEvent::from_edge(edge)) {
                            Ok(json) => json,
                            Err(err) => {
                                error!("Failed to serialise trace event: {}", err);
                                continue;
                            }
                        };

                        if sender.send(Message::Text(payload)).await.is_err() {
                            info!("WebSocket client disconnected (tenant {})", auth.tenant_id);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        error!(
                            "WebSocket trace stream lagged for tenant {} (skipped {} events)",
                            auth.tenant_id,
                            skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ServerMessage {
    Connected { timestamp: u64 },
}

#[derive(Serialize)]
struct TraceEvent {
    edge_id: String,
    timestamp_us: u64,
    operation: String,
    span_type: String,
    duration_ms: f64,
    tokens: u32,
    cost: f64,
    status: String,
    agent_id: u64,
    session_id: u64,
}

impl TraceEvent {
    fn from_edge(edge: AgentFlowEdge) -> Self {
        let span = edge.get_span_type();
        Self {
            edge_id: format!("{:#x}", edge.edge_id),
            timestamp_us: edge.timestamp_us,
            operation: format!("{:?}", span),
            span_type: format!("{:?}", span),
            duration_ms: edge.duration_us as f64 / 1_000.0,
            tokens: edge.token_count,
            cost: estimate_edge_cost(&edge),
            status: if edge.is_deleted() || matches!(span, SpanType::Error) {
                "error".to_string()
            } else {
                "success".to_string()
            },
            agent_id: edge.agent_id,
            session_id: edge.session_id,
        }
    }
}

fn estimate_edge_cost(edge: &AgentFlowEdge) -> f64 {
    const PRICE_PER_1K_TOKENS_USD: f64 = 0.002;
    (edge.token_count as f64 / 1_000.0) * PRICE_PER_1K_TOKENS_USD
}

fn current_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

/// Server-Sent Events endpoint for streaming traces (better alternative to WebSocket for unidirectional streams)
pub async fn sse_traces(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("SSE trace stream requested for tenant {}", auth.tenant_id);

    let mut rx = state.trace_broadcaster.subscribe();
    let tenant_id = auth.tenant_id;

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(edge) if edge.tenant_id == tenant_id => {
                    match serde_json::to_string(&TraceEvent::from_edge(edge)) {
                        Ok(json) => yield Ok(Event::default().data(json)),
                        Err(err) => {
                            error!("Failed to serialize trace event: {}", err);
                        }
                    }
                }
                Ok(_) => {}, // Different tenant
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!("SSE stream lagged for tenant {} (skipped {} events)", tenant_id, skipped);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
