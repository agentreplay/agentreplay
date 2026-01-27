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

// agents/src/observability.rs
//! Observability for Flowtrace and LLM applications
//!
//! Flowtrace observes itself using its own database (dogfooding).
//! Provides comprehensive tracing and metrics following semantic conventions.

pub mod batcher;
pub mod config;
pub mod genai_conventions;
pub mod genai_instrumentation;
pub mod metrics;
pub mod span_mapper;
pub mod storage_metrics;
pub mod tracer;

pub use metrics::{MetricKey, MetricValue, MetricsAggregator};
pub use storage_metrics::{WriteAmplificationMetrics, WriteAmplificationReport};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{span, Level, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Configuration for observability setup
///
/// Flowtrace observes itself using its own database (dogfooding).
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Service name for identification
    pub service_name: String,

    /// Flowtrace server endpoint (e.g., "http://localhost:9600")
    pub flowtrace_endpoint: String,

    /// Environment (production, staging, development)
    pub environment: String,

    /// Service version
    pub version: String,

    /// Whether observability is enabled
    pub enabled: bool,

    /// API key for Flowtrace authentication
    pub api_key: Option<String>,

    /// Project identifier
    pub project: Option<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "flowtrace-app".to_string(),
            flowtrace_endpoint: "http://localhost:9600".to_string(),
            environment: "development".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            enabled: true,
            api_key: None,
            project: None,
        }
    }
}

impl ObservabilityConfig {
    /// Create configuration from environment variables
    ///
    /// Supported environment variables:
    /// - FLOWTRACE_ENABLED: Enable/disable observability (default: true)
    /// - FLOWTRACE_API_KEY: API key for authentication
    /// - FLOWTRACE_ENDPOINT: Flowtrace server endpoint (default: http://localhost:9600)
    /// - FLOWTRACE_PROJECT: Project identifier
    /// - FLOWTRACE_SERVICE_NAME: Service name
    /// - FLOWTRACE_ENVIRONMENT: Environment (production/staging/development)
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Check if observability is enabled
        if let Ok(enabled) = std::env::var("FLOWTRACE_ENABLED") {
            config.enabled = enabled.to_lowercase() == "true" || enabled == "1";
        }

        // API key for Flowtrace authentication
        if let Ok(api_key) = std::env::var("FLOWTRACE_API_KEY") {
            config.api_key = Some(api_key);
        }

        // Flowtrace endpoint
        if let Ok(endpoint) = std::env::var("FLOWTRACE_ENDPOINT") {
            config.flowtrace_endpoint = endpoint;
        }

        // Project identifier
        if let Ok(project) = std::env::var("FLOWTRACE_PROJECT") {
            config.project = Some(project);
        }

        // Service name
        if let Ok(service_name) = std::env::var("FLOWTRACE_SERVICE_NAME") {
            config.service_name = service_name;
        }

        // Environment
        if let Ok(environment) = std::env::var("FLOWTRACE_ENVIRONMENT") {
            config.environment = environment;
        }

        config
    }
}

/// Initialize observability with Flowtrace self-hosting
///
/// Instead of exporting to external OTLP collectors or Langfuse,
/// Flowtrace observes itself by writing traces to its own database.
/// This is true dogfooding - the observability platform using its own capabilities.
pub fn init_observability(config: ObservabilityConfig) -> Result<()> {
    if !config.enabled {
        tracing::info!("Flowtrace observability disabled");
        return Ok(());
    }

    // Set up basic tracing subscriber without external exporters
    // Traces will be batched and sent to Flowtrace's own REST API
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(
        service = %config.service_name,
        environment = %config.environment,
        flowtrace_endpoint = %config.flowtrace_endpoint,
        "Flowtrace self-hosting observability initialized"
    );

    // TODO: Set up batching layer that sends traces to Flowtrace's REST API
    // This will use the batcher module to efficiently batch and send traces
    // to POST /api/v1/traces endpoint with proper authentication

    Ok(())
}

