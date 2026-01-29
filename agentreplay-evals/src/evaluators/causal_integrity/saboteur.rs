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

//! Saboteur Perturbation Module
//!
//! Generates perturbations for CIP evaluation:
//! - Critical perturbation: Semantically inverts key facts
//! - Null perturbation: Paraphrases while preserving semantics
//!
//! Uses LLM to generate high-quality perturbations with validation.

use crate::llm_client::{LLMClient, LLMResponse};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors that can occur during perturbation generation
#[derive(Debug, Error)]
pub enum SaboteurError {
    #[error("LLM call failed: {0}")]
    LLMError(String),

    #[error("Empty response from LLM")]
    EmptyResponse,

    #[error("Perturbation too short: {length} chars (min: {min})")]
    TooShort { length: usize, min: usize },

    #[error("Perturbation too long: {length} chars (max: {max})")]
    TooLong { length: usize, max: usize },

    #[error("Perturbation identical to original")]
    IdenticalToOriginal,

    #[error("Max retries exceeded after {attempts} attempts")]
    MaxRetriesExceeded { attempts: u32 },
}

/// Configuration for the Saboteur
#[derive(Debug, Clone)]
pub struct SaboteurConfig {
    /// Minimum length ratio (output/input)
    pub min_length_ratio: f64,
    /// Maximum length ratio (output/input)
    pub max_length_ratio: f64,
    /// Maximum attempts before failing
    pub max_attempts: u32,
    /// Temperature for LLM generation
    pub temperature: f64,
}

impl Default for SaboteurConfig {
    fn default() -> Self {
        Self {
            min_length_ratio: 0.5,
            max_length_ratio: 2.0,
            max_attempts: 3,
            temperature: 0.7,
        }
    }
}

/// Type of perturbation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerturbationType {
    /// Critical perturbation: Changes key facts
    Critical,
    /// Null perturbation: Paraphrases without changing meaning
    Null,
}

/// Result of a perturbation generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerturbationResult {
    /// The perturbed context
    pub perturbed_context: String,
    /// The original context
    pub original_context: String,
    /// Type of perturbation
    pub perturbation_type: PerturbationType,
    /// Number of attempts needed
    pub attempts: u32,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Cost of generation in USD
    pub cost_usd: f64,
    /// Tokens used
    pub tokens_used: u64,
}

/// Trait for perturbation generators
#[async_trait]
pub trait Perturbator: Send + Sync {
    /// Generate a critical perturbation (semantic change)
    async fn generate_critical(
        &self,
        query: &str,
        context: &str,
    ) -> Result<PerturbationResult, SaboteurError>;

    /// Generate a null perturbation (paraphrase)
    async fn generate_null(&self, context: &str) -> Result<PerturbationResult, SaboteurError>;
}

/// LLM-based Saboteur for generating perturbations
pub struct SaboteurPerturbator {
    llm_client: Arc<dyn LLMClient>,
    config: SaboteurConfig,
}

