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

//! GenAI Semantic Conventions Storage Schema
//!
//! Provides structured storage for OpenTelemetry GenAI semantic conventions.
//! This enables efficient querying and analytics on LLM operations.

use serde::{Deserialize, Serialize};

/// GenAI span data following OTEL v1.36+ semantic conventions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenAISpanData {
    // REQUIRED attributes (per OTEL spec)
    pub operation_name: String, // "chat", "completion", "embedding"
    pub system: String,         // "openai", "anthropic", "bedrock"
    pub request_model: String,  // "gpt-4o", "claude-sonnet-4.5"
    pub response_model: Option<String>, // Actual model used (may differ from request)

    // Token usage (CRITICAL for cost tracking)
    pub usage: TokenUsage,

    // Model parameters (for reproducibility & debugging)
    pub parameters: ModelParameters,

    // Response metadata
    pub response_id: Option<String>,
    pub finish_reasons: Vec<String>, // ["stop", "length", "content_filter"]

    // Optional: Prompts/responses (PII-sensitive, opt-in only)
    pub content: Option<ContentData>,
}

/// Token usage tracking with support for advanced features
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub reasoning_tokens: Option<u32>,      // OpenAI O1 models
    pub cache_read_tokens: Option<u32>,     // Anthropic cache hits
    pub cache_creation_tokens: Option<u32>, // Anthropic cache creation
    pub total_tokens: u32,
    pub cost_usd: Option<f64>, // Pre-computed cost
}

impl TokenUsage {
    /// Calculate total tokens
    pub fn calculate_total(&self) -> u32 {
        self.input_tokens + self.output_tokens + self.reasoning_tokens.unwrap_or(0)
    }

    /// Get effective input tokens (excluding cached)
    pub fn effective_input_tokens(&self) -> u32 {
        self.input_tokens
            .saturating_sub(self.cache_read_tokens.unwrap_or(0))
    }
}

/// Model parameters for reproducibility
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelParameters {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<i32>,
    pub max_tokens: Option<u32>,
    pub seed: Option<i64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub stop_sequences: Option<Vec<String>>,
}

/// Content data (prompts and responses)
/// Only captured when OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentData {
    pub input_messages: Vec<Message>,
    pub output_messages: Vec<Message>,
}

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user", "assistant", "system", "tool"
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Tool call in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String, // JSON string
}

impl GenAISpanData {
    /// Create a new GenAI span data with minimal required fields
    pub fn new(
        operation: impl Into<String>,
        system: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            operation_name: operation.into(),
            system: system.into(),
            request_model: model.into(),
            response_model: None,
            usage: TokenUsage::default(),
            parameters: ModelParameters::default(),
            response_id: None,
            finish_reasons: Vec::new(),
            content: None,
        }
    }

    /// Set token usage
    pub fn with_usage(mut self, input: u32, output: u32) -> Self {
        self.usage.input_tokens = input;
        self.usage.output_tokens = output;
        self.usage.total_tokens = input + output;
        self
    }

    /// Set cost
    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.usage.cost_usd = Some(cost_usd);
        self
    }

    /// Set model parameters
    pub fn with_parameters(mut self, params: ModelParameters) -> Self {
        self.parameters = params;
        self
    }

    /// Set response metadata
    pub fn with_response(
        mut self,
        response_id: impl Into<String>,
        finish_reason: impl Into<String>,
    ) -> Self {
        self.response_id = Some(response_id.into());
        self.finish_reasons = vec![finish_reason.into()];
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genai_span_data_creation() {
        let span_data = GenAISpanData::new("chat", "openai", "gpt-4o")
            .with_usage(100, 50)
            .with_cost(0.015);

        assert_eq!(span_data.operation_name, "chat");
        assert_eq!(span_data.system, "openai");
        assert_eq!(span_data.request_model, "gpt-4o");
        assert_eq!(span_data.usage.input_tokens, 100);
        assert_eq!(span_data.usage.output_tokens, 50);
        assert_eq!(span_data.usage.total_tokens, 150);
        assert_eq!(span_data.usage.cost_usd, Some(0.015));
    }

    #[test]
    fn test_token_usage_calculation() {
        let mut usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            reasoning_tokens: Some(25),
            ..Default::default()
        };

        assert_eq!(usage.calculate_total(), 175);

        usage.cache_read_tokens = Some(30);
        assert_eq!(usage.effective_input_tokens(), 70);
    }
}
