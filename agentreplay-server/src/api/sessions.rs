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

//! Sessions API - Group traces by session_id for conversational views
//!
//! Provides endpoints to list and view sessions (multi-turn conversations)

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use agentreplay_core::AgentFlowEdge;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

use crate::api::query::{ApiError, AppState};
use crate::auth::AuthContext;

/// Query parameters for listing sessions
#[derive(Debug, Deserialize)]
pub struct SessionQueryParams {
    /// Start timestamp (microseconds since epoch)
    pub start_ts: Option<u64>,

    /// End timestamp (microseconds since epoch)
    pub end_ts: Option<u64>,

    /// Limit number of results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,

    /// Filter by project ID
    pub project_id: Option<u16>,

    /// Search by session ID substring
    pub search: Option<String>,

    /// Filter by status (active, ended)
    pub status: Option<String>,
}

fn default_limit() -> usize {
    100
}

/// Session metadata
#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: u64,
    pub project_id: u16,
    pub agent_id: u64,
    pub started_at: u64,
    pub last_message_at: u64,
    pub message_count: usize,
    pub total_tokens: u32,
    pub total_duration_ms: u32,
    pub trace_ids: Vec<String>,
    pub status: String,
}

/// Response for GET /api/v1/sessions
#[derive(Debug, Serialize)]
pub struct SessionsResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Response for GET /api/v1/sessions/:session_id
#[derive(Debug, Serialize)]
pub struct SessionDetailResponse {
    pub session: SessionInfo,
    pub traces: Vec<SessionTrace>,
}

/// Simplified trace info for session view
#[derive(Debug, Serialize)]
pub struct SessionTrace {
    pub trace_id: String,
    pub span_id: String,
    pub timestamp_us: u64,
    pub span_type: u32,
    pub duration_us: u32,
    pub token_count: u32,
    pub status: String,
}

/// GET /api/v1/sessions - List all sessions
#[tracing::instrument(skip(state, auth), fields(tenant_id = auth.tenant_id))]
pub async fn list_sessions(
    State(state): State<AppState>,
    Query(params): Query<SessionQueryParams>,
    auth: Extension<AuthContext>,
) -> Result<Json<SessionsResponse>, ApiError> {
    debug!("Listing sessions with params: {:?}", params);

    // Default time range: last 7 days
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    let start_ts = params.start_ts.unwrap_or(now - 7 * 86_400_000_000); // 7 days ago
    let end_ts = params.end_ts.unwrap_or(now);

    // Query all traces in time range
    let edges = if let Some(ref pm) = state.project_manager {
        if let Some(project_id) = params.project_id {
            pm.query_project(project_id, auth.tenant_id, start_ts, end_ts)
                .map_err(|e| ApiError::Internal(e.to_string()))?
        } else {
            pm.query_all_projects(auth.tenant_id, start_ts, end_ts)
                .map_err(|e| ApiError::Internal(e.to_string()))?
        }
    } else {
        let mut all_edges = state
            .db
            .query_temporal_range_for_tenant(start_ts, end_ts, auth.tenant_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        if let Some(project_id) = params.project_id {
            all_edges.retain(|e| e.project_id == project_id);
        }
        all_edges
    };

    debug!("Found {} edges to group into sessions", edges.len());

    // Group edges by session_id
    let mut session_map: HashMap<u64, Vec<AgentFlowEdge>> = HashMap::new();
    for edge in edges {
        session_map.entry(edge.session_id).or_default().push(edge);
    }

    debug!("Grouped into {} unique sessions", session_map.len());

    // Convert to SessionInfo
    let mut sessions: Vec<SessionInfo> = session_map
        .into_iter()
        .map(|(session_id, mut traces)| {
            // Sort by timestamp
            traces.sort_by_key(|e| e.timestamp_us);

            let started_at = traces.first().map(|e| e.timestamp_us).unwrap_or(0);
            let last_message_at = traces.last().map(|e| e.timestamp_us).unwrap_or(0);
            let message_count = traces.len();
            let total_tokens: u32 = traces.iter().map(|e| e.token_count).sum::<u32>();
            let total_duration_ms: u32 = traces.iter().map(|e| e.duration_us).sum::<u32>() / 1000;

            let trace_ids: Vec<String> =
                traces.iter().map(|e| format!("{:#x}", e.edge_id)).collect();

            // Determine status (active if last message < 1 hour ago)
            let one_hour_ago = now - 3_600_000_000; // 1 hour in microseconds
            let status = if last_message_at > one_hour_ago {
                "active".to_string()
            } else {
                "ended".to_string()
            };

            let project_id = traces.first().map(|e| e.project_id).unwrap_or(0);
            let agent_id = traces.first().map(|e| e.agent_id).unwrap_or(0);

            SessionInfo {
                session_id,
                project_id,
                agent_id,
                started_at,
                last_message_at,
                message_count,
                total_tokens,
                total_duration_ms,
                trace_ids,
                status,
            }
        })
        .collect();

    // Filter by search query
    if let Some(ref search) = params.search {
        let search_lower = search.to_lowercase();
        sessions.retain(|s| s.session_id.to_string().contains(&search_lower));
    }

    // Filter by status
    if let Some(ref status_filter) = params.status {
        sessions.retain(|s| s.status == *status_filter);
    }

    // Sort by last_message_at descending (most recent first)
    sessions.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));

    let total = sessions.len();

    // Apply pagination
    let sessions = sessions
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .collect();

    Ok(Json(SessionsResponse { sessions, total }))
}

