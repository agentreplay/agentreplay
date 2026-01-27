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

//! OpenTelemetry GenAI Semantic Conventions v1.36.0
//!
//! Provides constants and types for instrumenting GenAI/LLM operations
//! according to OpenTelemetry specifications.
//!
//! Source: https://opentelemetry.io/docs/specs/semconv/gen-ai/

/// GenAI semantic convention attribute keys
pub mod keys {
    // Operation attributes (REQUIRED)
    pub const OPERATION_NAME: &str = "gen_ai.operation.name";
    pub const REQUEST_MODEL: &str = "gen_ai.request.model";
    pub const RESPONSE_MODEL: &str = "gen_ai.response.model";
    pub const PROVIDER_NAME: &str = "gen_ai.provider.name";
    pub const SYSTEM_NAME: &str = "gen_ai.system";

    // Usage metrics (REQUIRED for cost tracking)
    pub const INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
    pub const OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
    pub const TOTAL_TOKENS: &str = "gen_ai.usage.total_tokens";
    pub const COST_USD: &str = "gen_ai.usage.cost_usd";

    // Request parameters (OPTIONAL but recommended)
    pub const TEMPERATURE: &str = "gen_ai.request.temperature";
    pub const TOP_P: &str = "gen_ai.request.top_p";
    pub const TOP_K: &str = "gen_ai.request.top_k";
    pub const MAX_TOKENS: &str = "gen_ai.request.max_tokens";
    pub const FREQUENCY_PENALTY: &str = "gen_ai.request.frequency_penalty";
    pub const PRESENCE_PENALTY: &str = "gen_ai.request.presence_penalty";
    pub const STOP_SEQUENCES: &str = "gen_ai.request.stop_sequences";
    pub const SEED: &str = "gen_ai.request.seed";

    // Response metadata
    pub const RESPONSE_ID: &str = "gen_ai.response.id";
    pub const FINISH_REASONS: &str = "gen_ai.response.finish_reasons";
    pub const RESPONSE_PROMPT_TOKENS: &str = "gen_ai.response.prompt_tokens";
    pub const RESPONSE_COMPLETION_TOKENS: &str = "gen_ai.response.completion_tokens";

    // Content attributes (OPT-IN via OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT)
    pub const INPUT_MESSAGES: &str = "gen_ai.input.messages";
    pub const OUTPUT_MESSAGES: &str = "gen_ai.output.messages";
    pub const PROMPT: &str = "gen_ai.prompt";
    pub const COMPLETION: &str = "gen_ai.completion";
}

/// Enumerated values for gen_ai.operation.name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Chat,
    TextCompletion,
    Embeddings,
    CreateAgent,
    InvokeAgent,
    ExecuteTool,
}

impl Operation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::TextCompletion => "text_completion",
            Self::Embeddings => "embeddings",
            Self::CreateAgent => "create_agent",
            Self::InvokeAgent => "invoke_agent",
            Self::ExecuteTool => "execute_tool",
        }
    }
}

/// Enumerated values for gen_ai.provider.name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAI,
    Anthropic,
    AwsBedrock,
    AzureOpenAI,
    GcpVertexAI,
    Cohere,
    Huggingface,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Anthropic => "anthropic",
            Self::AwsBedrock => "aws.bedrock",
            Self::AzureOpenAI => "azure.ai.openai",
            Self::GcpVertexAI => "gcp.vertex_ai",
            Self::Cohere => "cohere",
            Self::Huggingface => "huggingface",
        }
    }
}

/// Helper to build span name following OTEL pattern: "{operation} {model}"
pub fn span_name(operation: Operation, model: &str) -> String {
    format!("{} {}", operation.as_str(), model)
}
