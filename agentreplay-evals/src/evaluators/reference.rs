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

//! Reference-Based Evaluation Metrics
//!
//! Implements deterministic NLP evaluation metrics for comparing generated text
//! against reference/golden answers. These metrics are essential for:
//!
//! - **Regression Testing**: Golden datasets need deterministic, reproducible metrics
//! - **Cost Efficiency**: These metrics cost ~$0.00001/eval vs LLM-as-judge at $0.01-0.05/eval
//! - **Speed**: Compute in <1ms vs 500-2000ms for LLM-as-judge
//! - **Academic Compatibility**: Standard benchmarks universally report ROUGE/BLEU
//!
//! ## Metrics Implemented
//!
//! - **ROUGE-N**: N-gram overlap recall (ROUGE-1, ROUGE-2)
//! - **ROUGE-L**: Longest Common Subsequence based F1
//! - **BLEU**: Bilingual Evaluation Understudy with brevity penalty
//! - **BERTScore**: Contextual embedding similarity (requires embedding client)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use agentreplay_evals::evaluators::reference::{ReferenceEvaluator, RougeScore};
//!
//! let evaluator = ReferenceEvaluator::new();
//!
//! let reference = "The capital of France is Paris.";
//! let candidate = "Paris is the capital city of France.";
//!
//! let rouge1 = evaluator.rouge_n(reference, candidate, 1);
//! let rouge_l = evaluator.rouge_l(reference, candidate);
//! let bleu = evaluator.bleu(reference, candidate, 4);
//! ```

