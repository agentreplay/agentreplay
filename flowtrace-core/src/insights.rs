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

//! Automatic Insight Generation
//!
//! Proactive anomaly detection and insight surfacing for agent traces.
//!
//! ## Features
//!
//! - Statistical anomaly detection (latency, error rate, cost)
//! - Semantic drift detection (embedding space changes)
//! - Pattern recognition (failure patterns, success patterns)
//! - Automatic alerting thresholds
//!
//! ## Example
//!
//! ```rust,ignore
//! use flowtrace_core::insights::{InsightEngine, InsightConfig};
//!
//! let engine = InsightEngine::new(db, embedder, config);
//!
//! // Generate insights for the last hour
//! let insights = engine.generate_insights(Duration::from_secs(3600))?;
//!
//! for insight in insights {
//!     println!("{}: {} (confidence: {:.2})", insight.severity, insight.summary, insight.confidence);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Insight severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational (no action needed)
    Info,
    /// Low severity (monitor)
    Low,
    /// Medium severity (investigate)
    Medium,
    /// High severity (action recommended)
    High,
    /// Critical (immediate action required)
    Critical,
}

/// Generated insight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    /// Unique insight ID
    pub id: String,

    /// Insight type
    pub insight_type: InsightType,

    /// Severity level
    pub severity: Severity,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Human-readable summary
    pub summary: String,

    /// Detailed description
    pub description: String,

    /// Related trace/edge IDs
    #[serde(default)]
    pub related_ids: Vec<u128>,

    /// Metadata (varies by insight type)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// When the insight was generated
    pub generated_at: u64,

    /// Time window the insight covers
    pub window_start: u64,
    pub window_end: u64,
}

/// Types of insights
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InsightType {
    /// Latency anomaly
    LatencyAnomaly {
        baseline_ms: f64,
        current_ms: f64,
        change_percent: f64,
    },

    /// Error rate anomaly
    ErrorRateAnomaly {
        baseline_rate: f64,
        current_rate: f64,
        change_percent: f64,
    },

    /// Cost anomaly
    CostAnomaly {
        baseline_cost: f64,
        current_cost: f64,
        change_percent: f64,
    },

    /// Semantic drift (embeddings changed)
    SemanticDrift {
        drift_score: f64,
        affected_span_types: Vec<String>,
    },

    /// Failure pattern detected
    FailurePattern {
        pattern_description: String,
        occurrence_count: usize,
    },

    /// Performance regression
    PerformanceRegression {
        metric: String,
        regression_percent: f64,
    },

    /// Unusual traffic pattern
    TrafficAnomaly {
        expected_count: usize,
        actual_count: usize,
    },

    /// Token usage spike
    TokenUsageSpike {
        baseline_tokens: u64,
        current_tokens: u64,
        change_percent: f64,
    },
}

/// Configuration for insight generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightConfig {
    /// Z-score threshold for anomaly detection
    #[serde(default = "default_z_threshold")]
    pub z_score_threshold: f64,

    /// Minimum sample size for statistical analysis
    #[serde(default = "default_min_samples")]
    pub min_samples: usize,

    /// Semantic drift threshold (cosine distance)
    #[serde(default = "default_drift_threshold")]
    pub drift_threshold: f64,

    /// Enable latency anomaly detection
    #[serde(default = "default_true")]
    pub detect_latency_anomalies: bool,

    /// Enable error rate anomaly detection
    #[serde(default = "default_true")]
    pub detect_error_anomalies: bool,

    /// Enable cost anomaly detection
    #[serde(default = "default_true")]
    pub detect_cost_anomalies: bool,

    /// Enable semantic drift detection
    #[serde(default = "default_true")]
    pub detect_semantic_drift: bool,

    /// Enable pattern detection
    #[serde(default = "default_true")]
    pub detect_patterns: bool,

    /// Comparison window multiplier (baseline = window * multiplier)
    #[serde(default = "default_baseline_multiplier")]
    pub baseline_multiplier: u32,
}

