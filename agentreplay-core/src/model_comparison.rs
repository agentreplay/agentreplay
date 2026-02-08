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

//! Model Comparison Types
//!
//! Core data structures for comparing responses from multiple LLM models.
//! Supports up to 3 models running in parallel with independent streaming.

use serde::{Deserialize, Serialize};

/// Maximum number of models that can be compared simultaneously
pub const MAX_COMPARISON_MODELS: usize = 3;

/// Request for comparing multiple models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelComparisonRequest {
    /// The user prompt to send to all models
    pub prompt: String,
    /// List of models to compare (max 3)
    pub models: Vec<ModelSelection>,
    /// Temperature for generation (0.0-2.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Maximum tokens to generate
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Optional system prompt
    pub system_prompt: Option<String>,
    /// Optional variables for prompt templating
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> u32 {
    2048
}

impl ModelComparisonRequest {
    /// Validate the request
    pub fn validate(&self) -> Result<(), ModelComparisonError> {
        if self.models.is_empty() {
            return Err(ModelComparisonError::NoModelsSelected);
        }
        if self.models.len() > MAX_COMPARISON_MODELS {
            return Err(ModelComparisonError::TooManyModels {
                requested: self.models.len(),
                max: MAX_COMPARISON_MODELS,
            });
        }
        if self.prompt.trim().is_empty() {
            return Err(ModelComparisonError::EmptyPrompt);
        }
        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err(ModelComparisonError::InvalidTemperature(self.temperature));
        }
        if self.max_tokens == 0 || self.max_tokens > 128_000 {
            return Err(ModelComparisonError::InvalidMaxTokens(self.max_tokens));
        }
        Ok(())
    }
}

/// Selection of a specific model for comparison
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModelSelection {
    /// Provider identifier (e.g., "openai", "anthropic", "ollama", "custom")
    pub provider: String,
    /// Model ID (e.g., "gpt-4o", "claude-3-5-sonnet-20241022", "llama3.2")
    pub model_id: String,
    /// Optional display name for UI
    #[serde(default)]
    pub display_name: Option<String>,
    /// Base URL for the API endpoint (required for proper routing)
    #[serde(default)]
    pub base_url: Option<String>,
    /// API key for the provider (stored temporarily for request)
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
}

impl ModelSelection {
    pub fn new(provider: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model_id: model_id.into(),
            display_name: None,
            base_url: None,
            api_key: None,
        }
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Get a display-friendly name
    pub fn name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.model_id)
    }

    /// Create a unique key for this model selection
    pub fn key(&self) -> String {
        format!("{}/{}", self.provider, self.model_id)
    }
}

/// Result from a single model in comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelComparisonResult {
    /// The model that was queried
    pub model: ModelSelection,
    /// The generated content (may be partial if streaming failed)
    pub content: String,
    /// Input token count
    pub input_tokens: u32,
    /// Output token count
    pub output_tokens: u32,
    /// Total latency in milliseconds
    pub latency_ms: u32,
    /// Time to first token in milliseconds (for streaming)
    pub time_to_first_token_ms: Option<u32>,
    /// Estimated cost in USD
    pub cost_usd: f64,
    /// Completion status
    pub status: ComparisonStatus,
    /// Error message if status is Error
    pub error: Option<String>,
    /// Finish reason from the model
    pub finish_reason: Option<String>,
    /// Timestamp when the response completed
    pub completed_at: u64,
}

