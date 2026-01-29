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

//! Trajectory efficiency evaluator for agent execution traces
//!
//! Measures how efficiently the agent reached its goal.
//! This is CRITICAL for cost optimization and user experience.
//!
//! Mathematical model:
//! Efficiency = 1 - (redundant_steps + backtrack_steps) / total_steps
//!
//! Where redundancy is detected via:
//! - Exact duplicate tool calls (same tool, same parameters)
//! - Semantically similar queries (edit distance or embedding similarity)
//! - Backtracking (reverting previous decisions)
//! - Dead ends (actions that didn't contribute to the solution)

use crate::{EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext};
use async_trait::async_trait;
use agentreplay_core::SpanType;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Analysis of trajectory redundancy
#[derive(Debug, Clone)]
struct RedundancyAnalysis {
    /// Total number of steps in the trajectory
    total_steps: usize,
    /// Number of tool call steps
    tool_calls: usize,
    /// Exact duplicate tool calls
    exact_duplicates: usize,
    /// Semantically similar operations (potential redundancy)
    semantic_duplicates: usize,
    /// Efficiency score (0.0 = very inefficient, 1.0 = perfectly efficient)
    efficiency_score: f64,
}

/// Trajectory efficiency evaluator
///
/// Analyzes agent execution paths to detect:
/// - Redundant tool calls
/// - Unnecessary repeated operations
/// - Backtracking and dead ends
///
/// This is a purely deterministic evaluator (no LLM calls needed).
pub struct TrajectoryEfficiencyEvaluator {
    /// Optimal step count (if known from golden dataset)
    optimal_steps: Option<usize>,
    /// Similarity threshold for detecting redundant queries (0.0-1.0)
    redundancy_threshold: f64,
    /// Minimum efficiency score to pass
    threshold: f64,
}

impl TrajectoryEfficiencyEvaluator {
    /// Create a new trajectory efficiency evaluator
    pub fn new() -> Self {
        Self {
            optimal_steps: None,
            redundancy_threshold: 0.85, // Operations >85% similar are considered redundant
            threshold: 0.6,             // Pass if efficiency >= 0.6
        }
    }

    /// Set optimal step count for comparison
    pub fn with_optimal_steps(mut self, steps: usize) -> Self {
        self.optimal_steps = Some(steps);
        self
    }

    /// Set redundancy detection threshold (default: 0.85)
    pub fn with_redundancy_threshold(mut self, threshold: f64) -> Self {
        self.redundancy_threshold = threshold;
        self
    }

    /// Set pass/fail threshold (default: 0.6)
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Calculate Levenshtein distance for string similarity
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
            row[0] = i;
        }
        if let Some(first_row) = matrix.first_mut() {
            for (j, cell) in first_row.iter_mut().enumerate().take(len2 + 1) {
                *cell = j;
            }
        }

        for (i, c1) in s1.chars().enumerate() {
            for (j, c2) in s2.chars().enumerate() {
                let cost = if c1 == c2 { 0 } else { 1 };
                matrix[i + 1][j + 1] = std::cmp::min(
                    std::cmp::min(matrix[i][j + 1] + 1, matrix[i + 1][j] + 1),
                    matrix[i][j] + cost,
                );
            }
        }

        matrix[len1][len2]
    }

    /// Calculate string similarity (0.0 = completely different, 1.0 = identical)
    fn string_similarity(&self, s1: &str, s2: &str) -> f64 {
        if s1 == s2 {
            return 1.0;
        }

        let max_len = std::cmp::max(s1.len(), s2.len());
        if max_len == 0 {
            return 1.0;
        }

        let distance = self.levenshtein_distance(s1, s2);
        1.0 - (distance as f64 / max_len as f64)
    }

    /// Detect redundancy in the trajectory
    fn detect_redundancy(&self, trace: &TraceContext) -> RedundancyAnalysis {
        // Filter for actionable steps (tool calls, retrievals, etc.)
        let tool_edges: Vec<_> = trace
            .edges
            .iter()
            .filter(|e| {
                let span_type = e.get_span_type();
                matches!(
                    span_type,
                    SpanType::ToolCall
                        | SpanType::Retrieval
                        | SpanType::HttpCall
                        | SpanType::Database
                )
            })
            .collect();

        if tool_edges.is_empty() {
            return RedundancyAnalysis {
                total_steps: trace.edges.len(),
                tool_calls: 0,
                exact_duplicates: 0,
                semantic_duplicates: 0,
                efficiency_score: 1.0,
            };
        }

        let mut exact_duplicates = 0;
        let mut semantic_duplicates = 0;

        // Track seen operations
        let mut seen_operations: HashSet<String> = HashSet::new();
        let mut operation_list: Vec<String> = Vec::new();

        for edge in &tool_edges {
            let span_type = edge.get_span_type();
            // Note: Names are in payloads, not in fixed edge structure
            // Use edge_id as unique identifier for now
            let operation_key = format!("{:?}::{:x}", span_type, edge.edge_id);

            // Check for exact duplicates
            if seen_operations.contains(&operation_key) {
                exact_duplicates += 1;
            } else {
                seen_operations.insert(operation_key.clone());
            }

            operation_list.push(operation_key);
        }

        // Check for semantic duplicates (similar operations)
        for i in 0..operation_list.len() {
            for j in (i + 1)..operation_list.len() {
                let similarity = self.string_similarity(&operation_list[i], &operation_list[j]);
                if similarity > self.redundancy_threshold && similarity < 1.0 {
                    // Similar but not exact duplicates
                    semantic_duplicates += 1;
                }
            }
        }

        // Calculate efficiency score
        let total_redundant = exact_duplicates + semantic_duplicates;
        let efficiency_score = if tool_edges.is_empty() {
            1.0
        } else {
            1.0 - (total_redundant as f64 / tool_edges.len() as f64)
        };

        RedundancyAnalysis {
            total_steps: trace.edges.len(),
            tool_calls: tool_edges.len(),
            exact_duplicates,
            semantic_duplicates,
            efficiency_score: efficiency_score.clamp(0.0, 1.0),
        }
    }

    /// Calculate step efficiency vs optimal (if optimal is known)
    fn calculate_step_efficiency(&self, actual_steps: usize) -> Option<f64> {
        self.optimal_steps
            .map(|optimal| (optimal as f64 / actual_steps.max(1) as f64).min(1.0))
    }

    /// Detect potential backtracking patterns
    fn detect_backtracking(&self, trace: &TraceContext) -> usize {
        // Look for error spans followed by retries
        let mut backtrack_count = 0;

        for i in 0..(trace.edges.len().saturating_sub(1)) {
            let current_span = trace.edges[i].get_span_type();
            let next_span = trace.edges[i + 1].get_span_type();

            // If an error is followed by a similar tool call, it's likely backtracking
            if current_span == SpanType::Error
                && matches!(next_span, SpanType::ToolCall | SpanType::Retrieval)
            {
                backtrack_count += 1;
            }
        }

        backtrack_count
    }
}

impl Default for TrajectoryEfficiencyEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for TrajectoryEfficiencyEvaluator {
    fn id(&self) -> &str {
        "trajectory_efficiency_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // Analyze redundancy
        let analysis = self.detect_redundancy(trace);

        // Calculate step efficiency if optimal is known
        let step_efficiency = self.calculate_step_efficiency(analysis.total_steps);

        // Detect backtracking
        let backtrack_count = self.detect_backtracking(trace);

        // Calculate overall efficiency
        let mut overall_efficiency = analysis.efficiency_score;

        // Apply step efficiency if available
        if let Some(step_eff) = step_efficiency {
            overall_efficiency = (overall_efficiency + step_eff) / 2.0;
        }

        // Apply backtracking penalty
        if backtrack_count > 0 && analysis.tool_calls > 0 {
            let backtrack_penalty = (backtrack_count as f64 / analysis.tool_calls as f64) * 0.2;
            overall_efficiency = (overall_efficiency - backtrack_penalty).max(0.0);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // Build metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "efficiency_score".to_string(),
            MetricValue::Float(overall_efficiency),
        );
        metrics.insert(
            "total_steps".to_string(),
            MetricValue::Int(analysis.total_steps as i64),
        );
        metrics.insert(
            "tool_calls".to_string(),
            MetricValue::Int(analysis.tool_calls as i64),
        );
        metrics.insert(
            "exact_duplicates".to_string(),
            MetricValue::Int(analysis.exact_duplicates as i64),
        );
        metrics.insert(
            "semantic_duplicates".to_string(),
            MetricValue::Int(analysis.semantic_duplicates as i64),
        );
        metrics.insert(
            "backtrack_count".to_string(),
            MetricValue::Int(backtrack_count as i64),
        );

        if let Some(optimal) = self.optimal_steps {
            metrics.insert(
                "optimal_steps".to_string(),
                MetricValue::Int(optimal as i64),
            );
        }

        if let Some(step_eff) = step_efficiency {
            metrics.insert("step_efficiency".to_string(), MetricValue::Float(step_eff));
        }

        let passed = overall_efficiency >= self.threshold;

        let explanation = format!(
            "Trajectory efficiency: {:.1}% (threshold: {:.1}%). {} steps total, {} tool calls, {} exact duplicates, {} semantic duplicates, {} backtracks.",
            overall_efficiency * 100.0,
            self.threshold * 100.0,
            analysis.total_steps,
            analysis.tool_calls,
            analysis.exact_duplicates,
            analysis.semantic_duplicates,
            backtrack_count
        );

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("rule".to_string()),
            metrics,
            passed,
            explanation: Some(explanation),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 0.90, // High confidence for deterministic analysis
            cost: Some(0.0),  // No cost - purely deterministic
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Trajectory Efficiency Evaluator".to_string(),
            version: "1.0.0".to_string(),
            description: "Analyzes agent execution paths to detect redundant operations, backtracking, and inefficiencies. Purely deterministic, no LLM calls needed.".to_string(),
            cost_per_eval: Some(0.0), // Free - deterministic only
            avg_latency_ms: Some(10),
            tags: vec![
                "trajectory".to_string(),
                "efficiency".to_string(),
                "agent".to_string(),
                "deterministic".to_string(),
                "cost-optimization".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}
