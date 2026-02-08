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

//! CIP Cost Model - Comprehensive cost tracking and estimation
//!
//! Tracks costs across all CIP evaluation components:
//! - Agent invocations (baseline, critical, null)
//! - Saboteur LLM calls for perturbation generation
//! - Embedding calls for similarity computation
//!
//! Provides budget enforcement and cost estimation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// COST BREAKDOWN
// =============================================================================

/// Detailed cost breakdown for CIP evaluation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CIPCostBreakdown {
    /// Cost of agent invocations (baseline + critical + null)
    pub agent_cost: AgentCosts,
    /// Cost of Saboteur LLM calls
    pub saboteur_cost: SaboteurCosts,
    /// Cost of embedding calls
    pub embedding_cost: EmbeddingCosts,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Token usage summary
    pub token_summary: TokenSummary,
}

/// Agent invocation costs breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCosts {
    /// Baseline invocation cost
    pub baseline_cost: f64,
    /// Critical path invocation cost
    pub critical_cost: f64,
    /// Null path invocation cost
    pub null_cost: f64,
    /// Total agent cost
    pub total: f64,
    /// Whether agent costs were estimated (not reported by agent)
    pub estimated: bool,
}

/// Saboteur LLM costs breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SaboteurCosts {
    /// Critical perturbation generation cost
    pub critical_generation_cost: f64,
    /// Null perturbation generation cost
    pub null_generation_cost: f64,
    /// Validation retry costs (if any)
    pub validation_retry_cost: f64,
    /// Total saboteur cost
    pub total: f64,
    /// Number of retries needed
    pub retry_count: u32,
}

/// Embedding costs breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbeddingCosts {
    /// Cost per embedding call
    pub cost_per_embedding: f64,
    /// Number of embeddings generated
    pub embedding_count: u32,
    /// Total embedding cost
    pub total: f64,
}

/// Token usage summary across all components
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenSummary {
    /// Total input tokens across all LLM calls
    pub total_input_tokens: u64,
    /// Total output tokens across all LLM calls
    pub total_output_tokens: u64,
    /// Total embedding tokens
    pub total_embedding_tokens: u64,
    /// Total tokens
    pub grand_total: u64,
}

impl CIPCostBreakdown {
    /// Create a new cost breakdown
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate total cost from components
    pub fn calculate_total(&mut self) {
        self.agent_cost.total = self.agent_cost.baseline_cost
            + self.agent_cost.critical_cost
            + self.agent_cost.null_cost;

        self.saboteur_cost.total = self.saboteur_cost.critical_generation_cost
            + self.saboteur_cost.null_generation_cost
            + self.saboteur_cost.validation_retry_cost;

        self.total_cost_usd =
            self.agent_cost.total + self.saboteur_cost.total + self.embedding_cost.total;

        self.token_summary.grand_total = self.token_summary.total_input_tokens
            + self.token_summary.total_output_tokens
            + self.token_summary.total_embedding_tokens;
    }

    /// Get cost as HashMap for MetricValue integration
    pub fn to_metrics(&self) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();

        metrics.insert("cip_cost_total".to_string(), self.total_cost_usd);
        metrics.insert("cip_cost_agent".to_string(), self.agent_cost.total);
        metrics.insert("cip_cost_saboteur".to_string(), self.saboteur_cost.total);
        metrics.insert("cip_cost_embedding".to_string(), self.embedding_cost.total);
        metrics.insert(
            "cip_tokens_total".to_string(),
            self.token_summary.grand_total as f64,
        );

        metrics
    }
}

// =============================================================================
// MODEL PRICING
// =============================================================================

/// Pricing configuration for a specific model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model identifier
    pub model_id: String,
    /// Cost per 1M input tokens in USD
    pub input_cost_per_million: f64,
    /// Cost per 1M output tokens in USD
    pub output_cost_per_million: f64,
}

impl ModelPricing {
    /// Create new model pricing
    pub fn new(model_id: &str, input_cost_per_million: f64, output_cost_per_million: f64) -> Self {
        Self {
            model_id: model_id.to_string(),
            input_cost_per_million,
            output_cost_per_million,
        }
    }

    /// Calculate cost for given token counts
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_cost_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output_cost_per_million;
        input_cost + output_cost
    }
}

