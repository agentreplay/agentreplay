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

// flowtrace-server/src/api/insights.rs
//
// Insights API endpoints for anomaly detection and pattern recognition

use super::query::{ApiError, AppState};
use axum::{
    extract::{Query, State},
    Json,
};
use flowtrace_core::insights::{Insight, InsightConfig, InsightEngine, InsightType, Severity};
use serde::{Deserialize, Serialize};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct InsightsQuery {
    /// Time window in seconds (default: 3600 = 1 hour)
    #[serde(default = "default_window_seconds")]
    pub window_seconds: u64,
    /// Minimum severity to include
    #[serde(default)]
    pub min_severity: Option<String>,
    /// Filter by insight type
    #[serde(default)]
    pub insight_type: Option<String>,
    /// Project ID filter
    #[serde(default)]
    pub project_id: Option<u16>,
    /// Maximum number of insights to return
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_window_seconds() -> u64 {
    3600 // 1 hour
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Serialize)]
pub struct InsightsResponse {
    pub insights: Vec<InsightView>,
    pub total_count: usize,
    pub window_seconds: u64,
    pub generated_at: u64,
}

#[derive(Debug, Serialize)]
pub struct InsightView {
    pub id: String,
    pub insight_type: String,
    pub severity: String,
    pub confidence: f32,
    pub summary: String,
    pub description: String,
    pub related_trace_ids: Vec<String>,
    pub metadata: serde_json::Value,
    pub generated_at: u64,
    /// Suggested actions
    pub suggestions: Vec<String>,
}