impl ModelComparisonResult {
    /// Create a successful result
    pub fn success(
        model: ModelSelection,
        content: String,
        input_tokens: u32,
        output_tokens: u32,
        latency_ms: u32,
        cost_usd: f64,
    ) -> Self {
        Self {
            model,
            content,
            input_tokens,
            output_tokens,
            latency_ms,
            time_to_first_token_ms: None,
            cost_usd,
            status: ComparisonStatus::Completed,
            error: None,
            finish_reason: Some("stop".to_string()),
            completed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Create an error result
    pub fn error(model: ModelSelection, error: impl Into<String>, latency_ms: u32) -> Self {
        Self {
            model,
            content: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            latency_ms,
            time_to_first_token_ms: None,
            cost_usd: 0.0,
            status: ComparisonStatus::Error,
            error: Some(error.into()),
            finish_reason: None,
            completed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Create a timeout result
    pub fn timeout(model: ModelSelection, timeout_ms: u32) -> Self {
        Self {
            model,
            content: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            latency_ms: timeout_ms,
            time_to_first_token_ms: None,
            cost_usd: 0.0,
            status: ComparisonStatus::Timeout,
            error: Some(format!("Request timed out after {}ms", timeout_ms)),
            finish_reason: None,
            completed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Check if this result was successful
    pub fn is_success(&self) -> bool {
        matches!(self.status, ComparisonStatus::Completed)
    }
}

/// Status of a model comparison execution
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    /// Request is pending/in-progress
    #[default]
    Pending,
    /// Request is actively streaming
    Streaming,
    /// Successfully completed
    Completed,
    /// An error occurred
    Error,
    /// Request timed out
    Timeout,
    /// Request was cancelled
    Cancelled,
}

impl ComparisonStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Error | Self::Timeout | Self::Cancelled
        )
    }
}

/// Full response for a model comparison request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelComparisonResponse {
    /// Unique ID for this comparison
    pub comparison_id: String,
    /// Original request
    pub request: ModelComparisonRequest,
    /// Results from each model
    pub results: Vec<ModelComparisonResult>,
    /// Total execution time in milliseconds
    pub total_latency_ms: u32,
    /// Total cost across all models
    pub total_cost_usd: f64,
    /// When the comparison was started
    pub started_at: u64,
    /// When the comparison completed
    pub completed_at: u64,
}

impl ModelComparisonResponse {
    pub fn new(comparison_id: String, request: ModelComparisonRequest) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            comparison_id,
            request,
            results: Vec::new(),
            total_latency_ms: 0,
            total_cost_usd: 0.0,
            started_at: now,
            completed_at: now,
        }
    }

    /// Add a result and update totals
    pub fn add_result(&mut self, result: ModelComparisonResult) {
        if result.latency_ms > self.total_latency_ms {
            self.total_latency_ms = result.latency_ms;
        }
        self.total_cost_usd += result.cost_usd;
        self.results.push(result);
    }

    /// Finalize the response
    pub fn finalize(&mut self) {
        self.completed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Get the fastest successful result
    pub fn fastest(&self) -> Option<&ModelComparisonResult> {
        self.results
            .iter()
            .filter(|r| r.is_success())
            .min_by_key(|r| r.latency_ms)
    }

    /// Get the cheapest successful result
    pub fn cheapest(&self) -> Option<&ModelComparisonResult> {
        self.results
            .iter()
            .filter(|r| r.is_success())
            .min_by(|a, b| {
                a.cost_usd
                    .partial_cmp(&b.cost_usd)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Count successful results
    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.is_success()).count()
    }

    /// Check if all models completed successfully
    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(|r| r.is_success())
    }
}

/// Streaming chunk for a model comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonStreamChunk {
    /// Comparison ID this chunk belongs to
    pub comparison_id: String,
    /// Index of the model (0-2)
    pub model_index: usize,
    /// The model that generated this chunk
    pub model: ModelSelection,
    /// Content delta
    pub delta: String,
    /// Is this the final chunk?
    pub is_final: bool,
    /// Current token count
    pub token_count: Option<u32>,
    /// Time since start in ms
    pub elapsed_ms: u32,
}