/// GenAI semantic convention attributes
pub mod gen_ai {
    pub const OPERATION_NAME: &str = "gen_ai.operation.name";
    pub const SYSTEM: &str = "gen_ai.system";
    pub const REQUEST_MODEL: &str = "gen_ai.request.model";
    pub const REQUEST_TEMPERATURE: &str = "gen_ai.request.temperature";
    pub const REQUEST_MAX_TOKENS: &str = "gen_ai.request.max_tokens";
    pub const REQUEST_TOP_P: &str = "gen_ai.request.top_p";
    pub const USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
    pub const USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
    pub const RESPONSE_FINISH_REASON: &str = "gen_ai.response.finish_reason";
    pub const RESPONSE_ID: &str = "gen_ai.response.id";
}

/// Custom agent semantic conventions
pub mod agent {
    pub const NAME: &str = "agent.name";
    pub const OPERATION: &str = "agent.operation";
    pub const POLICY: &str = "agent.policy";
    pub const VERSION: &str = "agent.version";
}

/// Retrieval semantic conventions
pub mod retrieval {
    pub const STRATEGY: &str = "retrieval.strategy";
    pub const CANDIDATES: &str = "retrieval.candidates";
    pub const RETURNED: &str = "retrieval.returned";
    pub const VECTOR_SCORE_AVG: &str = "retrieval.vector_score.avg";
    pub const KEYWORD_SCORE_AVG: &str = "retrieval.keyword_score.avg";
}

/// Evaluation semantic conventions
pub mod evaluation {
    pub const HALLUCINATION_SCORE: &str = "evaluation.hallucination_score";
    pub const RELEVANCE_SCORE: &str = "evaluation.relevance_score";
    pub const GROUNDEDNESS_SCORE: &str = "evaluation.groundedness_score";
    pub const TOXICITY_SCORE: &str = "evaluation.toxicity_score";
    pub const PASSED: &str = "evaluation.passed";
}

/// LLM usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub cost_usd: f64,
}

/// LLM response wrapper with tracing
#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: String,
    pub model: String,
    pub finish_reason: String,
    pub usage: LLMUsage,
    pub response_id: Option<String>,
}

/// Cost calculator for different models
pub struct CostCalculator {
    input_cost_per_1k: f64,
    output_cost_per_1k: f64,
}