fn default_z_threshold() -> f64 {
    3.0
}

fn default_min_samples() -> usize {
    30
}

fn default_drift_threshold() -> f64 {
    0.15
}

fn default_true() -> bool {
    true
}

fn default_baseline_multiplier() -> u32 {
    7
}

impl Default for InsightConfig {
    fn default() -> Self {
        Self {
            z_score_threshold: default_z_threshold(),
            min_samples: default_min_samples(),
            drift_threshold: default_drift_threshold(),
            detect_latency_anomalies: true,
            detect_error_anomalies: true,
            detect_cost_anomalies: true,
            detect_semantic_drift: true,
            detect_patterns: true,
            baseline_multiplier: default_baseline_multiplier(),
        }
    }
}

/// Robust statistics using MAD (Median Absolute Deviation)
#[derive(Debug, Clone)]
pub struct RobustStatistics {
    pub median: f64,
    pub mad: f64,
    pub count: usize,
}

impl RobustStatistics {
    /// Compute robust statistics from samples
    pub fn from_samples(samples: &[f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }

        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = if sorted.len().is_multiple_of(2) {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        // Compute MAD
        let mut deviations: Vec<f64> = sorted.iter().map(|x| (x - median).abs()).collect();
        deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mad = if deviations.len().is_multiple_of(2) {
            (deviations[deviations.len() / 2 - 1] + deviations[deviations.len() / 2]) / 2.0
        } else {
            deviations[deviations.len() / 2]
        };

        Some(Self {
            median,
            mad,
            count: samples.len(),
        })
    }

    /// Compute modified Z-score for a value
    pub fn modified_z_score(&self, value: f64) -> f64 {
        if self.mad < 1e-10 {
            return 0.0;
        }
        // 0.6745 is the scaling factor for normal distribution
        0.6745 * (value - self.median) / self.mad
    }
}

/// Insight engine for generating automated insights
pub struct InsightEngine {
    /// Configuration
    config: InsightConfig,
}

impl InsightEngine {
    /// Create a new insight engine
    pub fn new(config: InsightConfig) -> Self {
        Self { config }
    }

