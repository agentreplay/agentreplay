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

//! Pre-configured evaluation suites for common use cases
//!
//! Instead of manually configuring evaluators for common scenarios,
//! developers can use presets:
//!
//! ```rust,ignore
//! use agentreplay_evals::presets::EvalPreset;
//!
//! // Evaluate a RAG application
//! let results = EvalPreset::RAG
//!     .create_evaluators(llm_client)
//!     .evaluate(&trace)
//!     .await?;
//!
//! // Evaluate an agent
//! let results = EvalPreset::Agent
//!     .create_evaluators(llm_client)
//!     .with_threshold("task_completion", 0.8)
//!     .evaluate(&trace)
//!     .await?;
//! ```

use crate::{
    evaluators::*, llm_client::LLMClient, EvalConfig, EvalError, EvalResult, Evaluator,
    TraceContext,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Pre-configured evaluation suites for common use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalPreset {
    /// RAG (Retrieval-Augmented Generation) applications
    ///
    /// Evaluates:
    /// - Faithfulness (hallucination detection)
    /// - Context quality (RAGAS)
    /// - Answer relevance
    /// - Latency and cost
    RAG,

    /// Conversational AI agents
    ///
    /// Evaluates:
    /// - Task completion (PRIMARY metric)
    /// - Tool correctness
    /// - Trajectory efficiency
    /// - Latency and cost
    Agent,

    /// Code generation systems
    ///
    /// Evaluates:
    /// - Task completion
    /// - Code quality (G-Eval with custom criteria)
    /// - Latency and cost
    CodeGen,

    /// Content generation (articles, summaries, etc.)
    ///
    /// Evaluates:
    /// - Quality (G-Eval: coherence, fluency, relevance)
    /// - Toxicity
    /// - Latency and cost
    ContentGen,

    /// Minimal evaluation (latency and cost only)
    ///
    /// Use this for baseline performance measurements
    /// without quality evaluation
    Minimal,
}

impl EvalPreset {
    /// Create evaluators for this preset
    pub fn create_evaluators(&self, llm_client: Arc<dyn LLMClient>) -> Vec<Arc<dyn Evaluator>> {
        match self {
            EvalPreset::RAG => vec![
                Arc::new(HallucinationDetector::new(llm_client.clone())),
                Arc::new(RagasEvaluator::new(llm_client.clone())),
                Arc::new(RelevanceEvaluator::new()),
                Arc::new(LatencyBenchmark::new()),
                Arc::new(CostAnalyzer::new()),
            ],
            EvalPreset::Agent => vec![
                Arc::new(TaskCompletionEvaluator::new(llm_client.clone())),
                Arc::new(ToolCorrectnessEvaluator::new()),
                Arc::new(TrajectoryEfficiencyEvaluator::new()),
                Arc::new(LatencyBenchmark::new()),
                Arc::new(CostAnalyzer::new()),
            ],
            EvalPreset::CodeGen => vec![
                Arc::new(TaskCompletionEvaluator::new(llm_client.clone())),
                Arc::new(
                    GEval::with_criteria(
                        llm_client.clone(),
                        vec![
                            crate::evaluators::g_eval::EvalCriterion {
                                name: "correctness".to_string(),
                                description: "Does the code correctly implement the specification?"
                                    .to_string(),
                                scale: (1, 5),
                                weight: 2.0,
                            },
                            crate::evaluators::g_eval::EvalCriterion {
                                name: "efficiency".to_string(),
                                description: "Is the code efficient (time/space complexity)?"
                                    .to_string(),
                                scale: (1, 5),
                                weight: 1.0,
                            },
                            crate::evaluators::g_eval::EvalCriterion {
                                name: "readability".to_string(),
                                description: "Is the code well-structured and readable?"
                                    .to_string(),
                                scale: (1, 5),
                                weight: 1.0,
                            },
                            crate::evaluators::g_eval::EvalCriterion {
                                name: "robustness".to_string(),
                                description: "Does the code handle edge cases and errors?"
                                    .to_string(),
                                scale: (1, 5),
                                weight: 1.5,
                            },
                        ],
                    )
                    .with_probability_normalization(true),
                ),
                Arc::new(LatencyBenchmark::new()),
                Arc::new(CostAnalyzer::new()),
            ],
            EvalPreset::ContentGen => vec![
                Arc::new(GEval::new(llm_client.clone())),
                Arc::new(ToxicityDetector::new()),
                Arc::new(LatencyBenchmark::new()),
                Arc::new(CostAnalyzer::new()),
            ],
            EvalPreset::Minimal => vec![
                Arc::new(LatencyBenchmark::new()),
                Arc::new(CostAnalyzer::new()),
            ],
        }
    }

    /// Get a description of this preset
    pub fn description(&self) -> &'static str {
        match self {
            EvalPreset::RAG => {
                "Evaluates RAG applications: faithfulness, context quality, answer relevance"
            }
            EvalPreset::Agent => {
                "Evaluates agents: task completion, tool correctness, trajectory efficiency"
            }
            EvalPreset::CodeGen => {
                "Evaluates code generation: correctness, efficiency, readability, robustness"
            }
            EvalPreset::ContentGen => {
                "Evaluates content generation: coherence, fluency, relevance, toxicity"
            }
            EvalPreset::Minimal => "Minimal evaluation: latency and cost only",
        }
    }

    /// Get all available presets
    pub fn all() -> Vec<Self> {
        vec![
            EvalPreset::RAG,
            EvalPreset::Agent,
            EvalPreset::CodeGen,
            EvalPreset::ContentGen,
            EvalPreset::Minimal,
        ]
    }
}