/// Errors that can occur during model comparison
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum ModelComparisonError {
    #[error("No models selected for comparison")]
    NoModelsSelected,

    #[error("Too many models selected: {requested} (max: {max})")]
    TooManyModels { requested: usize, max: usize },

    #[error("Prompt cannot be empty")]
    EmptyPrompt,

    #[error("Invalid temperature: {0} (must be 0.0-2.0)")]
    InvalidTemperature(f32),

    #[error("Invalid max_tokens: {0} (must be 1-128000)")]
    InvalidMaxTokens(u32),

    #[error("Provider not configured: {0}")]
    ProviderNotConfigured(String),

    #[error("Model not found: {provider}/{model_id}")]
    ModelNotFound { provider: String, model_id: String },

    #[error("API error for {model}: {message}")]
    ApiError { model: String, message: String },

    #[error("Timeout waiting for {model}")]
    Timeout { model: String },

    #[error("All models failed")]
    AllModelsFailed,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// User rating for a model response
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseRating {
    /// User preferred this response
    Preferred,
    /// User found this response acceptable
    Acceptable,
    /// User rejected this response
    Rejected,
}

/// User feedback on a comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonFeedback {
    /// Comparison ID
    pub comparison_id: String,
    /// Model index that was rated
    pub model_index: usize,
    /// The rating given
    pub rating: ResponseRating,
    /// Optional free-form notes
    pub notes: Option<String>,
    /// When the feedback was given
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_selection() {
        let model = ModelSelection::new("openai", "gpt-4o");
        assert_eq!(model.key(), "openai/gpt-4o");
        assert_eq!(model.name(), "gpt-4o");

        let model = model.with_display_name("GPT-4o");
        assert_eq!(model.name(), "GPT-4o");
    }

    #[test]
    fn test_request_validation() {
        let request = ModelComparisonRequest {
            prompt: "Hello".to_string(),
            models: vec![ModelSelection::new("openai", "gpt-4o")],
            temperature: 0.7,
            max_tokens: 2048,
            system_prompt: None,
            variables: Default::default(),
        };
        assert!(request.validate().is_ok());

        let empty_prompt = ModelComparisonRequest {
            prompt: "  ".to_string(),
            models: vec![ModelSelection::new("openai", "gpt-4o")],
            temperature: 0.7,
            max_tokens: 2048,
            system_prompt: None,
            variables: Default::default(),
        };
        assert!(matches!(
            empty_prompt.validate(),
            Err(ModelComparisonError::EmptyPrompt)
        ));

        let too_many = ModelComparisonRequest {
            prompt: "Hello".to_string(),
            models: vec![
                ModelSelection::new("openai", "gpt-4o"),
                ModelSelection::new("anthropic", "claude"),
                ModelSelection::new("ollama", "llama"),
                ModelSelection::new("deepseek", "v3"),
            ],
            temperature: 0.7,
            max_tokens: 2048,
            system_prompt: None,
            variables: Default::default(),
        };
        assert!(matches!(
            too_many.validate(),
            Err(ModelComparisonError::TooManyModels { .. })
        ));
    }

    #[test]
    fn test_comparison_status() {
        assert!(!ComparisonStatus::Pending.is_terminal());
        assert!(!ComparisonStatus::Streaming.is_terminal());
        assert!(ComparisonStatus::Completed.is_terminal());
        assert!(ComparisonStatus::Error.is_terminal());
        assert!(ComparisonStatus::Timeout.is_terminal());
        assert!(ComparisonStatus::Cancelled.is_terminal());
    }

    #[test]
    fn test_result_constructors() {
        let model = ModelSelection::new("openai", "gpt-4o");

        let success = ModelComparisonResult::success(
            model.clone(),
            "Hello world".to_string(),
            10,
            5,
            100,
            0.001,
        );
        assert!(success.is_success());
        assert!(success.error.is_none());

        let error = ModelComparisonResult::error(model.clone(), "API failed", 50);
        assert!(!error.is_success());
        assert!(error.error.is_some());

        let timeout = ModelComparisonResult::timeout(model, 30000);
        assert!(!timeout.is_success());
        assert!(matches!(timeout.status, ComparisonStatus::Timeout));
    }
}
