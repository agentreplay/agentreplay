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

//! Relevance evaluation using semantic similarity

use crate::{
    llm_client::EmbeddingClient, EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue,
    TraceContext,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Relevance evaluator using semantic similarity
///
/// Computes the cosine similarity between input and output embeddings.
/// Supports both simple keyword-based approach and semantic embeddings.
pub struct RelevanceEvaluator {
    threshold: f64,
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
}

impl RelevanceEvaluator {
    /// Create a new relevance evaluator (defaulting to keyword matching)
    pub fn new() -> Self {
        Self {
            threshold: 0.5, // Fail if relevance score < 0.5
            embedding_client: None,
        }
    }

    /// Set embedding client for semantic relevance
    pub fn with_embedding_client(mut self, client: Arc<dyn EmbeddingClient>) -> Self {
        self.embedding_client = Some(client);
        self
    }

    /// Set threshold for pass/fail (default: 0.5)
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Calculate relevance score using simple keyword overlap
    /// This is a placeholder - in production, use embeddings
    fn calculate_keyword_relevance(&self, input: &str, output: &str) -> f64 {
        // Tokenize by whitespace and lowercase
        let input_tokens: std::collections::HashSet<String> = input
            .to_lowercase()
            .split_whitespace()
            .filter(|t| t.len() > 2) // Filter short words
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .collect();

        let output_tokens: std::collections::HashSet<String> = output
            .to_lowercase()
            .split_whitespace()
            .filter(|t| t.len() > 2)
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .collect();

        if input_tokens.is_empty() || output_tokens.is_empty() {
            return 0.0;
        }

        // Calculate Jaccard similarity
        let intersection: usize = input_tokens.intersection(&output_tokens).count();
        let union: usize = input_tokens.union(&output_tokens).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Calculate semantic relevance using embeddings
    async fn calculate_semantic_relevance(
        &self,
        input: &str,
        output: &str,
        client: &dyn EmbeddingClient,
    ) -> Result<f64, EvalError> {
        let embeddings = client
            .embed_batch(&[input.to_string(), output.to_string()])
            .await
            .map_err(|e| EvalError::LLMClientError(e.to_string()))?;

        if embeddings.len() != 2 {
            return Err(EvalError::LLMClientError(
                "Failed to get embeddings for input and output".to_string(),
            ));
        }

        let input_vec = &embeddings[0];
        let output_vec = &embeddings[1];

        // Cosine similarity: (A . B) / (||A|| * ||B||)
        let dot_product: f64 = input_vec
            .iter()
            .zip(output_vec.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f64 = input_vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = output_vec.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return Ok(0.0);
        }

        Ok(dot_product / (norm_a * norm_b))
    }

    /// Determine relevance category based on score
    fn relevance_category(&self, score: f64) -> &'static str {
        if score >= 0.8 {
            "highly_relevant"
        } else if score >= 0.5 {
            "relevant"
        } else if score >= 0.3 {
            "somewhat_relevant"
        } else {
            "not_relevant"
        }
    }
}

impl Default for RelevanceEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for RelevanceEvaluator {
    fn id(&self) -> &str {
        "relevance_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // Extract input and output
        let input = trace
            .input
            .as_ref()
            .ok_or_else(|| EvalError::InvalidInput("No input to evaluate".to_string()))?;

        let output = trace
            .output
            .as_ref()
            .ok_or_else(|| EvalError::InvalidInput("No output to evaluate".to_string()))?;

        // Calculate relevance score
        let (relevance_score, method) = if let Some(client) = &self.embedding_client {
            (
                self.calculate_semantic_relevance(input, output, client.as_ref())
                    .await?,
                "embeddings",
            )
        } else {
            (self.calculate_keyword_relevance(input, output), "keywords")
        };

        let category = self.relevance_category(relevance_score);
        let passed = relevance_score >= self.threshold;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Build metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "relevance_score".to_string(),
            MetricValue::Float(relevance_score),
        );
        metrics.insert(
            "category".to_string(),
            MetricValue::String(category.to_string()),
        );
        metrics.insert(
            "method".to_string(),
            MetricValue::String(method.to_string()),
        );

        let explanation = format!(
            "Relevance score: {:.2} ({}). Input and output {}.",
            relevance_score,
            category,
            if passed {
                "are relevant"
            } else {
                "lack sufficient relevance"
            }
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
            confidence: 0.85, // Lower confidence for keyword-based approach
            cost: Some(0.0),  // No cost for keyword-based approach
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Relevance Evaluator".to_string(),
            version: "1.0.0".to_string(),
            description: "Evaluates semantic relevance between input and output using keyword overlap (placeholder for embeddings).".to_string(),
            cost_per_eval: Some(0.0), // Free for keyword-based, ~$0.00001 with embeddings
            avg_latency_ms: Some(5), // Very fast for keyword-based
            tags: vec![
                "relevance".to_string(),
                "similarity".to_string(),
                "semantic".to_string(),
            ],
            author: Some("Flowtrace".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_relevance_evaluator_high_relevance() {
        let evaluator = RelevanceEvaluator::new();

        let trace = TraceContext {
            trace_id: 123,
            edges: vec![],
            input: Some("What is the capital of France?".to_string()),
            output: Some("The capital of France is Paris.".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let result = evaluator.evaluate(&trace).await.unwrap();

        assert!(result.passed);
        assert_eq!(result.evaluator_id, "relevance_v1");

        if let Some(MetricValue::Float(score)) = result.metrics.get("relevance_score") {
            assert!(*score > 0.3); // Should have some keyword overlap
        } else {
            panic!("Missing relevance_score metric");
        }
    }

    #[tokio::test]
    async fn test_relevance_evaluator_low_relevance() {
        let evaluator = RelevanceEvaluator::new().with_threshold(0.5);

        let trace = TraceContext {
            trace_id: 123,
            edges: vec![],
            input: Some("What is machine learning?".to_string()),
            output: Some("The weather today is sunny.".to_string()),
            context: None,
            metadata: HashMap::new(),
            eval_trace: None,
            timestamp_us: 0,
        };

        let result = evaluator.evaluate(&trace).await.unwrap();

        // Should fail - completely unrelated
        assert!(!result.passed);
    }

    #[test]
    fn test_keyword_relevance() {
        let evaluator = RelevanceEvaluator::new();

        // High similarity
        let score1 = evaluator.calculate_keyword_relevance(
            "machine learning algorithms",
            "algorithms for machine learning",
        );
        assert!(score1 > 0.5);

        // Low similarity
        let score2 = evaluator.calculate_keyword_relevance("machine learning", "weather forecast");
        assert!(score2 < 0.3);
    }
}