use crate::llm_client::EmbeddingClient;
use crate::{EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

/// ROUGE score components (precision, recall, F1)
#[derive(Debug, Clone, Default)]
pub struct RougeScore {
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

/// BERTScore components (precision, recall, F1)
#[derive(Debug, Clone, Default)]
pub struct BertScoreResult {
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

/// Complete reference evaluation result
#[derive(Debug, Clone)]
pub struct ReferenceMetrics {
    pub rouge_1: RougeScore,
    pub rouge_2: RougeScore,
    pub rouge_l: RougeScore,
    pub bleu: f64,
    pub bert_score: Option<BertScoreResult>,
}

/// Reference-based evaluator for deterministic text comparison
///
/// Computes ROUGE, BLEU, and optionally BERTScore metrics.
pub struct ReferenceEvaluator {
    /// Optional embedding client for BERTScore computation
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
    /// Minimum score threshold for passing (applied to primary metric)
    threshold: f64,
    /// Primary metric for pass/fail determination
    primary_metric: PrimaryMetric,
}

/// Which metric to use for pass/fail determination
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PrimaryMetric {
    Rouge1F1,
    Rouge2F1,
    #[default]
    RougeLF1,
    Bleu,
    BertScoreF1,
}

impl ReferenceEvaluator {
    /// Create a new reference evaluator without embedding support
    pub fn new() -> Self {
        Self {
            embedding_client: None,
            threshold: 0.5,
            primary_metric: PrimaryMetric::default(),
        }
    }

    /// Create with embedding client for BERTScore support
    pub fn with_embedding_client(mut self, client: Arc<dyn EmbeddingClient>) -> Self {
        self.embedding_client = Some(client);
        self
    }

    /// Set pass/fail threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set primary metric for pass/fail determination
    pub fn with_primary_metric(mut self, metric: PrimaryMetric) -> Self {
        self.primary_metric = metric;
        self
    }

    /// Tokenize text into words (lowercase)
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .map(|s| {
                // Remove punctuation from ends
                s.trim_matches(|c: char| !c.is_alphanumeric()).to_string()
            })
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Get n-grams from tokenized text
    fn get_ngrams(&self, tokens: &[String], n: usize) -> Vec<String> {
        if tokens.len() < n {
            return Vec::new();
        }

        tokens.windows(n).map(|window| window.join(" ")).collect()
    }

    /// Compute ROUGE-N score
    ///
    /// ROUGE-N = overlap(ref_ngrams, cand_ngrams) / count(ref_ngrams)
    pub fn rouge_n(&self, reference: &str, candidate: &str, n: usize) -> RougeScore {
        let ref_tokens = self.tokenize(reference);
        let cand_tokens = self.tokenize(candidate);

        let ref_ngrams = self.get_ngrams(&ref_tokens, n);
        let cand_ngrams = self.get_ngrams(&cand_tokens, n);

        if ref_ngrams.is_empty() || cand_ngrams.is_empty() {
            return RougeScore::default();
        }

        // Convert to sets for overlap counting
        let _ref_set: HashSet<_> = ref_ngrams.iter().collect();
        let _cand_set: HashSet<_> = cand_ngrams.iter().collect();

        // Count overlapping n-grams (with frequency consideration)
        let mut ref_counts: HashMap<&String, usize> = HashMap::new();
        for ng in &ref_ngrams {
            *ref_counts.entry(ng).or_insert(0) += 1;
        }

        let mut cand_counts: HashMap<&String, usize> = HashMap::new();
        for ng in &cand_ngrams {
            *cand_counts.entry(ng).or_insert(0) += 1;
        }

        // Clipped count: min(cand_count, ref_count) for each n-gram
        let overlap: usize = cand_counts
            .iter()
            .filter_map(|(ng, cand_count)| {
                ref_counts
                    .get(ng)
                    .map(|ref_count| (*cand_count).min(*ref_count))
            })
            .sum();

        let precision = overlap as f64 / cand_ngrams.len() as f64;
        let recall = overlap as f64 / ref_ngrams.len() as f64;
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        RougeScore {
            precision,
            recall,
            f1,
        }
    }

    /// Compute ROUGE-L score using Longest Common Subsequence
    ///
    /// Uses dynamic programming O(mn) time, O(min(m,n)) space
    pub fn rouge_l(&self, reference: &str, candidate: &str) -> RougeScore {
        let ref_tokens = self.tokenize(reference);
        let cand_tokens = self.tokenize(candidate);

        if ref_tokens.is_empty() || cand_tokens.is_empty() {
            return RougeScore::default();
        }

        let lcs_len = self.lcs_length(&ref_tokens, &cand_tokens);

        let precision = lcs_len as f64 / cand_tokens.len() as f64;
        let recall = lcs_len as f64 / ref_tokens.len() as f64;

        // F1 score with beta = P/R (when dF/dR = dF/dP)
        let f1 = if precision + recall > 0.0 {
            let beta_sq = if recall > 1e-10 {
                (precision / recall).powi(2)
            } else {
                1.0
            };
            (1.0 + beta_sq) * precision * recall / (recall + beta_sq * precision)
        } else {
            0.0
        };

        RougeScore {
            precision,
            recall,
            f1,
        }
    }

    /// Compute LCS length using DP with space optimization
    fn lcs_length(&self, a: &[String], b: &[String]) -> usize {
        // Use shorter sequence for the DP array
        let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) };

        let mut prev = vec![0usize; short.len() + 1];
        let mut curr = vec![0usize; short.len() + 1];

        for i in 1..=long.len() {
            for j in 1..=short.len() {
                curr[j] = if long[i - 1] == short[j - 1] {
                    prev[j - 1] + 1
                } else {
                    prev[j].max(curr[j - 1])
                };
            }
            std::mem::swap(&mut prev, &mut curr);
        }

        prev[short.len()]
    }

    /// Compute BLEU score with brevity penalty
    ///
    /// BLEU = BP × exp(∑ wₙ log pₙ)
    /// BP = exp(1 - r/c) if c ≤ r, else 1
    pub fn bleu(&self, reference: &str, candidate: &str, max_n: usize) -> f64 {
        let ref_tokens = self.tokenize(reference);
        let cand_tokens = self.tokenize(candidate);

        if cand_tokens.is_empty() {
            return 0.0;
        }

        // Brevity penalty
        let bp = if cand_tokens.len() >= ref_tokens.len() {
            1.0
        } else if cand_tokens.is_empty() {
            0.0
        } else {
            (1.0 - ref_tokens.len() as f64 / cand_tokens.len() as f64).exp()
        };

        // Compute modified n-gram precisions with smoothing
        let mut log_sum = 0.0;
        let weight = 1.0 / max_n as f64;

        for n in 1..=max_n {
            let precision = self.modified_precision(&ref_tokens, &cand_tokens, n);

            // Add-1 smoothing for zero counts (Chen & Cherry, 2014)
            let smoothed = if precision == 0.0 {
                1.0 / (cand_tokens.len() + 1) as f64
            } else {
                precision
            };

            log_sum += weight * smoothed.ln();
        }

        bp * log_sum.exp()
    }

    /// Compute modified n-gram precision for BLEU
    fn modified_precision(&self, ref_tokens: &[String], cand_tokens: &[String], n: usize) -> f64 {
        let ref_ngrams = self.get_ngrams(ref_tokens, n);
        let cand_ngrams = self.get_ngrams(cand_tokens, n);

        if cand_ngrams.is_empty() {
            return 0.0;
        }

        // Count reference n-grams
        let mut ref_counts: HashMap<&String, usize> = HashMap::new();
        for ng in &ref_ngrams {
            *ref_counts.entry(ng).or_insert(0) += 1;
        }

        // Count clipped matches
        let mut cand_counts: HashMap<&String, usize> = HashMap::new();
        for ng in &cand_ngrams {
            *cand_counts.entry(ng).or_insert(0) += 1;
        }

        let clipped_sum: usize = cand_counts
            .iter()
            .map(|(ng, count)| {
                let max_ref = ref_counts.get(ng).copied().unwrap_or(0);
                (*count).min(max_ref)
            })
            .sum();

        clipped_sum as f64 / cand_ngrams.len() as f64
    }

    /// Compute BERTScore using pre-computed embeddings
    ///
    /// Requires an embedding client to be configured.
    pub async fn bert_score(
        &self,
        reference: &str,
        candidate: &str,
    ) -> Result<BertScoreResult, EvalError> {
        let client = self
            .embedding_client
            .as_ref()
            .ok_or(EvalError::Internal("No embedding client configured".into()))?;

        let ref_tokens: Vec<String> = self.tokenize(reference);
        let cand_tokens: Vec<String> = self.tokenize(candidate);

        if ref_tokens.is_empty() || cand_tokens.is_empty() {
            return Ok(BertScoreResult::default());
        }

        // Get embeddings for each token
        let ref_embeddings = client
            .embed_batch(&ref_tokens)
            .await
            .map_err(|e| EvalError::LLMClientError(format!("Embedding error: {:?}", e)))?;
        let cand_embeddings = client
            .embed_batch(&cand_tokens)
            .await
            .map_err(|e| EvalError::LLMClientError(format!("Embedding error: {:?}", e)))?;

        // Precision: for each candidate token, find max similarity to any reference token
        let precision: f64 = cand_embeddings
            .iter()
            .map(|ce| {
                ref_embeddings
                    .iter()
                    .map(|re| cosine_similarity(ce, re))
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .sum::<f64>()
            / cand_embeddings.len() as f64;

        // Recall: for each reference token, find max similarity to any candidate token
        let recall: f64 = ref_embeddings
            .iter()
            .map(|re| {
                cand_embeddings
                    .iter()
                    .map(|ce| cosine_similarity(re, ce))
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .sum::<f64>()
            / ref_embeddings.len() as f64;

        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        Ok(BertScoreResult {
            precision,
            recall,
            f1,
        })
    }

    /// Compute all reference metrics
    pub async fn compute_all(
        &self,
        reference: &str,
        candidate: &str,
    ) -> Result<ReferenceMetrics, EvalError> {
        let rouge_1 = self.rouge_n(reference, candidate, 1);
        let rouge_2 = self.rouge_n(reference, candidate, 2);
        let rouge_l = self.rouge_l(reference, candidate);
        let bleu = self.bleu(reference, candidate, 4);

        let bert_score = if self.embedding_client.is_some() {
            Some(self.bert_score(reference, candidate).await?)
        } else {
            None
        };

        Ok(ReferenceMetrics {
            rouge_1,
            rouge_2,
            rouge_l,
            bleu,
            bert_score,
        })
    }

    /// Get the primary metric score for pass/fail determination
    fn get_primary_score(&self, metrics: &ReferenceMetrics) -> f64 {
        match self.primary_metric {
            PrimaryMetric::Rouge1F1 => metrics.rouge_1.f1,
            PrimaryMetric::Rouge2F1 => metrics.rouge_2.f1,
            PrimaryMetric::RougeLF1 => metrics.rouge_l.f1,
            PrimaryMetric::Bleu => metrics.bleu,
            PrimaryMetric::BertScoreF1 => metrics.bert_score.as_ref().map(|b| b.f1).unwrap_or(0.0),
        }
    }
}

impl Default for ReferenceEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[async_trait]
impl Evaluator for ReferenceEvaluator {
    fn id(&self) -> &str {
        "reference_metrics_v1"
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Reference Metrics".to_string(),
            version: "1.0.0".to_string(),
            description: "Computes ROUGE, BLEU, and BERTScore for reference-based text evaluation"
                .to_string(),
            cost_per_eval: Some(0.0), // Free for non-BERTScore metrics
            avg_latency_ms: Some(1),
            tags: vec![
                "reference".to_string(),
                "rouge".to_string(),
                "bleu".to_string(),
                "deterministic".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // Get output as candidate
        let candidate = trace
            .output
            .as_ref()
            .ok_or_else(|| EvalError::MissingField("output".to_string()))?;

        // Get reference from context (first element) or metadata
        let reference = trace
            .context
            .as_ref()
            .and_then(|c| c.first().cloned())
            .or_else(|| {
                trace
                    .metadata
                    .get("expected_output")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
            })
            .or_else(|| {
                trace
                    .metadata
                    .get("reference")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
            })
            .ok_or_else(|| {
                EvalError::MissingField(
                    "reference (in context[0] or metadata.expected_output)".to_string(),
                )
            })?;

        // Compute metrics
        let metrics_result = self.compute_all(&reference, candidate).await?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let primary_score = self.get_primary_score(&metrics_result);
        let passed = primary_score >= self.threshold;

        // Build metrics map
        let mut metrics = HashMap::new();

        // ROUGE-1
        metrics.insert(
            "rouge_1_precision".to_string(),
            MetricValue::Float(metrics_result.rouge_1.precision),
        );
        metrics.insert(
            "rouge_1_recall".to_string(),
            MetricValue::Float(metrics_result.rouge_1.recall),
        );
        metrics.insert(
            "rouge_1_f1".to_string(),
            MetricValue::Float(metrics_result.rouge_1.f1),
        );

        // ROUGE-2
        metrics.insert(
            "rouge_2_precision".to_string(),
            MetricValue::Float(metrics_result.rouge_2.precision),
        );
        metrics.insert(
            "rouge_2_recall".to_string(),
            MetricValue::Float(metrics_result.rouge_2.recall),
        );
        metrics.insert(
            "rouge_2_f1".to_string(),
            MetricValue::Float(metrics_result.rouge_2.f1),
        );

        // ROUGE-L
        metrics.insert(
            "rouge_l_precision".to_string(),
            MetricValue::Float(metrics_result.rouge_l.precision),
        );
        metrics.insert(
            "rouge_l_recall".to_string(),
            MetricValue::Float(metrics_result.rouge_l.recall),
        );
        metrics.insert(
            "rouge_l_f1".to_string(),
            MetricValue::Float(metrics_result.rouge_l.f1),
        );

        // BLEU
        metrics.insert("bleu".to_string(), MetricValue::Float(metrics_result.bleu));

        // BERTScore (if available)
        if let Some(ref bs) = metrics_result.bert_score {
            metrics.insert(
                "bert_score_precision".to_string(),
                MetricValue::Float(bs.precision),
            );
            metrics.insert(
                "bert_score_recall".to_string(),
                MetricValue::Float(bs.recall),
            );
            metrics.insert("bert_score_f1".to_string(), MetricValue::Float(bs.f1));
        }

        // Primary score
        metrics.insert(
            "primary_score".to_string(),
            MetricValue::Float(primary_score),
        );

        let explanation = Some(format!(
            "Reference metrics: ROUGE-L F1={:.3}, BLEU={:.3}, ROUGE-1 F1={:.3}{}",
            metrics_result.rouge_l.f1,
            metrics_result.bleu,
            metrics_result.rouge_1.f1,
            if let Some(ref bs) = metrics_result.bert_score {
                format!(", BERTScore F1={:.3}", bs.f1)
            } else {
                String::new()
            }
        ));

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("rule".to_string()),
            metrics,
            passed,
            explanation,
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 1.0, // Deterministic metrics have full confidence
            cost: Some(0.0),
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rouge_1_identical() {
        let evaluator = ReferenceEvaluator::new();
        let text = "The quick brown fox jumps over the lazy dog";

        let score = evaluator.rouge_n(text, text, 1);
        assert!((score.precision - 1.0).abs() < 0.001);
        assert!((score.recall - 1.0).abs() < 0.001);
        assert!((score.f1 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rouge_1_partial_overlap() {
        let evaluator = ReferenceEvaluator::new();
        let reference = "The cat sat on the mat";
        let candidate = "The cat is on the mat";

        let score = evaluator.rouge_n(reference, candidate, 1);

        // 5 words overlap: "the", "cat", "on", "the", "mat" (with "the" counted twice)
        // Actually: reference has ["the", "cat", "sat", "on", "the", "mat"] = 6 words
        // Candidate has ["the", "cat", "is", "on", "the", "mat"] = 6 words
        // Overlap: "the"(2), "cat"(1), "on"(1), "mat"(1) = 5 matches
        assert!(score.recall > 0.7);
        assert!(score.precision > 0.7);
    }

    #[test]
    fn test_rouge_l_identical() {
        let evaluator = ReferenceEvaluator::new();
        let text = "The quick brown fox";

        let score = evaluator.rouge_l(text, text);
        assert!((score.f1 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rouge_l_partial() {
        let evaluator = ReferenceEvaluator::new();
        let reference = "The quick brown fox jumps";
        let candidate = "The brown fox runs quickly";

        let score = evaluator.rouge_l(reference, candidate);

        // LCS should be "the", "brown", "fox" = 3 words
        // Reference has 5 words, candidate has 5 words
        assert!(score.recall > 0.5);
        assert!(score.precision > 0.5);
    }

    #[test]
    fn test_bleu_identical() {
        let evaluator = ReferenceEvaluator::new();
        let text = "The quick brown fox jumps over the lazy dog";

        let bleu = evaluator.bleu(text, text, 4);
        assert!((bleu - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_bleu_different() {
        let evaluator = ReferenceEvaluator::new();
        let reference = "The capital of France is Paris";
        let candidate = "Paris is the capital of France";

        let bleu = evaluator.bleu(reference, candidate, 4);

        // Should have reasonable BLEU (same words, different order)
        assert!(bleu > 0.3);
        assert!(bleu < 1.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);

        let d = vec![0.707, 0.707, 0.0];
        assert!((cosine_similarity(&a, &d) - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_lcs_length() {
        let evaluator = ReferenceEvaluator::new();

        let a: Vec<String> = vec!["a", "b", "c", "d"]
            .into_iter()
            .map(String::from)
            .collect();
        let b: Vec<String> = vec!["b", "c", "d", "e"]
            .into_iter()
            .map(String::from)
            .collect();

        let lcs = evaluator.lcs_length(&a, &b);
        assert_eq!(lcs, 3); // "b", "c", "d"
    }

    #[test]
    fn test_empty_inputs() {
        let evaluator = ReferenceEvaluator::new();

        let score = evaluator.rouge_n("", "test", 1);
        assert_eq!(score.f1, 0.0);

        let score = evaluator.rouge_n("test", "", 1);
        assert_eq!(score.f1, 0.0);

        let bleu = evaluator.bleu("test", "", 4);
        assert_eq!(bleu, 0.0);
    }
}