impl CostCalculator {
    /// Create a new cost calculator for a specific model
    ///
    /// Pricing as of 2024-2025 (update regularly):
    /// - OpenAI GPT-4o: $2.50/$10 per 1M tokens = $0.0025/$0.01 per 1K
    /// - OpenAI GPT-4o-mini: $0.15/$0.60 per 1M tokens = $0.00015/$0.0006 per 1K
    /// - OpenAI GPT-4-turbo: $10/$30 per 1M tokens = $0.01/$0.03 per 1K
    /// - OpenAI GPT-3.5-turbo: $0.50/$1.50 per 1M tokens = $0.0005/$0.0015 per 1K
    /// - Anthropic Claude 3.5 Sonnet: $3/$15 per 1M tokens = $0.003/$0.015 per 1K
    /// - Anthropic Claude 3 Opus: $15/$75 per 1M tokens = $0.015/$0.075 per 1K
    /// - Anthropic Claude 3 Haiku: $0.25/$1.25 per 1M tokens = $0.00025/$0.00125 per 1K
    /// - Google Gemini 1.5 Pro: $3.50/$10.50 per 1M tokens = $0.0035/$0.0105 per 1K
    /// - Google Gemini 1.5 Flash: $0.075/$0.30 per 1M tokens = $0.000075/$0.0003 per 1K
    /// - Cohere Command R+: $3/$15 per 1M tokens = $0.003/$0.015 per 1K
    /// - Cohere Command R: $0.50/$1.50 per 1M tokens = $0.0005/$0.0015 per 1K
    pub fn for_model(model: &str) -> Self {
        let model_lower = model.to_lowercase();

        // Normalize model names and match - return per-1K pricing
        match () {
            // OpenAI models
            _ if model_lower.contains("gpt-4o-mini") => Self {
                input_cost_per_1k: 0.00015,
                output_cost_per_1k: 0.0006,
            },
            _ if model_lower.contains("gpt-4o") => Self {
                input_cost_per_1k: 0.0025,
                output_cost_per_1k: 0.01,
            },
            _ if model_lower.contains("gpt-4-turbo") || model_lower.contains("gpt-4-1106") => {
                Self {
                    input_cost_per_1k: 0.01,
                    output_cost_per_1k: 0.03,
                }
            }
            _ if model_lower.contains("gpt-4") => Self {
                input_cost_per_1k: 0.03, // Legacy GPT-4
                output_cost_per_1k: 0.06,
            },
            _ if model_lower.contains("gpt-3.5-turbo") => Self {
                input_cost_per_1k: 0.0005,
                output_cost_per_1k: 0.0015,
            },

            // Anthropic Claude models
            _ if model_lower.contains("claude-opus-4") || model_lower.contains("claude-3-opus") => {
                Self {
                    input_cost_per_1k: 0.015,
                    output_cost_per_1k: 0.075,
                }
            }
            _ if model_lower.contains("claude-sonnet-4.5")
                || model_lower.contains("claude-sonnet-4")
                || model_lower.contains("claude-3.5-sonnet")
                || model_lower.contains("claude-3-5-sonnet") =>
            {
                Self {
                    input_cost_per_1k: 0.003,
                    output_cost_per_1k: 0.015,
                }
            }
            _ if model_lower.contains("claude-3-sonnet") => Self {
                input_cost_per_1k: 0.003,
                output_cost_per_1k: 0.015,
            },
            _ if model_lower.contains("claude-haiku-4")
                || model_lower.contains("claude-3-haiku") =>
            {
                Self {
                    input_cost_per_1k: 0.00025,
                    output_cost_per_1k: 0.00125,
                }
            }
            _ if model_lower.contains("claude-2") => Self {
                input_cost_per_1k: 0.008,
                output_cost_per_1k: 0.024,
            },

            // Google Gemini models
            _ if model_lower.contains("gemini-1.5-pro")
                || model_lower.contains("gemini-pro-1.5") =>
            {
                Self {
                    input_cost_per_1k: 0.0035,
                    output_cost_per_1k: 0.0105,
                }
            }
            _ if model_lower.contains("gemini-1.5-flash")
                || model_lower.contains("gemini-flash-1.5") =>
            {
                Self {
                    input_cost_per_1k: 0.000075,
                    output_cost_per_1k: 0.0003,
                }
            }
            _ if model_lower.contains("gemini-pro") => Self {
                input_cost_per_1k: 0.0005,
                output_cost_per_1k: 0.0015,
            },

            // Cohere models
            _ if model_lower.contains("command-r-plus") || model_lower.contains("command-r+") => {
                Self {
                    input_cost_per_1k: 0.003,
                    output_cost_per_1k: 0.015,
                }
            }
            _ if model_lower.contains("command-r") => Self {
                input_cost_per_1k: 0.0005,
                output_cost_per_1k: 0.0015,
            },

            // Mistral models
            _ if model_lower.contains("mistral-large") => Self {
                input_cost_per_1k: 0.008,
                output_cost_per_1k: 0.024,
            },
            _ if model_lower.contains("mistral-medium") => Self {
                input_cost_per_1k: 0.0027,
                output_cost_per_1k: 0.0081,
            },
            _ if model_lower.contains("mistral-small") => Self {
                input_cost_per_1k: 0.001,
                output_cost_per_1k: 0.003,
            },

            // Unknown model
            _ => {
                tracing::warn!(
                    model = %model,
                    "Unknown model for cost calculation, using default pricing of $0"
                );
                Self {
                    input_cost_per_1k: 0.0,
                    output_cost_per_1k: 0.0,
                }
            }
        }
    }