/// GET /api/v1/sessions/:session_id - Get session details with all traces
#[tracing::instrument(skip(state, auth), fields(tenant_id = auth.tenant_id, session_id))]
pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<u64>,
    auth: Extension<AuthContext>,
) -> Result<Json<SessionDetailResponse>, ApiError> {
    debug!("Getting session details for session_id: {}", session_id);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    // Use session index for O(log N + K_session) lookup instead of scanning 30 days
    let mut session_traces: Vec<AgentFlowEdge> = if let Some(ref _pm) = state.project_manager {
        // With project manager, use session index on the main DB
        state
            .db
            .get_session_edges_full(session_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .into_iter()
            .filter(|e| e.tenant_id == auth.tenant_id)
            .collect()
    } else {
        // Direct session index lookup — O(log N + K) instead of O(N_30days)
        state
            .db
            .get_session_edges_full(session_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .into_iter()
            .filter(|e| e.tenant_id == auth.tenant_id)
            .collect()
    };

    if session_traces.is_empty() {
        return Err(ApiError::NotFound(format!(
            "Session {} not found",
            session_id
        )));
    }

    // Sort by timestamp
    session_traces.sort_by_key(|e| e.timestamp_us);

    // Build SessionInfo
    let started_at = session_traces.first().map(|e| e.timestamp_us).unwrap_or(0);
    let last_message_at = session_traces.last().map(|e| e.timestamp_us).unwrap_or(0);
    let message_count = session_traces.len();
    let total_tokens: u32 = session_traces.iter().map(|e| e.token_count).sum::<u32>();
    let total_duration_ms: u32 = session_traces.iter().map(|e| e.duration_us).sum::<u32>() / 1000;

    let trace_ids: Vec<String> = session_traces
        .iter()
        .map(|e| format!("{:#x}", e.edge_id))
        .collect();

    let one_hour_ago = now - 3_600_000_000;
    let status = if last_message_at > one_hour_ago {
        "active".to_string()
    } else {
        "ended".to_string()
    };

    let project_id = session_traces.first().map(|e| e.project_id).unwrap_or(0);
    let agent_id = session_traces.first().map(|e| e.agent_id).unwrap_or(0);

    let session = SessionInfo {
        session_id,
        project_id,
        agent_id,
        started_at,
        last_message_at,
        message_count,
        total_tokens,
        total_duration_ms,
        trace_ids,
        status,
    };

    // Convert edges to SessionTrace
    let traces: Vec<SessionTrace> = session_traces
        .into_iter()
        .map(|e| SessionTrace {
            trace_id: format!("{:#x}", e.edge_id),
            span_id: format!("{:#x}", e.edge_id),
            timestamp_us: e.timestamp_us,
            span_type: e.span_type,
            duration_us: e.duration_us,
            token_count: e.token_count,
            status: if e.is_deleted() {
                "deleted".to_string()
            } else {
                "completed".to_string()
            },
        })
        .collect();

    Ok(Json(SessionDetailResponse { session, traces }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 100);
    }
}
