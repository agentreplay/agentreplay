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

//! Trace Summarization Module
//!
//! Implements hierarchical trace summarization from tauri.md Task 2:
//! - Information Bottleneck theory for relevance scoring
//! - TextRank for extractive summarization of spans
//! - Budget-aware compression with token limits
//!
//! Approach from tauri.md:
//!
//! # Hierarchical Trace Summarization
//!
//! For trace T = {span_1, span_2, ..., span_n}:
//!
//! ## 1. Relevance Scoring (Information Bottleneck)
//! `relevance(span_i) = I(span_i; eval_criteria) - beta * H(span_i)`
//!
//! Where:
//! - `I(span_i; eval_criteria)` = semantic similarity to evaluation criteria
//! - `H(span_i)` = entropy of span (complexity/length penalty)
//! - `beta` = compression-relevance tradeoff (0.1 default)
//!
//! ## 2. Budget Allocation
//! Given total budget B tokens:
//! `allocation(span_i) = B * relevance(span_i) / sum(relevance)`
//!
//! ## 3. Per-Span Summarization (TextRank)
//! - Split span into sentences
//! - Build similarity graph using word overlap
//! - Run PageRank to score sentences
//! - Select top-k sentences by score until budget exhausted

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for trace summarization
#[derive(Debug, Clone)]
pub struct SummarizerConfig {
    /// Total token budget for the summarized trace
    pub token_budget: usize,
    /// Compression-relevance tradeoff (higher = more compression)
    pub beta: f64,
    /// Minimum tokens per span (won't compress below this)
    pub min_tokens_per_span: usize,
    /// Maximum tokens per span (cap even if budget allows more)
    pub max_tokens_per_span: usize,
    /// TextRank damping factor (typically 0.85)
    pub damping_factor: f64,
    /// TextRank convergence threshold
    pub convergence_threshold: f64,
    /// Max TextRank iterations
    pub max_iterations: usize,
    /// Evaluation criteria for relevance scoring
    pub eval_criteria: Vec<String>,
}

impl Default for SummarizerConfig {
    fn default() -> Self {
        Self {
            token_budget: 2000,
            beta: 0.1,
            min_tokens_per_span: 50,
            max_tokens_per_span: 500,
            damping_factor: 0.85,
            convergence_threshold: 0.0001,
            max_iterations: 100,
            eval_criteria: vec![
                "coherence".to_string(),
                "relevance".to_string(),
                "correctness".to_string(),
            ],
        }
    }
}

/// A summarized span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanSummary {
    /// Original span ID
    pub span_id: u64,
    /// Original span name/type
    pub span_name: String,
    /// Summarized content
    pub summary: String,
    /// Relevance score (0.0 - 1.0)
    pub relevance_score: f64,
    /// Token count in summary
    pub token_count: usize,
    /// Key extracted sentences
    pub key_sentences: Vec<String>,
    /// Compression ratio (summary_tokens / original_tokens)
    pub compression_ratio: f64,
}