    pub fn calculate(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * self.input_cost_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * self.output_cost_per_1k;
        input_cost + output_cost
    }

    /// Get model pricing information as a string
    pub fn pricing_info(&self) -> String {
        format!(
            "${:.6}/1K input, ${:.6}/1K output",
            self.input_cost_per_1k, self.output_cost_per_1k
        )
    }
}

/// Tracer for LLM calls with automatic instrumentation
pub struct LLMTracer {
    model: String,
    system: String,
}

impl LLMTracer {
    pub fn new(model: String, system: String) -> Self {
        Self { model, system }
    }

    /// Create a span for an LLM generation
    pub fn trace_generation<F, Fut, T>(
        &self,
        operation: &str,
        f: F,
    ) -> impl std::future::Future<Output = Result<T>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let span = span!(
            Level::INFO,
            "gen_ai.chat_completion",
            gen_ai.operation.name = operation,
            gen_ai.system = %self.system,
            gen_ai.request.model = %self.model,
        );

        async move {
            let _enter = span.enter();
            let start = Instant::now();

            let result = f().await;

            let duration_ms = start.elapsed().as_millis();
            Span::current().record("llm.latency_ms", duration_ms as i64);

            result
        }
    }

    /// Record LLM usage metrics
    pub fn record_usage(&self, usage: &LLMUsage, finish_reason: &str) {
        let span = Span::current();

        span.record(gen_ai::USAGE_INPUT_TOKENS, usage.input_tokens as i64);
        span.record(gen_ai::USAGE_OUTPUT_TOKENS, usage.output_tokens as i64);
        span.record(gen_ai::RESPONSE_FINISH_REASON, finish_reason);
        span.record("gen_ai.usage.cost_usd", usage.cost_usd);

        // Also log as event for easier querying
        tracing::info!(
            model = %self.model,
            input_tokens = usage.input_tokens,
            output_tokens = usage.output_tokens,
            cost_usd = %usage.cost_usd,
            finish_reason = %finish_reason,
            "LLM generation completed"
        );
    }
}

/// Tracer for agent operations
pub struct AgentTracer {
    agent_name: String,
}

impl AgentTracer {
    pub fn new(agent_name: String) -> Self {
        Self { agent_name }
    }

    /// Trace a retrieval operation
    #[tracing::instrument(
        name = "agent.retrieval",
        skip(self, f),
        fields(
            agent.name = %self.agent_name,
            agent.operation = "context_retrieval"
        )
    )]
    pub async fn trace_retrieval<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration_ms = start.elapsed().as_millis();

        Span::current().record("retrieval.latency_ms", duration_ms as i64);

        result
    }

    /// Record retrieval metrics
    pub fn record_retrieval_metrics(
        &self,
        strategy: &str,
        total_candidates: usize,
        returned: usize,
        avg_vector_score: f64,
        avg_keyword_score: f64,
    ) {
        let span = Span::current();

        span.record(retrieval::STRATEGY, strategy);
        span.record(retrieval::CANDIDATES, total_candidates as i64);
        span.record(retrieval::RETURNED, returned as i64);
        span.record(retrieval::VECTOR_SCORE_AVG, avg_vector_score);
        span.record(retrieval::KEYWORD_SCORE_AVG, avg_keyword_score);

        tracing::info!(
            agent = %self.agent_name,
            strategy = %strategy,
            candidates = total_candidates,
            returned = returned,
            precision = %(returned as f64 / total_candidates as f64),
            "Retrieval completed"
        );
    }
}

/// Evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub hallucination_score: f64,
    pub relevance_score: f64,
    pub groundedness_score: f64,
    pub toxicity_score: f64,
    pub passed: bool,
}

/// Tracer for evaluation operations
pub struct EvaluationTracer;

