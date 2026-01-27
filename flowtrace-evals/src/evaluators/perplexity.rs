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

//! Perplexity Evaluation for Language Model Output Quality
//!
//! Perplexity measures how "surprised" a language model is by a text.
//! Lower perplexity = more fluent/likely text according to the model.
//!
//! ## Use Cases
//!
//! - **Model Comparison**: Compare LLM quality without task-specific evaluation
//! - **Fluency Assessment**: Detect disfluent or unnatural generations
//! - **OOD Detection**: Identify out-of-distribution outputs
//! - **Generation Quality**: Score output quality without human labels
//!
//! ## Implementation Notes
//!
//! True perplexity requires per-token log probabilities from the model.
//! Since most LLM APIs don't expose raw logprobs, we provide:
//! 1. **Pseudo-perplexity**: Using a reference model to score text
//! 2. **Approximations**: Based on available API data (if logprobs returned)
//!
//! ## Formula
//!
//! PPL = exp(-1/T × Σₜ log P(xₜ | x₍<t₎))
//!
//! Where T is the number of tokens and P is the model's probability.

use crate::llm_client::LLMClient;
use crate::{EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Perplexity evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityResult {
    /// Perplexity score (lower = better, 1 = perfect)
    pub perplexity: f64,
    /// Cross-entropy (log of perplexity)
    pub cross_entropy: f64,
    /// Number of tokens evaluated
    pub n_tokens: usize,
    /// Bits per character (alternative metric)
    pub bits_per_char: f64,
    /// Whether this is pseudo-perplexity (using reference model)
    pub is_pseudo: bool,
    /// Reference model used (if pseudo-perplexity)
    pub reference_model: Option<String>,
}

/// Perplexity evaluator using a reference LLM
pub struct PerplexityEvaluator {
    /// LLM client for scoring
    llm_client: Arc<dyn LLMClient>,
    /// Model name for reference
    model_name: String,
    /// Maximum context length
    max_context_length: usize,
    /// Threshold for "good" perplexity (depends on domain)
    threshold: f64,
}

impl PerplexityEvaluator {
    pub fn new(llm_client: Arc<dyn LLMClient>, model_name: String) -> Self {
        Self {
            llm_client,
            model_name,
            max_context_length: 4096,
            threshold: 50.0, // Reasonable for general text
        }
    }

    pub fn with_max_context(mut self, max_len: usize) -> Self {
        self.max_context_length = max_len;
        self
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Estimate pseudo-perplexity using a reference model
    ///
    /// This uses a sliding window approach where we ask the model
    /// to predict/score tokens given context.
    pub async fn pseudo_perplexity(&self, text: &str) -> Result<PerplexityResult, EvalError> {
        if text.is_empty() {
            return Ok(PerplexityResult {
                perplexity: 1.0,
                cross_entropy: 0.0,
                n_tokens: 0,
                bits_per_char: 0.0,
                is_pseudo: true,
                reference_model: Some(self.model_name.clone()),
            });
        }

        // Tokenize (simple whitespace for now)
        let tokens: Vec<&str> = text.split_whitespace().collect();
        let n_tokens = tokens.len();

        if n_tokens == 0 {
            return Ok(PerplexityResult {
                perplexity: 1.0,
                cross_entropy: 0.0,
                n_tokens: 0,
                bits_per_char: 0.0,
                is_pseudo: true,
                reference_model: Some(self.model_name.clone()),
            });
        }

        // For pseudo-perplexity without access to logprobs, we use a completion-based approach
        // This is an approximation: we check if the model "agrees" with the continuation

        // Use sliding window to get per-chunk scores
        let window_size = 10; // Tokens per window
        let mut total_log_prob = 0.0;
        let mut total_scored_tokens = 0;

        let mut i = 0;
        while i < n_tokens {
            let context_end = i.min(n_tokens);
            let context: String = tokens[..context_end].join(" ");

            let prediction_start = i;
            let prediction_end = (i + window_size).min(n_tokens);
            let target: String = tokens[prediction_start..prediction_end].join(" ");

            if target.is_empty() {
                break;
            }

            // Ask model to complete and score based on similarity
            let prompt = format!(
                "Continue this text with exactly the next few words:\n\n{}\n\nContinuation:",
                context
            );

            let response = self
                .llm_client
                .evaluate(prompt)
                .await
                .map_err(|e| EvalError::LLMClientError(format!("LLM error: {:?}", e)))?;

            // Score based on overlap with actual continuation
            let generated = response.content.to_lowercase();
            let target_lower = target.to_lowercase();

            // Use word overlap as probability proxy
            let target_words: std::collections::HashSet<&str> =
                target_lower.split_whitespace().collect();
            let generated_words: std::collections::HashSet<&str> =
                generated.split_whitespace().collect();

            let overlap = target_words.intersection(&generated_words).count();
            let prob = if target_words.is_empty() {
                1.0
            } else {
                (overlap as f64 / target_words.len() as f64).max(0.01)
            };

            total_log_prob += prob.ln();
            total_scored_tokens += prediction_end - prediction_start;

            i = prediction_end;
        }

        // Calculate perplexity
        let avg_log_prob = if total_scored_tokens > 0 {
            total_log_prob / total_scored_tokens as f64
        } else {
            0.0
        };

        let perplexity = (-avg_log_prob).exp();
        let bits_per_char = if !text.is_empty() {
            -total_log_prob / (text.len() as f64 * 2.0_f64.ln())
        } else {
            0.0
        };

        Ok(PerplexityResult {
            perplexity,
            cross_entropy: -avg_log_prob,
            n_tokens: total_scored_tokens,
            bits_per_char,
            is_pseudo: true,
            reference_model: Some(self.model_name.clone()),
        })
    }

    /// Calculate perplexity from provided log probabilities
    ///
    /// This is the accurate method when logprobs are available
    pub fn perplexity_from_logprobs(logprobs: &[f64]) -> PerplexityResult {
        if logprobs.is_empty() {
            return PerplexityResult {
                perplexity: 1.0,
                cross_entropy: 0.0,
                n_tokens: 0,
                bits_per_char: 0.0,
                is_pseudo: false,
                reference_model: None,
            };
        }

        let sum_log_prob: f64 = logprobs.iter().sum();
        let avg_log_prob = sum_log_prob / logprobs.len() as f64;
        let perplexity = (-avg_log_prob).exp();

        PerplexityResult {
            perplexity,
            cross_entropy: -avg_log_prob,
            n_tokens: logprobs.len(),
            bits_per_char: -avg_log_prob / 2.0_f64.ln(),
            is_pseudo: false,
            reference_model: None,
        }
    }
}

#[async_trait]
impl Evaluator for PerplexityEvaluator {
    fn id(&self) -> &str {
        "perplexity_v1"
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Perplexity Evaluator".to_string(),
            version: "1.0.0".to_string(),
            description: "Evaluates text fluency using perplexity scoring".to_string(),
            cost_per_eval: Some(0.001),
            avg_latency_ms: Some(500),
            tags: vec!["fluency".to_string(), "quality".to_string()],
            author: Some("Flowtrace Team".to_string()),
        }
    }

    async fn evaluate(&self, context: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = std::time::Instant::now();

        // Get the output text to evaluate
        let output = context
            .output
            .as_ref()
            .ok_or(EvalError::Internal("No output to evaluate".into()))?;

        let result = self.pseudo_perplexity(output).await?;

        let passed = result.perplexity <= self.threshold;
        let confidence = if result.perplexity > 0.0 {
            // Normalize: lower perplexity = higher confidence
            1.0 / (1.0 + (result.perplexity - 1.0) / self.threshold)
        } else {
            0.0
        };

        let mut metrics = HashMap::new();
        metrics.insert(
            "perplexity".to_string(),
            MetricValue::Float(result.perplexity),
        );
        metrics.insert(
            "cross_entropy".to_string(),
            MetricValue::Float(result.cross_entropy),
        );
        metrics.insert(
            "n_tokens".to_string(),
            MetricValue::Int(result.n_tokens as i64),
        );
        metrics.insert(
            "bits_per_char".to_string(),
            MetricValue::Float(result.bits_per_char),
        );

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("llm_judge".to_string()),
            metrics,
            passed,
            explanation: Some(format!(
                "Perplexity: {:.2} (threshold: {:.1}). {}",
                result.perplexity,
                self.threshold,
                if passed {
                    "Text appears fluent."
                } else {
                    "Text may be disfluent."
                }
            )),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence,
            cost: self.metadata().cost_per_eval,
            duration_ms: Some(start.elapsed().as_millis() as u64),
            actionable_feedback: None,
        })
    }
}

