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

use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Query, State},
    Json,
};
use agentreplay_core::{AgentFlowEdge, SpanType};
use serde::{Deserialize, Serialize};

use crate::{api::query::ApiError, api::AppState, auth::AuthContext};

const DEFAULT_LOOKBACK_US: u64 = 3_600_000_000; // 1 hour
const DEFAULT_INTERVAL_SECONDS: u64 = 300; // 5 minutes
const MAX_BUCKETS: usize = 5_000;

#[derive(Debug, Deserialize)]
pub struct TimeseriesParams {
    pub start_ts: Option<u64>,
    pub end_ts: Option<u64>,
    pub interval_seconds: Option<u64>,
    /// Filter by environment (dev, staging, prod, test)
    pub environment: Option<String>,
    /// Filter by agent ID
    pub agent_id: Option<u64>,
    /// Group metrics by: "agent", "model", or none for aggregate
    pub group_by: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TimeseriesResponse {
    pub data: Vec<TimeseriesData>,
    pub metadata: TimeseriesMetadata,
    /// Optional grouped breakdowns (when group_by is specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<GroupedMetrics>>,
}

#[derive(Debug, Serialize)]
pub struct TimeseriesMetadata {
    pub start_ts: u64,
    pub end_ts: u64,
    pub interval_seconds: u64,
    pub bucket_count: usize,
}

#[derive(Debug, Serialize)]
pub struct GroupedMetrics {
    pub group_key: String,  // agent_id or model name
    pub group_name: String, // human-readable name (e.g., "support_bot.v1")
    pub data: Vec<TimeseriesData>,
}

#[derive(Debug, Serialize)]
pub struct TimeseriesData {
    pub timestamp: u64,
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub avg_duration: f64,
    pub error_count: u64,
    pub p50_duration: f64,
    pub p90_duration: f64,
    pub p95_duration: f64,
    pub p99_duration: f64,
}

#[derive(Debug, Serialize)]
pub struct ProjectMetricsResponse {
    pub latency_ms: LatencyMetrics,
    pub tokens: TokenMetrics,
    pub cost_usd: CostMetrics,
}

#[derive(Debug, Serialize)]
pub struct LatencyMetrics {
    pub p50: f64,
    pub p80: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
}

#[derive(Debug, Serialize)]
pub struct TokenMetrics {
    pub p50: f64,
    pub p80: f64,
    pub p90: f64,
}

#[derive(Debug, Serialize)]
pub struct CostMetrics {
    pub avg: f64,
    pub total: f64,
}

/// GET /api/v1/projects/:project_id/metrics
pub async fn get_project_metrics(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<u16>,
    axum::Extension(auth): axum::Extension<AuthContext>,
) -> Result<Json<ProjectMetricsResponse>, ApiError> {
    // Default lookback: 24 hours
    let end_ts = current_timestamp_us();
    let start_ts = end_ts.saturating_sub(24 * 3_600_000_000);

    // Query traces
    let edges = if let Some(ref pm) = state.project_manager {
        pm.query_project(project_id, auth.tenant_id, start_ts, end_ts)
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        let mut all_edges = state
            .db
            .query_temporal_range_for_tenant(start_ts, end_ts, auth.tenant_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        all_edges.retain(|e| e.project_id == project_id);
        all_edges
    };

    // Calculate metrics
    let mut durations = Vec::with_capacity(edges.len());
    let mut tokens = Vec::with_capacity(edges.len());
    let mut total_cost = 0.0;

    for edge in &edges {
        durations.push(edge.duration_us);
        tokens.push(edge.token_count);
        total_cost += estimate_edge_cost(edge);
    }

    durations.sort_unstable();
    tokens.sort_unstable();

    let avg_cost = if !edges.is_empty() {
        total_cost / edges.len() as f64
    } else {
        0.0
    };

    Ok(Json(ProjectMetricsResponse {
        latency_ms: LatencyMetrics {
            p50: percentile_ms(&durations, 50.0),
            p80: percentile_ms(&durations, 80.0),
            p90: percentile_ms(&durations, 90.0),
            p95: percentile_ms(&durations, 95.0),
            p99: percentile_ms(&durations, 99.0),
        },
        tokens: TokenMetrics {
            p50: percentile_val(&tokens, 50.0),
            p80: percentile_val(&tokens, 80.0),
            p90: percentile_val(&tokens, 90.0),
        },
        cost_usd: CostMetrics {
            avg: avg_cost,
            total: total_cost,
        },
    }))
}

fn percentile_val(values: &[u32], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let percentile = percentile.clamp(0.0, 100.0);
    let idx = ((percentile / 100.0) * ((values.len() - 1) as f64)).round() as usize;
    values[idx] as f64
}

#[derive(Clone, Default)]
struct BucketAccum {
    request_count: u64,
    total_tokens: u64,
    total_cost: f64,
    total_duration_us: u64,
    error_count: u64,
    durations_us: Vec<u32>,
}

/// GET /api/v1/metrics/timeseries
pub async fn get_timeseries_metrics(
    State(state): State<AppState>,
    Query(params): Query<TimeseriesParams>,
    axum::Extension(auth): axum::Extension<AuthContext>,
) -> Result<Json<TimeseriesResponse>, ApiError> {
    let end_ts = params.end_ts.unwrap_or_else(current_timestamp_us);
    let start_ts = params
        .start_ts
        .unwrap_or(end_ts.saturating_sub(DEFAULT_LOOKBACK_US));

    if end_ts <= start_ts {
        return Err(ApiError::BadRequest(
            "end_ts must be greater than start_ts".into(),
        ));
    }

    let interval_seconds = params
        .interval_seconds
        .unwrap_or(DEFAULT_INTERVAL_SECONDS)
        .max(1);

    let bucket_duration_us = interval_seconds.saturating_mul(1_000_000);
    let range_us = end_ts.saturating_sub(start_ts);
    let bucket_count = range_us
        .div_ceil(bucket_duration_us)
        .max(1)
        .min(MAX_BUCKETS as u64) as usize;

    if bucket_count == MAX_BUCKETS {
        tracing::warn!(
            "Timeseries bucket count capped at {} (range_us={}, interval={})",
            MAX_BUCKETS,
            range_us,
            interval_seconds
        );
    }

    let mut edges = state
        .db
        .query_temporal_range_for_tenant(start_ts, end_ts, auth.tenant_id)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Apply environment filter if requested
    if let Some(env_str) = &params.environment {
        use agentreplay_core::Environment;
        let target_env = Environment::parse(env_str);
        let target_env_u8 = target_env as u8;
        edges.retain(|e| e.environment == target_env_u8);
    }

    // Apply agent_id filter if requested
    if let Some(agent_id) = params.agent_id {
        edges.retain(|e| e.agent_id == agent_id);
    }

    // Check if grouping is requested
    let group_by = params.group_by.as_deref();

    match group_by {
        Some("agent") => {
            // Group by agent_id
            let (data, groups) = compute_grouped_metrics_by_agent(
                &edges,
                start_ts,
                bucket_duration_us,
                bucket_count,
                &state.agent_registry,
            );

            Ok(Json(TimeseriesResponse {
                data,
                metadata: TimeseriesMetadata {
                    start_ts,
                    end_ts,
                    interval_seconds,
                    bucket_count,
                },
                groups: Some(groups),
            }))
        }
        Some("model") => {
            // TODO: Group by model (requires reading payload attributes)
            // For now, return error
            Err(ApiError::BadRequest(
                "group_by=model not yet implemented. Use group_by=agent instead.".into(),
            ))
        }
        _ => {
            // No grouping - compute aggregate metrics
            let data =
                compute_aggregate_metrics(&edges, start_ts, bucket_duration_us, bucket_count);

            Ok(Json(TimeseriesResponse {
                data,
                metadata: TimeseriesMetadata {
                    start_ts,
                    end_ts,
                    interval_seconds,
                    bucket_count,
                },
                groups: None,
            }))
        }
    }
}

fn percentile_ms(values: &[u32], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let percentile = percentile.clamp(0.0, 100.0);
    let idx = ((percentile / 100.0) * ((values.len() - 1) as f64)).round() as usize;
    values[idx] as f64 / 1_000.0
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

/// Compute aggregate metrics (no grouping)
fn compute_aggregate_metrics(
    edges: &[AgentFlowEdge],
    start_ts: u64,
    bucket_duration_us: u64,
    bucket_count: usize,
) -> Vec<TimeseriesData> {
    let mut buckets = vec![BucketAccum::default(); bucket_count];

    for edge in edges {
        if edge.timestamp_us < start_ts {
            continue;
        }
        let mut bucket_idx = ((edge.timestamp_us - start_ts) / bucket_duration_us) as usize;
        if bucket_idx >= bucket_count {
            bucket_idx = bucket_count - 1;
        }

        let bucket = &mut buckets[bucket_idx];
        bucket.request_count += 1;
        bucket.total_tokens += edge.token_count as u64;
        bucket.total_cost += estimate_edge_cost(edge);
        bucket.total_duration_us += edge.duration_us as u64;
        bucket.durations_us.push(edge.duration_us);

        if edge.is_deleted() || matches!(edge.get_span_type(), SpanType::Error) {
            bucket.error_count += 1;
        }
    }

    buckets_to_timeseries_data(buckets, start_ts, bucket_duration_us)
}

/// Compute grouped metrics by agent_id
fn compute_grouped_metrics_by_agent(
    edges: &[AgentFlowEdge],
    start_ts: u64,
    bucket_duration_us: u64,
    bucket_count: usize,
    agent_registry: &crate::agent_registry::AgentRegistry,
) -> (Vec<TimeseriesData>, Vec<GroupedMetrics>) {
    use std::collections::HashMap;

    // Group edges by agent_id
    let mut agent_edges: HashMap<u64, Vec<&AgentFlowEdge>> = HashMap::new();
    for edge in edges {
        agent_edges.entry(edge.agent_id).or_default().push(edge);
    }

    // Compute aggregate (total across all agents)
    let aggregate_data =
        compute_aggregate_metrics(edges, start_ts, bucket_duration_us, bucket_count);

    // Compute per-agent metrics
    let mut groups = Vec::new();
    let mut agent_ids: Vec<u64> = agent_edges.keys().copied().collect();
    agent_ids.sort_unstable();

    for agent_id in agent_ids {
        let agent_edges = agent_edges.get(&agent_id).unwrap();

        // Copy edges for processing (needed since compute_aggregate_metrics takes &[])
        let edges_vec: Vec<AgentFlowEdge> = agent_edges.iter().map(|&e| *e).collect();

        let data =
            compute_aggregate_metrics(&edges_vec, start_ts, bucket_duration_us, bucket_count);

        let group_key = agent_id.to_string();
        let group_name = agent_registry.get_display_name(agent_id);

        groups.push(GroupedMetrics {
            group_key,
            group_name,
            data,
        });
    }

    (aggregate_data, groups)
}

/// Convert bucket accumulators to TimeseriesData
fn buckets_to_timeseries_data(
    buckets: Vec<BucketAccum>,
    start_ts: u64,
    bucket_duration_us: u64,
) -> Vec<TimeseriesData> {
    let mut data = Vec::with_capacity(buckets.len());
    for (idx, bucket) in buckets.into_iter().enumerate() {
        let timestamp = start_ts + (idx as u64) * bucket_duration_us;
        let avg_duration = if bucket.request_count > 0 {
            bucket.total_duration_us as f64 / bucket.request_count as f64 / 1_000.0
        } else {
            0.0
        };

        let mut durations = bucket.durations_us;
        durations.sort_unstable();

        data.push(TimeseriesData {
            timestamp,
            request_count: bucket.request_count,
            total_tokens: bucket.total_tokens,
            total_cost: bucket.total_cost,
            avg_duration,
            error_count: bucket.error_count,
            p50_duration: percentile_ms(&durations, 50.0),
            p90_duration: percentile_ms(&durations, 90.0),
            p95_duration: percentile_ms(&durations, 95.0),
            p99_duration: percentile_ms(&durations, 99.0),
        });
    }
    data
}