impl EvaluationTracer {
    /// Trace an evaluation run
    #[tracing::instrument(
        name = "evaluation.run",
        skip(f),
        fields(
            evaluation.type = "llm_response"
        )
    )]
    pub async fn trace_evaluation<F, Fut>(f: F) -> Result<EvaluationResult>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<EvaluationResult>>,
    {
        let start = Instant::now();
        let result = f().await?;
        let duration_ms = start.elapsed().as_millis();

        // Record evaluation results
        let span = Span::current();
        span.record(evaluation::HALLUCINATION_SCORE, result.hallucination_score);
        span.record(evaluation::RELEVANCE_SCORE, result.relevance_score);
        span.record(evaluation::GROUNDEDNESS_SCORE, result.groundedness_score);
        span.record(evaluation::TOXICITY_SCORE, result.toxicity_score);
        span.record(evaluation::PASSED, result.passed);
        span.record("evaluation.latency_ms", duration_ms as i64);

        tracing::info!(
            hallucination = %result.hallucination_score,
            relevance = %result.relevance_score,
            groundedness = %result.groundedness_score,
            toxicity = %result.toxicity_score,
            passed = result.passed,
            "Evaluation completed"
        );

        Ok(result)
    }
}

/// Example: Instrumented agent service
#[cfg(feature = "example")]
pub mod example {
    use super::*;

    pub struct InstrumentedAgent {
        agent_tracer: AgentTracer,
        llm_tracer: LLMTracer,
    }

    impl InstrumentedAgent {
        pub fn new(agent_name: &str, model: &str) -> Self {
            Self {
                agent_tracer: AgentTracer::new(agent_name.to_string()),
                llm_tracer: LLMTracer::new(model.to_string(), "anthropic".to_string()),
            }
        }

        /// Process a user query with full tracing
        #[tracing::instrument(
            name = "agent.process_query",
            skip(self),
            fields(
                query.length = %query.len()
            )
        )]
        pub async fn process_query(&self, query: &str) -> Result<String> {
            // Step 1: Retrieve context
            let context = self
                .agent_tracer
                .trace_retrieval(|| async {
                    // Simulate retrieval
                    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                    Ok(vec!["context1".to_string(), "context2".to_string()])
                })
                .await?;

            // Record retrieval metrics
            self.agent_tracer.record_retrieval_metrics(
                "hybrid",
                100,           // total candidates
                context.len(), // returned
                0.85,          // avg vector score
                0.72,          // avg keyword score
            );

            // Step 2: Generate response
            let response = self
                .llm_tracer
                .trace_generation("chat_completion", || async {
                    // Simulate LLM call
                    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

                    let usage = LLMUsage {
                        input_tokens: 450,
                        output_tokens: 312,
                        total_tokens: 762,
                        cost_usd: CostCalculator::for_model("claude-sonnet-4.5")
                            .calculate(450, 312),
                    };

                    Ok(LLMResponse {
                        content: "Generated response".to_string(),
                        model: "claude-sonnet-4.5".to_string(),
                        finish_reason: "stop".to_string(),
                        usage,
                        response_id: Some("resp_123".to_string()),
                    })
                })
                .await?;

            // Record usage
            self.llm_tracer
                .record_usage(&response.usage, &response.finish_reason);

            // Step 3: Evaluate response
            let eval_result = EvaluationTracer::trace_evaluation(|| async {
                // Simulate evaluation
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

                Ok(EvaluationResult {
                    hallucination_score: 0.03,
                    relevance_score: 0.92,
                    groundedness_score: 0.89,
                    toxicity_score: 0.01,
                    passed: true,
                })
            })
            .await?;

            if !eval_result.passed {
                tracing::warn!("Response failed evaluation checks");
            }

            Ok(response.content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculator() {
        let calc = CostCalculator::for_model("claude-sonnet-4.5");
        let cost = calc.calculate(1000, 1000);

        // Should be 0.003 + 0.015 = 0.018
        assert!((cost - 0.018).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_tracing_setup() {
        let config = ObservabilityConfig::default();
        // This would normally initialize tracing, but we skip in tests
        assert_eq!(config.service_name, "flowtrace-app");
    }
}
