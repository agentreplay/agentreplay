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

//! Custom Metrics Framework
//!
//! This module provides a plugin-based system for defining and executing custom evaluation metrics.
//!
//! ## Example
//!
//! ```rust,ignore
//! use flowtrace_evals::custom_metrics::{CustomMetric, MetricDefinition, MetricType, MetricRegistry};
//!
//! // Define a custom metric
//! let metric = CustomMetric::new(
//!     "response_length",
//!     MetricDefinition {
//!         name: "Response Length".to_string(),
//!         description: "Measures the length of the response".to_string(),
//!         metric_type: MetricType::Numeric,
//!         formula: Some("len(response)".to_string()),
//!         threshold: Some(100.0),
//!         direction: MetricDirection::Lower,
//!     },
//! );
//!
//! // Register and use
//! let registry = MetricRegistry::new();
//! registry.register(metric);
//! ```

use crate::{EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Type of metric value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetricType {
    /// Numeric value (0.0 - 1.0 or any float)
    Numeric,
    /// Boolean pass/fail
    Boolean,
    /// Categorical label
    Categorical,
    /// Count-based metric
    Count,
    /// Latency in milliseconds
    Latency,
    /// Cost in USD
    Cost,
}

/// Direction for metric optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MetricDirection {
    /// Higher values are better
    Higher,
    /// Lower values are better
    Lower,
    /// Target a specific value
    Target(f64),
}

/// Definition of a custom metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    /// Human-readable name
    pub name: String,
    /// Description of what the metric measures
    pub description: String,
    /// Type of metric value
    pub metric_type: MetricType,
    /// Optional formula for calculation (for documentation/display)
    pub formula: Option<String>,
    /// Threshold for pass/fail
    pub threshold: Option<f64>,
    /// Direction for optimization
    pub direction: MetricDirection,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Required fields from trace context
    #[serde(default)]
    pub required_fields: Vec<String>,
}

/// Custom metric calculator function type
pub type MetricCalculator = Arc<dyn Fn(&TraceContext) -> Result<f64, String> + Send + Sync>;

/// A custom metric that can be registered and evaluated
pub struct CustomMetric {
    /// Unique identifier for the metric
    id: String,
    /// Metric definition
    definition: MetricDefinition,
    /// Calculator function
    calculator: MetricCalculator,
    /// Version
    version: String,
}

impl CustomMetric {
    /// Create a new custom metric with a calculator function
    pub fn new(
        id: impl Into<String>,
        definition: MetricDefinition,
        calculator: MetricCalculator,
    ) -> Self {
        Self {
            id: id.into(),
            definition,
            calculator,
            version: "1.0.0".to_string(),
        }
    }

    /// Create a new custom metric with a simple extractor
    pub fn from_extractor<F>(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        extractor: F,
    ) -> Self
    where
        F: Fn(&TraceContext) -> Result<f64, String> + Send + Sync + 'static,
    {
        Self {
            id: id.into(),
            definition: MetricDefinition {
                name: name.into(),
                description: description.into(),
                metric_type: MetricType::Numeric,
                formula: None,
                threshold: None,
                direction: MetricDirection::Higher,
                tags: Vec::new(),
                required_fields: Vec::new(),
            },
            calculator: Arc::new(extractor),
            version: "1.0.0".to_string(),
        }
    }

    /// Set the version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set the threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.definition.threshold = Some(threshold);
        self
    }

    /// Set the direction
    pub fn with_direction(mut self, direction: MetricDirection) -> Self {
        self.definition.direction = direction;
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.definition.tags = tags;
        self
    }

    /// Get the metric definition
    pub fn definition(&self) -> &MetricDefinition {
        &self.definition
    }

    /// Calculate the metric value
    pub fn calculate(&self, trace: &TraceContext) -> Result<f64, String> {
        (self.calculator)(trace)
    }

    /// Check if the metric passes based on threshold and direction
    pub fn passes(&self, value: f64) -> bool {
        match (&self.definition.threshold, &self.definition.direction) {
            (Some(threshold), MetricDirection::Higher) => value >= *threshold,
            (Some(threshold), MetricDirection::Lower) => value <= *threshold,
            (Some(threshold), MetricDirection::Target(target)) => {
                (value - target).abs() <= (threshold - target).abs()
            }
            (None, _) => true, // No threshold means always pass
        }
    }
}