impl SaboteurPerturbator {
    /// Create a new Saboteur with the given LLM client
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            config: SaboteurConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(mut self, config: SaboteurConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the prompt for critical perturbation
    fn build_critical_prompt(&self, query: &str, context: &str) -> String {
        format!(
            r#"You are a factual perturbation assistant. Your task is to modify the given context by changing ONE key fact that would affect the answer to the question.

IMPORTANT RULES:
1. Change exactly ONE factual claim that is relevant to answering the question
2. The change should be semantically significant (e.g., change a number, name, date, or key attribute)
3. Keep the overall structure and length similar
4. Do NOT add new information or remove significant content
5. The modified context should still be grammatically correct and coherent

QUESTION: {query}

ORIGINAL CONTEXT:
{context}

Output ONLY the modified context, nothing else. No explanations, no prefixes."#
        )
    }

    /// Build the prompt for null perturbation
    fn build_null_prompt(&self, context: &str) -> String {
        format!(
            r#"You are a paraphrasing assistant. Your task is to rewrite the given context while preserving ALL factual information exactly.

IMPORTANT RULES:
1. Preserve ALL facts, numbers, names, and claims exactly
2. Change the wording, sentence structure, and phrasing
3. Keep the same length (within 20%)
4. The meaning must be IDENTICAL to the original
5. Do NOT add or remove any information

ORIGINAL CONTEXT:
{context}

Output ONLY the paraphrased context, nothing else. No explanations, no prefixes."#
        )
    }

    /// Extract and validate the perturbation from LLM response
    fn extract_perturbation(
        &self,
        response: &LLMResponse,
        original: &str,
    ) -> Result<String, SaboteurError> {
        // Clean up the response
        let cleaned = response
            .content
            .trim()
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        if cleaned.is_empty() {
            return Err(SaboteurError::EmptyResponse);
        }

        // Length validation
        let min_length = (original.len() as f64 * self.config.min_length_ratio) as usize;
        let max_length = (original.len() as f64 * self.config.max_length_ratio) as usize;

        if cleaned.len() < min_length {
            return Err(SaboteurError::TooShort {
                length: cleaned.len(),
                min: min_length,
            });
        }

        if cleaned.len() > max_length {
            return Err(SaboteurError::TooLong {
                length: cleaned.len(),
                max: max_length,
            });
        }

        // Check not identical
        if cleaned == original.trim() {
            return Err(SaboteurError::IdenticalToOriginal);
        }

        Ok(cleaned.to_string())
    }

    /// Calculate cost from response
    fn calculate_cost(&self, response: &LLMResponse) -> f64 {
        let (input_cost, output_cost) = self.llm_client.cost_per_token();
        let input_tokens = response.usage.prompt_tokens as f64;
        let output_tokens = response.usage.completion_tokens as f64;
        input_tokens * input_cost + output_tokens * output_cost
    }
}

#[async_trait]
impl Perturbator for SaboteurPerturbator {
    async fn generate_critical(
        &self,
        query: &str,
        context: &str,
    ) -> Result<PerturbationResult, SaboteurError> {
        let start = Instant::now();
        let mut total_cost = 0.0;
        let mut total_tokens = 0u64;

        for attempt in 1..=self.config.max_attempts {
            debug!(
                "Critical perturbation attempt {}/{}",
                attempt, self.config.max_attempts
            );

            let prompt = self.build_critical_prompt(query, context);

            let response = self
                .llm_client
                .evaluate(prompt)
                .await
                .map_err(|e| SaboteurError::LLMError(e.to_string()))?;

            total_cost += self.calculate_cost(&response);
            total_tokens +=
                response.usage.prompt_tokens as u64 + response.usage.completion_tokens as u64;

            match self.extract_perturbation(&response, context) {
                Ok(perturbed) => {
                    info!(
                        "Critical perturbation generated successfully (attempt {})",
                        attempt
                    );

                    return Ok(PerturbationResult {
                        perturbed_context: perturbed,
                        original_context: context.to_string(),
                        perturbation_type: PerturbationType::Critical,
                        attempts: attempt,
                        duration_ms: start.elapsed().as_millis() as u64,
                        cost_usd: total_cost,
                        tokens_used: total_tokens,
                    });
                }
                Err(e) => {
                    warn!("Critical perturbation attempt {} failed: {:?}", attempt, e);
                    if attempt == self.config.max_attempts {
                        return Err(SaboteurError::MaxRetriesExceeded {
                            attempts: self.config.max_attempts,
                        });
                    }
                }
            }
        }

        Err(SaboteurError::MaxRetriesExceeded {
            attempts: self.config.max_attempts,
        })
    }

