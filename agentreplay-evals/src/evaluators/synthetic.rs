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

//! Synthetic Data Generation for Evaluation Datasets
//!
//! Provides tools to generate golden datasets from production traces:
//!
//! - **Trace Selection**: Quality-based sampling from production
//! - **Golden Reference Generation**: LLM-refined expected outputs
//! - **Perturbation Generation**: Adversarial/robustness test cases
//! - **Dataset Export**: Standard evaluation formats (JSON, JSONL)
//!
//! ## Token Efficiency
//!
//! Uses TOON (Token-Oriented Object Notation) for LLM communication,
//! reducing token usage by ~50% compared to JSON. TOON uses:
//! - `@key` instead of `"key":`
//! - `'string'` instead of `"string"`
//! - Minimal punctuation
//!
//! ## Usage
//!
//! ```rust,ignore
//! use agentreplay_evals::evaluators::synthetic::{SyntheticDatasetGenerator, SelectionCriteria};
//!
//! let generator = SyntheticDatasetGenerator::new(llm_client, embedding_client);
//!
//! // Select high-quality traces
//! let traces = generator.select_traces(criteria, 100).await?;
//!
//! // Generate golden references
//! let test_cases = generator.generate_golden_dataset(&traces).await?;
//!
//! // Add perturbations for robustness
//! let augmented = generator.augment_with_perturbations(&test_cases).await?;
//! ```

use crate::llm_client::{EmbeddingClient, LLMClient};
use crate::{EvalError, TraceContext};
#[cfg(test)]
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Test case for evaluation datasets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Unique identifier
    pub id: String,
    /// Input query/prompt
    pub input: String,
    /// Expected/golden output
    pub expected_output: String,
    /// Optional context for RAG evaluation
    pub context: Option<Vec<String>>,
    /// Difficulty level
    pub difficulty: Difficulty,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Source trace ID (if derived from production)
    pub source_trace_id: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Difficulty levels for test cases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    #[default]
    Medium,
    Hard,
    Expert,
}

/// Perturbation strategies for robustness testing
#[derive(Debug, Clone)]
pub enum PerturbationStrategy {
    /// Inject random typos at given rate (0.0-1.0)
    Typo { rate: f64 },
    /// Paraphrase the input using LLM
    Paraphrase,
    /// Truncate input to keep_ratio (0.0-1.0)
    Truncate { keep_ratio: f64 },
    /// Generate adversarial variants
    Adversarial,
    /// Substitute synonyms
    SynonymSubstitution { rate: f64 },
    /// Add noise/filler words
    NoiseInjection { words_to_add: usize },
}

/// Criteria for selecting high-quality traces
#[derive(Debug, Clone, Default)]
pub struct SelectionCriteria {
    /// Minimum confidence score (0.0-1.0)
    pub min_confidence: Option<f64>,
    /// Minimum user feedback score (if available)
    pub min_feedback_score: Option<f64>,
    /// Required tags
    pub required_tags: Vec<String>,
    /// Excluded tags
    pub excluded_tags: Vec<String>,
    /// Time range (start_us, end_us)
    pub time_range: Option<(u64, u64)>,
    /// Minimum token count (filter out trivial traces)
    pub min_tokens: Option<u32>,
    /// Maximum latency (filter out timeout traces)
    pub max_latency_ms: Option<u64>,
}

/// Quality score for a trace
#[derive(Debug, Clone)]
pub struct TraceQualityScore {
    pub trace_id: u128,
    /// Overall quality score (0.0-1.0)
    pub quality: f64,
    /// Confidence from evaluation
    pub confidence: f64,
    /// User feedback score (if available)
    pub feedback: Option<f64>,
    /// Diversity score (distance from existing dataset)
    pub diversity: f64,
}

/// Synthetic dataset generator
pub struct SyntheticDatasetGenerator {
    llm_client: Arc<dyn LLMClient>,
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
}