/// Fluent builder API for custom evaluations
///
/// Example usage:
/// ```rust,ignore
/// let results = EvalBuilder::new()
///     .with_preset(EvalPreset::Agent)
///     .add_evaluator(CustomEvaluator::new())
///     .with_threshold("task_completion", 0.8)
///     .with_threshold("hallucination", 0.2)
///     .fail_fast(true)
///     .parallel(true)
///     .evaluate(&trace)
///     .await?;
/// ```
pub struct EvalBuilder {
    evaluators: Vec<Arc<dyn Evaluator>>,
    thresholds: HashMap<String, f64>,
    config: EvalConfig,
    fail_fast: bool,
    parallel: bool,
}

impl EvalBuilder {
    /// Create a new evaluation builder
    pub fn new() -> Self {
        Self {
            evaluators: Vec::new(),
            thresholds: HashMap::new(),
            config: EvalConfig::default(),
            fail_fast: false,
            parallel: true, // Parallel by default for performance
        }
    }

    /// Add evaluators from a preset
    pub fn with_preset(mut self, preset: EvalPreset, llm_client: Arc<dyn LLMClient>) -> Self {
        let preset_evaluators = preset.create_evaluators(llm_client);
        self.evaluators.extend(preset_evaluators);
        self
    }

    /// Add a custom evaluator
    pub fn add_evaluator(mut self, evaluator: Arc<dyn Evaluator>) -> Self {
        self.evaluators.push(evaluator);
        self
    }

    /// Set pass/fail threshold for a specific evaluator
    pub fn with_threshold(mut self, evaluator_id: &str, threshold: f64) -> Self {
        self.thresholds.insert(evaluator_id.to_string(), threshold);
        self
    }

    /// Stop evaluation on first failure (default: false)
    pub fn fail_fast(mut self, enabled: bool) -> Self {
        self.fail_fast = enabled;
        self
    }

    /// Run evaluators in parallel (default: true)
    pub fn parallel(mut self, enabled: bool) -> Self {
        self.parallel = enabled;
        self
    }

    /// Set evaluation configuration
    pub fn with_config(mut self, config: EvalConfig) -> Self {
        self.config = config;
        self
    }