// ============================================================================
// N-gram Based Perplexity (No LLM Required)
// ============================================================================

/// N-gram based perplexity calculator (no LLM required)
///
/// Builds a language model from a reference corpus and scores text.
/// Faster but less accurate than LLM-based perplexity.
pub struct NGramPerplexity {
    /// N-gram order (typically 3 or 4)
    n: usize,
    /// N-gram counts from training corpus
    ngram_counts: HashMap<Vec<String>, usize>,
    /// (N-1)-gram counts for probability calculation
    context_counts: HashMap<Vec<String>, usize>,
    /// Vocabulary size for smoothing
    vocab_size: usize,
    /// Smoothing parameter (Laplace smoothing)
    smoothing: f64,
}

impl NGramPerplexity {
    pub fn new(n: usize) -> Self {
        Self {
            n,
            ngram_counts: HashMap::new(),
            context_counts: HashMap::new(),
            vocab_size: 0,
            smoothing: 1.0,
        }
    }

    /// Train on a reference corpus
    pub fn train(&mut self, texts: &[&str]) {
        let mut vocab: std::collections::HashSet<String> = std::collections::HashSet::new();

        for text in texts {
            let tokens: Vec<String> = format!("<s> {} </s>", text)
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            // Update vocabulary
            for token in &tokens {
                vocab.insert(token.clone());
            }

            // Count n-grams
            for window in tokens.windows(self.n) {
                let ngram: Vec<String> = window.to_vec();
                *self.ngram_counts.entry(ngram).or_insert(0) += 1;
            }

            // Count contexts (n-1 grams)
            for window in tokens.windows(self.n - 1) {
                let context: Vec<String> = window.to_vec();
                *self.context_counts.entry(context).or_insert(0) += 1;
            }
        }

        self.vocab_size = vocab.len();
    }

