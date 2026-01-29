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

// agentreplay-evals/src/evaluators/anomaly.rs
//
// Anomaly detection evaluator using statistical methods
//
// ## Features
//
// - **EWMA Statistics**: Exponentially Weighted Moving Average for temporal decay
// - **Seasonal Patterns**: Hourly/daily bucket tracking (168 buckets for week)
// - **Persistent State**: Can save/restore baselines to avoid cold start
// - **O(1) Updates**: Welford's algorithm for incremental statistics

use crate::{EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Number of seasonal buckets (24 hours × 7 days = 168)
const SEASONAL_BUCKETS: usize = 168;

/// Persisted anomaly detection state for baseline hydration
///
/// This structure can be serialized/deserialized to avoid cold start problems.
/// All statistics use O(1) incremental updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedAnomalyState {
    // EWMA statistics for each metric
    pub latency_mean: f64,
    pub latency_var: f64,
    pub token_mean: f64,
    pub token_var: f64,
    pub cost_mean: f64,
    pub cost_var: f64,
    pub error_mean: f64,
    pub error_var: f64,

    // Sample count for EWMA alpha calculation
    pub count: u64,

    // Welford's algorithm state for precise variance
    pub m2_latency: f64,
    pub m2_tokens: f64,
    pub m2_cost: f64,

    // Seasonal buckets for hourly/daily patterns (168 = 24h × 7d)
    pub seasonal_latency_means: Vec<f64>,
    pub seasonal_latency_counts: Vec<u64>,

    // Metadata
    pub last_update_us: i64,
    pub version: u32,
}

impl Default for PersistedAnomalyState {
    fn default() -> Self {
        Self {
            latency_mean: 0.0,
            latency_var: 0.0,
            token_mean: 0.0,
            token_var: 0.0,
            cost_mean: 0.0,
            cost_var: 0.0,
            error_mean: 0.0,
            error_var: 0.0,
            count: 0,
            m2_latency: 0.0,
            m2_tokens: 0.0,
            m2_cost: 0.0,
            seasonal_latency_means: vec![0.0; SEASONAL_BUCKETS],
            seasonal_latency_counts: vec![0; SEASONAL_BUCKETS],
            last_update_us: 0,
            version: 1,
        }
    }
}

impl PersistedAnomalyState {
    /// Update state with a new sample using EWMA
    pub fn update(&mut self, latency_ms: f64, tokens: f64, cost: f64, error_rate: f64) {
        self.count += 1;

        // Adaptive alpha: starts high (fast learning) and decreases (stable baseline)
        // Alpha = 2/(N+1), capped at 1000 samples for minimum alpha of 0.002
        let alpha = 2.0 / (self.count as f64 + 1.0).min(1000.0);

        // EWMA update for latency
        let delta_latency = latency_ms - self.latency_mean;
        self.latency_mean += alpha * delta_latency;
        self.latency_var =
            (1.0 - alpha) * (self.latency_var + alpha * delta_latency * delta_latency);

        // EWMA update for tokens
        let delta_tokens = tokens - self.token_mean;
        self.token_mean += alpha * delta_tokens;
        self.token_var = (1.0 - alpha) * (self.token_var + alpha * delta_tokens * delta_tokens);

        // EWMA update for cost
        let delta_cost = cost - self.cost_mean;
        self.cost_mean += alpha * delta_cost;
        self.cost_var = (1.0 - alpha) * (self.cost_var + alpha * delta_cost * delta_cost);

        // EWMA update for error rate
        let delta_error = error_rate - self.error_mean;
        self.error_mean += alpha * delta_error;
        self.error_var = (1.0 - alpha) * (self.error_var + alpha * delta_error * delta_error);

        // Welford's algorithm for precise variance (used for comparison)
        self.m2_latency += delta_latency * (latency_ms - self.latency_mean);
        self.m2_tokens += delta_tokens * (tokens - self.token_mean);
        self.m2_cost += delta_cost * (cost - self.cost_mean);

        self.last_update_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as i64)
            .unwrap_or(0);
    }

    /// Update seasonal bucket for hourly/daily patterns
    pub fn update_seasonal(&mut self, latency_ms: f64, hour: usize, weekday: usize) {
        let bucket = hour + (weekday * 24);
        if bucket < SEASONAL_BUCKETS {
            // Exponential decay for seasonal means (alpha = 0.01 for slow adaptation)
            self.seasonal_latency_means[bucket] =
                self.seasonal_latency_means[bucket] * 0.99 + latency_ms * 0.01;
            self.seasonal_latency_counts[bucket] += 1;
        }
    }

    /// Compute z-score with seasonal adjustment
    pub fn z_score(&self, value: f64, hour: Option<usize>, weekday: Option<usize>) -> f64 {
        let std_dev = self.latency_var.sqrt().max(1e-10);

        // Apply seasonal adjustment if time info available
        let adjusted_mean = match (hour, weekday) {
            (Some(h), Some(w)) => {
                let bucket = h + (w * 24);
                if bucket < SEASONAL_BUCKETS && self.seasonal_latency_counts[bucket] > 10 {
                    // Blend global and seasonal means
                    0.7 * self.latency_mean + 0.3 * self.seasonal_latency_means[bucket]
                } else {
                    self.latency_mean
                }
            }
            _ => self.latency_mean,
        };

        (value - adjusted_mean) / std_dev
    }

    /// Check if the baseline has enough samples for reliable detection
    pub fn is_warmed_up(&self) -> bool {
        self.count >= 30 // Minimum samples for statistical significance
    }
}