/// Get standard pricing for common models (as of late 2024)
pub fn get_standard_pricing() -> HashMap<String, ModelPricing> {
    let mut pricing = HashMap::new();

    // OpenAI models
    pricing.insert(
        "gpt-4o".to_string(),
        ModelPricing::new("gpt-4o", 2.50, 10.00),
    );
    pricing.insert(
        "gpt-4o-mini".to_string(),
        ModelPricing::new("gpt-4o-mini", 0.15, 0.60),
    );
    pricing.insert(
        "gpt-4-turbo".to_string(),
        ModelPricing::new("gpt-4-turbo", 10.00, 30.00),
    );
    pricing.insert(
        "gpt-3.5-turbo".to_string(),
        ModelPricing::new("gpt-3.5-turbo", 0.50, 1.50),
    );

    // Anthropic models
    pricing.insert(
        "claude-3-5-sonnet".to_string(),
        ModelPricing::new("claude-3-5-sonnet", 3.00, 15.00),
    );
    pricing.insert(
        "claude-3-haiku".to_string(),
        ModelPricing::new("claude-3-haiku", 0.25, 1.25),
    );
    pricing.insert(
        "claude-3-opus".to_string(),
        ModelPricing::new("claude-3-opus", 15.00, 75.00),
    );

    // Embedding models (no output tokens)
    pricing.insert(
        "text-embedding-3-small".to_string(),
        ModelPricing::new("text-embedding-3-small", 0.02, 0.0),
    );
    pricing.insert(
        "text-embedding-3-large".to_string(),
        ModelPricing::new("text-embedding-3-large", 0.13, 0.0),
    );
    pricing.insert(
        "text-embedding-ada-002".to_string(),
        ModelPricing::new("text-embedding-ada-002", 0.10, 0.0),
    );

    pricing
}

// =============================================================================
// COST ESTIMATOR
// =============================================================================

/// Estimates CIP evaluation cost before running
#[derive(Debug, Clone)]
pub struct CIPCostEstimator {
    pricing: HashMap<String, ModelPricing>,
    saboteur_model: String,
    embedding_model: String,
}

impl CIPCostEstimator {
    /// Create a new cost estimator with default pricing
    pub fn new(saboteur_model: &str, embedding_model: &str) -> Self {
        Self {
            pricing: get_standard_pricing(),
            saboteur_model: saboteur_model.to_string(),
            embedding_model: embedding_model.to_string(),
        }
    }

    /// Add custom pricing for a model
    pub fn with_custom_pricing(mut self, model: &str, pricing: ModelPricing) -> Self {
        self.pricing.insert(model.to_string(), pricing);
        self
    }

    /// Estimate cost for a single CIP evaluation
    ///
    /// # Arguments
    /// * `context_length` - Approximate length of context in characters
    /// * `agent_model` - Model used by the agent (for cost estimation)
    /// * `include_retries` - Whether to include estimated retry costs
    pub fn estimate(
        &self,
        context_length: usize,
        agent_model: Option<&str>,
        include_retries: bool,
    ) -> CostEstimate {
        // Token estimation (rough: 1 token ≈ 4 characters for English)
        let context_tokens = (context_length / 4) as u64;

        // Saboteur costs
        let saboteur_input_tokens = context_tokens + 200; // Context + prompt overhead
        let saboteur_output_tokens = context_tokens; // Output ≈ same as input
        let saboteur_calls = if include_retries { 4 } else { 2 }; // critical + null, with retries

        let saboteur_cost = self
            .pricing
            .get(&self.saboteur_model)
            .map(|p| {
                p.calculate_cost(
                    saboteur_input_tokens * saboteur_calls,
                    saboteur_output_tokens * saboteur_calls,
                )
            })
            .unwrap_or(0.001 * saboteur_calls as f64); // Default fallback

        // Embedding costs
        // 5 embeddings: original context, perturbed contexts (2), agent outputs (3)
        let embedding_tokens = context_tokens * 2 + 500 * 3; // Contexts + outputs
        let embedding_cost = self
            .pricing
            .get(&self.embedding_model)
            .map(|p| p.calculate_cost(embedding_tokens, 0))
            .unwrap_or(0.0001 * 5.0);

        // Agent costs (if model provided)
        let agent_cost = agent_model
            .and_then(|m| self.pricing.get(m))
            .map(|p| {
                let agent_input = context_tokens + 100; // Context + query
                let agent_output = 500; // Typical response
                p.calculate_cost(agent_input * 3, agent_output * 3) // 3 invocations
            })
            .unwrap_or(0.0);

        let total = saboteur_cost + embedding_cost + agent_cost;

        CostEstimate {
            estimated_total_usd: total,
            saboteur_cost_usd: saboteur_cost,
            embedding_cost_usd: embedding_cost,
            agent_cost_usd: agent_cost,
            estimated_tokens: saboteur_input_tokens * saboteur_calls
                + saboteur_output_tokens * saboteur_calls
                + embedding_tokens,
            confidence: if agent_model.is_some() { 0.8 } else { 0.5 },
            assumptions: vec![
                "Token count estimated at 4 chars/token".to_string(),
                format!("Saboteur model: {}", self.saboteur_model),
                format!("Embedding model: {}", self.embedding_model),
                if include_retries {
                    "Includes 2 retry attempts for validation".to_string()
                } else {
                    "No retries included".to_string()
                },
            ],
        }
    }