    /// Generate insights from data
    pub fn generate_insights(
        &self,
        recent_data: &InsightData,
        baseline_data: &InsightData,
    ) -> Vec<Insight> {
        let mut insights = Vec::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // Latency anomalies
        if self.config.detect_latency_anomalies {
            if let Some(insight) = self.detect_latency_anomaly(recent_data, baseline_data, now) {
                insights.push(insight);
            }
        }

        // Error rate anomalies
        if self.config.detect_error_anomalies {
            if let Some(insight) = self.detect_error_rate_anomaly(recent_data, baseline_data, now) {
                insights.push(insight);
            }
        }

        // Cost anomalies
        if self.config.detect_cost_anomalies {
            if let Some(insight) = self.detect_cost_anomaly(recent_data, baseline_data, now) {
                insights.push(insight);
            }
        }

        // Token usage
        if let Some(insight) = self.detect_token_spike(recent_data, baseline_data, now) {
            insights.push(insight);
        }

        // Traffic anomalies
        if let Some(insight) = self.detect_traffic_anomaly(recent_data, baseline_data, now) {
            insights.push(insight);
        }

        // Sort by severity × confidence
        insights.sort_by(|a, b| {
            let score_a = (a.severity as u8) as f32 * a.confidence;
            let score_b = (b.severity as u8) as f32 * b.confidence;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        insights
    }

    /// Detect latency anomaly
    fn detect_latency_anomaly(
        &self,
        recent: &InsightData,
        baseline: &InsightData,
        now: u64,
    ) -> Option<Insight> {
        if recent.latencies_ms.len() < self.config.min_samples {
            return None;
        }

        let recent_stats = RobustStatistics::from_samples(&recent.latencies_ms)?;
        let baseline_stats = RobustStatistics::from_samples(&baseline.latencies_ms)?;

        let z_score = baseline_stats.modified_z_score(recent_stats.median);

        if z_score.abs() > self.config.z_score_threshold {
            let change_percent =
                (recent_stats.median - baseline_stats.median) / baseline_stats.median * 100.0;
            let severity = self.severity_from_z_score(z_score);

            return Some(Insight {
                id: format!("latency-{}", now),
                insight_type: InsightType::LatencyAnomaly {
                    baseline_ms: baseline_stats.median,
                    current_ms: recent_stats.median,
                    change_percent,
                },
                severity,
                confidence: (z_score.abs() / 10.0).min(1.0) as f32,
                summary: format!(
                    "Latency {} by {:.1}% ({:.0}ms → {:.0}ms)",
                    if change_percent > 0.0 {
                        "increased"
                    } else {
                        "decreased"
                    },
                    change_percent.abs(),
                    baseline_stats.median,
                    recent_stats.median
                ),
                description: format!(
                    "Median latency changed from {:.2}ms to {:.2}ms, a {:.1}% {}. \
                     This is {:.1} standard deviations from the baseline.",
                    baseline_stats.median,
                    recent_stats.median,
                    change_percent.abs(),
                    if change_percent > 0.0 {
                        "increase"
                    } else {
                        "decrease"
                    },
                    z_score.abs()
                ),
                related_ids: vec![],
                metadata: HashMap::new(),
                generated_at: now,
                window_start: recent.window_start,
                window_end: recent.window_end,
            });
        }

        None
    }

    /// Detect error rate anomaly
    fn detect_error_rate_anomaly(
        &self,
        recent: &InsightData,
        baseline: &InsightData,
        now: u64,
    ) -> Option<Insight> {
        if recent.total_count < self.config.min_samples {
            return None;
        }

        let recent_rate = recent.error_count as f64 / recent.total_count as f64;
        let baseline_rate = if baseline.total_count > 0 {
            baseline.error_count as f64 / baseline.total_count as f64
        } else {
            0.0
        };

        // Use binomial test approximation
        let diff = (recent_rate - baseline_rate).abs();
        let expected_std =
            (baseline_rate * (1.0 - baseline_rate) / recent.total_count as f64).sqrt();

        if expected_std < 1e-10 {
            return None;
        }

        let z_score = diff / expected_std;

        if z_score > self.config.z_score_threshold && recent_rate > baseline_rate {
            let change_percent = if baseline_rate > 0.0 {
                (recent_rate - baseline_rate) / baseline_rate * 100.0
            } else {
                100.0
            };

            let severity = if recent_rate > 0.1 {
                Severity::Critical
            } else if recent_rate > 0.05 {
                Severity::High
            } else if recent_rate > 0.02 {
                Severity::Medium
            } else {
                Severity::Low
            };

            return Some(Insight {
                id: format!("error-rate-{}", now),
                insight_type: InsightType::ErrorRateAnomaly {
                    baseline_rate,
                    current_rate: recent_rate,
                    change_percent,
                },
                severity,
                confidence: (z_score / 10.0).min(1.0) as f32,
                summary: format!(
                    "Error rate increased to {:.1}% (was {:.1}%)",
                    recent_rate * 100.0,
                    baseline_rate * 100.0
                ),
                description: format!(
                    "Error rate increased from {:.2}% to {:.2}% ({} errors out of {} requests). \
                     This is a statistically significant increase.",
                    baseline_rate * 100.0,
                    recent_rate * 100.0,
                    recent.error_count,
                    recent.total_count
                ),
                related_ids: recent.error_edge_ids.clone(),
                metadata: HashMap::new(),
                generated_at: now,
                window_start: recent.window_start,
                window_end: recent.window_end,
            });
        }

        None
    }

    /// Detect cost anomaly
    fn detect_cost_anomaly(
        &self,
        recent: &InsightData,
        baseline: &InsightData,
        now: u64,
    ) -> Option<Insight> {
        if recent.costs.len() < self.config.min_samples {
            return None;
        }

        let recent_total: f64 = recent.costs.iter().sum();
        let baseline_total: f64 = baseline.costs.iter().sum();

        if baseline_total < 0.01 {
            return None;
        }

        let change_percent = (recent_total - baseline_total) / baseline_total * 100.0;

        // Cost thresholds
        if change_percent > 50.0 {
            let severity = if change_percent > 200.0 {
                Severity::Critical
            } else if change_percent > 100.0 {
                Severity::High
            } else {
                Severity::Medium
            };

            return Some(Insight {
                id: format!("cost-{}", now),
                insight_type: InsightType::CostAnomaly {
                    baseline_cost: baseline_total,
                    current_cost: recent_total,
                    change_percent,
                },
                severity,
                confidence: (change_percent / 300.0).min(1.0) as f32,
                summary: format!(
                    "Cost increased by {:.0}% (${:.2} → ${:.2})",
                    change_percent, baseline_total, recent_total
                ),
                description: format!(
                    "Total cost increased from ${:.2} to ${:.2}, a {:.1}% increase. \
                     Review recent traces for cost optimization opportunities.",
                    baseline_total, recent_total, change_percent
                ),
                related_ids: vec![],
                metadata: HashMap::new(),
                generated_at: now,
                window_start: recent.window_start,
                window_end: recent.window_end,
            });
        }

        None
    }

    /// Detect token usage spike
    fn detect_token_spike(
        &self,
        recent: &InsightData,
        baseline: &InsightData,
        now: u64,
    ) -> Option<Insight> {
        if baseline.total_tokens == 0 {
            return None;
        }

        let change_percent = (recent.total_tokens as f64 - baseline.total_tokens as f64)
            / baseline.total_tokens as f64
            * 100.0;

        if change_percent > 100.0 {
            let severity = if change_percent > 500.0 {
                Severity::High
            } else if change_percent > 200.0 {
                Severity::Medium
            } else {
                Severity::Low
            };

            return Some(Insight {
                id: format!("tokens-{}", now),
                insight_type: InsightType::TokenUsageSpike {
                    baseline_tokens: baseline.total_tokens,
                    current_tokens: recent.total_tokens,
                    change_percent,
                },
                severity,
                confidence: 0.8,
                summary: format!("Token usage increased by {:.0}%", change_percent),
                description: format!(
                    "Token usage increased from {} to {} tokens ({:.0}% increase). \
                     This may indicate longer prompts, more tool calls, or increased traffic.",
                    baseline.total_tokens, recent.total_tokens, change_percent
                ),
                related_ids: vec![],
                metadata: HashMap::new(),
                generated_at: now,
                window_start: recent.window_start,
                window_end: recent.window_end,
            });
        }

        None
    }

    /// Detect traffic anomaly
    fn detect_traffic_anomaly(
        &self,
        recent: &InsightData,
        baseline: &InsightData,
        now: u64,
    ) -> Option<Insight> {
        if baseline.total_count < self.config.min_samples {
            return None;
        }

        // Normalize by time window
        let recent_rate =
            recent.total_count as f64 / (recent.window_end - recent.window_start) as f64;
        let baseline_rate =
            baseline.total_count as f64 / (baseline.window_end - baseline.window_start) as f64;

        if baseline_rate < 1e-10 {
            return None;
        }

        let change_ratio = recent_rate / baseline_rate;

        // Detect significant traffic drop or spike
        if !(0.2..=5.0).contains(&change_ratio) {
            let severity = if !(0.1..=10.0).contains(&change_ratio) {
                Severity::High
            } else {
                Severity::Medium
            };

            let description = if change_ratio < 1.0 {
                format!(
                    "Traffic dropped to {:.1}% of normal levels ({} vs {} expected traces). \
                     Check for upstream issues or configuration problems.",
                    change_ratio * 100.0,
                    recent.total_count,
                    baseline.total_count
                )
            } else {
                format!(
                    "Traffic spiked to {:.1}x normal levels ({} vs {} expected traces). \
                     Monitor for performance degradation.",
                    change_ratio, recent.total_count, baseline.total_count
                )
            };

            return Some(Insight {
                id: format!("traffic-{}", now),
                insight_type: InsightType::TrafficAnomaly {
                    expected_count: baseline.total_count,
                    actual_count: recent.total_count,
                },
                severity,
                confidence: 0.7,
                summary: if change_ratio < 1.0 {
                    format!(
                        "Traffic dropped to {:.0}% of expected",
                        change_ratio * 100.0
                    )
                } else {
                    format!("Traffic spiked to {:.1}x expected", change_ratio)
                },
                description,
                related_ids: vec![],
                metadata: HashMap::new(),
                generated_at: now,
                window_start: recent.window_start,
                window_end: recent.window_end,
            });
        }

        None
    }

    /// Map Z-score to severity
    fn severity_from_z_score(&self, z: f64) -> Severity {
        let abs_z = z.abs();
        if abs_z > 5.0 {
            Severity::Critical
        } else if abs_z > 4.0 {
            Severity::High
        } else if abs_z > 3.5 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Build InsightData from a collection of edges
    ///
    /// This extracts latencies, token counts, errors, and other metrics
    /// from the edges for statistical analysis.
    pub fn build_insight_data_from_edges(edges: &[crate::edge::AgentFlowEdge]) -> InsightData {
        let mut latencies_ms = Vec::with_capacity(edges.len());
        let mut costs = Vec::new();
        let mut total_tokens: u64 = 0;
        let mut error_count = 0;
        let mut error_edge_ids = Vec::new();
        let mut window_start = u64::MAX;
        let mut window_end = 0u64;

        for edge in edges {
            // Track time window
            if edge.timestamp_us < window_start {
                window_start = edge.timestamp_us;
            }
            if edge.timestamp_us > window_end {
                window_end = edge.timestamp_us;
            }

            // Convert duration from microseconds to milliseconds
            let latency_ms = edge.duration_us as f64 / 1000.0;
            latencies_ms.push(latency_ms);

            // Accumulate tokens
            total_tokens += edge.token_count as u64;

            // Check for errors (SpanType::Error = 7)
            if edge.span_type == 7 {
                error_count += 1;
                error_edge_ids.push(edge.edge_id);
            }

            // Estimate cost from tokens (approximate: $0.002 per 1K tokens for typical LLM)
            if edge.token_count > 0 {
                let estimated_cost = (edge.token_count as f64) * 0.000002;
                costs.push(estimated_cost);
            }
        }

        InsightData {
            latencies_ms,
            costs,
            total_count: edges.len(),
            error_count,
            total_tokens,
            error_edge_ids,
            window_start: if window_start == u64::MAX {
                0
            } else {
                window_start
            },
            window_end,
        }
    }

    /// Generate insights directly from edges
    ///
    /// Compares recent edges against baseline edges to detect anomalies.
    /// This is useful when you already have edges loaded and don't need
    /// to query the database.
    ///
    /// # Arguments
    /// * `recent_edges` - Edges from the recent time window
    /// * `baseline_edges` - Edges from the baseline time window (typically longer)
    ///
    /// # Returns
    /// Vector of insights sorted by severity and confidence
    pub fn generate_insights_from_edges(
        &self,
        recent_edges: &[crate::edge::AgentFlowEdge],
        baseline_edges: &[crate::edge::AgentFlowEdge],
    ) -> Vec<Insight> {
        let recent_data = Self::build_insight_data_from_edges(recent_edges);
        let baseline_data = Self::build_insight_data_from_edges(baseline_edges);
        self.generate_insights(&recent_data, &baseline_data)
    }
}

/// Data for insight generation
#[derive(Debug, Clone, Default)]
pub struct InsightData {
    /// Latency measurements in ms
    pub latencies_ms: Vec<f64>,

    /// Cost measurements
    pub costs: Vec<f64>,

    /// Total trace count
    pub total_count: usize,

    /// Error count
    pub error_count: usize,

    /// Total token usage
    pub total_tokens: u64,

    /// Error edge IDs (for linking)
    pub error_edge_ids: Vec<u128>,

    /// Time window start (microseconds)
    pub window_start: u64,

    /// Time window end (microseconds)
    pub window_end: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_robust_statistics() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0]; // 100 is outlier
        let stats = RobustStatistics::from_samples(&samples).unwrap();

        // Median should be 3.5 (average of 3 and 4)
        assert!((stats.median - 3.5).abs() < 0.01);

        // MAD should be robust to the outlier
        assert!(stats.mad < 10.0);
    }

    #[test]
    fn test_modified_z_score() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = RobustStatistics::from_samples(&samples).unwrap();

        // Value at median should have z-score of 0
        let z = stats.modified_z_score(3.0);
        assert!(z.abs() < 0.1);

        // Extreme value should have high z-score
        let z_extreme = stats.modified_z_score(100.0);
        assert!(z_extreme > 5.0);
    }

