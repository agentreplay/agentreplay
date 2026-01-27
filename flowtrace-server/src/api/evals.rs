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

// flowtrace-server/src/api/evals.rs
//
// Evaluation metrics API endpoints (Task 3)

use axum::{
    extract::{Query, State},
    Json,
};
use flowtrace_core::{evaluators, EvalMetric};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::query::{ApiError, AppState};

/// Request body for storing evaluation metrics
#[derive(Debug, Deserialize)]
pub struct StoreEvalMetricsRequest {
    /// The edge/trace ID (hex string like "0x1234567890abcdef")
    pub edge_id: String,

    /// List of metrics to store
    pub metrics: Vec<EvalMetricInput>,
}

/// Single evaluation metric input
#[derive(Debug, Deserialize)]
pub struct EvalMetricInput {
    /// Metric name (e.g., "accuracy", "hallucination")
    pub name: String,

    /// Metric value (typically 0.0-1.0)
    pub value: f64,

    /// Evaluator name (e.g., "ragas", "deepeval", "custom")
    #[serde(default = "default_evaluator")]
    pub evaluator: String,
}

fn default_evaluator() -> String {
    evaluators::CUSTOM.to_string()
}

/// Query parameters for retrieving evaluation metrics
#[derive(Debug, Deserialize)]
pub struct GetEvalMetricsParams {
    /// The edge/trace ID (hex string)
    pub edge_id: String,

    /// Optional: filter by metric name
    pub metric_name: Option<String>,

    /// Optional: filter by evaluator
    pub evaluator: Option<String>,
}

/// Response for evaluation metrics
#[derive(Debug, Serialize)]
pub struct EvalMetricsResponse {
    pub edge_id: String,
    pub metrics: Vec<EvalMetricOutput>,
}

/// Single evaluation metric output
#[derive(Debug, Serialize)]
pub struct EvalMetricOutput {
    pub name: String,
    pub value: f64,
    pub evaluator: String,
    pub timestamp_us: u64,
}

impl From<&EvalMetric> for EvalMetricOutput {
    fn from(metric: &EvalMetric) -> Self {
        Self {
            name: metric.get_metric_name().to_string(),
            value: metric.metric_value,
            evaluator: metric.get_evaluator().to_string(),
            timestamp_us: metric.timestamp_us,
        }
    }
}

/// POST /api/v1/evals/metrics - Store evaluation metrics
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:9600/api/v1/evals/metrics \
///   -H "Content-Type: application/json" \
///   -d '{
///     "edge_id": "0x1234567890abcdef",
///     "metrics": [
///       {"name": "accuracy", "value": 0.95, "evaluator": "ragas"},
///       {"name": "hallucination", "value": 0.02, "evaluator": "ragas"}
///     ]
///   }'
/// ```
pub async fn store_eval_metrics(
    State(state): State<AppState>,
    Json(req): Json<StoreEvalMetricsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Parse edge_id
    let edge_id = parse_edge_id(&req.edge_id)?;

    // Get current timestamp
    let timestamp_us = current_timestamp_us();

    // Count metrics before consuming the vector
    let metrics_count = req.metrics.len();

    // Convert input metrics to EvalMetric structs
    let mut eval_metrics = Vec::new();
    for metric_input in req.metrics {
        let eval_metric = EvalMetric::new(
            edge_id,
            &metric_input.name,
            metric_input.value,
            &metric_input.evaluator,
            timestamp_us,
        )
        .ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Metric name or evaluator too long (max 31 chars): {} / {}",
                metric_input.name, metric_input.evaluator
            ))
        })?;

        eval_metrics.push(eval_metric);
    }

    // Store metrics
    state
        .db
        .store_eval_metrics(edge_id, eval_metrics)
        .map_err(|e| ApiError::Internal(format!("Failed to store eval metrics: {}", e)))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "edge_id": format!("0x{:x}", edge_id),
        "metrics_stored": metrics_count,
    })))
}

/// GET /api/v1/evals/metrics - Retrieve evaluation metrics
///
/// # Example
/// ```bash
/// # Get all metrics for a trace
/// curl 'http://localhost:9600/api/v1/evals/metrics?edge_id=0x1234567890abcdef'
///
/// # Get specific metric
/// curl 'http://localhost:9600/api/v1/evals/metrics?edge_id=0x1234567890abcdef&metric_name=accuracy&evaluator=ragas'
/// ```
pub async fn get_eval_metrics(
    State(state): State<AppState>,
    Query(params): Query<GetEvalMetricsParams>,
) -> Result<Json<EvalMetricsResponse>, ApiError> {
    // Parse edge_id
    let edge_id = parse_edge_id(&params.edge_id)?;

    // Get metrics
    let metrics =
        if let (Some(metric_name), Some(evaluator)) = (&params.metric_name, &params.evaluator) {
            // Get specific metric
            state
                .db
                .get_eval_metric(edge_id, metric_name, evaluator)
                .map_err(|e| ApiError::Internal(format!("Failed to get eval metric: {}", e)))?
                .map(|m| vec![m])
                .unwrap_or_default()
        } else {
            // Get all metrics for edge
            state
                .db
                .get_eval_metrics(edge_id)
                .map_err(|e| ApiError::Internal(format!("Failed to get eval metrics: {}", e)))?
        };

    // Convert to output format
    let metrics_output: Vec<EvalMetricOutput> = metrics.iter().map(|m| m.into()).collect();

    Ok(Json(EvalMetricsResponse {
        edge_id: format!("0x{:x}", edge_id),
        metrics: metrics_output,
    }))
}

// =================================================================
// Helper Functions
// =================================================================

/// Parse edge_id from hex string
fn parse_edge_id(edge_id_str: &str) -> Result<u128, ApiError> {
    let hex_str = edge_id_str.trim_start_matches("0x");
    u128::from_str_radix(hex_str, 16)
        .map_err(|_| ApiError::BadRequest(format!("Invalid edge_id format: {}", edge_id_str)))
}

/// Get current timestamp in microseconds
fn current_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

// =================================================================
// Common Metrics Helpers (for documentation/examples)
// =================================================================

/// List of common evaluation metrics
///
/// These are provided as constants for convenience. Users can define custom metrics.
#[allow(dead_code)]
pub mod common_metrics {
    pub use flowtrace_core::metrics::*;
}

/// List of common evaluators
///
/// These are provided as constants for convenience. Users can define custom evaluators.
#[allow(dead_code)]
pub mod common_evaluators {
    pub use flowtrace_core::evaluators::*;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_edge_id() {
        assert_eq!(
            parse_edge_id("0x1234567890abcdef").unwrap(),
            0x1234567890abcdef
        );
        assert_eq!(
            parse_edge_id("1234567890abcdef").unwrap(),
            0x1234567890abcdef
        );
        assert!(parse_edge_id("invalid").is_err());
        assert!(parse_edge_id("0x").is_err());
    }

    #[test]
    fn test_eval_metric_conversion() {
        let eval_metric = EvalMetric::new(
            0x123,
            flowtrace_core::metrics::ACCURACY,
            0.95,
            evaluators::RAGAS,
            1234567890000000,
        )
        .unwrap();

        let output: EvalMetricOutput = (&eval_metric).into();
        assert_eq!(output.name, "accuracy");
        assert_eq!(output.value, 0.95);
        assert_eq!(output.evaluator, "ragas");
        assert_eq!(output.timestamp_us, 1234567890000000);
    }
}
