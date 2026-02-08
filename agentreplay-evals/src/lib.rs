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

//! # Agentreplay Evaluation Framework
//!
//! A modular, extensible evaluation framework for LLM agent traces.
//!
//! ## Features
//!
//! - **Trait-based evaluator system**: Easy to implement custom evaluators
//! - **Built-in evaluators**: Hallucination, relevance, toxicity, latency, cost
//! - **Batch evaluation**: High-performance parallel execution
//! - **Result caching**: Avoid redundant evaluations
//! - **LLM-as-judge**: Use LLMs to evaluate LLM outputs
//!
//! ## Example
//!
//! ```rust,ignore
//! use agentreplay_evals::{Evaluator, TraceContext, EvaluatorRegistry};
//! use agentreplay_evals::evaluators::HallucinationDetector;
//! use agentreplay_evals::llm_client::OpenAIClient;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create registry
//!     let registry = EvaluatorRegistry::new();
//!
//!     // Create LLM client
//!     let llm_client = Arc::new(OpenAIClient::new(
//!         std::env::var("OPENAI_API_KEY").unwrap(),
//!         "gpt-4o-mini".to_string()
//!     ));
//!
//!     // Register evaluators
//!     let hallucination = HallucinationDetector::new(llm_client);
//!     registry.register(Arc::new(hallucination)).unwrap();
//!
//!     // Evaluate trace
//!     let trace = TraceContext { /* ... */ };
//!     let results = registry.evaluate_trace(&trace, vec!["hallucination_v1".to_string()]).await.unwrap();
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub mod actionable_feedback;
pub mod cache;
pub mod comparator;
pub mod custom_metrics;
pub mod evaluators;
pub mod llm_client;
pub mod online_evaluator;
pub mod presets;
pub mod progressive_eval;
pub mod registry;
pub mod statistics;
pub mod trial_sandbox;
pub mod trace_summarizer;

pub use actionable_feedback::{
    ActionableFeedback, FailureCategory, FailureLocation, FailureMode, FeedbackBuilder,
    ImprovementSuggestion, Severity, SimilarPassingTrace,
};
pub use agentreplay_core::{EvalResultV1 as EvalResult, MetricValueV1 as MetricValue};
pub use comparator::{
    Comparator, ComparisonResult, EffectSize, MetricComparison, RecommendedAction, Winner,
};
pub use custom_metrics::{
    CustomMetric, MetricDefinition, MetricDirection, MetricRegistry, MetricType,
};
pub use online_evaluator::OnlineEvaluator;
pub use presets::{EvalBuilder, EvalPreset, EvalResults, EvalSummary};
pub use progressive_eval::{
    EvalPhase, HeuristicEvaluator, PhaseResult, ProgressiveEvalConfig, ProgressiveEvalUpdate,
    ProgressiveEvaluator,
};
pub use registry::{EvaluatorRegistry, TaskEvaluationOutput};
pub use statistics::{
    Bootstrap, BootstrapCI, BootstrapMethod, InterRaterReliability, KappaInterpretation,
    KappaResult, PowerAnalysis, PowerAnalyzer, PowerInterpretation, RetrospectivePower, TestType,
    WeightedKappaResult,
};
pub use trial_sandbox::{apply_outcome_v2, TrialRunner, TrialSandbox};
pub use trace_summarizer::{HierarchicalSummary, SpanSummary, SummarizerConfig, TraceSummarizer};

/// Core trait that all evaluators must implement
#[async_trait]
pub trait Evaluator: Send + Sync {
    /// Unique identifier for this evaluator (e.g., "hallucination_v1")
    fn id(&self) -> &str;

    /// Evaluate a single trace
    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError>;

    /// Batch evaluation for performance optimization
    /// Default implementation calls evaluate() for each trace
    async fn evaluate_batch(
        &self,
        traces: Vec<&TraceContext>,
    ) -> Result<Vec<EvalResult>, EvalError> {
        let mut results = Vec::new();
        for trace in traces {
            results.push(self.evaluate(trace).await?);
        }
        Ok(results)
    }

    /// Metadata about this evaluator (name, version, costs, etc.)
    fn metadata(&self) -> EvaluatorMetadata;

    /// Whether this evaluator can run in parallel with others
    fn is_parallelizable(&self) -> bool {
        true
    }

    /// Estimated cost per evaluation in USD
    fn cost_per_eval(&self) -> Option<f64> {
        self.metadata().cost_per_eval
    }
}

/// Context for a trace being evaluated
#[derive(Debug, Clone)]
pub struct TraceContext {
    /// Unique trace identifier
    pub trace_id: u128,

    /// All edges in this trace (ordered by causal relationships)
    pub edges: Vec<agentreplay_core::AgentFlowEdge>,

    /// Input to the agent (prompt, query, etc.)
    pub input: Option<String>,

    /// Final output from the agent
    pub output: Option<String>,

    /// Retrieved context (for RAG evaluation)
    pub context: Option<Vec<String>>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,

    /// Canonical transcript/outcome representation (EvalTraceV1)
    pub eval_trace: Option<agentreplay_core::EvalTraceV1>,

    /// Timestamp when trace was created
    pub timestamp_us: u64,
}

/// Result and metric contracts are shared from agentreplay-core

/// Metadata about an evaluator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorMetadata {
    /// Human-readable name
    pub name: String,

    /// Version string (e.g., "1.0.0")
    pub version: String,

    /// Description of what this evaluator does
    pub description: String,

    /// Cost per evaluation in USD (for LLM-based evaluators)
    pub cost_per_eval: Option<f64>,

    /// Average latency in milliseconds
    pub avg_latency_ms: Option<u64>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Author/maintainer
    pub author: Option<String>,
}

/// Errors that can occur during evaluation
#[derive(Debug, Error)]
pub enum EvalError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("LLM client error: {0}")]
    LLMClientError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Evaluation timeout")]
    Timeout,

    #[error("Task panicked: {0}")]
    Panic(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Configuration for evaluation execution
#[derive(Debug, Clone)]
pub struct EvalConfig {
    /// Maximum number of concurrent evaluations
    pub max_concurrent: usize,

    /// Timeout per evaluation in seconds
    pub timeout_secs: u64,

    /// Whether to retry failed evaluations
    pub retry_on_failure: bool,

    /// Maximum number of retries
    pub max_retries: u32,

    /// Whether to cache results
    pub enable_cache: bool,

    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            timeout_secs: 30,
            retry_on_failure: true,
            max_retries: 2,
            enable_cache: true,
            cache_ttl_secs: 3600, // 1 hour
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_value_serialization() {
        let value = MetricValue::Float(0.85);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "0.85");

        let value = MetricValue::Bool(true);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "true");
    }

    #[test]
    fn test_eval_config_default() {
        let config = EvalConfig::default();
        assert_eq!(config.max_concurrent, 10);
        assert_eq!(config.timeout_secs, 30);
        assert!(config.enable_cache);
    }
}