impl From<Insight> for InsightView {
    fn from(insight: Insight) -> Self {
        let (insight_type_str, suggestions) = match &insight.insight_type {
            InsightType::LatencyAnomaly {
                baseline_ms,
                current_ms,
                change_percent,
            } => (
                "latency_anomaly".to_string(),
                vec![
                    format!(
                        "Latency increased by {:.1}% ({:.0}ms → {:.0}ms)",
                        change_percent, baseline_ms, current_ms
                    ),
                    "Consider: caching, connection pooling, or reducing payload size".to_string(),
                    "Check for slow database queries or external API calls".to_string(),
                ],
            ),
            InsightType::ErrorRateAnomaly {
                baseline_rate,
                current_rate,
                change_percent,
            } => (
                "error_rate_anomaly".to_string(),
                vec![
                    format!(
                        "Error rate increased by {:.1}% ({:.2}% → {:.2}%)",
                        change_percent,
                        baseline_rate * 100.0,
                        current_rate * 100.0
                    ),
                    "Review recent deployments or configuration changes".to_string(),
                    "Check external dependency health".to_string(),
                ],
            ),
            InsightType::CostAnomaly {
                baseline_cost,
                current_cost,
                change_percent,
            } => (
                "cost_anomaly".to_string(),
                vec![
                    format!(
                        "Cost increased by {:.1}% (${:.4} → ${:.4})",
                        change_percent, baseline_cost, current_cost
                    ),
                    "Consider: prompt optimization, caching, or model switching".to_string(),
                    "Review token usage patterns".to_string(),
                ],
            ),
            InsightType::SemanticDrift {
                drift_score,
                affected_span_types,
            } => (
                "semantic_drift".to_string(),
                vec![
                    format!("Agent behavior has drifted (score: {:.2})", drift_score),
                    format!("Affected span types: {}", affected_span_types.join(", ")),
                    "Compare recent traces with baseline to understand changes".to_string(),
                ],
            ),
            InsightType::FailurePattern {
                pattern_description,
                occurrence_count,
            } => (
                "failure_pattern".to_string(),
                vec![
                    format!(
                        "Pattern detected {} times: {}",
                        occurrence_count, pattern_description
                    ),
                    "Review failing traces for common root causes".to_string(),
                ],
            ),
            InsightType::PerformanceRegression {
                metric,
                regression_percent,
            } => (
                "performance_regression".to_string(),
                vec![
                    format!("{} regressed by {:.1}%", metric, regression_percent),
                    "Compare recent code changes or dependency updates".to_string(),
                ],
            ),
            InsightType::TrafficAnomaly {
                expected_count,
                actual_count,
            } => (
                "traffic_anomaly".to_string(),
                vec![
                    format!(
                        "Unusual traffic: expected ~{}, got {}",
                        expected_count, actual_count
                    ),
                    "Check for bot traffic or usage spikes".to_string(),
                ],
            ),
            InsightType::TokenUsageSpike {
                baseline_tokens,
                current_tokens,
                change_percent,
            } => (
                "token_usage_spike".to_string(),
                vec![
                    format!(
                        "Token usage increased by {:.1}% ({} → {})",
                        change_percent, baseline_tokens, current_tokens
                    ),
                    "Review prompt lengths and context windows".to_string(),
                    "Consider prompt compression or summarization".to_string(),
                ],
            ),
        };

        InsightView {
            id: insight.id,
            insight_type: insight_type_str,
            severity: format!("{:?}", insight.severity).to_lowercase(),
            confidence: insight.confidence,
            summary: insight.summary,
            description: insight.description,
            related_trace_ids: insight
                .related_ids
                .iter()
                .map(|id| format!("{:#x}", id))
                .collect(),
            metadata: serde_json::to_value(&insight.metadata).unwrap_or_default(),
            generated_at: insight.generated_at,
            suggestions,
        }
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// GET /api/v1/insights
///
/// Generate and return insights for the specified time window
pub async fn get_insights(
    State(state): State<AppState>,
    Query(query): Query<InsightsQuery>,
) -> Result<Json<InsightsResponse>, ApiError> {
    let config = InsightConfig::default();
    let baseline_multiplier = config.baseline_multiplier as u64;
    let engine = InsightEngine::new(config);

    // Get traces from the time window
    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let recent_start_us = now_us.saturating_sub(query.window_seconds * 1_000_000);

    // Baseline is 7x the window (configurable via InsightConfig)
    let baseline_start_us =
        recent_start_us.saturating_sub(query.window_seconds * 1_000_000 * baseline_multiplier);

    // Query recent edges
    let recent_edges = state
        .db
        .query_temporal_range(recent_start_us, now_us)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Query baseline edges
    let baseline_edges = state
        .db
        .query_temporal_range(baseline_start_us, recent_start_us)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Generate insights by comparing recent vs baseline
    let insights = engine.generate_insights_from_edges(&recent_edges, &baseline_edges);

    // Apply filters
    let min_severity = query.min_severity.as_ref().and_then(|s| parse_severity(s));

    let filtered: Vec<InsightView> = insights
        .into_iter()
        .filter(|i| {
            if let Some(min_sev) = min_severity {
                i.severity >= min_sev
            } else {
                true
            }
        })
        .filter(|i| {
            if let Some(ref type_filter) = query.insight_type {
                match_insight_type(&i.insight_type, type_filter)
            } else {
                true
            }
        })
        .take(query.limit)
        .map(InsightView::from)
        .collect();

    let total_count = filtered.len();

    Ok(Json(InsightsResponse {
        insights: filtered,
        total_count,
        window_seconds: query.window_seconds,
        generated_at: now_us,
    }))
}

/// GET /api/v1/insights/summary
///
/// Get a summary of current insights by severity
pub async fn get_insights_summary(
    State(state): State<AppState>,
) -> Result<Json<InsightsSummary>, ApiError> {
    let config = InsightConfig::default();
    let engine = InsightEngine::new(config);

    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);

    // Recent = last hour
    let recent_start_us = now_us.saturating_sub(3600 * 1_000_000);

    // Baseline = 7 hours before the recent window
    let baseline_start_us = recent_start_us.saturating_sub(7 * 3600 * 1_000_000);

    let recent_edges = state
        .db
        .query_temporal_range(recent_start_us, now_us)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let baseline_edges = state
        .db
        .query_temporal_range(baseline_start_us, recent_start_us)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let insights = engine.generate_insights_from_edges(&recent_edges, &baseline_edges);

    let mut by_severity = std::collections::HashMap::new();
    let mut by_type = std::collections::HashMap::new();

    for insight in &insights {
        *by_severity
            .entry(format!("{:?}", insight.severity).to_lowercase())
            .or_insert(0) += 1;
        *by_type
            .entry(insight_type_name(&insight.insight_type))
            .or_insert(0) += 1;
    }

    let critical_count = insights
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    let high_count = insights
        .iter()
        .filter(|i| i.severity == Severity::High)
        .count();

    Ok(Json(InsightsSummary {
        total_insights: insights.len(),
        critical_count,
        high_count,
        by_severity,
        by_type,
        health_score: calculate_health_score(&insights),
        top_insights: insights
            .into_iter()
            .filter(|i| i.severity >= Severity::Medium)
            .take(5)
            .map(InsightView::from)
            .collect(),
    }))
}

#[derive(Debug, Serialize)]
pub struct InsightsSummary {
    pub total_insights: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub by_severity: std::collections::HashMap<String, usize>,
    pub by_type: std::collections::HashMap<String, usize>,
    /// Health score 0-100 (100 = no issues)
    pub health_score: u8,
    /// Top 5 most important insights
    pub top_insights: Vec<InsightView>,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_severity(s: &str) -> Option<Severity> {
    match s.to_lowercase().as_str() {
        "info" => Some(Severity::Info),
        "low" => Some(Severity::Low),
        "medium" => Some(Severity::Medium),
        "high" => Some(Severity::High),
        "critical" => Some(Severity::Critical),
        _ => None,
    }
}

fn match_insight_type(insight: &InsightType, filter: &str) -> bool {
    let type_name = insight_type_name(insight);
    type_name.contains(&filter.to_lowercase())
}

fn insight_type_name(insight: &InsightType) -> String {
    match insight {
        InsightType::LatencyAnomaly { .. } => "latency_anomaly",
        InsightType::ErrorRateAnomaly { .. } => "error_rate_anomaly",
        InsightType::CostAnomaly { .. } => "cost_anomaly",
        InsightType::SemanticDrift { .. } => "semantic_drift",
        InsightType::FailurePattern { .. } => "failure_pattern",
        InsightType::PerformanceRegression { .. } => "performance_regression",
        InsightType::TrafficAnomaly { .. } => "traffic_anomaly",
        InsightType::TokenUsageSpike { .. } => "token_usage_spike",
    }
    .to_string()
}

fn calculate_health_score(insights: &[Insight]) -> u8 {
    if insights.is_empty() {
        return 100;
    }

    let mut penalty = 0;
    for insight in insights {
        penalty += match insight.severity {
            Severity::Critical => 30,
            Severity::High => 15,
            Severity::Medium => 5,
            Severity::Low => 2,
            Severity::Info => 0,
        };
    }

    100u8.saturating_sub(penalty.min(100) as u8)
}
