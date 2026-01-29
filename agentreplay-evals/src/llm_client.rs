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

//! LLM client abstraction for LLM-as-judge evaluators

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Trait for LLM clients used in evaluations
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// Send a prompt and get structured JSON response
    async fn evaluate(&self, prompt: String) -> Result<LLMResponse, LLMError>;

    /// Evaluate with token log probabilities for G-Eval probability normalization
    /// Returns response with logprobs for the top_k most likely tokens at each position
    async fn evaluate_with_logprobs(
        &self,
        prompt: String,
        _top_k: usize,
    ) -> Result<LLMResponseWithLogprobs, LLMError> {
        // Default implementation: fall back to regular evaluate without logprobs
        let response = self.evaluate(prompt).await?;
        Ok(LLMResponseWithLogprobs {
            content: response.content,
            usage: response.usage,
            model: response.model,
            logprobs: None,
        })
    }

    /// Get model name
    fn model_name(&self) -> &str;

    /// Get cost per token (input, output)
    fn cost_per_token(&self) -> (f64, f64);
}

/// Response from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub content: String,
    pub usage: TokenUsage,
    pub model: String,
}

impl LLMResponse {
    /// Parse response as JSON
    pub fn as_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.content)
    }

    /// Get a specific field from JSON response
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.as_json().ok()?.get(key).cloned()
    }
}

/// Response from LLM with token log probabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponseWithLogprobs {
    pub content: String,
    pub usage: TokenUsage,
    pub model: String,
    pub logprobs: Option<Vec<TokenLogprob>>,
}

impl LLMResponseWithLogprobs {
    /// Parse response as JSON
    pub fn as_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.content)
    }

    /// Extract probability distribution for score tokens (1-5 for G-Eval)
    /// Returns array of probabilities [P(1), P(2), P(3), P(4), P(5)]
    pub fn extract_score_probabilities(&self, scale_max: u8) -> Vec<f64> {
        let mut probs = vec![0.0; scale_max as usize];

        if let Some(logprobs) = &self.logprobs {
            for token_logprob in logprobs {
                // Check if this token represents a score (1-5)
                if let Ok(score) = token_logprob.token.trim().parse::<u8>() {
                    if score >= 1 && score <= scale_max {
                        let idx = (score - 1) as usize;
                        // Convert logprob to probability: P = exp(logprob)
                        probs[idx] += token_logprob.logprob.exp();
                    }
                }

                // Also check top alternatives
                for alt in &token_logprob.top_logprobs {
                    if let Ok(score) = alt.token.trim().parse::<u8>() {
                        if score >= 1 && score <= scale_max {
                            let idx = (score - 1) as usize;
                            probs[idx] += alt.logprob.exp();
                        }
                    }
                }
            }
        }

        // Normalize if we have any probabilities
        let sum: f64 = probs.iter().sum();
        if sum > 0.0 {
            probs.iter_mut().for_each(|p| *p /= sum);
        }

        probs
    }
}

/// Token-level log probability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogprob {
    pub token: String,
    pub logprob: f64,
    pub top_logprobs: Vec<TopLogprob>,
}

/// Alternative token with its log probability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLogprob {
    pub token: String,
    pub logprob: f64,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Calculate cost based on per-token pricing
    pub fn calculate_cost(&self, cost_per_input: f64, cost_per_output: f64) -> f64 {
        (self.prompt_tokens as f64 * cost_per_input)
            + (self.completion_tokens as f64 * cost_per_output)
    }
}

/// Errors from LLM clients
#[derive(Debug, Error)]
pub enum LLMError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Errors from embedding clients
#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Trait for embedding clients used in evaluations
#[async_trait]
pub trait EmbeddingClient: Send + Sync {
    /// Embed a single text string
    async fn embed(&self, text: &str) -> Result<Vec<f64>, EmbedError>;

    /// Embed a batch of texts
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f64>>, EmbedError>;
}

/// OpenAI client implementation
pub struct OpenAIClient {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://api.openai.com/v1".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl EmbeddingClient for OpenAIClient {
    async fn embed(&self, text: &str) -> Result<Vec<f64>, EmbedError> {
        let embeddings = self.embed_batch(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbedError::ApiError("No embedding returned".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f64>>, EmbedError> {
        // OpenAI embedding model usually defaults to text-embedding-3-small or ada-002
        // If self.model is a chat model (gpt-4o), we should use a default embedding model
        // or expect the user to have passed an embedding model name.
        // For simplicity here, we assume if the model name contains "embedding", use it,
        // otherwise default to text-embedding-3-small.
        let embedding_model = if self.model.contains("embedding") {
            &self.model
        } else {
            "text-embedding-3-small"
        };

        let request = serde_json::json!({
            "model": embedding_model,
            "input": texts
        });

        let response = self
            .client
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(EmbedError::RateLimitExceeded);
            }
            return Err(EmbedError::ApiError(error_text));
        }

        let response_data: serde_json::Value = response.json().await?;

        let mut embeddings = Vec::new();
        if let Some(data) = response_data["data"].as_array() {
            for item in data {
                if let Some(embedding_vec) = item["embedding"].as_array() {
                    let vec: Vec<f64> = embedding_vec.iter().filter_map(|v| v.as_f64()).collect();
                    embeddings.push(vec);
                }
            }
        }

        if embeddings.len() != texts.len() {
            return Err(EmbedError::ApiError(format!(
                "Expected {} embeddings, got {}",
                texts.len(),
                embeddings.len()
            )));
        }

        Ok(embeddings)
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    async fn evaluate(&self, prompt: String) -> Result<LLMResponse, LLMError> {
        let request = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert evaluator. Respond only with valid JSON."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.0,
            "response_format": { "type": "json_object" }
        });

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(LLMError::ApiError(error_text));
        }