    /// Estimate cost for batch CIP evaluation
    pub fn estimate_batch(
        &self,
        samples: &[(usize, Option<String>)], // (context_length, agent_model)
    ) -> BatchCostEstimate {
        if samples.is_empty() {
            return BatchCostEstimate {
                sample_count: 0,
                estimated_total_usd: 0.0,
                estimated_min_per_sample: 0.0,
                estimated_max_per_sample: 0.0,
                estimated_avg_per_sample: 0.0,
            };
        }

        let estimates: Vec<CostEstimate> = samples
            .iter()
            .map(|(len, model)| self.estimate(*len, model.as_deref(), true))
            .collect();

        let total: f64 = estimates.iter().map(|e| e.estimated_total_usd).sum();
        let min = estimates
            .iter()
            .map(|e| e.estimated_total_usd)
            .fold(f64::MAX, f64::min);
        let max = estimates
            .iter()
            .map(|e| e.estimated_total_usd)
            .fold(f64::MIN, f64::max);

        BatchCostEstimate {
            sample_count: samples.len(),
            estimated_total_usd: total,
            estimated_min_per_sample: min,
            estimated_max_per_sample: max,
            estimated_avg_per_sample: total / samples.len() as f64,
        }
    }
}

/// Cost estimate for a single CIP evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub estimated_total_usd: f64,
    pub saboteur_cost_usd: f64,
    pub embedding_cost_usd: f64,
    pub agent_cost_usd: f64,
    pub estimated_tokens: u64,
    pub confidence: f64,
    pub assumptions: Vec<String>,
}

/// Cost estimate for batch CIP evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCostEstimate {
    pub sample_count: usize,
    pub estimated_total_usd: f64,
    pub estimated_min_per_sample: f64,
    pub estimated_max_per_sample: f64,
    pub estimated_avg_per_sample: f64,
}

// =============================================================================
// BUDGET ENFORCER
// =============================================================================

/// Enforces budget limits during CIP evaluation
#[derive(Debug, Clone)]
pub struct BudgetEnforcer {
    /// Maximum total cost in USD
    max_total_cost: f64,
    /// Maximum cost per evaluation
    max_cost_per_eval: Option<f64>,
    /// Current accumulated cost
    accumulated_cost: f64,
    /// Number of evaluations run
    eval_count: u32,
    /// Whether to fail or just warn on budget exceed
    fail_on_exceed: bool,
}

impl BudgetEnforcer {
    /// Create a new budget enforcer with total budget limit
    pub fn new(max_total_cost: f64) -> Self {
        Self {
            max_total_cost,
            max_cost_per_eval: None,
            accumulated_cost: 0.0,
            eval_count: 0,
            fail_on_exceed: true,
        }
    }

    /// Set per-evaluation cost limit
    pub fn with_per_eval_limit(mut self, limit: f64) -> Self {
        self.max_cost_per_eval = Some(limit);
        self
    }

    /// Set to warn only instead of failing on budget exceed
    pub fn warn_only(mut self) -> Self {
        self.fail_on_exceed = false;
        self
    }