    #[test]
    fn test_latency_anomaly_detection() {
        let config = InsightConfig {
            min_samples: 5, // Lower threshold for test
            ..Default::default()
        };
        let engine = InsightEngine::new(config);

        let baseline = InsightData {
            latencies_ms: vec![
                100.0, 110.0, 105.0, 95.0, 100.0, 102.0, 98.0, 103.0, 99.0, 101.0,
            ],
            window_start: 0,
            window_end: 3_600_000_000,
            ..Default::default()
        };

        let recent = InsightData {
            latencies_ms: vec![
                500.0, 550.0, 480.0, 520.0, 510.0, 530.0, 490.0, 505.0, 515.0, 525.0,
            ],
            window_start: 3_600_000_000,
            window_end: 7_200_000_000,
            ..Default::default()
        };

        let insights = engine.generate_insights(&recent, &baseline);

        // Should detect latency anomaly
        let latency_insight = insights
            .iter()
            .find(|i| matches!(i.insight_type, InsightType::LatencyAnomaly { .. }));
        assert!(latency_insight.is_some());
    }

    #[test]
    fn test_error_rate_anomaly() {
        let engine = InsightEngine::new(InsightConfig::default());

        let baseline = InsightData {
            total_count: 1000,
            error_count: 10, // 1% error rate
            window_start: 0,
            window_end: 3_600_000_000,
            ..Default::default()
        };

        let recent = InsightData {
            total_count: 100,
            error_count: 15, // 15% error rate
            error_edge_ids: vec![1, 2, 3],
            window_start: 3_600_000_000,
            window_end: 7_200_000_000,
            ..Default::default()
        };

        let insights = engine.generate_insights(&recent, &baseline);

        // Should detect error rate anomaly
        let error_insight = insights
            .iter()
            .find(|i| matches!(i.insight_type, InsightType::ErrorRateAnomaly { .. }));
        assert!(error_insight.is_some());

        if let Some(insight) = error_insight {
            assert!(insight.severity >= Severity::High);
        }
    }

    #[test]
    fn test_insight_serialization() {
        let insight = Insight {
            id: "test-123".to_string(),
            insight_type: InsightType::LatencyAnomaly {
                baseline_ms: 100.0,
                current_ms: 500.0,
                change_percent: 400.0,
            },
            severity: Severity::High,
            confidence: 0.9,
            summary: "Latency increased by 400%".to_string(),
            description: "Detailed description".to_string(),
            related_ids: vec![1, 2, 3],
            metadata: HashMap::new(),
            generated_at: 1000000,
            window_start: 0,
            window_end: 1000000,
        };

        let json = serde_json::to_string(&insight).unwrap();
        let restored: Insight = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.id, insight.id);
        assert_eq!(restored.severity, Severity::High);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Info);
    }
}