    async fn generate_null(&self, context: &str) -> Result<PerturbationResult, SaboteurError> {
        let start = Instant::now();
        let mut total_cost = 0.0;
        let mut total_tokens = 0u64;

        for attempt in 1..=self.config.max_attempts {
            debug!(
                "Null perturbation attempt {}/{}",
                attempt, self.config.max_attempts
            );

            let prompt = self.build_null_prompt(context);

            let response = self
                .llm_client
                .evaluate(prompt)
                .await
                .map_err(|e| SaboteurError::LLMError(e.to_string()))?;

            total_cost += self.calculate_cost(&response);
            total_tokens +=
                response.usage.prompt_tokens as u64 + response.usage.completion_tokens as u64;

            match self.extract_perturbation(&response, context) {
                Ok(perturbed) => {
                    info!(
                        "Null perturbation generated successfully (attempt {})",
                        attempt
                    );

                    return Ok(PerturbationResult {
                        perturbed_context: perturbed,
                        original_context: context.to_string(),
                        perturbation_type: PerturbationType::Null,
                        attempts: attempt,
                        duration_ms: start.elapsed().as_millis() as u64,
                        cost_usd: total_cost,
                        tokens_used: total_tokens,
                    });
                }
                Err(e) => {
                    warn!("Null perturbation attempt {} failed: {:?}", attempt, e);
                    if attempt == self.config.max_attempts {
                        return Err(SaboteurError::MaxRetriesExceeded {
                            attempts: self.config.max_attempts,
                        });
                    }
                }
            }
        }

        Err(SaboteurError::MaxRetriesExceeded {
            attempts: self.config.max_attempts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_client::{LLMError, TokenUsage};

    struct MockLLMClient {
        response: String,
    }

    impl MockLLMClient {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn evaluate(&self, _prompt: String) -> Result<LLMResponse, LLMError> {
            Ok(LLMResponse {
                content: self.response.clone(),
                usage: TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                },
                model: "mock-model".to_string(),
            })
        }

        fn model_name(&self) -> &str {
            "mock-model"
        }

        fn cost_per_token(&self) -> (f64, f64) {
            (0.000001, 0.000002)
        }
    }

    #[tokio::test]
    async fn test_critical_perturbation() {
        let mock = Arc::new(MockLLMClient::new(
            "The capital of Germany is Munich. It is a large city in Bavaria.",
        ));
        let saboteur = SaboteurPerturbator::new(mock);

        let result = saboteur
            .generate_critical(
                "What is the capital of France?",
                "The capital of France is Paris. It is a large city on the Seine river.",
            )
            .await
            .unwrap();

        assert_eq!(result.perturbation_type, PerturbationType::Critical);
        assert_eq!(result.attempts, 1);
        assert!(result.cost_usd > 0.0);
        assert!(result.tokens_used > 0);
    }

    #[tokio::test]
    async fn test_null_perturbation() {
        let mock = Arc::new(MockLLMClient::new(
            "Paris serves as France's capital city. This major urban center is situated along the Seine river.",
        ));
        let saboteur = SaboteurPerturbator::new(mock);

        let result = saboteur
            .generate_null("The capital of France is Paris. It is a large city on the Seine river.")
            .await
            .unwrap();

        assert_eq!(result.perturbation_type, PerturbationType::Null);
        assert_eq!(result.attempts, 1);
    }

    #[tokio::test]
    async fn test_empty_response_error() {
        let mock = Arc::new(MockLLMClient::new(""));
        let saboteur = SaboteurPerturbator::new(mock);

        let result = saboteur
            .generate_null("The capital of France is Paris.")
            .await;

        assert!(matches!(
            result,
            Err(SaboteurError::MaxRetriesExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn test_identical_response_error() {
        let context = "The capital of France is Paris.";
        let mock = Arc::new(MockLLMClient::new(context));
        let saboteur = SaboteurPerturbator::new(mock);

        let result = saboteur.generate_null(context).await;

        assert!(matches!(
            result,
            Err(SaboteurError::MaxRetriesExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn test_too_short_response() {
        let mock = Arc::new(MockLLMClient::new("Short."));
        let saboteur = SaboteurPerturbator::new(mock);

        let result = saboteur
            .generate_null(
                "The capital of France is Paris. It is a large city on the Seine river with many famous landmarks.",
            )
            .await;

        assert!(matches!(
            result,
            Err(SaboteurError::MaxRetriesExceeded { .. })
        ));
    }

    #[test]
    fn test_saboteur_config_default() {
        let config = SaboteurConfig::default();
        assert_eq!(config.min_length_ratio, 0.5);
        assert_eq!(config.max_length_ratio, 2.0);
        assert_eq!(config.max_attempts, 3);
    }
}