    /// Set maximum concurrent evaluations
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.config.max_concurrent = max;
        self
    }

    /// Set timeout per evaluation (seconds)
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.config.timeout_secs = secs;
        self
    }

    /// Enable result caching
    pub fn with_cache(mut self, enabled: bool) -> Self {
        self.config.enable_cache = enabled;
        self
    }

    /// Build and return the configured evaluators and config
    pub fn build(self) -> (Vec<Arc<dyn Evaluator>>, EvalConfig, EvalBuilderOptions) {
        let options = EvalBuilderOptions {
            thresholds: self.thresholds,
            fail_fast: self.fail_fast,
            parallel: self.parallel,
        };
        (self.evaluators, self.config, options)
    }

    /// Evaluate a trace with the configured evaluators
    pub async fn evaluate(self, trace: &TraceContext) -> Result<EvalResults, EvalError> {
        let (evaluators, _config, options) = self.build();

        if evaluators.is_empty() {
            return Err(EvalError::InvalidInput(
                "No evaluators configured".to_string(),
            ));
        }

        let mut results = Vec::new();
        let mut all_passed = true;

        if options.parallel {
            // Run evaluators in parallel
            let mut futures = Vec::new();

            for evaluator in evaluators {
                let trace_clone = trace.clone();
                let future = async move { evaluator.evaluate(&trace_clone).await };
                futures.push(future);
            }

            // Execute all in parallel
            let eval_results = futures::future::join_all(futures).await;

            for result in eval_results {
                match result {
                    Ok(eval_result) => {
                        let passed = eval_result.passed;
                        results.push(eval_result);

                        if !passed {
                            all_passed = false;
                            if options.fail_fast {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        } else {
            // Run evaluators sequentially
            for evaluator in evaluators {
                let eval_result = evaluator.evaluate(trace).await?;
                let passed = eval_result.passed;
                results.push(eval_result);

                if !passed {
                    all_passed = false;
                    if options.fail_fast {
                        break;
                    }
                }
            }
        }

        Ok(EvalResults {
            results,
            all_passed,
        })
    }
}

impl Default for EvalBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Options for evaluation execution
#[derive(Debug, Clone)]
pub struct EvalBuilderOptions {
    pub thresholds: HashMap<String, f64>,
    pub fail_fast: bool,
    pub parallel: bool,
}

/// Results from evaluating a trace
#[derive(Debug, Clone)]
pub struct EvalResults {
    pub results: Vec<EvalResult>,
    pub all_passed: bool,
}

impl EvalResults {
    /// Check if all evaluations passed
    pub fn passed(&self) -> bool {
        self.all_passed
    }

    /// Get evaluations that failed
    pub fn failures(&self) -> Vec<&EvalResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }

    /// Get evaluations that passed
    pub fn successes(&self) -> Vec<&EvalResult> {
        self.results.iter().filter(|r| r.passed).collect()
    }

    /// Get overall summary statistics
    pub fn summary(&self) -> EvalSummary {
        let total = self.results.len();
        let passed = self.successes().len();
        let failed = self.failures().len();

        let total_cost: f64 = self.results.iter().filter_map(|r| r.cost).sum();

        let avg_confidence: f64 = if total > 0 {
            self.results.iter().map(|r| r.confidence).sum::<f64>() / total as f64
        } else {
            0.0
        };

        let total_duration_ms: u64 = self.results.iter().filter_map(|r| r.duration_ms).sum();

        EvalSummary {
            total,
            passed,
            failed,
            total_cost,
            avg_confidence,
            total_duration_ms,
        }
    }
}

/// Summary statistics for evaluation results
#[derive(Debug, Clone)]
pub struct EvalSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub total_cost: f64,
    pub avg_confidence: f64,
    pub total_duration_ms: u64,
}

impl std::fmt::Display for EvalSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Evaluations: {}/{} passed, ${:.4} cost, {:.1}% avg confidence, {}ms total",
            self.passed,
            self.total,
            self.total_cost,
            self.avg_confidence * 100.0,
            self.total_duration_ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_descriptions() {
        for preset in EvalPreset::all() {
            assert!(!preset.description().is_empty());
        }
    }

    #[test]
    fn test_eval_builder() {
        let builder = EvalBuilder::new()
            .with_threshold("task_completion", 0.8)
            .with_threshold("hallucination", 0.2)
            .fail_fast(true)
            .parallel(false)
            .max_concurrent(5)
            .timeout_secs(60);

        let (_evaluators, config, options) = builder.build();

        assert_eq!(config.max_concurrent, 5);
        assert_eq!(config.timeout_secs, 60);
        assert!(options.fail_fast);
        assert!(!options.parallel);
        assert_eq!(options.thresholds.len(), 2);
    }
}
