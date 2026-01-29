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

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::query::{ApiError, AppState};
use crate::auth::AuthContext;

#[derive(Debug, Deserialize)]
pub struct StorageDumpParams {
    pub project_id: Option<u16>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Serialize)]
pub struct StorageRecord {
    pub key: String,
    pub timestamp_us: u64,
    pub record_type: String, // "Edge (LSM)" or "Payload (Blob)"
    pub size_bytes: usize,
    pub content: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct StorageDumpResponse {
    pub records: Vec<StorageRecord>,
    pub total_records: usize, // Approximate
}

/// GET /api/v1/storage/dump
/// Dump raw storage records for debugging
pub async fn dump_storage(
    State(state): State<AppState>,
    Query(params): Query<StorageDumpParams>,
    auth: axum::Extension<AuthContext>,
) -> Result<Json<StorageDumpResponse>, ApiError> {
    // Determine which DB to use
    let db = if let Some(ref pm) = state.project_manager {
        if let Some(project_id) = params.project_id {
            pm.get_or_open_project(project_id)
                .map_err(|e| ApiError::Internal(format!("Failed to open project DB: {}", e)))?
        } else {
            // If no project specified but PM exists, we can't easily dump "all" without iterating projects
            // For now, fallback to main DB if project_id is missing, or error?
            // Let's error if project_id is required for PM mode
            return Err(ApiError::BadRequest(
                "project_id is required in multi-project mode".into(),
            ));
        }
    } else {
        state.db.clone()
    };

    // Query recent edges (reverse chronological)
    // We use a large range, but we'll paginate in memory for now as we don't have a "scan all" iterator exposed easily
    // Optimally we should use range_scan_iter but we need to reverse it for "newest first"

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    // Scan last 30 days by default for dump
    let start_ts = now.saturating_sub(30 * 24 * 3600 * 1_000_000);
    let end_ts = now;

    // Get edges
    // Note: In a real "dump", we might want raw iteration.
    // query_filtered sorts by timestamp? No, range_scan usually returns in timestamp order (oldest first).
    let mut edges = if let Some(project_id) = params.project_id {
        db.query_filtered(start_ts, end_ts, Some(auth.tenant_id), Some(project_id))
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        db.query_temporal_range_for_tenant(start_ts, end_ts, auth.tenant_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    // Reverse to show newest first
    edges.reverse();

    // Apply pagination to EDGES (this is an approximation, as records include payloads)
    // A better way is to collect records then paginate, but that's memory heavy.
    // We'll just paginate the edges and show associated payloads.
    let total_edges = edges.len();
    let page_edges = edges.into_iter().skip(params.offset).take(params.limit);

    let mut records = Vec::new();

    for edge in page_edges {
        // 1. Edge Record (LSM)
        records.push(StorageRecord {
            key: format!("edge:{:#x}", edge.edge_id),
            timestamp_us: edge.timestamp_us,
            record_type: "Edge (LSM)".to_string(),
            size_bytes: 128, // Fixed size
            content: serde_json::to_value(edge).unwrap_or_default(),
        });

        // 2. Payload Record (Blob) - if exists
        if edge.has_payload != 0 {
            if let Ok(Some(payload_bytes)) = db.get_payload(edge.edge_id) {
                let size = payload_bytes.len();
                let content = serde_json::from_slice::<serde_json::Value>(&payload_bytes)
                    .unwrap_or_else(
                        |_| serde_json::json!({"raw_hex": hex::encode(&payload_bytes)}),
                    );

                records.push(StorageRecord {
                    key: format!("payload:{:#x}", edge.edge_id),
                    timestamp_us: edge.timestamp_us,
                    record_type: "Payload (Blob)".to_string(),
                    size_bytes: size,
                    content,
                });
            }
        }
    }

    Ok(Json(StorageDumpResponse {
        records,
        total_records: total_edges * 2, // Rough estimate
    }))
}

/// Response for storage statistics
#[derive(Debug, Serialize)]
pub struct StorageStatsResponse {
    pub total_traces: usize,
    pub total_edges: usize,
    pub storage_bytes: usize,
    pub avg_trace_size_bytes: usize,
    pub oldest_timestamp_us: Option<u64>,
    pub newest_timestamp_us: Option<u64>,
    pub projects: Vec<ProjectStats>,
}

#[derive(Debug, Serialize)]
pub struct ProjectStats {
    pub project_id: u16,
    pub trace_count: usize,
    pub storage_bytes: usize,
}

/// GET /api/v1/storage/stats
/// Get storage statistics for calibration
pub async fn get_storage_stats(
    State(state): State<AppState>,
    auth: axum::Extension<AuthContext>,
) -> Result<Json<StorageStatsResponse>, ApiError> {
    let mut total_traces = 0usize;
    let mut total_storage_bytes = 0usize;
    let mut oldest_ts: Option<u64> = None;
    let mut newest_ts: Option<u64> = None;
    let mut project_stats: Vec<ProjectStats> = Vec::new();

    // Query all time range
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    let start_ts = 0u64; // Beginning of time
    let end_ts = now;

    if let Some(ref pm) = state.project_manager {
        // Multi-project mode: iterate through all projects
        let projects = pm.discover_projects().unwrap_or_default();
        for project_id in projects {
            if let Ok(db) = pm.get_or_open_project(project_id) {
                let edges = db
                    .query_filtered(start_ts, end_ts, Some(auth.tenant_id), Some(project_id))
                    .unwrap_or_default();

                let trace_count = edges.len();
                let mut project_storage = trace_count * 128; // Base edge size

                for edge in &edges {
                    // Update timestamp bounds
                    if oldest_ts.is_none() || edge.timestamp_us < oldest_ts.unwrap() {
                        oldest_ts = Some(edge.timestamp_us);
                    }
                    if newest_ts.is_none() || edge.timestamp_us > newest_ts.unwrap() {
                        newest_ts = Some(edge.timestamp_us);
                    }

                    // Count payload sizes
                    if edge.has_payload != 0 {
                        if let Ok(Some(payload)) = db.get_payload(edge.edge_id) {
                            project_storage += payload.len();
                        }
                    }
                }

                total_traces += trace_count;
                total_storage_bytes += project_storage;

                project_stats.push(ProjectStats {
                    project_id,
                    trace_count,
                    storage_bytes: project_storage,
                });
            }
        }
    } else {
        // Single DB mode
        let edges = state
            .db
            .query_temporal_range_for_tenant(start_ts, end_ts, auth.tenant_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let trace_count = edges.len();
        let mut storage = trace_count * 128;

        for edge in &edges {
            if oldest_ts.is_none() || edge.timestamp_us < oldest_ts.unwrap() {
                oldest_ts = Some(edge.timestamp_us);
            }
            if newest_ts.is_none() || edge.timestamp_us > newest_ts.unwrap() {
                newest_ts = Some(edge.timestamp_us);
            }

            if edge.has_payload != 0 {
                if let Ok(Some(payload)) = state.db.get_payload(edge.edge_id) {
                    storage += payload.len();
                }
            }
        }

        total_traces = trace_count;
        total_storage_bytes = storage;
    }

    let avg_trace_size = if total_traces > 0 {
        total_storage_bytes / total_traces
    } else {
        0
    };

    Ok(Json(StorageStatsResponse {
        total_traces,
        total_edges: total_traces, // For now, edges = traces
        storage_bytes: total_storage_bytes,
        avg_trace_size_bytes: avg_trace_size,
        oldest_timestamp_us: oldest_ts,
        newest_timestamp_us: newest_ts,
        projects: project_stats,
    }))
}