/// Anomaly detection evaluator
///
/// Detects anomalies in trace metrics using statistical methods:
/// - Z-score analysis with EWMA for temporal decay
/// - Seasonal pattern detection (hourly/daily)
/// - IQR method for cost outliers
/// - Persistent baseline to avoid cold start
#[derive(Clone)]
pub struct AnomalyDetector {
    /// Persisted state for EWMA statistics
    persisted_state: Arc<tokio::sync::RwLock<PersistedAnomalyState>>,

    /// Legacy historical data for IQR calculations
    historical_data: Arc<tokio::sync::RwLock<HistoricalData>>,

    /// Sensitivity threshold (number of standard deviations)
    sensitivity: f64,

    /// Whether to use EWMA-based detection (vs legacy)
    use_ewma: bool,
}

#[derive(Default)]
struct HistoricalData {
    latencies: Vec<f64>,
    token_counts: Vec<f64>,
    costs: Vec<f64>,
    error_rates: Vec<f64>,
}

impl HistoricalData {
    fn add_sample(&mut self, latency: f64, tokens: f64, cost: f64, errors: f64) {
        self.latencies.push(latency);
        self.token_counts.push(tokens);
        self.costs.push(cost);
        self.error_rates.push(errors);

        // Keep only recent samples (sliding window of 1000)
        const MAX_SAMPLES: usize = 1000;
        if self.latencies.len() > MAX_SAMPLES {
            self.latencies.remove(0);
            self.token_counts.remove(0);
            self.costs.remove(0);
            self.error_rates.remove(0);
        }
    }

    fn calculate_stats(&self, values: &[f64]) -> (f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0);
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        (mean, std_dev)
    }

    fn calculate_iqr(&self, values: &[f64]) -> (f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0);
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let q1_idx = sorted.len() / 4;
        let q3_idx = (3 * sorted.len()) / 4;

        let q1 = sorted[q1_idx];
        let q3 = sorted[q3_idx];

        (q1, q3)
    }
}

#[derive(Debug)]
struct AnomalyScore {
    is_anomalous: bool,
    confidence: f64,
    anomaly_type: String,
    details: HashMap<String, f64>,
}

impl AnomalyDetector {
    /// Create a new anomaly detector with default sensitivity (3.0 sigma)
    pub fn new() -> Self {
        Self {
            persisted_state: Arc::new(tokio::sync::RwLock::new(PersistedAnomalyState::default())),
            historical_data: Arc::new(tokio::sync::RwLock::new(HistoricalData::default())),
            sensitivity: 3.0,
            use_ewma: true,
        }
    }

    /// Create with pre-existing persisted state (for cold start prevention)
    pub fn with_persisted_state(state: PersistedAnomalyState) -> Self {
        Self {
            persisted_state: Arc::new(tokio::sync::RwLock::new(state)),
            historical_data: Arc::new(tokio::sync::RwLock::new(HistoricalData::default())),
            sensitivity: 3.0,
            use_ewma: true,
        }
    }

