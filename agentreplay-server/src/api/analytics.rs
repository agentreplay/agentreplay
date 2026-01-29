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

// agentreplay-server/src/api/analytics.rs
//
// Enhanced time-series analytics API endpoints

use super::query::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use agentreplay_core::{AgentFlowEdge, SpanType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

// Use DataPoint from core
use crate::otel_genai::{GenAIPayload, ModelPricing};
use agentreplay_core::enterprise::DataPoint;

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

#[derive(Debug, Serialize)]
pub struct TimeSeriesResponse {
    pub metric: String,
    pub granularity: String,
    pub data_points: Vec<DataPoint>,
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

fn calculate_summary(data_points: &[DataPoint]) -> TimeSeriesSummary {
    if data_points.is_empty() {
        return TimeSeriesSummary {
            total: 0.0,
            average: 0.0,
            min: 0.0,
            max: 0.0,
            std_dev: 0.0,
            trend: "stable".to_string(),
            percent_change: 0.0,
        };
    }

    let values: Vec<f64> = data_points.iter().map(|dp| dp.value).collect();
    let total: f64 = values.iter().sum();
    let average = total / values.len() as f64;
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let variance = values.iter().map(|v| (v - average).powi(2)).sum::<f64>() / values.len() as f64;
    let std_dev = variance.sqrt();

    // Calculate trend
    let (trend, percent_change) = if data_points.len() >= 2 {
        let first_half: f64 = values.iter().take(values.len() / 2).sum();
        let second_half: f64 = values.iter().skip(values.len() / 2).sum();

        let pct_change = if first_half != 0.0 {
            ((second_half - first_half) / first_half) * 100.0
        } else {
            0.0
        };

        let trend_str = if pct_change.abs() < 5.0 {
            "stable"
        } else if pct_change > 0.0 {
            "increasing"
        } else {
            "decreasing"
        };

        (trend_str.to_string(), pct_change)
    } else {
        ("stable".to_string(), 0.0)
    };

    TimeSeriesSummary {
        total,
        average,
        min,
        max,
        std_dev,
        trend,
        percent_change,
    }
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

// ============================================================================
// API Handlers
// ============================================================================

/// GET /api/v1/analytics/timeseries
/// Get time-series data for a metric
pub async fn get_timeseries(
    State(state): State<AppState>,
    Query(params): Query<TimeSeriesQuery>,
) -> Result<Json<TimeSeriesResponse>, (StatusCode, String)> {
    let interval = calculate_granularity_interval(&params.granularity);

    // Get data points
    let data_points = state
        .db
        .get_timeseries_data(
            &params.metric,
            params.start_time,
            params.end_time,
            interval,
            params.project_id,
            params.agent_id,
            params.model.as_deref(),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let summary = calculate_summary(&data_points);

    Ok(Json(TimeSeriesResponse {
        metric: params.metric,
        granularity: params.granularity,
        data_points,
        summary,
    }))
}

/// GET /api/v1/analytics/trends
/// Get trend analysis for a metric
pub async fn get_trend_analysis(
    State(state): State<AppState>,
    Query(params): Query<TrendAnalysisQuery>,
) -> Result<Json<TrendAnalysisResponse>, (StatusCode, String)> {
    let days = params.days.unwrap_or(7);
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    let period_us = days as u64 * 86_400_000_000; // days to microseconds
    let start_time = current_time.saturating_sub(period_us);

    // Get current period data
    let current_data = state
        .db
        .get_metric_value(&params.metric, start_time, current_time)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get previous period data for comparison
    let previous_start = start_time.saturating_sub(period_us);
    let previous_data = state
        .db
        .get_metric_value(&params.metric, previous_start, start_time)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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

    // Simple linear forecast (basic extrapolation)
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
pub async fn get_comparative_analysis(
    State(state): State<AppState>,
    Query(params): Query<ComparativeAnalysisQuery>,
) -> Result<Json<ComparativeAnalysisResponse>, (StatusCode, String)> {
    let group_data = state
        .db
        .get_grouped_metrics(
            &params.metric,
            params.start_time,
            params.end_time,
            &params.group_by,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total: f64 = group_data.values().map(|(v, _)| v).sum();

    let mut groups = HashMap::new();
    for (group_name, (value, count)) in group_data {
        let percentage = if total != 0.0 {
            (value / total) * 100.0
        } else {
            0.0
        };

        groups.insert(
            group_name,
            GroupMetrics {
                value,
                count,
                percentage,
                trend: "stable".to_string(), // TODO: Calculate trend per group
            },
        );
    }

    Ok(Json(ComparativeAnalysisResponse {
        metric: params.metric,
        groups,
        total,
    }))
}

/// GET /api/v1/analytics/correlation
/// Get correlation analysis between two metrics
pub async fn get_correlation(
    State(state): State<AppState>,
    Query(params): Query<CorrelationQuery>,
) -> Result<Json<CorrelationResponse>, (StatusCode, String)> {
    let data1 = state
        .db
        .get_timeseries_values(&params.metric1, params.start_time, params.end_time)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let data2 = state
        .db
        .get_timeseries_values(&params.metric2, params.start_time, params.end_time)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let coefficient = calculate_correlation(&data1, &data2);
    let relationship = describe_correlation(coefficient);

    // Simple p-value estimation (for demonstration)
    let n = data1.len();
    let t_stat = coefficient * ((n as f64 - 2.0) / (1.0 - coefficient * coefficient)).sqrt();
    let p_value = if t_stat.abs() > 2.0 { 0.05 } else { 0.1 }; // Simplified

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
pub async fn get_latency_breakdown(
    State(state): State<AppState>,
    Query(params): Query<LatencyBreakdownQuery>,
) -> Result<Json<LatencyBreakdown>, (StatusCode, String)> {
    debug!(
        "Getting latency breakdown for session {}",
        params.session_id
    );

    // Query all spans - use query_temporal_range (correct API method)
    let start_time = 0u64;
    let end_time = u64::MAX;
    let edges = state
        .db
        .query_temporal_range(start_time, end_time)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Filter by session
    let session_spans: Vec<AgentFlowEdge> = edges
        .into_iter()
        .filter(|e| e.session_id == params.session_id)
        .collect();

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
pub async fn get_cost_breakdown(
    State(state): State<AppState>,
    Query(params): Query<CostBreakdownQuery>,
) -> Result<Json<CostBreakdown>, (StatusCode, String)> {
    debug!("Getting cost breakdown for session {}", params.session_id);

    let edges = state
        .db
        .query_temporal_range(0, u64::MAX)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let session_spans: Vec<AgentFlowEdge> = edges
        .into_iter()
        .filter(|e| e.session_id == params.session_id)
        .collect();

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