/// Complete hierarchical summary of a trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalSummary {
    /// Original trace ID
    pub trace_id: u128,
    /// Summarized spans
    pub span_summaries: Vec<SpanSummary>,
    /// Overall trace summary (if requested)
    pub trace_summary: Option<String>,
    /// Total tokens used
    pub total_tokens: usize,
    /// Original token count
    pub original_tokens: usize,
    /// Overall compression ratio
    pub compression_ratio: f64,
    /// Metadata about the summarization
    pub metadata: SummaryMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetadata {
    /// Algorithm used for summarization
    pub algorithm: String,
    /// Time taken in milliseconds
    pub duration_ms: u64,
    /// Number of spans processed
    pub spans_processed: usize,
    /// Configuration used
    pub config: SummaryConfigSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfigSnapshot {
    pub token_budget: usize,
    pub beta: f64,
}

/// Trace summarizer using Information Bottleneck and TextRank
pub struct TraceSummarizer {
    config: SummarizerConfig,
}

impl Default for TraceSummarizer {
    fn default() -> Self {
        Self::new(SummarizerConfig::default())
    }
}

impl TraceSummarizer {
    pub fn new(config: SummarizerConfig) -> Self {
        Self { config }
    }

    /// Set the token budget
    pub fn with_budget(mut self, budget: usize) -> Self {
        self.config.token_budget = budget;
        self
    }

    /// Set evaluation criteria for relevance scoring
    pub fn with_criteria(mut self, criteria: Vec<String>) -> Self {
        self.config.eval_criteria = criteria;
        self
    }

    /// Summarize a trace
    pub fn summarize(&self, spans: &[SpanContent]) -> HierarchicalSummary {
        let start = std::time::Instant::now();

        // Step 1: Calculate relevance scores for each span
        let relevance_scores = self.calculate_relevance_scores(spans);

        // Step 2: Allocate budget to each span
        let allocations = self.allocate_budget(&relevance_scores);

        // Step 3: Summarize each span within its budget
        let mut span_summaries = Vec::new();
        let mut total_tokens = 0;
        let original_tokens: usize = spans.iter().map(|s| self.count_tokens(&s.content)).sum();

        for (i, span) in spans.iter().enumerate() {
            let budget = allocations
                .get(i)
                .copied()
                .unwrap_or(self.config.min_tokens_per_span);
            let relevance = relevance_scores.get(i).copied().unwrap_or(0.5);

            let summary = self.summarize_span(span, budget, relevance);
            total_tokens += summary.token_count;
            span_summaries.push(summary);
        }

        // Create trace-level summary if needed
        let trace_summary = if spans.len() > 3 {
            Some(self.create_trace_summary(&span_summaries))
        } else {
            None
        };

        let duration = start.elapsed().as_millis() as u64;

        HierarchicalSummary {
            trace_id: spans.first().map(|s| s.trace_id).unwrap_or(0),
            span_summaries,
            trace_summary,
            total_tokens,
            original_tokens,
            compression_ratio: if original_tokens > 0 {
                total_tokens as f64 / original_tokens as f64
            } else {
                1.0
            },
            metadata: SummaryMetadata {
                algorithm: "TextRank+InfoBottleneck".to_string(),
                duration_ms: duration,
                spans_processed: spans.len(),
                config: SummaryConfigSnapshot {
                    token_budget: self.config.token_budget,
                    beta: self.config.beta,
                },
            },
        }
    }

    /// Calculate relevance scores using Information Bottleneck approximation
    fn calculate_relevance_scores(&self, spans: &[SpanContent]) -> Vec<f64> {
        spans
            .iter()
            .map(|span| {
                // I(span; eval_criteria) approximated by keyword overlap + structural importance
                let criteria_relevance = self.criteria_relevance(span);

                // H(span) approximated by normalized length
                let entropy = self.span_entropy(span);

                // relevance = I - β·H
                (criteria_relevance - self.config.beta * entropy).max(0.0)
            })
            .collect()
    }

    /// Approximate mutual information with eval criteria
    fn criteria_relevance(&self, span: &SpanContent) -> f64 {
        let content_lower = span.content.to_lowercase();

        // Score based on evaluation-relevant keywords and patterns
        let mut score: f64 = 0.0;

        // Check for criteria keywords
        for criterion in &self.config.eval_criteria {
            let criterion_lower = criterion.to_lowercase();
            if content_lower.contains(&criterion_lower) {
                score += 0.2;
            }
        }

        // Structural importance: input/output spans are more important
        let span_type = span.span_type.to_lowercase();
        if span_type.contains("input")
            || span_type.contains("query")
            || span_type.contains("prompt")
        {
            score += 0.3;
        }
        if span_type.contains("output")
            || span_type.contains("response")
            || span_type.contains("result")
        {
            score += 0.3;
        }
        if span_type.contains("llm") || span_type.contains("agent") {
            score += 0.2;
        }
        if span_type.contains("tool") || span_type.contains("function") {
            score += 0.15;
        }

        // Content quality signals
        if content_lower.contains("error") || content_lower.contains("failed") {
            score += 0.1; // Errors are relevant for evaluation
        }
        if content_lower.contains("reason") || content_lower.contains("because") {
            score += 0.1; // Reasoning is relevant
        }

        // Normalize to [0, 1]
        score.min(1.0)
    }

    /// Approximate entropy (complexity) of a span
    fn span_entropy(&self, span: &SpanContent) -> f64 {
        let token_count = self.count_tokens(&span.content);

        // Simple entropy approximation based on length
        // Longer spans have higher entropy (more to compress)
        let length_factor = (token_count as f64).ln().max(0.0) / 10.0;

        // Vocabulary diversity (unique words / total words)
        let words: Vec<&str> = span.content.split_whitespace().collect();
        let unique_words: std::collections::HashSet<_> = words.iter().collect();
        let diversity = if words.is_empty() {
            0.0
        } else {
            unique_words.len() as f64 / words.len() as f64
        };

        // Combined entropy score
        (length_factor + diversity * 0.3).min(1.0)
    }

    /// Allocate token budget based on relevance scores
    fn allocate_budget(&self, relevance_scores: &[f64]) -> Vec<usize> {
        let total_relevance: f64 = relevance_scores.iter().sum();

        if total_relevance == 0.0 {
            // Equal distribution if no relevance
            let per_span = self.config.token_budget / relevance_scores.len().max(1);
            return vec![per_span.max(self.config.min_tokens_per_span); relevance_scores.len()];
        }

        relevance_scores
            .iter()
            .map(|&relevance| {
                let allocation =
                    (self.config.token_budget as f64 * relevance / total_relevance) as usize;
                allocation
                    .max(self.config.min_tokens_per_span)
                    .min(self.config.max_tokens_per_span)
            })
            .collect()
    }

    /// Summarize a single span using TextRank
    fn summarize_span(&self, span: &SpanContent, budget: usize, relevance: f64) -> SpanSummary {
        let original_tokens = self.count_tokens(&span.content);

        // If already within budget, return as-is
        if original_tokens <= budget {
            return SpanSummary {
                span_id: span.span_id,
                span_name: span.span_type.clone(),
                summary: span.content.clone(),
                relevance_score: relevance,
                token_count: original_tokens,
                key_sentences: vec![],
                compression_ratio: 1.0,
            };
        }

        // Split into sentences
        let sentences = self.split_sentences(&span.content);

        if sentences.is_empty() {
            return SpanSummary {
                span_id: span.span_id,
                span_name: span.span_type.clone(),
                summary: span.content.clone(),
                relevance_score: relevance,
                token_count: original_tokens,
                key_sentences: vec![],
                compression_ratio: 1.0,
            };
        }

        // Run TextRank to score sentences
        let sentence_scores = self.text_rank(&sentences);

        // Select top sentences until budget is reached
        let mut scored: Vec<(usize, f64, &str)> = sentences
            .iter()
            .enumerate()
            .zip(sentence_scores.iter())
            .map(|((i, s), &score)| (i, score, *s))
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected = Vec::new();
        let mut token_count = 0;
        let mut key_sentences = Vec::new();

        for (original_idx, score, sentence) in scored {
            let sentence_tokens = self.count_tokens(sentence);
            if token_count + sentence_tokens <= budget {
                selected.push((original_idx, sentence.to_string()));
                token_count += sentence_tokens;
                if score > 0.1 {
                    key_sentences.push(sentence.to_string());
                }
            }
            if token_count >= budget {
                break;
            }
        }

        // Restore original order
        selected.sort_by_key(|(idx, _)| *idx);

        let summary = selected
            .iter()
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        SpanSummary {
            span_id: span.span_id,
            span_name: span.span_type.clone(),
            summary,
            relevance_score: relevance,
            token_count,
            key_sentences: key_sentences.into_iter().take(3).collect(),
            compression_ratio: token_count as f64 / original_tokens as f64,
        }
    }

    /// TextRank algorithm for sentence scoring
    fn text_rank(&self, sentences: &[&str]) -> Vec<f64> {
        let n = sentences.len();
        if n == 0 {
            return vec![];
        }
        if n == 1 {
            return vec![1.0];
        }

        // Build similarity matrix
        let mut similarity_matrix = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let sim = self.sentence_similarity(sentences[i], sentences[j]);
                similarity_matrix[i][j] = sim;
                similarity_matrix[j][i] = sim;
            }
        }

        // Initialize scores
        let mut scores = vec![1.0 / n as f64; n];
        let d = self.config.damping_factor;

        // Iterate until convergence
        for _ in 0..self.config.max_iterations {
            let mut new_scores = vec![0.0; n];
            let mut max_diff: f64 = 0.0;

            for i in 0..n {
                let mut sum = 0.0;
                for j in 0..n {
                    if i != j {
                        let out_sum: f64 = similarity_matrix[j].iter().sum();
                        if out_sum > 0.0 {
                            sum += similarity_matrix[j][i] * scores[j] / out_sum;
                        }
                    }
                }
                new_scores[i] = (1.0 - d) + d * sum;
                max_diff = max_diff.max((new_scores[i] - scores[i]).abs());
            }

            scores = new_scores;

            if max_diff < self.config.convergence_threshold {
                break;
            }
        }

        // Normalize
        let max_score = scores.iter().cloned().fold(0.0, f64::max);
        if max_score > 0.0 {
            scores.iter_mut().for_each(|s| *s /= max_score);
        }

        scores
    }

    /// Calculate similarity between two sentences
    fn sentence_similarity(&self, s1: &str, s2: &str) -> f64 {
        // Simple word overlap similarity (Jaccard-like)
        let s1_lower = s1.to_lowercase();
        let s2_lower = s2.to_lowercase();
        let words1: std::collections::HashSet<_> = s1_lower.split_whitespace().collect();
        let words2: std::collections::HashSet<_> = s2_lower.split_whitespace().collect();

        if words1.is_empty() && words2.is_empty() {
            return 0.0;
        }

        let intersection = words1.intersection(&words2).count() as f64;
        let union = words1.union(&words2).count() as f64;

        if union == 0.0 {
            0.0
        } else {
            intersection / union
        }
    }

    /// Split text into sentences
    fn split_sentences<'a>(&self, text: &'a str) -> Vec<&'a str> {
        // Simple sentence splitting on . ! ? followed by space or end
        let mut sentences = Vec::new();
        let mut start = 0;

        for (i, c) in text.char_indices() {
            if c == '.' || c == '!' || c == '?' {
                // Check if followed by space or end
                let next_char = text.get(i + 1..i + 2).and_then(|s| s.chars().next());
                if next_char.is_none() || next_char == Some(' ') || next_char == Some('\n') {
                    let sentence = text[start..=i].trim();
                    if !sentence.is_empty() && sentence.split_whitespace().count() >= 3 {
                        sentences.push(sentence);
                    }
                    start = i + 1;
                }
            }
        }

        // Add remaining text if any
        let remaining = text[start..].trim();
        if !remaining.is_empty() && remaining.split_whitespace().count() >= 3 {
            sentences.push(remaining);
        }

        sentences
    }

    /// Simple token count (word-based approximation)
    fn count_tokens(&self, text: &str) -> usize {
        // Rough approximation: ~4 chars per token
        text.len().div_ceil(4)
    }

    /// Create a high-level trace summary from span summaries
    fn create_trace_summary(&self, span_summaries: &[SpanSummary]) -> String {
        // Extract key sentences from most relevant spans
        let mut key_info: Vec<(f64, &str)> = span_summaries
            .iter()
            .flat_map(|s| {
                s.key_sentences
                    .iter()
                    .map(move |ks| (s.relevance_score, ks.as_str()))
            })
            .collect();

        key_info.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let summary_parts: Vec<_> = key_info.iter().take(5).map(|(_, s)| *s).collect();

        if summary_parts.is_empty() {
            format!(
                "Trace with {} spans. Key types: {}",
                span_summaries.len(),
                span_summaries
                    .iter()
                    .take(3)
                    .map(|s| s.span_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            summary_parts.join(" ")
        }
    }
}

/// Input span content for summarization
#[derive(Debug, Clone)]
pub struct SpanContent {
    /// Trace ID this span belongs to
    pub trace_id: u128,
    /// Unique span ID
    pub span_id: u64,
    /// Span type (e.g., "llm_call", "tool_use", "agent_step")
    pub span_type: String,
    /// Text content to summarize
    pub content: String,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

impl SpanContent {
    pub fn new(
        trace_id: u128,
        span_id: u64,
        span_type: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            trace_id,
            span_id,
            span_type: span_type.into(),
            content: content.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spans() -> Vec<SpanContent> {
        vec![
            SpanContent::new(
                1,
                101,
                "llm_input",
                "What is the capital of France? Please provide a detailed answer with historical context."
            ),
            SpanContent::new(
                1,
                102,
                "llm_output",
                "Paris is the capital of France. It has been the capital since the 10th century. \
                 The city is known for its rich history, art, and culture. The Eiffel Tower is a famous landmark. \
                 Paris is located in the north-central part of France. The Seine River runs through the city. \
                 Many important historical events occurred in Paris."
            ),
            SpanContent::new(
                1,
                103,
                "tool_call",
                "Called wikipedia_search with query='Paris capital France'. \
                 Returned article about Paris with population statistics and geography."
            ),
        ]
    }

    #[test]
    fn test_summarizer_creation() {
        let summarizer = TraceSummarizer::default();
        assert_eq!(summarizer.config.token_budget, 2000);
        assert_eq!(summarizer.config.beta, 0.1);
    }

    #[test]
    fn test_summarize_spans() {
        let summarizer = TraceSummarizer::default().with_budget(200);
        let spans = sample_spans();

        let summary = summarizer.summarize(&spans);

        assert_eq!(summary.span_summaries.len(), 3);
        assert!(summary.total_tokens <= 200 + 50); // Some overhead allowed
        assert!(summary.compression_ratio <= 1.0);
    }

    #[test]
    fn test_relevance_scoring() {
        let summarizer = TraceSummarizer::default();
        let spans = sample_spans();

        let scores = summarizer.calculate_relevance_scores(&spans);

        // Input and output should have higher relevance
        assert!(scores[0] > 0.0); // llm_input
        assert!(scores[1] > 0.0); // llm_output
    }

    #[test]
    fn test_budget_allocation() {
        let summarizer = TraceSummarizer::default().with_budget(1000);

        let relevance_scores = vec![0.8, 0.5, 0.3];
        let allocations = summarizer.allocate_budget(&relevance_scores);

        // Higher relevance should get more budget
        assert!(allocations[0] > allocations[2]);
        assert_eq!(allocations.len(), 3);
    }

    #[test]
    fn test_sentence_splitting() {
        let summarizer = TraceSummarizer::default();

        let text = "This is sentence one. This is sentence two! Is this sentence three?";
        let sentences = summarizer.split_sentences(text);

        assert_eq!(sentences.len(), 3);
        assert!(sentences[0].contains("one"));
        assert!(sentences[1].contains("two"));
        assert!(sentences[2].contains("three"));
    }

    #[test]
    fn test_text_rank() {
        let summarizer = TraceSummarizer::default();

        let sentences = vec![
            "Paris is the capital of France.",
            "France is a country in Europe.",
            "Paris has many famous landmarks.",
            "The weather in Antarctica is cold.",
        ];

        let scores = summarizer.text_rank(&sentences);

        assert_eq!(scores.len(), 4);
        // Sentences about Paris/France should score higher (more connected)
        assert!(scores[0] > scores[3] || scores[1] > scores[3] || scores[2] > scores[3]);
    }

    #[test]
    fn test_sentence_similarity() {
        let summarizer = TraceSummarizer::default();

        let s1 = "Paris is the capital of France";
        let s2 = "France has Paris as its capital";
        let s3 = "The weather is sunny today";

        let sim12 = summarizer.sentence_similarity(s1, s2);
        let sim13 = summarizer.sentence_similarity(s1, s3);

        // s1 and s2 should be more similar than s1 and s3
        assert!(sim12 > sim13);
    }

    #[test]
    fn test_compression_ratio() {
        let summarizer = TraceSummarizer::default().with_budget(50);

        let spans = vec![SpanContent::new(
            1,
            101,
            "llm_output",
            "This is a very long response that contains many sentences. \
             Each sentence adds more information to the response. \
             The summarizer should compress this text significantly. \
             Only the most important sentences should remain. \
             This ensures we stay within the token budget.",
        )];

        let summary = summarizer.summarize(&spans);

        assert!(summary.compression_ratio < 1.0);
        assert!(summary.span_summaries[0].token_count <= 60); // Allow some overhead
    }

    #[test]
    fn test_hierarchical_summary_with_trace_summary() {
        let summarizer = TraceSummarizer::default().with_budget(500);

        // Create more than 3 spans to trigger trace summary
        let spans = vec![
            SpanContent::new(1, 101, "input", "User asked about quantum computing."),
            SpanContent::new(
                1,
                102,
                "llm_call",
                "Processing the quantum computing query.",
            ),
            SpanContent::new(
                1,
                103,
                "tool_use",
                "Searched for quantum computing information.",
            ),
            SpanContent::new(
                1,
                104,
                "output",
                "Quantum computing uses qubits for parallel processing.",
            ),
        ];

        let summary = summarizer.summarize(&spans);

        // Should have a trace-level summary
        assert!(summary.trace_summary.is_some());
    }

    #[test]
    fn test_empty_spans() {
        let summarizer = TraceSummarizer::default();
        let spans: Vec<SpanContent> = vec![];

        let summary = summarizer.summarize(&spans);

        assert_eq!(summary.span_summaries.len(), 0);
        assert_eq!(summary.total_tokens, 0);
    }

    #[test]
    fn test_with_criteria() {
        let summarizer = TraceSummarizer::default()
            .with_criteria(vec!["accuracy".to_string(), "completeness".to_string()]);

        assert!(summarizer
            .config
            .eval_criteria
            .contains(&"accuracy".to_string()));
        assert!(summarizer
            .config
            .eval_criteria
            .contains(&"completeness".to_string()));
    }
}