    /// Check if evaluation should proceed given estimated cost
    pub fn check_budget(&self, estimated_cost: f64) -> BudgetCheckResult {
        let projected_total = self.accumulated_cost + estimated_cost;

        // Check per-eval limit
        if let Some(per_eval_limit) = self.max_cost_per_eval {
            if estimated_cost > per_eval_limit {
                return BudgetCheckResult::PerEvalLimitExceeded {
                    estimated: estimated_cost,
                    limit: per_eval_limit,
                };
            }
        }

        // Check total budget
        if projected_total > self.max_total_cost {
            return BudgetCheckResult::TotalBudgetExceeded {
                accumulated: self.accumulated_cost,
                estimated: estimated_cost,
                limit: self.max_total_cost,
            };
        }

        // Check if we're approaching the limit (>80%)
        if projected_total > self.max_total_cost * 0.8 {
            return BudgetCheckResult::ApproachingLimit {
                accumulated: self.accumulated_cost,
                remaining: self.max_total_cost - self.accumulated_cost,
                percent_used: (self.accumulated_cost / self.max_total_cost) * 100.0,
            };
        }

        BudgetCheckResult::Ok {
            remaining: self.max_total_cost - projected_total,
        }
    }

    /// Record actual cost after evaluation
    pub fn record_cost(&mut self, actual_cost: f64) {
        self.accumulated_cost += actual_cost;
        self.eval_count += 1;
    }

    /// Get current budget status
    pub fn status(&self) -> BudgetStatus {
        BudgetStatus {
            accumulated_cost: self.accumulated_cost,
            remaining_budget: self.max_total_cost - self.accumulated_cost,
            eval_count: self.eval_count,
            avg_cost_per_eval: if self.eval_count > 0 {
                self.accumulated_cost / self.eval_count as f64
            } else {
                0.0
            },
            percent_used: (self.accumulated_cost / self.max_total_cost) * 100.0,
        }
    }

    /// Check if budget allows proceeding
    pub fn should_proceed(&self, estimated_cost: f64) -> bool {
        self.check_budget(estimated_cost)
            .should_proceed(self.fail_on_exceed)
    }
}

/// Result of a budget check
#[derive(Debug, Clone)]
pub enum BudgetCheckResult {
    /// Within budget
    Ok { remaining: f64 },
    /// Approaching budget limit (>80%)
    ApproachingLimit {
        accumulated: f64,
        remaining: f64,
        percent_used: f64,
    },
    /// Total budget would be exceeded
    TotalBudgetExceeded {
        accumulated: f64,
        estimated: f64,
        limit: f64,
    },
    /// Per-evaluation limit exceeded
    PerEvalLimitExceeded { estimated: f64, limit: f64 },
}

impl BudgetCheckResult {
    /// Check if evaluation should proceed based on result and fail policy
    pub fn should_proceed(&self, fail_on_exceed: bool) -> bool {
        match self {
            BudgetCheckResult::Ok { .. } => true,
            BudgetCheckResult::ApproachingLimit { .. } => true,
            BudgetCheckResult::TotalBudgetExceeded { .. } => !fail_on_exceed,
            BudgetCheckResult::PerEvalLimitExceeded { .. } => !fail_on_exceed,
        }
    }

    /// Check if this is an exceeded state
    pub fn is_exceeded(&self) -> bool {
        matches!(
            self,
            BudgetCheckResult::TotalBudgetExceeded { .. }
                | BudgetCheckResult::PerEvalLimitExceeded { .. }
        )
    }
}

/// Current budget status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub accumulated_cost: f64,
    pub remaining_budget: f64,
    pub eval_count: u32,
    pub avg_cost_per_eval: f64,
    pub percent_used: f64,
}

// =============================================================================
// COST TRACKER (Runtime)
// =============================================================================

/// Tracks costs during CIP evaluation execution
#[derive(Debug, Default)]
pub struct CIPCostTracker {
    breakdown: CIPCostBreakdown,
}

impl CIPCostTracker {
    /// Create a new cost tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Record agent baseline invocation
    pub fn record_agent_baseline(&mut self, cost: f64, tokens: Option<u64>) {
        self.breakdown.agent_cost.baseline_cost = cost;
        if let Some(t) = tokens {
            self.breakdown.token_summary.total_input_tokens += t / 2;
            self.breakdown.token_summary.total_output_tokens += t / 2;
        }
    }

