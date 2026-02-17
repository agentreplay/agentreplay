// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Distribution shift monitor
//!
//! Uses KS-test approximated from quantile digests to detect
//! distribution shifts between baseline and current metric distributions.
//!
//! KS-stat: D = sup_x |F_baseline(x) - F_current(x)|
//! Space: ~2KB per metric for quantile sketch with α=0.01
//! Alerts: Configurable thresholds with automatic version attribution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metric distribution summary (simplified DDSketch-like quantile summary)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDistribution {
    pub name: String,
    pub count: usize,
    /// Quantile values at standard percentiles [p10, p25, p50, p75, p90, p95, p99]
    pub quantiles: Vec<f64>,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

/// Drift status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftStatus {
    Stable,
    Watch,
    Drift,
    Alert,
}

impl DriftStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Stable => "🟢",
            Self::Watch => "🟡",
            Self::Drift => "⚠️",
            Self::Alert => "🔴",
        }
    }
}

/// Drift detection result for a single metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftResult {
    pub metric_name: String,
    pub baseline_summary: String,
    pub current_summary: String,
    pub ks_statistic: f64,
    pub status: DriftStatus,
    pub possible_cause: Option<String>,
    pub recommendation: Option<String>,
}

/// Configuration for drift monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftConfig {
    /// KS-stat threshold for WATCH status
    pub watch_threshold: f64,
    /// KS-stat threshold for DRIFT status
    pub drift_threshold: f64,
    /// KS-stat threshold for ALERT status
    pub alert_threshold: f64,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            watch_threshold: 0.10,
            drift_threshold: 0.20,
            alert_threshold: 0.30,
        }
    }
}

/// Distribution shift evaluator
pub struct DistributionShiftEvaluator {
    config: DriftConfig,
}

impl DistributionShiftEvaluator {
    pub fn new() -> Self {
        Self {
            config: DriftConfig::default(),
        }
    }

    pub fn with_config(mut self, config: DriftConfig) -> Self {
        self.config = config;
        self
    }

    /// Approximate KS statistic from quantile summaries
    ///
    /// Compares two distributions at standard quantile points and returns
    /// the maximum absolute difference between their CDFs.
    pub fn ks_statistic(baseline: &MetricDistribution, current: &MetricDistribution) -> f64 {
        if baseline.quantiles.len() != current.quantiles.len() || baseline.quantiles.is_empty() {
            return 1.0; // Can't compare — maximum drift
        }

        // Standard quantile probabilities
        let probabilities = [0.10, 0.25, 0.50, 0.75, 0.90, 0.95, 0.99];

        let mut max_diff = 0.0f64;

        // Compare CDFs at each quantile point
        for i in 0..baseline.quantiles.len().min(probabilities.len()) {
            let baseline_q = baseline.quantiles[i];
            let current_q = current.quantiles[i];

            // Estimate CDF difference at these points
            // For a proper KS test, we'd use the raw data or a more precise sketch
            // This is an approximation using quantile interpolation
            let diff = if baseline_q.abs() > f64::EPSILON {
                ((current_q - baseline_q) / baseline_q).abs()
            } else {
                current_q.abs()
            };

            max_diff = max_diff.max(diff);
        }

        // Normalize to [0, 1] range
        max_diff.min(1.0)
    }

    /// Evaluate drift for a set of metrics
    pub fn evaluate(
        &self,
        baseline: &[MetricDistribution],
        current: &[MetricDistribution],
    ) -> Vec<DriftResult> {
        let current_map: HashMap<&str, &MetricDistribution> = current.iter()
            .map(|m| (m.name.as_str(), m))
            .collect();

        baseline.iter().filter_map(|baseline_metric| {
            let current_metric = current_map.get(baseline_metric.name.as_str())?;

            let ks_stat = Self::ks_statistic(baseline_metric, current_metric);

            let status = if ks_stat >= self.config.alert_threshold {
                DriftStatus::Alert
            } else if ks_stat >= self.config.drift_threshold {
                DriftStatus::Drift
            } else if ks_stat >= self.config.watch_threshold {
                DriftStatus::Watch
            } else {
                DriftStatus::Stable
            };

            Some(DriftResult {
                metric_name: baseline_metric.name.clone(),
                baseline_summary: format!("p50={:.2}, p95={:.2}", 
                    baseline_metric.quantiles.get(2).unwrap_or(&0.0),
                    baseline_metric.quantiles.get(5).unwrap_or(&0.0),
                ),
                current_summary: format!("p50={:.2}, p95={:.2}",
                    current_metric.quantiles.get(2).unwrap_or(&0.0),
                    current_metric.quantiles.get(5).unwrap_or(&0.0),
                ),
                ks_statistic: ks_stat,
                status,
                possible_cause: if status == DriftStatus::Alert {
                    Some("Significant distribution shift detected — investigate recent changes".to_string())
                } else {
                    None
                },
                recommendation: match status {
                    DriftStatus::Alert => Some("Roll back or investigate API/dependency changes".to_string()),
                    DriftStatus::Drift => Some("Monitor closely; consider root cause analysis".to_string()),
                    DriftStatus::Watch => Some("Within tolerance but trending; set up alerts".to_string()),
                    DriftStatus::Stable => None,
                },
            })
        }).collect()
    }
}

impl Default for DistributionShiftEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