        let response_data: serde_json::Value = response.json().await?;

        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or(LLMError::InvalidResponse("Missing content".to_string()))?
            .to_string();

        let usage_data = &response_data["usage"];
        let usage = TokenUsage {
            prompt_tokens: usage_data["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage_data["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage_data["total_tokens"].as_u64().unwrap_or(0) as u32,
        };

        Ok(LLMResponse {
            content,
            usage,
            model: self.model.clone(),
        })
    }

    async fn evaluate_with_logprobs(
        &self,
        prompt: String,
        top_k: usize,
    ) -> Result<LLMResponseWithLogprobs, LLMError> {
        let request = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert evaluator. Respond only with valid JSON."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.0,
            "response_format": { "type": "json_object" },
            "logprobs": true,
            "top_logprobs": top_k.min(20) // OpenAI allows max 20
        });

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(LLMError::ApiError(error_text));
        }

        let response_data: serde_json::Value = response.json().await?;

        let content = response_data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or(LLMError::InvalidResponse("Missing content".to_string()))?
            .to_string();

        let usage_data = &response_data["usage"];
        let usage = TokenUsage {
            prompt_tokens: usage_data["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage_data["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage_data["total_tokens"].as_u64().unwrap_or(0) as u32,
        };

        // Parse logprobs if available
        let logprobs = if let Some(logprobs_data) = response_data["choices"][0]
            .get("logprobs")
            .and_then(|lp| lp.get("content"))
            .and_then(|c| c.as_array())
        {
            let mut token_logprobs = Vec::new();
            for token_data in logprobs_data {
                if let (Some(token), Some(logprob)) =
                    (token_data["token"].as_str(), token_data["logprob"].as_f64())
                {
                    let mut top_logprobs = Vec::new();
                    if let Some(top_array) = token_data["top_logprobs"].as_array() {
                        for top_item in top_array {
                            if let (Some(t), Some(lp)) =
                                (top_item["token"].as_str(), top_item["logprob"].as_f64())
                            {
                                top_logprobs.push(TopLogprob {
                                    token: t.to_string(),
                                    logprob: lp,
                                });
                            }
                        }
                    }

                    token_logprobs.push(TokenLogprob {
                        token: token.to_string(),
                        logprob,
                        top_logprobs,
                    });
                }
            }
            Some(token_logprobs)
        } else {
            None
        };

        Ok(LLMResponseWithLogprobs {
            content,
            usage,
            model: self.model.clone(),
            logprobs,
        })
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn cost_per_token(&self) -> (f64, f64) {
        // Pricing for GPT-4o-mini (as of 2025)
        // $0.15 per 1M input tokens, $0.60 per 1M output tokens
        match self.model.as_str() {
            "gpt-4o" => (0.0000025, 0.000010),        // $2.50/$10 per 1M
            "gpt-4o-mini" => (0.00000015, 0.0000006), // $0.15/$0.60 per 1M
            "gpt-4-turbo" => (0.000010, 0.000030),    // $10/$30 per 1M
            _ => (0.00000015, 0.0000006),             // Default to mini pricing
        }
    }
}

/// Anthropic Claude client implementation
pub struct AnthropicClient {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl AnthropicClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://api.anthropic.com/v1".to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    async fn evaluate(&self, prompt: String) -> Result<LLMResponse, LLMError> {
        let request = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "system": "You are an expert evaluator. Respond only with valid JSON.",
            "temperature": 0.0
        });

        let response = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(LLMError::ApiError(error_text));
        }

        let response_data: serde_json::Value = response.json().await?;

        let content = response_data["content"][0]["text"]
            .as_str()
            .ok_or(LLMError::InvalidResponse("Missing content".to_string()))?
            .to_string();

        let usage_data = &response_data["usage"];
        let usage = TokenUsage {
            prompt_tokens: usage_data["input_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage_data["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: (usage_data["input_tokens"].as_u64().unwrap_or(0)
                + usage_data["output_tokens"].as_u64().unwrap_or(0))
                as u32,
        };

        Ok(LLMResponse {
            content,
            usage,
            model: self.model.clone(),
        })
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn cost_per_token(&self) -> (f64, f64) {
        // Pricing for Claude models (as of 2025)
        match self.model.as_str() {
            "claude-sonnet-4.5" | "claude-3-5-sonnet-20241022" => {
                (0.000003, 0.000015) // $3/$15 per 1M
            }
            "claude-3-5-haiku-20241022" => {
                (0.0000008, 0.000004) // $0.80/$4 per 1M
            }
            _ => (0.000003, 0.000015), // Default to Sonnet pricing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_cost() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };

        // GPT-4o-mini pricing: $0.15/$0.60 per 1M
        let cost = usage.calculate_cost(0.00000015, 0.0000006);

        // 100 * 0.00000015 + 50 * 0.0000006 = 0.000015 + 0.00003 = 0.000045
        assert!((cost - 0.000045).abs() < 0.0000001);
    }

    #[test]
    fn test_openai_cost_per_token() {
        let client = OpenAIClient::new("test".to_string(), "gpt-4o-mini".to_string());
        let (input, output) = client.cost_per_token();

        assert_eq!(input, 0.00000015);
        assert_eq!(output, 0.0000006);
    }
}
