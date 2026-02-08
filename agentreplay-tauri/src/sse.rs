// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! SSE (Server-Sent Events) support for real-time trace streaming and evaluation progress

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;

use crate::server::ServerState;
use axum::extract::State as AxumState;

/// Evaluation progress event for SSE streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalProgressEvent {
    /// Type of event: "started", "progress", "completed", "error"
    pub event_type: String,
    /// Evaluation run ID
    pub eval_run_id: String,
    /// Evaluator name (for progress events)
    pub evaluator: Option<String>,
    /// Progress percentage (0-100)
    pub progress_percent: Option<u8>,
    /// Current evaluation result (for progress events)
    pub result: Option<EvalResultSnippet>,
    /// Total evaluators count
    pub total_evaluators: Option<usize>,
    /// Completed evaluators count
    pub completed_evaluators: Option<usize>,
    /// Error message (for error events)
    pub error: Option<String>,
    /// Timestamp in microseconds
    pub timestamp_us: u64,
}

/// Snippet of evaluation result for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResultSnippet {
    pub metric_name: String,
    pub score: f64,
    pub passed: Option<bool>,
}

/// GET /api/v1/traces/stream - Server-Sent Events stream for real-time traces
///
/// This endpoint allows clients to subscribe to real-time trace updates.
/// Instead of polling, clients receive events as traces are ingested.
pub async fn sse_traces_handler(
    AxumState(state): AxumState<ServerState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.tauri_state.trace_broadcaster.subscribe();
    let stream = BroadcastStream::new(rx);
    
    let mapped_stream = async_stream::stream! {
        tokio::pin!(stream);
        
        while let Some(result) = futures::StreamExt::next(&mut stream).await {
            match result {
                Ok(edge) => {
                    // Convert edge to JSON event
                    let event_data = serde_json::json!({
                        "trace_id": format!("{:#x}", edge.session_id),
                        "span_id": format!("{:#x}", edge.edge_id),
                        "parent_span_id": if edge.causal_parent != 0 {
                            Some(format!("{:#x}", edge.causal_parent))
                        } else {
                            None
                        },
                        "timestamp_us": edge.timestamp_us,
                        "duration_us": edge.duration_us,
                        "token_count": edge.token_count,
                        "span_type": edge.span_type,
                        "project_id": edge.project_id,
                        "session_id": edge.session_id,
                        "agent_id": edge.agent_id,
                    });
                    
                    let event_str = serde_json::to_string(&event_data).unwrap_or_default();
                    yield Ok(Event::default().data(event_str));
                }
                Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                    // Client is too slow, skip n messages
                    tracing::warn!("SSE client lagged behind by {} messages", n);
                    let lag_event = serde_json::json!({
                        "type": "lag",
                        "skipped": n
                    });
                    yield Ok(Event::default().event("lag").data(serde_json::to_string(&lag_event).unwrap_or_default()));
                }
            }
        }
    };
    
    Sse::new(mapped_stream).keep_alive(KeepAlive::default())
}

/// Global eval broadcaster for SSE streaming
/// This should be initialized in AppState and shared across the application
pub type EvalBroadcaster = tokio::sync::broadcast::Sender<EvalProgressEvent>;

/// GET /api/v1/evals/stream - Server-Sent Events stream for real-time evaluation progress
///
/// This endpoint allows clients to subscribe to evaluation progress updates.
/// Events are sent as evaluators complete their work.
pub async fn sse_evals_handler(
    AxumState(_state): AxumState<ServerState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    // Get the eval broadcaster from state (if available)
    // For now, we'll use a placeholder stream that sends keepalive only
    // Real implementation would subscribe to eval_broadcaster in AppState
    
    let mapped_stream = async_stream::stream! {
        // Send initial connection event
        let connect_event = serde_json::json!({
            "event_type": "connected",
            "message": "Connected to evaluation stream",
            "timestamp_us": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64
        });
        yield Ok(Event::default().event("connected").data(
            serde_json::to_string(&connect_event).unwrap_or_default()
        ));
        
        // If we have an eval broadcaster in state, subscribe to it
        // TODO: Add eval_broadcaster to AppState and ServerState
        // For now, this endpoint just stays connected and waits
        
        // Keep connection alive with periodic heartbeats
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let heartbeat = serde_json::json!({
                "event_type": "heartbeat",
                "timestamp_us": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64
            });
            yield Ok(Event::default().event("heartbeat").data(
                serde_json::to_string(&heartbeat).unwrap_or_default()
            ));
        }
    };
    
    Sse::new(mapped_stream).keep_alive(KeepAlive::default())
}
