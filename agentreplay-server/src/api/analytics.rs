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

// agentreplay-server/src/api/analytics.rs
//
// Enhanced time-series analytics API endpoints
//
// PERFORMANCE: All handlers use PRE-COMPUTED minute/hour buckets maintained
// during ingestion (O(B) where B = number of buckets), NOT raw edge scanning
// (which was O(N) where N = millions of edges → 30-60s latency).

use super::query::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use agentreplay_core::SpanType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use tracing::debug;

use crate::otel_genai::{GenAIPayload, ModelPricing};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct TimeSeriesQuery {
    pub metric: String,
    pub start_time: u64,
    pub end_time: u64,
    #[serde(default = "default_granularity")]
    pub granularity: String, // "minute", "hour", "day"
    #[serde(default)]
    pub project_id: Option<u16>,
    #[serde(default)]
    pub agent_id: Option<u64>,
    #[serde(default)]
    pub model: Option<String>,
}

fn default_granularity() -> String {
    "hour".to_string()
}

/// Rich data point matching what the frontend expects.
/// Includes per-bucket request_count, error_count, total_tokens, avg_duration.
#[derive(Debug, Serialize)]
pub struct RichDataPoint {
    pub timestamp: u64,
    pub request_count: u64,
    pub error_count: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub avg_duration: f64,    // milliseconds
    pub total_duration: u64,  // microseconds
    /// Generic value field for backward compatibility
    pub value: f64,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesResponse {
    pub metric: String,
    pub granularity: String,
    pub data_points: Vec<RichDataPoint>,
    pub summary: TimeSeriesSummary,
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesSummary {
    pub total: f64,
    pub average: f64,
    pub min: f64,
    pub max: f64,
    pub std_dev: f64,
    pub trend: String, // "increasing", "decreasing", "stable"
    pub percent_change: f64,
    // Rich summary fields for frontend
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
}

#[derive(Debug, Deserialize)]
pub struct TrendAnalysisQuery {
    pub metric: String,
    pub days: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TrendAnalysisResponse {
    pub metric: String,
    pub period_days: u32,
    pub current_value: f64,
    pub previous_value: f64,
    pub percent_change: f64,
    pub trend: String,
    pub forecast_next_day: Option<f64>,
    pub forecast_next_week: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ComparativeAnalysisQuery {
    pub metric: String,
    pub start_time: u64,
    pub end_time: u64,
    pub group_by: String, // "agent", "model", "project", "environment"
}

#[derive(Debug, Serialize)]
pub struct ComparativeAnalysisResponse {
    pub metric: String,
    pub groups: HashMap<String, GroupMetrics>,
    pub total: f64,
}

#[derive(Debug, Serialize)]
pub struct GroupMetrics {
    pub value: f64,
    pub count: usize,
    pub percentage: f64,
    pub trend: String,
}

#[derive(Debug, Deserialize)]
pub struct CorrelationQuery {
    pub metric1: String,
    pub metric2: String,
    pub start_time: u64,
    pub end_time: u64,
}

#[derive(Debug, Serialize)]
pub struct CorrelationResponse {
    pub metric1: String,
    pub metric2: String,
    pub correlation_coefficient: f64,
    pub relationship: String, // "strong positive", "weak negative", etc.
    pub p_value: f64,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn calculate_granularity_interval(granularity: &str) -> u64 {
    match granularity {
        "minute" => 60_000_000,  // 60 seconds in microseconds
        "hour" => 3_600_000_000, // 1 hour in microseconds
        "day" => 86_400_000_000, // 1 day in microseconds
        _ => 3_600_000_000,      // Default to hour
    }
}

/// Compute trend from data point values
fn compute_trend(values: &[f64]) -> (String, f64) {
    if values.len() < 2 {
        return ("stable".to_string(), 0.0);
    }
    let first_half: f64 = values.iter().take(values.len() / 2).sum();
    let second_half: f64 = values.iter().skip(values.len() / 2).sum();
    let pct_change = if first_half != 0.0 {
        ((second_half - first_half) / first_half) * 100.0
    } else {
        0.0
    };
    let trend = if pct_change.abs() < 5.0 {
        "stable"
    } else if pct_change > 0.0 {
        "increasing"
    } else {
        "decreasing"
    };
    (trend.to_string(), pct_change)
}

fn calculate_correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.is_empty() {
        return 0.0;
    }

    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut numerator = 0.0;
    let mut sum_sq_x = 0.0;
    let mut sum_sq_y = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        numerator += dx * dy;
        sum_sq_x += dx * dx;
        sum_sq_y += dy * dy;
    }

    let denominator = (sum_sq_x * sum_sq_y).sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

fn describe_correlation(coefficient: f64) -> String {
    let abs_coef = coefficient.abs();
    let strength = if abs_coef >= 0.7 {
        "strong"
    } else if abs_coef >= 0.4 {
        "moderate"
    } else if abs_coef >= 0.2 {
        "weak"
    } else {
        "very weak"
    };

    let direction = if coefficient > 0.0 {
        "positive"
    } else {
        "negative"
    };

    format!("{} {}", strength, direction)
}

/// Extract metric value from a pre-computed bucket tuple
fn extract_metric(metric: &str, req: u64, err: u64, tok: u64, dur_us: u64) -> f64 {
    match metric {
        "latency" | "duration" | "duration_ms" | "avg_latency" => {
            if req > 0 { dur_us as f64 / req as f64 / 1000.0 } else { 0.0 }
        }
        "tokens" | "token_count" | "total_tokens" | "avg_tokens" => tok as f64,
        "trace_count" | "count" | "request_count" => req as f64,
        "error_rate" => {
            if req > 0 { err as f64 / req as f64 * 100.0 } else { 0.0 }
        }
        "error_count" => err as f64,
        _ => req as f64,
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// GET /api/v1/analytics/timeseries
/// Get time-series data for a metric
///
/// PERFORMANCE: Uses pre-computed minute buckets (O(B) where B ≈ 1440 for 24h)
/// instead of scanning all edges (O(N) where N = millions → 30-60s).
pub async fn get_timeseries(
    State(state): State<AppState>,
    Query(params): Query<TimeSeriesQuery>,
) -> Result<Json<TimeSeriesResponse>, (StatusCode, String)> {
    let interval = calculate_granularity_interval(&params.granularity);

    // Use pre-computed minute buckets — O(B) not O(N)
    // project_id=0 means "all projects" (wildcard)
    let minute_buckets = state.db.query_metrics_timeseries(
        params.project_id.unwrap_or(0) as u64,
        params.start_time,
        params.end_time,
    );

    debug!(
        "Timeseries query: {} minute buckets for project {:?}, range [{}, {}]",
        minute_buckets.len(),
        params.project_id,
        params.start_time,
        params.end_time,
    );

    // Re-aggregate minute buckets into the requested granularity (hour/day/minute)
    // Key = interval-aligned timestamp, Value = (request_count, error_count, total_tokens, total_duration_us)
    let mut agg: BTreeMap<u64, (u64, u64, u64, u64)> = BTreeMap::new();

    for (ts, bucket) in &minute_buckets {
        let aligned = (ts / interval) * interval;
        let entry = agg.entry(aligned).or_insert((0, 0, 0, 0));
        entry.0 += bucket.request_count;
        entry.1 += bucket.error_count;
        entry.2 += bucket.total_tokens;
        entry.3 += bucket.total_duration_us;
    }

    // Build data points covering the full time range (with zero-fill for gaps)
    let range = params.end_time.saturating_sub(params.start_time);
    let num_intervals = (range / interval.max(1)) as usize + 1;
    let mut data_points = Vec::with_capacity(num_intervals);
    let mut values_for_stats: Vec<f64> = Vec::with_capacity(num_intervals);

    // Totals for summary
    let mut total_req: u64 = 0;
    let mut total_err: u64 = 0;
    let mut total_tok: u64 = 0;
    let mut total_dur: u64 = 0;

    for i in 0..num_intervals {
        let ts = params.start_time + (i as u64 * interval);
        let (req, err, tok, dur) = agg.get(&ts).copied().unwrap_or((0, 0, 0, 0));

        total_req += req;
        total_err += err;
        total_tok += tok;
        total_dur += dur;

        let avg_dur_ms = if req > 0 { dur as f64 / req as f64 / 1000.0 } else { 0.0 };
        let value = extract_metric(&params.metric, req, err, tok, dur);
        values_for_stats.push(value);

        data_points.push(RichDataPoint {
            timestamp: ts,
            request_count: req,
            error_count: err,
            total_tokens: tok,
            total_cost: 0.0,
            avg_duration: avg_dur_ms,
            total_duration: dur,
            value,
            count: req as usize,
        });
    }

    // Compute summary statistics
    let total: f64 = values_for_stats.iter().sum();
    let n = values_for_stats.len();
    let average = if n > 0 { total / n as f64 } else { 0.0 };
    let min = values_for_stats.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values_for_stats.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let variance = if n > 0 {
        values_for_stats.iter().map(|v| (v - average).powi(2)).sum::<f64>() / n as f64
    } else {
        0.0
    };
    let std_dev = variance.sqrt();
    let (trend, percent_change) = compute_trend(&values_for_stats);

    let error_rate = if total_req > 0 { total_err as f64 / total_req as f64 * 100.0 } else { 0.0 };
    let avg_duration_ms = if total_req > 0 { total_dur as f64 / total_req as f64 / 1000.0 } else { 0.0 };

    Ok(Json(TimeSeriesResponse {
        metric: params.metric,
        granularity: params.granularity,
        data_points,
        summary: TimeSeriesSummary {
            total,
            average,
            min: if min.is_infinite() { 0.0 } else { min },
            max: if max.is_infinite() { 0.0 } else { max },
            std_dev,
            trend,
            percent_change,
            total_requests: total_req,
            total_errors: total_err,
            total_tokens: total_tok,
            total_cost: 0.0,
            avg_duration_ms,
            error_rate,
        },
    }))
}

/// GET /api/v1/analytics/trends
/// Get trend analysis for a metric
///
/// PERFORMANCE: Uses pre-computed metrics buckets — O(B) not O(N)
pub async fn get_trend_analysis(
    State(state): State<AppState>,
    Query(params): Query<TrendAnalysisQuery>,
) -> Result<Json<TrendAnalysisResponse>, (StatusCode, String)> {
    let days = params.days.unwrap_or(7);
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    let period_us = days as u64 * 86_400_000_000;
    let start_time = current_time.saturating_sub(period_us);

    // Use pre-computed buckets instead of scanning raw edges
    let current_bucket = state.db.query_metrics(0, 0, start_time, current_time);
    let previous_start = start_time.saturating_sub(period_us);
    let previous_bucket = state.db.query_metrics(0, 0, previous_start, start_time);

    let current_data = extract_metric(
        &params.metric,
        current_bucket.request_count,
        current_bucket.error_count,
        current_bucket.total_tokens,
        current_bucket.total_duration_us,
    );
    let previous_data = extract_metric(
        &params.metric,
        previous_bucket.request_count,
        previous_bucket.error_count,
        previous_bucket.total_tokens,
        previous_bucket.total_duration_us,
    );

    let percent_change = if previous_data != 0.0 {
        ((current_data - previous_data) / previous_data) * 100.0
    } else {
        0.0
    };

    let trend = if percent_change.abs() < 5.0 {
        "stable"
    } else if percent_change > 0.0 {
        "increasing"
    } else {
        "decreasing"
    };

    let forecast_next_day = if percent_change != 0.0 {
        Some(current_data * (1.0 + (percent_change / 100.0) / days as f64))
    } else {
        Some(current_data)
    };

    let forecast_next_week = if percent_change != 0.0 {
        Some(current_data * (1.0 + (percent_change / 100.0) * 7.0 / days as f64))
    } else {
        Some(current_data)
    };

    Ok(Json(TrendAnalysisResponse {
        metric: params.metric,
        period_days: days,
        current_value: current_data,
        previous_value: previous_data,
        percent_change,
        trend: trend.to_string(),
        forecast_next_day,
        forecast_next_week,
    }))
}

/// GET /api/v1/analytics/comparative
/// Get comparative analysis across groups
///
/// PERFORMANCE: Uses pre-computed per-project metrics buckets.
/// For "project" grouping this is O(P * B) where P = number of projects.
/// Falls back to dashboard summary for non-project groupings.
pub async fn get_comparative_analysis(
    State(state): State<AppState>,
    Query(params): Query<ComparativeAnalysisQuery>,
) -> Result<Json<ComparativeAnalysisResponse>, (StatusCode, String)> {
    // Use pre-computed dashboard summary to get top-level data
    let summary = state.db.get_dashboard_summary();

    let mut groups = HashMap::new();
    let mut total = 0.0;

    match params.group_by.as_str() {
        "model" => {
            // Use pre-computed top_models from DashboardSummary (O(1))
            for (model_name, (call_count, token_count)) in &summary.top_models {
                let value = match params.metric.as_str() {
                    "count" | "trace_count" | "request_count" => *call_count as f64,
                    "tokens" | "total_tokens" => *token_count as f64,
                    _ => *call_count as f64,
                };
                total += value;
                groups.insert(model_name.clone(), (value, *call_count as usize));
            }
        }
        "provider" => {
            // Use pre-computed top_providers from DashboardSummary (O(1))
            for (provider_name, call_count) in &summary.top_providers {
                let value = *call_count as f64;
                total += value;
                groups.insert(provider_name.clone(), (value, *call_count as usize));
            }
        }
        _ => {
            // For other groupings, use aggregate metrics
            let bucket = state.db.query_metrics(0, 0, params.start_time, params.end_time);
            let value = extract_metric(
                &params.metric,
                bucket.request_count,
                bucket.error_count,
                bucket.total_tokens,
                bucket.total_duration_us,
            );
            total = value;
            groups.insert("all".to_string(), (value, bucket.request_count as usize));
        }
    }

    let mut result_groups = HashMap::new();
    for (group_name, (value, count)) in groups {
        let percentage = if total != 0.0 {
            (value / total) * 100.0
        } else {
            0.0
        };
        result_groups.insert(
            group_name,
            GroupMetrics {
                value,
                count,
                percentage,
                trend: "stable".to_string(),
            },
        );
    }

    Ok(Json(ComparativeAnalysisResponse {
        metric: params.metric,
        groups: result_groups,
        total,
    }))
}

/// GET /api/v1/analytics/correlation
/// Get correlation analysis between two metrics
///
/// PERFORMANCE: Uses pre-computed minute buckets — O(B) not O(N)
pub async fn get_correlation(
    State(state): State<AppState>,
    Query(params): Query<CorrelationQuery>,
) -> Result<Json<CorrelationResponse>, (StatusCode, String)> {
    // Use pre-computed minute buckets for both metrics
    let minute_buckets = state.db.query_metrics_timeseries(
        0, // all projects
        params.start_time,
        params.end_time,
    );

    let data1: Vec<f64> = minute_buckets
        .iter()
        .map(|(_, b)| extract_metric(&params.metric1, b.request_count, b.error_count, b.total_tokens, b.total_duration_us))
        .collect();

    let data2: Vec<f64> = minute_buckets
        .iter()
        .map(|(_, b)| extract_metric(&params.metric2, b.request_count, b.error_count, b.total_tokens, b.total_duration_us))
        .collect();

    let coefficient = calculate_correlation(&data1, &data2);
    let relationship = describe_correlation(coefficient);

    let n = data1.len();
    let t_stat = if coefficient.abs() < 1.0 && n > 2 {
        coefficient * ((n as f64 - 2.0) / (1.0 - coefficient * coefficient)).sqrt()
    } else {
        0.0
    };
    let p_value = if t_stat.abs() > 2.0 { 0.05 } else { 0.1 };

    Ok(Json(CorrelationResponse {
        metric1: params.metric1,
        metric2: params.metric2,
        correlation_coefficient: coefficient,
        relationship,
        p_value,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];

        let corr = calculate_correlation(&x, &y);
        assert!((corr - 1.0).abs() < 0.001); // Perfect positive correlation
    }

    #[test]
    fn test_describe_correlation() {
        assert_eq!(describe_correlation(0.8), "strong positive");
        assert_eq!(describe_correlation(-0.5), "moderate negative");
        assert_eq!(describe_correlation(0.1), "very weak positive");
    }

    #[test]
    fn test_granularity_interval() {
        assert_eq!(calculate_granularity_interval("minute"), 60_000_000);
        assert_eq!(calculate_granularity_interval("hour"), 3_600_000_000);
        assert_eq!(calculate_granularity_interval("day"), 86_400_000_000);
    }
}

// ============================================================================
// NEW: Latency & Cost Breakdown APIs (from OTEL plan)
// ============================================================================

/// Query parameters for latency breakdown
#[derive(Debug, Deserialize)]
pub struct LatencyBreakdownQuery {
    pub session_id: u64,
}

/// Latency statistics by span type
#[derive(Debug, Serialize)]
pub struct LatencyStats {
    pub total_ms: f64,
    pub count: u32,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

/// Latency breakdown response
#[derive(Debug, Serialize)]
pub struct LatencyBreakdown {
    pub total_ms: f64,
    pub breakdown: HashMap<String, LatencyStats>,
    pub recommendations: Vec<String>,
}

/// GET /api/v1/analytics/latency-breakdown
///
/// Returns latency breakdown by component type, answering:
/// "Why is it slow? Which components dominate latency?"
///
/// PERFORMANCE: Uses session index for O(K_session) lookup instead of
/// scanning all edges in a 30-day range (O(N)).
pub async fn get_latency_breakdown(
    State(state): State<AppState>,
    Query(params): Query<LatencyBreakdownQuery>,
) -> Result<Json<LatencyBreakdown>, (StatusCode, String)> {
    debug!(
        "Getting latency breakdown for session {}",
        params.session_id
    );

    // Use session index — O(K_session) not O(N)
    let session_spans = state
        .db
        .get_session_edges_full(params.session_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if session_spans.is_empty() {
        return Err((StatusCode::NOT_FOUND, "No spans found for session".into()));
    }

    // Calculate breakdown by span type
    let mut breakdown: HashMap<SpanType, Vec<f64>> = HashMap::new();
    let mut total_ms = 0.0;

    for span in &session_spans {
        let duration_ms = span.duration_us as f64 / 1000.0;
        total_ms += duration_ms;

        // Track by span type
        breakdown
            .entry(span.get_span_type())
            .or_default()
            .push(duration_ms);
    }

    // Calculate statistics for each span type
    let mut stats_map = HashMap::new();
    for (span_type, durations) in breakdown {
        let count = durations.len() as u32;
        let total: f64 = durations.iter().sum();
        let avg = total / count as f64;
        let min = durations.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = durations.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Convert SpanType to string using format! (SpanType doesn't implement Display)
        let span_type_name = format!("{:?}", span_type);

        stats_map.insert(
            span_type_name,
            LatencyStats {
                total_ms: total,
                count,
                avg_ms: avg,
                min_ms: min,
                max_ms: max,
            },
        );
    }

    // Generate recommendations
    let mut recommendations = Vec::new();
    if let Some(reasoning_stats) = stats_map.get("Reasoning") {
        if reasoning_stats.avg_ms > 2000.0 {
            recommendations.push(format!(
                "LLM calls are slow (avg {}ms). Consider: smaller models, caching, or streaming.",
                reasoning_stats.avg_ms as i32
            ));
        }
    }

    if recommendations.is_empty() {
        recommendations.push("Performance looks good! No major bottlenecks detected.".to_string());
    }

    Ok(Json(LatencyBreakdown {
        total_ms,
        breakdown: stats_map,
        recommendations,
    }))
}

/// Cost breakdown response
#[derive(Debug, Serialize)]
pub struct CostBreakdown {
    pub total_cost_usd: f64,
    pub by_model: HashMap<String, ModelCost>,
    pub token_usage: TokenUsageSummary,
}

#[derive(Debug, Serialize)]
pub struct ModelCost {
    pub cost_usd: f64,
    pub call_count: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct TokenUsageSummary {
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cached_tokens: u32,
}

/// Query parameters for cost breakdown
#[derive(Debug, Deserialize)]
pub struct CostBreakdownQuery {
    pub session_id: u64,
}

/// GET /api/v1/analytics/cost-breakdown
///
/// Returns cost breakdown by model, answering:
/// "How much did it cost? Which models are expensive?"
///
/// PERFORMANCE: Uses session index for O(K_session) lookup instead of
/// scanning all edges in a 30-day range (O(N)).
pub async fn get_cost_breakdown(
    State(state): State<AppState>,
    Query(params): Query<CostBreakdownQuery>,
) -> Result<Json<CostBreakdown>, (StatusCode, String)> {
    debug!("Getting cost breakdown for session {}", params.session_id);

    // Use session index — O(K_session) not O(N)
    let session_spans = state
        .db
        .get_session_edges_full(params.session_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut total_cost = 0.0;
    let mut by_model: HashMap<String, ModelCost> = HashMap::new();
    let mut token_summary = TokenUsageSummary {
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cached_tokens: 0,
    };

    for span in session_spans {
        // Load GenAI payload
        if let Ok(Some(payload_bytes)) = state.db.get_payload(span.edge_id) {
            if let Ok(genai) = serde_json::from_slice::<GenAIPayload>(&payload_bytes) {
                let model = genai
                    .response_model
                    .clone()
                    .or_else(|| genai.request_model.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                let system = genai
                    .system
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());

                // Calculate cost
                let pricing = ModelPricing::for_model(&system, &model);
                let cost = genai.calculate_cost(&pricing);

                total_cost += cost;

                // Track by model
                let model_cost = by_model.entry(model.clone()).or_insert(ModelCost {
                    cost_usd: 0.0,
                    call_count: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                });

                model_cost.cost_usd += cost;
                model_cost.call_count += 1;
                model_cost.input_tokens += genai.input_tokens.unwrap_or(0);
                model_cost.output_tokens += genai.output_tokens.unwrap_or(0);

                // Update token summary
                token_summary.total_input_tokens += genai.input_tokens.unwrap_or(0);
                token_summary.total_output_tokens += genai.output_tokens.unwrap_or(0);
                token_summary.total_cached_tokens += genai.cache_read_tokens.unwrap_or(0);
            }
        }
    }

    Ok(Json(CostBreakdown {
        total_cost_usd: total_cost,
        by_model,
        token_usage: token_summary,
    }))
}