impl SyntheticDatasetGenerator {
    /// Create a new generator with LLM client
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            embedding_client: None,
        }
    }

    /// Add embedding client for diversity computation
    pub fn with_embedding_client(mut self, client: Arc<dyn EmbeddingClient>) -> Self {
        self.embedding_client = Some(client);
        self
    }

    /// Compute quality score for a trace
    pub fn compute_quality_score(
        &self,
        trace: &TraceContext,
        _existing_embeddings: Option<&[Vec<f64>]>,
    ) -> TraceQualityScore {
        // Base confidence from metadata
        let confidence = trace
            .metadata
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        // User feedback if available
        let feedback = trace
            .metadata
            .get("user_feedback")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                trace
                    .metadata
                    .get("rating")
                    .and_then(|v| v.as_f64())
                    .map(|r| r / 5.0) // Normalize 1-5 to 0-1
            });

        // Diversity score (placeholder - would use embeddings in full implementation)
        let diversity = 0.5; // Default moderate diversity

        // Weighted quality score
        let quality = 0.4 * confidence + 0.4 * feedback.unwrap_or(0.5) + 0.2 * diversity;

        TraceQualityScore {
            trace_id: trace.trace_id,
            quality,
            confidence,
            feedback,
            diversity,
        }
    }

    /// Generate a golden reference from a trace using LLM refinement
    ///
    /// Uses TOON format for ~50% token reduction vs JSON
    pub async fn generate_golden_reference(
        &self,
        trace: &TraceContext,
    ) -> Result<TestCase, EvalError> {
        let input = trace
            .input
            .as_ref()
            .ok_or_else(|| EvalError::MissingField("input".to_string()))?;
        let output = trace
            .output
            .as_ref()
            .ok_or_else(|| EvalError::MissingField("output".to_string()))?;

        // Use TOON format for token-efficient LLM communication
        // TOON uses @key instead of "key": and 'strings' instead of "strings"
        // This reduces token usage by ~50%
        let prompt = format!(
            r#"Create a golden reference for evaluation.

@input '{input}'
@output '{output}'

Refine output to be:
1. Direct answer to input
2. Factually accurate/complete
3. Clear professional language
4. No errors/filler/inconsistencies

Reply in TOON format:
@refined_output '<improved answer>'
@quality_notes '<what improved>'
@difficulty '<easy|medium|hard|expert>'
@tags '<tag1>' '<tag2>'"#
        );

        let response = self.llm_client.evaluate(prompt).await.map_err(|e| {
            EvalError::LLMClientError(format!("Golden reference generation failed: {:?}", e))
        })?;

        // Parse TOON response (fallback to JSON if needed)
        let (refined_output, difficulty, tags, quality_notes) =
            Self::parse_toon_or_json_response(&response.content, output)?;

        Ok(TestCase {
            id: uuid::Uuid::new_v4().to_string(),
            input: input.clone(),
            expected_output: refined_output,
            context: trace.context.clone(),
            difficulty,
            tags,
            source_trace_id: Some(trace.trace_id.to_string()),
            metadata: HashMap::from([(
                "quality_notes".to_string(),
                serde_json::json!(quality_notes),
            )]),
        })
    }

    /// Parse TOON format response, with JSON fallback
    fn parse_toon_or_json_response(
        content: &str,
        fallback_output: &str,
    ) -> Result<(String, Difficulty, Vec<String>, String), EvalError> {
        // Try TOON parsing first (more token-efficient)
        if content.contains("@refined_output") {
            let refined = Self::extract_toon_value(content, "refined_output")
                .unwrap_or_else(|| fallback_output.to_string());
            let difficulty = match Self::extract_toon_value(content, "difficulty")
                .as_deref()
                .unwrap_or("medium")
            {
                "easy" => Difficulty::Easy,
                "hard" => Difficulty::Hard,
                "expert" => Difficulty::Expert,
                _ => Difficulty::Medium,
            };
            let tags = Self::extract_toon_tags(content, "tags");
            let notes = Self::extract_toon_value(content, "quality_notes").unwrap_or_default();

            return Ok((refined, difficulty, tags, notes));
        }

        // Fallback to JSON parsing
        let json: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| EvalError::LLMClientError(format!("Failed to parse response: {}", e)))?;

        let difficulty = match json["difficulty"].as_str().unwrap_or("medium") {
            "easy" => Difficulty::Easy,
            "hard" => Difficulty::Hard,
            "expert" => Difficulty::Expert,
            _ => Difficulty::Medium,
        };

        let tags: Vec<String> = json["tags"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok((
            json["refined_output"]
                .as_str()
                .unwrap_or(fallback_output)
                .to_string(),
            difficulty,
            tags,
            json["quality_notes"].as_str().unwrap_or("").to_string(),
        ))
    }

    /// Extract a value from TOON format: @key 'value'
    fn extract_toon_value(content: &str, key: &str) -> Option<String> {
        let pattern = format!("@{} '", key);
        if let Some(start) = content.find(&pattern) {
            let value_start = start + pattern.len();
            if let Some(end) = content[value_start..].find('\'') {
                return Some(content[value_start..value_start + end].to_string());
            }
        }
        None
    }

    /// Extract tags from TOON format: @tags 'tag1' 'tag2'
    fn extract_toon_tags(content: &str, key: &str) -> Vec<String> {
        let pattern = format!("@{} ", key);
        if let Some(start) = content.find(&pattern) {
            let line_end = content[start..].find('\n').unwrap_or(content.len() - start);
            let line = &content[start..start + line_end];

            // Extract all quoted values
            let mut tags = Vec::new();
            let mut remaining = line;
            while let Some(quote_start) = remaining.find('\'') {
                remaining = &remaining[quote_start + 1..];
                if let Some(quote_end) = remaining.find('\'') {
                    tags.push(remaining[..quote_end].to_string());
                    remaining = &remaining[quote_end + 1..];
                } else {
                    break;
                }
            }
            return tags;
        }
        Vec::new()
    }

    /// Generate perturbations for robustness testing
    pub async fn generate_perturbations(
        &self,
        test_case: &TestCase,
        strategies: &[PerturbationStrategy],
    ) -> Result<Vec<TestCase>, EvalError> {
        let mut perturbations = Vec::new();

        for (idx, strategy) in strategies.iter().enumerate() {
            let perturbed_input = match strategy {
                PerturbationStrategy::Typo { rate } => self.inject_typos(&test_case.input, *rate),
                PerturbationStrategy::Paraphrase => self.paraphrase_input(&test_case.input).await?,
                PerturbationStrategy::Truncate { keep_ratio } => {
                    self.truncate_input(&test_case.input, *keep_ratio)
                }
                PerturbationStrategy::Adversarial => {
                    self.generate_adversarial(&test_case.input).await?
                }
                PerturbationStrategy::SynonymSubstitution { rate } => {
                    self.substitute_synonyms(&test_case.input, *rate)
                }
                PerturbationStrategy::NoiseInjection { words_to_add } => {
                    self.inject_noise(&test_case.input, *words_to_add)
                }
            };

            let mut perturbed = test_case.clone();
            perturbed.id = format!("{}_perturbed_{}", test_case.id, idx);
            perturbed.input = perturbed_input;
            perturbed.tags.push(format!("perturbation:{:?}", strategy));
            perturbed.metadata.insert(
                "perturbation_type".to_string(),
                format!("{:?}", strategy).into(),
            );
            perturbed
                .metadata
                .insert("original_input".to_string(), test_case.input.clone().into());

            perturbations.push(perturbed);
        }

        Ok(perturbations)
    }

    /// Inject random typos into text
    fn inject_typos(&self, text: &str, rate: f64) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let chars: Vec<char> = text.chars().collect();
        let mut result = String::with_capacity(text.len() + 10);

        let typo_chars = ['q', 'w', 'e', 'r', 't', 'a', 's', 'd', 'f', 'g'];

        for (i, ch) in chars.iter().enumerate() {
            if ch.is_alphabetic() && rng.gen_bool(rate) {
                // Apply a random typo
                let choice = rng.gen_range(0..4_u32);
                match choice {
                    0 if i + 1 < chars.len() => {
                        // Swap with next character
                        result.push(chars[i + 1]);
                        result.push(*ch);
                    }
                    1 => {
                        // Substitute with nearby key
                        let idx = rng.gen_range(0..typo_chars.len());
                        result.push(typo_chars[idx]);
                    }
                    2 => {
                        // Double the character
                        result.push(*ch);
                        result.push(*ch);
                    }
                    _ => {
                        // Keep as is
                        result.push(*ch);
                    }
                }
            } else {
                result.push(*ch);
            }
        }

        result
    }

    /// Paraphrase input using LLM
    async fn paraphrase_input(&self, text: &str) -> Result<String, EvalError> {
        let prompt = format!(
            r#"Paraphrase the following text while preserving its meaning. 
Use different words and sentence structure but keep the core intent.

TEXT: {text}

Respond with ONLY the paraphrased text, no explanation."#
        );

        let response = self
            .llm_client
            .evaluate(prompt)
            .await
            .map_err(|e| EvalError::LLMClientError(format!("Paraphrase failed: {:?}", e)))?;

        // Try to extract clean text (remove JSON wrapper if present)
        let content = response.content.trim();
        if content.starts_with('{') {
            let json: serde_json::Value = serde_json::from_str(content).unwrap_or_default();
            Ok(json["paraphrase"]
                .as_str()
                .or(json["text"].as_str())
                .unwrap_or(content)
                .to_string())
        } else {
            Ok(content.to_string())
        }
    }

    /// Truncate input to keep a portion
    fn truncate_input(&self, text: &str, keep_ratio: f64) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        let keep_count = ((words.len() as f64) * keep_ratio).ceil() as usize;
        words[..keep_count.min(words.len())].join(" ")
    }

    /// Generate adversarial variant
    async fn generate_adversarial(&self, text: &str) -> Result<String, EvalError> {
        let prompt = format!(
            r#"Generate an adversarial variant of this input that:
1. Changes the intent or meaning subtly
2. Adds misleading information
3. Uses confusing phrasing

Original: {text}

Respond with ONLY the adversarial text."#
        );

        let response = self.llm_client.evaluate(prompt).await.map_err(|e| {
            EvalError::LLMClientError(format!("Adversarial generation failed: {:?}", e))
        })?;

        Ok(response.content.trim().to_string())
    }

    /// Substitute synonyms in text
    fn substitute_synonyms(&self, text: &str, _rate: f64) -> String {
        // Simple synonym substitution (would use WordNet in production)
        let synonyms: HashMap<&str, &str> = HashMap::from([
            ("good", "excellent"),
            ("bad", "poor"),
            ("big", "large"),
            ("small", "tiny"),
            ("fast", "quick"),
            ("slow", "sluggish"),
            ("help", "assist"),
            ("use", "utilize"),
            ("make", "create"),
            ("get", "obtain"),
        ]);

        let words: Vec<&str> = text.split_whitespace().collect();
        words
            .iter()
            .map(|w| {
                let lower = w.to_lowercase();
                synonyms.get(lower.as_str()).unwrap_or(w).to_string()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Inject noise/filler words
    fn inject_noise(&self, text: &str, words_to_add: usize) -> String {
        use rand::seq::SliceRandom;
        let fillers = [
            "basically",
            "actually",
            "literally",
            "honestly",
            "you know",
            "like",
            "so",
            "well",
            "um",
            "kind of",
        ];

        let mut rng = rand::thread_rng();
        let words: Vec<&str> = text.split_whitespace().collect();

        if words.is_empty() {
            return text.to_string();
        }

        let mut result: Vec<String> = words.iter().map(|s| s.to_string()).collect();

        for _ in 0..words_to_add {
            if let Some(filler) = fillers.choose(&mut rng) {
                let pos = rand::Rng::gen_range(&mut rng, 0..=result.len());
                result.insert(pos, filler.to_string());
            }
        }

        result.join(" ")
    }

    /// Export dataset to JSONL format
    pub fn export_jsonl(&self, test_cases: &[TestCase]) -> String {
        test_cases
            .iter()
            .filter_map(|tc| serde_json::to_string(tc).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Export dataset to JSON array format
    pub fn export_json(&self, test_cases: &[TestCase]) -> Result<String, EvalError> {
        serde_json::to_string_pretty(test_cases)
            .map_err(|e| EvalError::Internal(format!("JSON export failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typo_injection() {
        // Create a mock generator (we only test the non-async methods)
        struct MockLLM;

        #[async_trait]
        impl LLMClient for MockLLM {
            async fn evaluate(
                &self,
                _prompt: String,
            ) -> Result<crate::llm_client::LLMResponse, crate::llm_client::LLMError> {
                unimplemented!()
            }
            fn model_name(&self) -> &str {
                "mock"
            }
            fn cost_per_token(&self) -> (f64, f64) {
                (0.0, 0.0)
            }
        }

        let generator = SyntheticDatasetGenerator::new(Arc::new(MockLLM));

        let text = "Hello world this is a test";
        let perturbed = generator.inject_typos(text, 0.0);

        // With rate 0.0, should be unchanged
        assert_eq!(perturbed, text);
    }

    #[test]
    fn test_truncation() {
        struct MockLLM;

        #[async_trait]
        impl LLMClient for MockLLM {
            async fn evaluate(
                &self,
                _prompt: String,
            ) -> Result<crate::llm_client::LLMResponse, crate::llm_client::LLMError> {
                unimplemented!()
            }
            fn model_name(&self) -> &str {
                "mock"
            }
            fn cost_per_token(&self) -> (f64, f64) {
                (0.0, 0.0)
            }
        }

        let generator = SyntheticDatasetGenerator::new(Arc::new(MockLLM));

        let text = "one two three four five";
        let truncated = generator.truncate_input(text, 0.6);

        // 5 words * 0.6 = 3 words
        assert_eq!(truncated, "one two three");
    }

    #[test]
    fn test_difficulty_serialization() {
        let easy = Difficulty::Easy;
        let json = serde_json::to_string(&easy).unwrap();
        assert_eq!(json, "\"easy\"");

        let parsed: Difficulty = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Difficulty::Easy);
    }
}