    /// Get the current persisted state (for saving)
    pub async fn get_persisted_state(&self) -> PersistedAnomalyState {
        self.persisted_state.read().await.clone()
    }

    /// Restore from persisted state (for hydration on restart)
    pub async fn restore_state(&self, state: PersistedAnomalyState) {
        let mut current = self.persisted_state.write().await;
        *current = state;
    }

    /// Use legacy mode (non-EWMA) for compatibility
    pub fn with_legacy_mode(mut self) -> Self {
        self.use_ewma = false;
        self
    }

    /// Create a new anomaly detector with custom sensitivity
    pub fn with_sensitivity(mut self, sensitivity: f64) -> Self {
        self.sensitivity = sensitivity;
        self
    }

    async fn detect_anomaly(&self, trace: &TraceContext) -> AnomalyScore {
        let data = self.historical_data.read().await;

        // Extract current metrics from edges
        let total_duration_us: u64 = trace.edges.iter().map(|e| e.duration_us as u64).sum();
        let current_latency = (total_duration_us as f64) / 1000.0; // Convert to milliseconds

        let total_tokens: u32 = trace.edges.iter().map(|e| e.token_count).sum();
        let current_tokens = total_tokens as f64;

        // Estimate cost (simple model: $0.002 per 1K tokens for GPT-3.5-turbo)
        let current_cost = (current_tokens / 1000.0) * 0.002;

        // Check for errors in metadata
        let _current_error_rate = trace
            .metadata
            .get("has_error")
            .and_then(|v| v.as_bool())
            .map(|has_error| if has_error { 1.0 } else { 0.0 })
            .unwrap_or(0.0);

        let mut anomalies = Vec::new();
        let mut details = HashMap::new();

        // Check latency anomaly
        if !data.latencies.is_empty() {
            let (mean, std_dev) = data.calculate_stats(&data.latencies);
            if std_dev > 0.0 {
                let z_score = (current_latency - mean) / std_dev;
                details.insert("latency_z_score".to_string(), z_score);

                if z_score.abs() > self.sensitivity {
                    anomalies.push(format!("Latency (z={:.2})", z_score));
                }
            }
        }

        // Check token count anomaly
        if !data.token_counts.is_empty() {
            let (mean, std_dev) = data.calculate_stats(&data.token_counts);
            if std_dev > 0.0 {
                let z_score = (current_tokens - mean) / std_dev;
                details.insert("tokens_z_score".to_string(), z_score);

                if z_score.abs() > self.sensitivity {
                    anomalies.push(format!("Token count (z={:.2})", z_score));
                }
            }
        }

        // Check cost anomaly using IQR method
        if !data.costs.is_empty() {
            let (q1, q3) = data.calculate_iqr(&data.costs);
            let iqr = q3 - q1;
            let lower_bound = q1 - 1.5 * iqr;
            let upper_bound = q3 + 1.5 * iqr;

            details.insert("cost_iqr_lower".to_string(), lower_bound);
            details.insert("cost_iqr_upper".to_string(), upper_bound);
            details.insert("cost_value".to_string(), current_cost);

            if current_cost < lower_bound || current_cost > upper_bound {
                anomalies.push(format!("Cost (${:.4})", current_cost));
            }
        }

        // Determine overall anomaly status
        let is_anomalous = !anomalies.is_empty();
        let confidence = if is_anomalous {
            // Higher confidence with more anomalous metrics
            (anomalies.len() as f64 / 3.0).min(1.0)
        } else {
            0.0
        };

        let anomaly_type = if anomalies.is_empty() {
            "none".to_string()
        } else {
            anomalies.join(", ")
        };

        AnomalyScore {
            is_anomalous,
            confidence,
            anomaly_type,
            details,
        }
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for AnomalyDetector {
    fn id(&self) -> &str {
        "anomaly_detector_v1"
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Anomaly Detector".to_string(),
            description: if self.use_ewma {
                "Detects anomalies using EWMA statistics with seasonal adjustment and persistent baselines"
            } else {
                "Detects anomalies using Z-score and IQR methods"
            }.to_string(),
            version: "2.0.0".to_string(),
            cost_per_eval: None,
            avg_latency_ms: Some(5), // Very fast, statistical computation
            tags: vec![
                "anomaly".to_string(),
                "statistical".to_string(),
                "monitoring".to_string(),
                "ewma".to_string(),
            ],
            author: Some("Agentreplay Team".to_string()),
        }
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // Extract metrics from edges
        let total_duration_us: u64 = trace.edges.iter().map(|e| e.duration_us as u64).sum();
        let latency_ms = (total_duration_us as f64) / 1000.0;

        let total_tokens: u32 = trace.edges.iter().map(|e| e.token_count).sum();
        let tokens = total_tokens as f64;

        let cost = (tokens / 1000.0) * 0.002;

        let has_error = trace
            .metadata
            .get("has_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let error_rate = if has_error { 1.0 } else { 0.0 };

        // Use EWMA-based detection if enabled
        let (anomaly_score, is_warmed_up) = if self.use_ewma {
            let state = self.persisted_state.read().await;
            let warmed = state.is_warmed_up();

            // Compute z-score using EWMA statistics
            let z_latency = if warmed {
                state.z_score(latency_ms, None, None)
            } else {
                0.0
            };

            let z_tokens = if warmed && state.token_var > 0.0 {
                (tokens - state.token_mean) / state.token_var.sqrt().max(1e-10)
            } else {
                0.0
            };

            let mut anomalies = Vec::new();
            let mut details = HashMap::new();

            if warmed {
                details.insert("latency_z_score".to_string(), z_latency);
                details.insert("tokens_z_score".to_string(), z_tokens);
                details.insert("baseline_latency_mean".to_string(), state.latency_mean);
                details.insert("baseline_latency_std".to_string(), state.latency_var.sqrt());
                details.insert("sample_count".to_string(), state.count as f64);

                if z_latency.abs() > self.sensitivity {
                    anomalies.push(format!("Latency (z={:.2})", z_latency));
                }
                if z_tokens.abs() > self.sensitivity {
                    anomalies.push(format!("Tokens (z={:.2})", z_tokens));
                }
            }

            let is_anomalous = !anomalies.is_empty();
            let confidence = if is_anomalous {
                (anomalies.len() as f64 / 3.0).min(1.0)
            } else {
                0.0
            };

            drop(state);

            // Update persisted state
            {
                let mut state = self.persisted_state.write().await;
                state.update(latency_ms, tokens, cost, error_rate);
            }

            (
                AnomalyScore {
                    is_anomalous,
                    confidence,
                    anomaly_type: if anomalies.is_empty() {
                        "none".to_string()
                    } else {
                        anomalies.join(", ")
                    },
                    details,
                },
                warmed,
            )
        } else {
            // Legacy detection
            let score = self.detect_anomaly(trace).await;
            let warmed = true; // Legacy always considers itself warmed
            (score, warmed)
        };

        // Update legacy historical data (for IQR calculations)
        {
            let mut data = self.historical_data.write().await;
            data.add_sample(latency_ms, tokens, cost, error_rate);
        }

        let confidence = if anomaly_score.is_anomalous {
            anomaly_score.confidence
        } else {
            1.0 - anomaly_score.confidence
        };

        let mut metrics = HashMap::new();
        metrics.insert(
            "is_anomalous".to_string(),
            MetricValue::Bool(anomaly_score.is_anomalous),
        );
        metrics.insert(
            "anomaly_type".to_string(),
            MetricValue::String(anomaly_score.anomaly_type.clone()),
        );
        metrics.insert(
            "confidence".to_string(),
            MetricValue::Float(anomaly_score.confidence),
        );
        metrics.insert(
            "sensitivity_sigma".to_string(),
            MetricValue::Float(self.sensitivity),
        );
        metrics.insert(
            "detection_mode".to_string(),
            MetricValue::String(if self.use_ewma { "ewma" } else { "legacy" }.to_string()),
        );
        metrics.insert(
            "baseline_warmed_up".to_string(),
            MetricValue::Bool(is_warmed_up),
        );

        // Add details
        for (key, value) in anomaly_score.details {
            metrics.insert(key, MetricValue::Float(value));
        }

        let explanation = if anomaly_score.is_anomalous {
            Some(format!(
                "Anomaly detected: {} (confidence: {:.2})",
                anomaly_score.anomaly_type, anomaly_score.confidence
            ))
        } else {
            Some("No anomalies detected".to_string())
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("statistical".to_string()),
            metrics,
            passed: !anomaly_score.is_anomalous,
            explanation,
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence,
            cost: None,
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_anomaly_detection_normal() {
        let detector = AnomalyDetector::new();

        // Add normal samples
        // Cost is calculated as (tokens / 1000) * 0.002, so 500 tokens = $0.001
        {
            let mut data = detector.historical_data.write().await;
            for i in 0..100 {
                data.add_sample(100.0 + i as f64, 500.0, 0.001, 0.0);
            }
        }

        // Create a test edge with normal latency (150ms = 150_000us) and 500 tokens
        let mut edge = agentreplay_core::AgentFlowEdge::new(
            1, // tenant_id
            0, // project_id
            1, // agent_id
            1, // session_id
            agentreplay_core::SpanType::Reasoning,
            0, // causal_parent
        );
        edge.duration_us = 150_000; // 150ms
        edge.token_count = 500;

        // Test normal trace
        let trace = TraceContext {
            trace_id: 1u128,
            edges: vec![edge],
            input: Some("test input".to_string()),
            output: Some("test output".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let result = detector.evaluate(&trace).await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_anomaly_detection_outlier() {
        // Use legacy mode since we're populating historical_data, not persisted_state
        let detector = AnomalyDetector::new().with_legacy_mode();

        // Add normal samples with variance (90-110ms range)
        // Cost is calculated as (tokens / 1000) * 0.002, so 500 tokens = $0.001
        {
            let mut data = detector.historical_data.write().await;
            for i in 0..100 {
                // Add variance: 90-110ms range, centered around 100ms
                let latency = 90.0 + (i % 21) as f64;
                data.add_sample(latency, 500.0, 0.001, 0.0);
            }
        }

        // Create a test edge with outlier latency (1000ms = 1_000_000us, 10x normal)
        let mut edge = agentreplay_core::AgentFlowEdge::new(
            1, // tenant_id
            0, // project_id
            1, // agent_id
            1, // session_id
            agentreplay_core::SpanType::Reasoning,
            0, // causal_parent
        );
        edge.duration_us = 1_000_000; // 1000ms (10x normal, ~100x std dev away)
        edge.token_count = 500;

        // Test outlier trace (extremely high latency)
        let trace = TraceContext {
            trace_id: 1u128,
            edges: vec![edge],
            input: Some("test input".to_string()),
            output: Some("test output".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let result = detector.evaluate(&trace).await.unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_iqr_calculation() {
        let data = HistoricalData::default();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let (q1, q3) = data.calculate_iqr(&values);

        assert!(q1 < q3);
        assert!(q1 >= values[0]);
        assert!(q3 <= values[values.len() - 1]);
    }
}

// ============================================================================
// CUSUM Control Chart for Mean Shift Detection (TASK 11 Enhancement)
// ============================================================================

/// CUSUM (Cumulative Sum) control chart for detecting mean shifts
///
/// CUSUM is more sensitive than EWMA for detecting small, sustained shifts.
/// It accumulates deviations from a target mean and signals when the cumulative
/// sum exceeds a threshold.
///
/// ## Parameters
/// - `k` (slack/allowance): Typically 0.5σ, controls sensitivity
/// - `h` (decision interval): Typically 4-5σ, controls alarm threshold
///
/// ## Algorithm
/// C⁺ₜ = max(0, C⁺ₜ₋₁ + (Yₜ - μ₀ - k))  // Detects upward shifts
/// C⁻ₜ = max(0, C⁻ₜ₋₁ + (μ₀ - k - Yₜ))  // Detects downward shifts
///
/// Alarm when C⁺ₜ > h or C⁻ₜ > h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CUSUMState {
    /// Target mean (μ₀)
    pub target_mean: f64,
    /// Standard deviation estimate
    pub std_dev: f64,
    /// Cumulative sum for upward shifts
    pub c_plus: f64,
    /// Cumulative sum for downward shifts
    pub c_minus: f64,
    /// Slack parameter (k), typically 0.5σ
    pub k: f64,
    /// Decision interval (h), typically 4-5σ
    pub h: f64,
    /// Number of samples processed
    pub count: u64,
    /// Number of alarms triggered
    pub alarm_count: u64,
    /// Last sample that caused an alarm
    pub last_alarm_value: Option<f64>,
}

impl CUSUMState {
    /// Create a new CUSUM state with target parameters
    pub fn new(target_mean: f64, std_dev: f64) -> Self {
        Self {
            target_mean,
            std_dev,
            c_plus: 0.0,
            c_minus: 0.0,
            k: 0.5 * std_dev, // Standard slack
            h: 5.0 * std_dev, // Standard decision interval
            count: 0,
            alarm_count: 0,
            last_alarm_value: None,
        }
    }

    /// Create with custom sensitivity parameters
    pub fn with_params(target_mean: f64, std_dev: f64, k_sigma: f64, h_sigma: f64) -> Self {
        Self {
            target_mean,
            std_dev,
            c_plus: 0.0,
            c_minus: 0.0,
            k: k_sigma * std_dev,
            h: h_sigma * std_dev,
            count: 0,
            alarm_count: 0,
            last_alarm_value: None,
        }
    }

    /// Update with a new observation and return alarm status
    pub fn update(&mut self, value: f64) -> CUSUMResult {
        self.count += 1;

        // Update cumulative sums
        self.c_plus = (self.c_plus + value - self.target_mean - self.k).max(0.0);
        self.c_minus = (self.c_minus + self.target_mean - self.k - value).max(0.0);

        // Check for alarms
        let upward_alarm = self.c_plus > self.h;
        let downward_alarm = self.c_minus > self.h;
        let has_alarm = upward_alarm || downward_alarm;

        if has_alarm {
            self.alarm_count += 1;
            self.last_alarm_value = Some(value);
        }

        CUSUMResult {
            value,
            c_plus: self.c_plus,
            c_minus: self.c_minus,
            upward_alarm,
            downward_alarm,
            shift_estimate: if upward_alarm {
                Some(self.c_plus / self.count as f64 + self.k)
            } else if downward_alarm {
                Some(-self.c_minus / self.count as f64 - self.k)
            } else {
                None
            },
        }
    }

    /// Reset after alarm (restart from zero)
    pub fn reset(&mut self) {
        self.c_plus = 0.0;
        self.c_minus = 0.0;
    }

    /// Update target mean (for adaptive CUSUM)
    pub fn update_target(&mut self, new_mean: f64) {
        self.target_mean = new_mean;
        self.reset();
    }
}

/// Result from CUSUM update
#[derive(Debug, Clone)]
pub struct CUSUMResult {
    /// The observed value
    pub value: f64,
    /// Current C⁺ value
    pub c_plus: f64,
    /// Current C⁻ value
    pub c_minus: f64,
    /// Whether upward shift alarm triggered
    pub upward_alarm: bool,
    /// Whether downward shift alarm triggered
    pub downward_alarm: bool,
    /// Estimated shift size (if alarm)
    pub shift_estimate: Option<f64>,
}

impl CUSUMResult {
    /// Check if any alarm was triggered
    pub fn has_alarm(&self) -> bool {
        self.upward_alarm || self.downward_alarm
    }
}

#[cfg(test)]
mod cusum_tests {
    use super::*;

    #[test]
    fn test_cusum_normal_operation() {
        let mut cusum = CUSUMState::new(100.0, 10.0);

        // Normal values should not trigger alarms
        for _ in 0..50 {
            let _ = cusum.update(100.0 + (rand::random::<f64>() - 0.5) * 10.0);
            // Most values should not trigger alarms
        }

        assert!(cusum.alarm_count < 5);
    }

    #[test]
    fn test_cusum_mean_shift() {
        let mut cusum = CUSUMState::new(100.0, 10.0);

        // Normal period
        for _ in 0..20 {
            cusum.update(100.0);
        }

        // Shift mean to 120 (2σ shift)
        let mut alarm_triggered = false;
        for _ in 0..20 {
            let result = cusum.update(120.0);
            if result.upward_alarm {
                alarm_triggered = true;
                break;
            }
        }

        assert!(alarm_triggered, "CUSUM should detect mean shift");
    }
}
