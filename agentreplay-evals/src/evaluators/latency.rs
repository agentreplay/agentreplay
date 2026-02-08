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

//! Latency and performance benchmarking

use crate::{EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;

/// Latency benchmark evaluator
///
/// Analyzes timing information from trace edges to compute performance metrics.
/// Computes p50, p95, p99 latencies, total duration, and identifies slow operations.
pub struct LatencyBenchmark {
    p99_threshold_ms: u64,
    total_threshold_ms: u64,
}

impl LatencyBenchmark {
    /// Create a new latency benchmark evaluator
    pub fn new() -> Self {
        Self {
            p99_threshold_ms: 5000,    // Fail if p99 > 5 seconds
            total_threshold_ms: 10000, // Fail if total duration > 10 seconds
        }
    }

    /// Set p99 latency threshold in milliseconds (default: 5000ms)
    pub fn with_p99_threshold_ms(mut self, threshold_ms: u64) -> Self {
        self.p99_threshold_ms = threshold_ms;
        self
    }

    /// Set total duration threshold in milliseconds (default: 10000ms)
    pub fn with_total_threshold_ms(mut self, threshold_ms: u64) -> Self {
        self.total_threshold_ms = threshold_ms;
        self
    }

    /// Calculate percentile value from sorted array with linear interpolation
    ///
    /// CRITICAL FIX: Now uses proper linear interpolation instead of floor.
    /// This matches the standard percentile calculation used by numpy, R, Excel.
    fn percentile(sorted_values: &[u64], p: f64) -> u64 {
        if sorted_values.is_empty() {
            return 0;
        }

        if sorted_values.len() == 1 {
            return sorted_values[0];
        }

        // Linear interpolation method (matches numpy default)
        let index = (p / 100.0) * (sorted_values.len() - 1) as f64;
        let lower = index.floor() as usize;
        let upper = index.ceil() as usize;
        let weight = index - lower as f64;

        // Clamp indices to valid range
        let lower = lower.min(sorted_values.len() - 1);
        let upper = upper.min(sorted_values.len() - 1);

        let lower_val = sorted_values[lower] as f64;
        let upper_val = sorted_values[upper] as f64;

        // Linear interpolation: lower + weight * (upper - lower)
        (lower_val + weight * (upper_val - lower_val)).round() as u64
    }

    /// Get span type name from u32 value
    fn span_type_name(span_type: u32) -> &'static str {
        match span_type {
            0 => "Root",
            1 => "Planning",
            2 => "Reasoning",
            3 => "ToolCall",
            4 => "ToolResponse",
            5 => "Synthesis",
            6 => "Response",
            7 => "Error",
            _ => "Custom",
        }
    }
}

impl Default for LatencyBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for LatencyBenchmark {
    fn id(&self) -> &str {
        "latency_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        if trace.edges.is_empty() {
            return Err(EvalError::InvalidInput(
                "No edges in trace to analyze".to_string(),
            ));
        }

        // Extract duration from each edge (in microseconds)
        let mut durations_us: Vec<u64> = trace
            .edges
            .iter()
            .map(|edge| edge.duration_us as u64)
            .filter(|&d| d > 0) // Filter out zero durations
            .collect();

        if durations_us.is_empty() {
            return Err(EvalError::InvalidInput(
                "No valid durations found in trace edges".to_string(),
            ));
        }

        // Sort for percentile calculation
        durations_us.sort_unstable();

        // Calculate percentiles in microseconds, then convert to milliseconds
        let p50_us = Self::percentile(&durations_us, 50.0);
        let p95_us = Self::percentile(&durations_us, 95.0);
        let p99_us = Self::percentile(&durations_us, 99.0);
        let min_us = *durations_us.first().unwrap();
        let max_us = *durations_us.last().unwrap();
        let mean_us = durations_us.iter().sum::<u64>() / durations_us.len() as u64;

        // Convert to milliseconds for user-friendly metrics
        let p50_ms = p50_us as f64 / 1000.0;
        let p95_ms = p95_us as f64 / 1000.0;
        let p99_ms = p99_us as f64 / 1000.0;
        let min_ms = min_us as f64 / 1000.0;
        let max_ms = max_us as f64 / 1000.0;
        let mean_ms = mean_us as f64 / 1000.0;

        // Calculate total trace duration (from first to last edge)
        let first_timestamp = trace
            .edges
            .iter()
            .map(|e| e.timestamp_us)
            .min()
            .unwrap_or(0);
        let last_timestamp = trace
            .edges
            .iter()
            .map(|e| e.timestamp_us)
            .max()
            .unwrap_or(0);
        let total_duration_us = last_timestamp.saturating_sub(first_timestamp);
        let total_duration_ms = total_duration_us as f64 / 1000.0;

        // Analyze by span type
        let mut span_type_stats: HashMap<String, Vec<u64>> = HashMap::new();
        for edge in &trace.edges {
            let span_name = Self::span_type_name(edge.span_type).to_string();
            span_type_stats
                .entry(span_name)
                .or_default()
                .push(edge.duration_us as u64);
        }

        // Find slowest span type
        let slowest_span = span_type_stats
            .iter()
            .map(|(name, durations)| {
                let avg = durations.iter().sum::<u64>() / durations.len() as u64;
                (name.clone(), avg)
            })
            .max_by_key(|(_, avg)| *avg);

        // Determine if trace passes thresholds
        let p99_passed = (p99_us / 1000) <= self.p99_threshold_ms;
        let total_passed = (total_duration_us / 1000) <= self.total_threshold_ms;
        let passed = p99_passed && total_passed;

        let eval_duration_ms = start.elapsed().as_millis() as u64;