    /// Calculate perplexity of a text
    pub fn perplexity(&self, text: &str) -> PerplexityResult {
        let tokens: Vec<String> = format!("<s> {} </s>", text)
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if tokens.len() < self.n {
            return PerplexityResult {
                perplexity: 1.0,
                cross_entropy: 0.0,
                n_tokens: 0,
                bits_per_char: 0.0,
                is_pseudo: false,
                reference_model: Some(format!("{}-gram", self.n)),
            };
        }

        let mut log_prob_sum = 0.0;
        let n_tokens = tokens.len() - (self.n - 1);

        for window in tokens.windows(self.n) {
            let ngram: Vec<String> = window.to_vec();
            let context: Vec<String> = window[..self.n - 1].to_vec();

            // Laplace smoothed probability
            let ngram_count = *self.ngram_counts.get(&ngram).unwrap_or(&0) as f64;
            let context_count = *self.context_counts.get(&context).unwrap_or(&0) as f64;

            let prob = (ngram_count + self.smoothing)
                / (context_count + self.smoothing * self.vocab_size as f64);

            log_prob_sum += prob.ln();
        }

        let avg_log_prob = log_prob_sum / n_tokens as f64;
        let perplexity = (-avg_log_prob).exp();

        PerplexityResult {
            perplexity,
            cross_entropy: -avg_log_prob,
            n_tokens,
            bits_per_char: -avg_log_prob / 2.0_f64.ln(),
            is_pseudo: false,
            reference_model: Some(format!("{}-gram", self.n)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perplexity_from_logprobs() {
        // Perfect predictions (prob = 1, logprob = 0)
        let logprobs = vec![0.0, 0.0, 0.0, 0.0];
        let result = PerplexityEvaluator::perplexity_from_logprobs(&logprobs);
        assert!((result.perplexity - 1.0).abs() < 0.01);

        // Uncertain predictions (logprob = -2, prob ≈ 0.135)
        // perplexity = exp(-avg_logprob) = exp(-(-2)) = exp(2) ≈ 7.389
        let logprobs = vec![-2.0, -2.0, -2.0, -2.0];
        let result = PerplexityEvaluator::perplexity_from_logprobs(&logprobs);
        assert!((result.perplexity - 2.0_f64.exp()).abs() < 0.1);
    }

    #[test]
    fn test_ngram_perplexity() {
        let training_texts = vec![
            "the cat sat on the mat",
            "the dog sat on the floor",
            "the cat slept on the couch",
        ];

        let mut ngram = NGramPerplexity::new(3);
        ngram.train(&training_texts);

        // In-distribution text should have lower perplexity
        let in_dist = ngram.perplexity("the cat sat on the mat");

        // Out-of-distribution text should have higher perplexity
        let out_dist = ngram.perplexity("quantum mechanics explains reality");

        assert!(in_dist.perplexity < out_dist.perplexity);
    }
}
