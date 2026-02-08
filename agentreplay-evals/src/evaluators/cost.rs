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

//! Cost analysis and budget tracking

use crate::{EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;

/// Cost analyzer for token usage and budget tracking
///
/// Analyzes token counts across trace edges and calculates costs.
/// Supports multiple models with different pricing.
pub struct CostAnalyzer {
    budget_threshold_usd: f64,
    model_pricing: HashMap<String, (f64, f64)>, // (input_cost_per_1M, output_cost_per_1M)
}

impl CostAnalyzer {
    /// Create a new cost analyzer with default pricing
    pub fn new() -> Self {
        Self {
            budget_threshold_usd: 1.0, // Fail if total cost > $1.00
            model_pricing: Self::default_model_pricing(),
        }
    }

    /// Set budget threshold in USD (default: $1.00)
    pub fn with_budget_threshold(mut self, threshold_usd: f64) -> Self {
        self.budget_threshold_usd = threshold_usd;
        self
    }

    /// Add custom model pricing (input_cost_per_1M_tokens, output_cost_per_1M_tokens)
    pub fn with_model_pricing(mut self, model: String, input_cost: f64, output_cost: f64) -> Self {
        self.model_pricing.insert(model, (input_cost, output_cost));
        self
    }

    /// Default pricing for common models (as of 2025)
    fn default_model_pricing() -> HashMap<String, (f64, f64)> {
        let mut pricing = HashMap::new();

        // OpenAI models
        pricing.insert("gpt-4o".to_string(), (2.50, 10.0));
        pricing.insert("gpt-4o-mini".to_string(), (0.15, 0.60));
        pricing.insert("gpt-4-turbo".to_string(), (10.0, 30.0));
        pricing.insert("gpt-3.5-turbo".to_string(), (0.50, 1.50));

        // Anthropic models
        pricing.insert("claude-sonnet-4.5".to_string(), (3.0, 15.0));
        pricing.insert("claude-3-5-sonnet-20241022".to_string(), (3.0, 15.0));
        pricing.insert("claude-3-5-haiku-20241022".to_string(), (0.80, 4.0));
        pricing.insert("claude-opus-3".to_string(), (15.0, 75.0));

        // Default pricing for unknown models (GPT-4o-mini pricing)
        pricing.insert("default".to_string(), (0.15, 0.60));

        pricing
    }

    /// Calculate cost for a given token count and model
    fn calculate_cost(&self, tokens: u32, model: Option<&str>) -> f64 {
        let model_key = model.unwrap_or("default");
        let (input_cost_per_1m, _output_cost_per_1m) = self
            .model_pricing
            .get(model_key)
            .or_else(|| self.model_pricing.get("default"))
            .copied()
            .unwrap_or((0.15, 0.60));

        // Assume all tokens are input tokens for simplicity
        // In a real implementation, we'd distinguish input vs output tokens
        (tokens as f64 / 1_000_000.0) * input_cost_per_1m
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

impl Default for CostAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for CostAnalyzer {
    fn id(&self) -> &str {
        "cost_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        if trace.edges.is_empty() {
            return Err(EvalError::InvalidInput(
                "No edges in trace to analyze".to_string(),
            ));
        }

        // Aggregate token counts across all edges
        let total_tokens: u32 = trace.edges.iter().map(|e| e.token_count).sum();

        // For now, assume default model pricing
        // In production, extract model name from edge metadata/payload
        let model_name = trace
            .metadata
            .get("model")
            .and_then(|v| v.as_str())
            .or(Some("default"));

        let total_cost = self.calculate_cost(total_tokens, model_name);

        // Break down by span type
        let mut span_type_costs: HashMap<String, (u32, f64)> = HashMap::new();
        for edge in &trace.edges {
            let span_name = Self::span_type_name(edge.span_type).to_string();
            let cost = self.calculate_cost(edge.token_count, model_name);

            let entry = span_type_costs.entry(span_name).or_insert((0, 0.0));
            entry.0 += edge.token_count;
            entry.1 += cost;
        }

        // Find most expensive span type
        let most_expensive_span = span_type_costs
            .iter()
            .max_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap())
            .map(|(name, (tokens, cost))| (name.clone(), *tokens, *cost));

        // Determine if trace passes budget threshold
        let passed = total_cost <= self.budget_threshold_usd;

        let eval_duration_ms = start.elapsed().as_millis() as u64;

        // Build metrics
        let mut metrics = HashMap::new();
        metrics.insert("total_cost_usd".to_string(), MetricValue::Float(total_cost));
        metrics.insert(
            "total_tokens".to_string(),
            MetricValue::Int(total_tokens as i64),
        );
        metrics.insert(
            "avg_cost_per_edge_usd".to_string(),
            MetricValue::Float(total_cost / trace.edges.len() as f64),
        );
        metrics.insert(
            "edge_count".to_string(),
            MetricValue::Int(trace.edges.len() as i64),
        );

        if let Some(model) = model_name {
            metrics.insert("model".to_string(), MetricValue::String(model.to_string()));
        }

        // Add span type cost breakdown
        let mut cost_breakdown = HashMap::new();
        for (span_name, (tokens, cost)) in &span_type_costs {
            let mut span_metrics = HashMap::new();
            span_metrics.insert("tokens".to_string(), MetricValue::Int(*tokens as i64));
            span_metrics.insert("cost_usd".to_string(), MetricValue::Float(*cost));
            cost_breakdown.insert(span_name.clone(), MetricValue::Object(span_metrics));
        }
        metrics.insert(
            "span_type_costs".to_string(),
            MetricValue::Object(cost_breakdown),
        );

        if let Some((expensive_name, expensive_tokens, expensive_cost)) = most_expensive_span {
            metrics.insert(
                "most_expensive_span_type".to_string(),
                MetricValue::String(expensive_name),
            );
            metrics.insert(
                "most_expensive_span_tokens".to_string(),
                MetricValue::Int(expensive_tokens as i64),
            );
            metrics.insert(
                "most_expensive_span_cost_usd".to_string(),
                MetricValue::Float(expensive_cost),
            );
        }

        let explanation = if passed {
            format!(
                "Cost analysis: ${:.4} for {} tokens across {} edges. Within budget (${:.2}).",
                total_cost,
                total_tokens,
                trace.edges.len(),
                self.budget_threshold_usd
            )
        } else {
            format!(
                "Cost analysis: ${:.4} for {} tokens across {} edges. Exceeds budget (${:.2}).",
                total_cost,
                total_tokens,
                trace.edges.len(),
                self.budget_threshold_usd
            )
        };

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("rule".to_string()),
            metrics,
            passed,
            explanation: Some(explanation),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 0.95, // High confidence for deterministic metrics
            cost: Some(0.0),  // No cost for local computation
            duration_ms: Some(eval_duration_ms),
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Cost Analyzer".to_string(),
            version: "1.0.0".to_string(),
            description:
                "Analyzes token costs across trace and breaks down by model and operation type."
                    .to_string(),
            cost_per_eval: Some(0.0), // Free - local computation
            avg_latency_ms: Some(5),  // Very fast
            tags: vec![
                "cost".to_string(),
                "budget".to_string(),
                "tokens".to_string(),
                "pricing".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentreplay_core::AgentFlowEdge;

    fn create_test_edge(edge_id: u128, token_count: u32, span_type: u32) -> AgentFlowEdge {
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
        edge.token_count = token_count;
        edge.duration_us = 100000;
        edge
    }

    #[tokio::test]
    async fn test_cost_analyzer() {
        let analyzer = CostAnalyzer::new().with_budget_threshold(0.01); // $0.01 budget

        // Create trace with token usage
        let edges = vec![
            create_test_edge(1, 1000, 1), // 1000 tokens - Planning
            create_test_edge(2, 2000, 2), // 2000 tokens - Reasoning
            create_test_edge(3, 1500, 3), // 1500 tokens - ToolCall
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

        let result = analyzer.evaluate(&trace).await.unwrap();

        assert!(result.passed);
        assert_eq!(result.evaluator_id, "cost_v1");

        // Check total tokens
        if let Some(MetricValue::Int(tokens)) = result.metrics.get("total_tokens") {
            assert_eq!(*tokens, 4500);
        } else {
            panic!("Missing total_tokens metric");
        }

        // Check total cost
        if let Some(MetricValue::Float(cost)) = result.metrics.get("total_cost_usd") {
            assert!(*cost > 0.0);
            assert!(*cost < 0.01); // Should be under budget
        } else {
            panic!("Missing total_cost_usd metric");
        }
    }

    #[tokio::test]
    async fn test_cost_analyzer_over_budget() {
        let analyzer = CostAnalyzer::new().with_budget_threshold(0.0001); // Very low budget

        let edges = vec![
            create_test_edge(1, 100000, 1), // 100k tokens - will exceed budget
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

        let result = analyzer.evaluate(&trace).await.unwrap();

        // Should fail - over budget
        assert!(!result.passed);
    }

    #[test]
    fn test_cost_calculation() {
        let analyzer = CostAnalyzer::new();

        // Test default model pricing
        let cost1 = analyzer.calculate_cost(1_000_000, Some("gpt-4o-mini"));
        assert!((cost1 - 0.15).abs() < 0.001); // $0.15 per 1M tokens

        // Test GPT-4o pricing
        let cost2 = analyzer.calculate_cost(1_000_000, Some("gpt-4o"));
        assert!((cost2 - 2.50).abs() < 0.001); // $2.50 per 1M tokens
    }
}