    /// Record agent critical invocation
    pub fn record_agent_critical(&mut self, cost: f64, tokens: Option<u64>) {
        self.breakdown.agent_cost.critical_cost = cost;
        if let Some(t) = tokens {
            self.breakdown.token_summary.total_input_tokens += t / 2;
            self.breakdown.token_summary.total_output_tokens += t / 2;
        }
    }

    /// Record agent null invocation
    pub fn record_agent_null(&mut self, cost: f64, tokens: Option<u64>) {
        self.breakdown.agent_cost.null_cost = cost;
        if let Some(t) = tokens {
            self.breakdown.token_summary.total_input_tokens += t / 2;
            self.breakdown.token_summary.total_output_tokens += t / 2;
        }
    }

    /// Record saboteur critical generation
    pub fn record_saboteur_critical(&mut self, cost: f64, input_tokens: u64, output_tokens: u64) {
        self.breakdown.saboteur_cost.critical_generation_cost = cost;
        self.breakdown.token_summary.total_input_tokens += input_tokens;
        self.breakdown.token_summary.total_output_tokens += output_tokens;
    }

    /// Record saboteur null generation
    pub fn record_saboteur_null(&mut self, cost: f64, input_tokens: u64, output_tokens: u64) {
        self.breakdown.saboteur_cost.null_generation_cost = cost;
        self.breakdown.token_summary.total_input_tokens += input_tokens;
        self.breakdown.token_summary.total_output_tokens += output_tokens;
    }

    /// Record saboteur retry
    pub fn record_saboteur_retry(&mut self, cost: f64, tokens: u64) {
        self.breakdown.saboteur_cost.validation_retry_cost += cost;
        self.breakdown.saboteur_cost.retry_count += 1;
        self.breakdown.token_summary.total_input_tokens += tokens / 2;
        self.breakdown.token_summary.total_output_tokens += tokens / 2;
    }

    /// Record embedding cost
    pub fn record_embeddings(&mut self, count: u32, total_tokens: u64, total_cost: f64) {
        self.breakdown.embedding_cost.embedding_count = count;
        self.breakdown.embedding_cost.total = total_cost;
        self.breakdown.embedding_cost.cost_per_embedding = if count > 0 {
            total_cost / count as f64
        } else {
            0.0
        };
        self.breakdown.token_summary.total_embedding_tokens = total_tokens;
    }

    /// Finalize and get the cost breakdown
    pub fn finalize(mut self) -> CIPCostBreakdown {
        self.breakdown.calculate_total();
        self.breakdown
    }