#[async_trait]
impl Evaluator for CustomMetric {
    fn id(&self) -> &str {
        &self.id
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let value = self.calculate(trace).map_err(EvalError::Internal)?;
        let passed = self.passes(value);

        let mut metrics = HashMap::new();
        metrics.insert(self.id.clone(), MetricValue::Float(value));

        Ok(EvalResult {
            evaluator_id: self.id.clone(),
            evaluator_type: Some("custom".to_string()),
            metrics,
            passed,
            explanation: Some(format!(
                "{}: {:.4} (threshold: {:?}, direction: {:?})",
                self.definition.name, value, self.definition.threshold, self.definition.direction
            )),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 1.0,      // Custom metrics are deterministic
            cost: Some(0.0),      // Custom metrics are free
            duration_ms: Some(0), // Very fast computation
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: self.definition.name.clone(),
            description: self.definition.description.clone(),
            version: self.version.clone(),
            cost_per_eval: Some(0.0), // Custom metrics are free (computed locally)
            avg_latency_ms: Some(1),
            tags: self.definition.tags.clone(),
            author: None,
        }
    }

    fn is_parallelizable(&self) -> bool {
        true // Custom metrics should be parallelizable by default
    }
}

/// Registry for managing custom metrics
pub struct MetricRegistry {
    metrics: RwLock<HashMap<String, Arc<CustomMetric>>>,
}

impl Default for MetricRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricRegistry {
    /// Create a new empty metric registry
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(HashMap::new()),
        }
    }

    /// Register a custom metric
    pub fn register(&self, metric: CustomMetric) -> Result<(), String> {
        let id = metric.id.clone();
        if self.metrics.read().contains_key(&id) {
            return Err(format!("Metric '{}' is already registered", id));
        }
        self.metrics.write().insert(id, Arc::new(metric));
        Ok(())
    }

    /// Unregister a custom metric
    pub fn unregister(&self, id: &str) -> bool {
        self.metrics.write().remove(id).is_some()
    }

    /// Get a metric by ID
    pub fn get(&self, id: &str) -> Option<Arc<CustomMetric>> {
        self.metrics.read().get(id).cloned()
    }

    /// List all registered metrics
    pub fn list(&self) -> Vec<(String, MetricDefinition)> {
        self.metrics
            .read()
            .iter()
            .map(|(id, m)| (id.clone(), m.definition.clone()))
            .collect()
    }

    /// Evaluate all registered metrics on a trace
    pub async fn evaluate_all(&self, trace: &TraceContext) -> Result<Vec<EvalResult>, EvalError> {
        let metrics: Vec<_> = self.metrics.read().values().cloned().collect();
        let mut results = Vec::new();

        for metric in metrics {
            match metric.evaluate(trace).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    // Continue on error but log it
                    let mut error_metrics = HashMap::new();
                    error_metrics.insert("error".to_string(), MetricValue::String(e.to_string()));

                    results.push(EvalResult {
                        evaluator_id: metric.id.clone(),
                        evaluator_type: Some("custom".to_string()),
                        metrics: error_metrics,
                        passed: false,
                        explanation: Some(format!("Evaluation error: {}", e)),
                        assertions: Vec::new(),
                        judge_votes: Vec::new(),
                        evidence_refs: Vec::new(),
                        confidence: 0.0,
                        cost: None,
                        duration_ms: None,
                        actionable_feedback: None,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Evaluate specific metrics by ID
    pub async fn evaluate_metrics(
        &self,
        trace: &TraceContext,
        metric_ids: &[String],
    ) -> Result<Vec<EvalResult>, EvalError> {
        let mut results = Vec::new();

        for id in metric_ids {
            if let Some(metric) = self.get(id) {
                results.push(metric.evaluate(trace).await?);
            } else {
                return Err(EvalError::Internal(format!("Metric '{}' not found", id)));
            }
        }

        Ok(results)
    }
}

/// Pre-built custom metrics for common use cases
pub mod prebuilt {
    use super::*;

    /// Response length metric
    pub fn response_length() -> CustomMetric {
        CustomMetric::from_extractor(
            "response_length",
            "Response Length",
            "Measures the character length of the agent's response",
            |trace| Ok(trace.output.as_ref().map(|s| s.len() as f64).unwrap_or(0.0)),
        )
        .with_direction(MetricDirection::Target(500.0))
        .with_threshold(1000.0)
    }

    /// Response word count metric
    pub fn word_count() -> CustomMetric {
        CustomMetric::from_extractor(
            "word_count",
            "Word Count",
            "Counts the number of words in the response",
            |trace| {
                Ok(trace
                    .output
                    .as_ref()
                    .map(|s| s.split_whitespace().count() as f64)
                    .unwrap_or(0.0))
            },
        )
    }

    /// Input/Output ratio metric
    pub fn io_ratio() -> CustomMetric {
        CustomMetric::from_extractor(
            "io_ratio",
            "Input/Output Ratio",
            "Ratio of output length to input length",
            |trace| {
                let input_len = trace.input.as_ref().map(|s| s.len()).unwrap_or(1).max(1) as f64;
                let output_len = trace.output.as_ref().map(|s| s.len()).unwrap_or(0) as f64;
                Ok(output_len / input_len)
            },
        )
        .with_direction(MetricDirection::Target(2.0))
    }

    /// Edge count metric (number of spans/operations in trace)
    pub fn edge_count() -> CustomMetric {
        CustomMetric::from_extractor(
            "edge_count",
            "Edge Count",
            "Number of edges/spans in the trace",
            |trace| Ok(trace.edges.len() as f64),
        )
    }

    /// Total duration metric from trace edges
    pub fn total_duration_ms() -> CustomMetric {
        CustomMetric::from_extractor(
            "total_duration_ms",
            "Total Duration (ms)",
            "Total duration of all edges in milliseconds",
            |trace| {
                let total_us: u64 = trace.edges.iter().map(|e| e.duration_us as u64).sum();
                Ok(total_us as f64 / 1000.0) // Convert microseconds to milliseconds
            },
        )
        .with_direction(MetricDirection::Lower)
        .with_threshold(5000.0) // 5 second threshold
    }

    /// Total token count metric
    pub fn total_tokens() -> CustomMetric {
        CustomMetric::from_extractor(
            "total_tokens",
            "Total Tokens",
            "Total tokens consumed across all edges",
            |trace| {
                let total: u32 = trace.edges.iter().map(|e| e.token_count).sum();
                Ok(total as f64)
            },
        )
        .with_direction(MetricDirection::Lower)
    }

    /// Register all prebuilt metrics
    pub fn register_all(registry: &MetricRegistry) {
        let _ = registry.register(response_length());
        let _ = registry.register(word_count());
        let _ = registry.register(io_ratio());
        let _ = registry.register(edge_count());
        let _ = registry.register(total_duration_ms());
        let _ = registry.register(total_tokens());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowtrace_core::AgentFlowEdge;

    fn sample_trace() -> TraceContext {
        TraceContext {
            trace_id: 1,
            edges: vec![AgentFlowEdge::default()],
            input: Some("Hello, how are you?".to_string()),
            output: Some("I'm doing great! How can I help you today?".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        }
    }

    #[tokio::test]
    async fn test_response_length_metric() {
        let metric = prebuilt::response_length();
        let trace = sample_trace();
        let result = metric.evaluate(&trace).await.unwrap();

        // Check the metric value
        if let Some(MetricValue::Float(score)) = result.metrics.get("response_length") {
            assert_eq!(*score, 42.0); // Length of "I'm doing great! How can I help you today?"
        } else {
            panic!("Expected float metric");
        }
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_word_count_metric() {
        let metric = prebuilt::word_count();
        let trace = sample_trace();
        let result = metric.evaluate(&trace).await.unwrap();

        if let Some(MetricValue::Float(score)) = result.metrics.get("word_count") {
            assert_eq!(*score, 9.0); // 9 words
        } else {
            panic!("Expected float metric");
        }
    }

    #[tokio::test]
    async fn test_registry() {
        let registry = MetricRegistry::new();
        prebuilt::register_all(&registry);

        let trace = sample_trace();
        let results = registry.evaluate_all(&trace).await.unwrap();

        assert_eq!(results.len(), 6); // 6 prebuilt metrics
    }

    #[test]
    fn test_custom_metric_threshold() {
        let metric = CustomMetric::from_extractor("test", "Test", "Test metric", |_| Ok(50.0))
            .with_threshold(60.0)
            .with_direction(MetricDirection::Higher);

        assert!(!metric.passes(50.0)); // 50 < 60, fails
        assert!(metric.passes(60.0)); // 60 >= 60, passes
        assert!(metric.passes(70.0)); // 70 >= 60, passes
    }
}