        // Build metrics
        let mut metrics = HashMap::new();
        metrics.insert("p50_ms".to_string(), MetricValue::Float(p50_ms));
        metrics.insert("p95_ms".to_string(), MetricValue::Float(p95_ms));
        metrics.insert("p99_ms".to_string(), MetricValue::Float(p99_ms));
        metrics.insert("min_ms".to_string(), MetricValue::Float(min_ms));
        metrics.insert("max_ms".to_string(), MetricValue::Float(max_ms));
        metrics.insert("mean_ms".to_string(), MetricValue::Float(mean_ms));
        metrics.insert(
            "total_duration_ms".to_string(),
            MetricValue::Float(total_duration_ms),
        );
        metrics.insert(
            "edge_count".to_string(),
            MetricValue::Int(trace.edges.len() as i64),
        );

        // Add span type breakdown
        let mut span_breakdown = HashMap::new();
        for (span_name, durations) in &span_type_stats {
            let avg_ms = (durations.iter().sum::<u64>() / durations.len() as u64) as f64 / 1000.0;
            span_breakdown.insert(span_name.clone(), MetricValue::Float(avg_ms));
        }
        metrics.insert(
            "span_type_avg_ms".to_string(),
            MetricValue::Object(span_breakdown),
        );

        if let Some((slowest_name, slowest_avg_us)) = slowest_span {
            metrics.insert(
                "slowest_span_type".to_string(),
                MetricValue::String(slowest_name),
            );
            metrics.insert(
                "slowest_span_avg_ms".to_string(),
                MetricValue::Float(slowest_avg_us as f64 / 1000.0),
            );
        }

        let explanation = format!(
            "Latency analysis: p50={:.1}ms, p95={:.1}ms, p99={:.1}ms, total={:.1}ms. {} edges analyzed.",
            p50_ms, p95_ms, p99_ms, total_duration_ms, trace.edges.len()
        );

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("statistical".to_string()),
            metrics,
            passed,
            explanation: Some(explanation),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 1.0, // High confidence for deterministic metrics
            cost: Some(0.0), // No cost for local computation
            duration_ms: Some(eval_duration_ms),
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Latency Benchmark".to_string(),
            version: "1.0.0".to_string(),
            description: "Analyzes trace timing to compute p50, p95, p99 latencies and identify slow operations.".to_string(),
            cost_per_eval: Some(0.0), // Free - local computation
            avg_latency_ms: Some(5), // Very fast
            tags: vec![
                "latency".to_string(),
                "performance".to_string(),
                "benchmark".to_string(),
                "timing".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentreplay_core::AgentFlowEdge;

    fn create_test_edge(
        edge_id: u128,
        duration_us: u32,
        span_type: u32,
        timestamp_us: u64,
    ) -> AgentFlowEdge {
        use agentreplay_core::SpanType;

        let mut edge = AgentFlowEdge::new(
            1, // tenant_id
            0, // project_id
            1, // agent_id
            1, // session_id
            SpanType::from_u64(span_type as u64),
            0, // causal_parent
        );

        // Override auto-generated values for testing
        edge.edge_id = edge_id;
        edge.timestamp_us = timestamp_us;
        edge.duration_us = duration_us;
        edge
    }

    #[tokio::test]
    async fn test_latency_benchmark() {
        let benchmark = LatencyBenchmark::new();

        // Create trace with varying latencies
        let edges = vec![
            create_test_edge(1, 100_000, 1, 1000000), // 100ms - Planning
            create_test_edge(2, 200_000, 2, 1100000), // 200ms - Reasoning
            create_test_edge(3, 300_000, 3, 1300000), // 300ms - ToolCall
            create_test_edge(4, 150_000, 4, 1600000), // 150ms - ToolResponse
            create_test_edge(5, 250_000, 5, 1750000), // 250ms - Synthesis
        ];

        let trace = TraceContext {
            trace_id: 123,
            edges,
            input: None,
            output: None,
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 1000000,
        };

        let result = benchmark.evaluate(&trace).await.unwrap();

        assert!(result.passed);
        assert_eq!(result.evaluator_id, "latency_v1");

        // Check that p50 exists and is reasonable
        if let Some(MetricValue::Float(p50)) = result.metrics.get("p50_ms") {
            assert!(*p50 >= 100.0 && *p50 <= 300.0);
        } else {
            panic!("Missing p50_ms metric");
        }

        // Check p99
        if let Some(MetricValue::Float(p99)) = result.metrics.get("p99_ms") {
            assert!(*p99 >= 200.0);
        } else {
            panic!("Missing p99_ms metric");
        }
    }

    #[tokio::test]
    async fn test_latency_benchmark_slow_trace() {
        let benchmark = LatencyBenchmark::new()
            .with_p99_threshold_ms(100) // Set low threshold
            .with_total_threshold_ms(500);

        let edges = vec![
            create_test_edge(1, 6_000_000, 1, 1000000), // 6 seconds - should fail
        ];

        let trace = TraceContext {
            trace_id: 123,
            edges,
            input: None,
            output: None,
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 1000000,
        };

        let result = benchmark.evaluate(&trace).await.unwrap();

        // Should fail due to slow p99
        assert!(!result.passed);
    }

    #[test]
    fn test_percentile_calculation() {
        let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

        // p50 with 10 values should be around the middle
        let p50 = LatencyBenchmark::percentile(&values, 50.0);
        assert!((40..=60).contains(&p50), "p50 should be 40-60, got {}", p50);

        // p95 and p99 should be near the high end (with floor, p95 gives 90, p99 gives 100)
        let p95 = LatencyBenchmark::percentile(&values, 95.0);
        assert!(p95 >= 80, "p95 should be >= 80, got {}", p95);

        let p99 = LatencyBenchmark::percentile(&values, 99.0);
        assert!(p99 >= 90, "p99 should be >= 90, got {}", p99);
    }
}