    /// Get current breakdown without finalizing
    pub fn current(&self) -> &CIPCostBreakdown {
        &self.breakdown
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing_calculation() {
        let pricing = ModelPricing::new("gpt-4o-mini", 0.15, 0.60);

        // 1M input + 500K output
        let cost = pricing.calculate_cost(1_000_000, 500_000);
        // $0.15/1M input + $0.60/1M output * 0.5 = $0.15 + $0.30 = $0.45
        assert!((cost - 0.45).abs() < 0.01);
    }

    #[test]
    fn test_cost_estimator() {
        let estimator = CIPCostEstimator::new("gpt-4o-mini", "text-embedding-3-small");

        // 1000 character context
        let estimate = estimator.estimate(1000, Some("gpt-4o-mini"), false);

        assert!(estimate.estimated_total_usd > 0.0);
        assert!(estimate.saboteur_cost_usd > 0.0);
        assert!(estimate.embedding_cost_usd > 0.0);
        assert!(estimate.confidence > 0.0);
        assert!(!estimate.assumptions.is_empty());
    }

    #[test]
    fn test_cost_estimator_without_agent() {
        let estimator = CIPCostEstimator::new("gpt-4o-mini", "text-embedding-3-small");

        let estimate = estimator.estimate(1000, None, false);

        assert!(estimate.agent_cost_usd == 0.0);
        assert!(estimate.confidence == 0.5); // Lower confidence without agent model
    }

    #[test]
    fn test_batch_cost_estimate() {
        let estimator = CIPCostEstimator::new("gpt-4o-mini", "text-embedding-3-small");

        let samples = vec![
            (500, Some("gpt-4o-mini".to_string())),
            (1000, Some("gpt-4o-mini".to_string())),
            (2000, None),
        ];

        let batch = estimator.estimate_batch(&samples);

        assert_eq!(batch.sample_count, 3);
        assert!(batch.estimated_total_usd > 0.0);
        assert!(batch.estimated_min_per_sample <= batch.estimated_avg_per_sample);
        assert!(batch.estimated_avg_per_sample <= batch.estimated_max_per_sample);
    }

    #[test]
    fn test_budget_enforcer_ok() {
        let mut enforcer = BudgetEnforcer::new(1.0); // $1 budget

        let result = enforcer.check_budget(0.05);
        assert!(matches!(result, BudgetCheckResult::Ok { .. }));
        assert!(enforcer.should_proceed(0.05));

        enforcer.record_cost(0.05);
        assert_eq!(enforcer.eval_count, 1);
    }

    #[test]
    fn test_budget_enforcer_approaching_limit() {
        let mut enforcer = BudgetEnforcer::new(1.0);

        enforcer.record_cost(0.85);
        let result = enforcer.check_budget(0.05);

        assert!(matches!(result, BudgetCheckResult::ApproachingLimit { .. }));
        assert!(result.should_proceed(true)); // Should still proceed
    }

    #[test]
    fn test_budget_enforcer_exceeded() {
        let mut enforcer = BudgetEnforcer::new(1.0);

        enforcer.record_cost(0.90);
        let result = enforcer.check_budget(0.20);

        assert!(matches!(
            result,
            BudgetCheckResult::TotalBudgetExceeded { .. }
        ));
        assert!(!result.should_proceed(true)); // Should not proceed with fail_on_exceed
        assert!(result.should_proceed(false)); // Should proceed with warn_only
    }

    #[test]
    fn test_budget_enforcer_per_eval_limit() {
        let enforcer = BudgetEnforcer::new(10.0).with_per_eval_limit(0.1);

        let result = enforcer.check_budget(0.15);

        assert!(matches!(
            result,
            BudgetCheckResult::PerEvalLimitExceeded { .. }
        ));
    }

    #[test]
    fn test_budget_status() {
        let mut enforcer = BudgetEnforcer::new(1.0);

        enforcer.record_cost(0.25);
        enforcer.record_cost(0.25);

        let status = enforcer.status();

        assert_eq!(status.accumulated_cost, 0.50);
        assert_eq!(status.remaining_budget, 0.50);
        assert_eq!(status.eval_count, 2);
        assert!((status.avg_cost_per_eval - 0.25).abs() < 0.001);
        assert!((status.percent_used - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_cost_tracker() {
        let mut tracker = CIPCostTracker::new();

        tracker.record_agent_baseline(0.01, Some(500));
        tracker.record_agent_critical(0.01, Some(500));
        tracker.record_agent_null(0.01, Some(500));
        tracker.record_saboteur_critical(0.002, 400, 300);
        tracker.record_saboteur_null(0.001, 300, 250);
        tracker.record_embeddings(5, 2000, 0.0001);

        let breakdown = tracker.finalize();

        assert!((breakdown.agent_cost.total - 0.03).abs() < 0.001);
        assert!(breakdown.total_cost_usd > 0.03);
        assert!(breakdown.token_summary.grand_total > 0);
    }

    #[test]
    fn test_cost_breakdown_to_metrics() {
        let mut breakdown = CIPCostBreakdown::new();
        breakdown.total_cost_usd = 0.05;
        breakdown.agent_cost.total = 0.03;
        breakdown.saboteur_cost.total = 0.015;
        breakdown.embedding_cost.total = 0.005;
        breakdown.token_summary.grand_total = 5000;

        let metrics = breakdown.to_metrics();

        assert_eq!(metrics.get("cip_cost_total"), Some(&0.05));
        assert_eq!(metrics.get("cip_cost_agent"), Some(&0.03));
        assert_eq!(metrics.get("cip_tokens_total"), Some(&5000.0));
    }

    #[test]
    fn test_standard_pricing() {
        let pricing = get_standard_pricing();

        assert!(pricing.contains_key("gpt-4o"));
        assert!(pricing.contains_key("gpt-4o-mini"));
        assert!(pricing.contains_key("claude-3-5-sonnet"));
        assert!(pricing.contains_key("text-embedding-3-small"));
    }
}
