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

/// OpenTelemetry GenAI Semantic Conventions Support
///
/// This module provides utilities for extracting and storing GenAI attributes
/// according to OpenTelemetry semantic conventions v1.36+.
///
/// Reference: https://opentelemetry.io/docs/specs/semconv/gen-ai/
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OpenTelemetry GenAI attribute names (constants for type safety)
/// Reference: https://opentelemetry.io/docs/specs/semconv/gen-ai/
pub mod attrs {
    // =========================================================================
    // PROVIDER IDENTIFICATION (REQUIRED)
    // =========================================================================
    /// The GenAI system (deprecated, use PROVIDER_NAME)
    pub const GEN_AI_SYSTEM: &str = "gen_ai.system";
    /// Provider name: "openai", "anthropic", "aws.bedrock", "azure.ai.openai",
    /// "gcp.gemini", "gcp.vertex_ai", "cohere", "deepseek", "groq", etc.
    pub const GEN_AI_PROVIDER_NAME: &str = "gen_ai.provider.name";
    /// Operation: "chat", "embeddings", "text_completion", "image_generation"
    pub const GEN_AI_OPERATION_NAME: &str = "gen_ai.operation.name";

    // =========================================================================
    // MODEL INFORMATION (REQUIRED)
    // =========================================================================
    pub const GEN_AI_REQUEST_MODEL: &str = "gen_ai.request.model";
    pub const GEN_AI_RESPONSE_MODEL: &str = "gen_ai.response.model";
    pub const GEN_AI_RESPONSE_ID: &str = "gen_ai.response.id";

    // =========================================================================
    // TOKEN USAGE (CRITICAL for cost calculation)
    // =========================================================================
    pub const GEN_AI_USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
    pub const GEN_AI_USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
    pub const GEN_AI_USAGE_TOTAL_TOKENS: &str = "gen_ai.usage.total_tokens";
    pub const GEN_AI_USAGE_REASONING_TOKENS: &str = "gen_ai.usage.reasoning_tokens";
    pub const GEN_AI_USAGE_CACHE_READ_TOKENS: &str = "gen_ai.usage.cache_read_tokens";
    /// Cached creation tokens (Anthropic)
    pub const GEN_AI_USAGE_CACHE_CREATION_TOKENS: &str = "gen_ai.usage.cache_creation_tokens";

    // =========================================================================
    // FINISH REASONS
    // =========================================================================
    pub const GEN_AI_RESPONSE_FINISH_REASONS: &str = "gen_ai.response.finish_reasons";

    // =========================================================================
    // REQUEST PARAMETERS / HYPERPARAMETERS (RECOMMENDED)
    // =========================================================================
    pub const GEN_AI_REQUEST_TEMPERATURE: &str = "gen_ai.request.temperature";
    pub const GEN_AI_REQUEST_TOP_P: &str = "gen_ai.request.top_p";
    pub const GEN_AI_REQUEST_TOP_K: &str = "gen_ai.request.top_k";
    pub const GEN_AI_REQUEST_MAX_TOKENS: &str = "gen_ai.request.max_tokens";
    pub const GEN_AI_REQUEST_FREQUENCY_PENALTY: &str = "gen_ai.request.frequency_penalty";
    pub const GEN_AI_REQUEST_PRESENCE_PENALTY: &str = "gen_ai.request.presence_penalty";
    pub const GEN_AI_REQUEST_STOP_SEQUENCES: &str = "gen_ai.request.stop_sequences";
    pub const GEN_AI_REQUEST_SEED: &str = "gen_ai.request.seed";
    pub const GEN_AI_REQUEST_CHOICE_COUNT: &str = "gen_ai.request.choice.count";

    // =========================================================================
    // SERVER INFORMATION (REQUIRED for distributed tracing)
    // =========================================================================
    pub const SERVER_ADDRESS: &str = "server.address";
    pub const SERVER_PORT: &str = "server.port";

    // =========================================================================
    // ERROR TRACKING (REQUIRED when errors occur)
    // =========================================================================
    pub const ERROR_TYPE: &str = "error.type";

    // =========================================================================
    // AGENT ATTRIBUTES (NEW - for agentic systems)
    // =========================================================================
    pub const GEN_AI_AGENT_ID: &str = "gen_ai.agent.id";
    pub const GEN_AI_AGENT_NAME: &str = "gen_ai.agent.name";
    pub const GEN_AI_AGENT_DESCRIPTION: &str = "gen_ai.agent.description";
    pub const GEN_AI_CONVERSATION_ID: &str = "gen_ai.conversation.id";

    // =========================================================================
    // TOOL CALL ATTRIBUTES (REQUIRED for tool-using agents)
    // =========================================================================
    pub const GEN_AI_TOOL_NAME: &str = "gen_ai.tool.name";
    pub const GEN_AI_TOOL_TYPE: &str = "gen_ai.tool.type"; // "function", "extension", "datastore"
    pub const GEN_AI_TOOL_DESCRIPTION: &str = "gen_ai.tool.description";
    pub const GEN_AI_TOOL_CALL_ID: &str = "gen_ai.tool.call.id";
    pub const GEN_AI_TOOL_CALL_ARGUMENTS: &str = "gen_ai.tool.call.arguments";
    pub const GEN_AI_TOOL_CALL_RESULT: &str = "gen_ai.tool.call.result";
    pub const GEN_AI_TOOL_DEFINITIONS: &str = "gen_ai.tool.definitions";

    // =========================================================================
    // CONTENT ATTRIBUTES
    // =========================================================================
    pub const GEN_AI_SYSTEM_INSTRUCTIONS: &str = "gen_ai.system_instructions";
    pub const GEN_AI_INPUT_MESSAGES: &str = "gen_ai.input.messages";
    pub const GEN_AI_OUTPUT_MESSAGES: &str = "gen_ai.output.messages";

    // =========================================================================
    // STRUCTURED PROMPTS/RESPONSES (indexed format)
    // =========================================================================
    pub const GEN_AI_PROMPT_PREFIX: &str = "gen_ai.prompt";
    pub const GEN_AI_COMPLETION_PREFIX: &str = "gen_ai.completion";

    // =========================================================================
    // LEGACY/NON-STANDARD ATTRIBUTES (backward compatibility)
    // =========================================================================
    pub const LEGACY_MODEL_NAME: &str = "model_name";
    pub const LEGACY_TOKENS: &str = "tokens";
    pub const LEGACY_TOKEN_COUNT: &str = "token_count";
}

/// GenAI-compliant payload structure
/// Matches OpenTelemetry GenAI semantic conventions v1.36+
/// Reference: https://opentelemetry.io/docs/specs/semconv/gen-ai/
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenAIPayload {
    // =========================================================================
    // PROVIDER IDENTIFICATION (REQUIRED)
    // =========================================================================
    /// Legacy system identifier (use provider_name for new implementations)
    #[serde(rename = "gen_ai.system", skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,

    /// Provider name: "openai", "anthropic", "aws.bedrock", "azure.ai.openai", etc.
    #[serde(
        rename = "gen_ai.provider.name",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider_name: Option<String>,

    /// Operation type: "chat", "embeddings", "text_completion", "image_generation"
    #[serde(
        rename = "gen_ai.operation.name",
        skip_serializing_if = "Option::is_none"
    )]
    pub operation_name: Option<String>,

    // =========================================================================
    // MODEL INFORMATION (REQUIRED)
    // =========================================================================
    #[serde(
        rename = "gen_ai.request.model",
        skip_serializing_if = "Option::is_none"
    )]
    pub request_model: Option<String>,

    #[serde(
        rename = "gen_ai.response.model",
        skip_serializing_if = "Option::is_none"
    )]
    pub response_model: Option<String>,

    #[serde(rename = "gen_ai.response.id", skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,

    // =========================================================================
    // TOKEN USAGE (CRITICAL for cost calculation)
    // =========================================================================
    #[serde(
        rename = "gen_ai.usage.input_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_tokens: Option<u32>,

    #[serde(
        rename = "gen_ai.usage.output_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_tokens: Option<u32>,

    #[serde(
        rename = "gen_ai.usage.total_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub total_tokens: Option<u32>,

    /// Reasoning tokens (OpenAI o1 models)
    #[serde(
        rename = "gen_ai.usage.reasoning_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub reasoning_tokens: Option<u32>,

    /// Cache read tokens (Anthropic prompt caching)
    #[serde(
        rename = "gen_ai.usage.cache_read_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub cache_read_tokens: Option<u32>,

    /// Cache creation tokens (Anthropic prompt caching)
    #[serde(
        rename = "gen_ai.usage.cache_creation_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub cache_creation_tokens: Option<u32>,

    // =========================================================================
    // FINISH REASONS
    // =========================================================================
    #[serde(
        rename = "gen_ai.response.finish_reasons",
        skip_serializing_if = "Option::is_none"
    )]
    pub finish_reasons: Option<Vec<String>>,

    // =========================================================================
    // REQUEST PARAMETERS / HYPERPARAMETERS (RECOMMENDED)
    // =========================================================================
    #[serde(
        rename = "gen_ai.request.temperature",
        skip_serializing_if = "Option::is_none"
    )]
    pub temperature: Option<f32>,

    #[serde(
        rename = "gen_ai.request.top_p",
        skip_serializing_if = "Option::is_none"
    )]
    pub top_p: Option<f32>,

    /// Top-k sampling (Anthropic/Google)
    #[serde(
        rename = "gen_ai.request.top_k",
        skip_serializing_if = "Option::is_none"
    )]
    pub top_k: Option<f32>,

    #[serde(
        rename = "gen_ai.request.max_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_tokens: Option<u32>,

    #[serde(
        rename = "gen_ai.request.frequency_penalty",
        skip_serializing_if = "Option::is_none"
    )]
    pub frequency_penalty: Option<f32>,

    #[serde(
        rename = "gen_ai.request.presence_penalty",
        skip_serializing_if = "Option::is_none"
    )]
    pub presence_penalty: Option<f32>,

    /// Stop sequences for generation
    #[serde(
        rename = "gen_ai.request.stop_sequences",
        skip_serializing_if = "Option::is_none"
    )]
    pub stop_sequences: Option<Vec<String>>,

    /// Seed for reproducibility
    #[serde(
        rename = "gen_ai.request.seed",
        skip_serializing_if = "Option::is_none"
    )]
    pub seed: Option<i64>,

    /// Number of choices to generate (n parameter)
    #[serde(
        rename = "gen_ai.request.choice.count",
        skip_serializing_if = "Option::is_none"
    )]
    pub choice_count: Option<u32>,

    // =========================================================================
    // SERVER INFORMATION (REQUIRED for distributed tracing)
    // =========================================================================
    #[serde(rename = "server.address", skip_serializing_if = "Option::is_none")]
    pub server_address: Option<String>,

    #[serde(rename = "server.port", skip_serializing_if = "Option::is_none")]
    pub server_port: Option<u16>,

    // =========================================================================
    // ERROR TRACKING (REQUIRED when errors occur)
    // =========================================================================
    /// Error type: "RateLimitError", "timeout", exception class name, etc.
    #[serde(rename = "error.type", skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,

    // =========================================================================
    // AGENT ATTRIBUTES (for agentic systems)
    // =========================================================================
    #[serde(rename = "gen_ai.agent.id", skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    #[serde(rename = "gen_ai.agent.name", skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,

    #[serde(
        rename = "gen_ai.agent.description",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_description: Option<String>,

    #[serde(
        rename = "gen_ai.conversation.id",
        skip_serializing_if = "Option::is_none"
    )]
    pub conversation_id: Option<String>,

    // =========================================================================
    // TOOL DEFINITIONS (JSON array of tool schemas)
    // =========================================================================
    #[serde(
        rename = "gen_ai.tool.definitions",
        skip_serializing_if = "Option::is_none"
    )]
    pub tool_definitions: Option<serde_json::Value>,

    // =========================================================================
    // ADDITIONAL ATTRIBUTES (catch-all for non-standard fields)
    // =========================================================================
    #[serde(flatten)]
    pub additional: HashMap<String, serde_json::Value>,
}

impl GenAIPayload {
    /// Extract GenAI attributes from span attributes
    pub fn from_attributes(attributes: &HashMap<String, String>) -> Self {
        let mut payload = Self {
            system: None,
            provider_name: None,
            operation_name: None,
            request_model: None,
            response_model: None,
            response_id: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            finish_reasons: None,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop_sequences: None,
            seed: None,
            choice_count: None,
            server_address: None,
            server_port: None,
            error_type: None,
            agent_id: None,
            agent_name: None,
            agent_description: None,
            conversation_id: None,
            tool_definitions: None,
            additional: HashMap::new(),
        };

        // =========================================================================
        // PROVIDER IDENTIFICATION
        // =========================================================================
        payload.system = attributes.get(attrs::GEN_AI_SYSTEM).cloned();
        payload.provider_name = attributes.get(attrs::GEN_AI_PROVIDER_NAME).cloned();
        payload.operation_name = attributes.get(attrs::GEN_AI_OPERATION_NAME).cloned();

        // =========================================================================
        // MODEL INFORMATION
        // =========================================================================
        payload.request_model = attributes
            .get(attrs::GEN_AI_REQUEST_MODEL)
            .or_else(|| attributes.get(attrs::LEGACY_MODEL_NAME))
            .cloned();
        payload.response_model = attributes.get(attrs::GEN_AI_RESPONSE_MODEL).cloned();
        payload.response_id = attributes.get(attrs::GEN_AI_RESPONSE_ID).cloned();

        // =========================================================================
        // TOKEN USAGE
        // =========================================================================
        payload.input_tokens = attributes
            .get(attrs::GEN_AI_USAGE_INPUT_TOKENS)
            .and_then(|s| s.parse().ok());
        payload.output_tokens = attributes
            .get(attrs::GEN_AI_USAGE_OUTPUT_TOKENS)
            .and_then(|s| s.parse().ok());
        payload.total_tokens = attributes
            .get(attrs::GEN_AI_USAGE_TOTAL_TOKENS)
            .or_else(|| attributes.get(attrs::LEGACY_TOKENS))
            .or_else(|| attributes.get(attrs::LEGACY_TOKEN_COUNT))
            .and_then(|s| s.parse().ok());
        payload.reasoning_tokens = attributes
            .get(attrs::GEN_AI_USAGE_REASONING_TOKENS)
            .and_then(|s| s.parse().ok());
        payload.cache_read_tokens = attributes
            .get(attrs::GEN_AI_USAGE_CACHE_READ_TOKENS)
            .and_then(|s| s.parse().ok());
        payload.cache_creation_tokens = attributes
            .get(attrs::GEN_AI_USAGE_CACHE_CREATION_TOKENS)
            .and_then(|s| s.parse().ok());

        // =========================================================================
        // FINISH REASONS
        // =========================================================================
        if let Some(finish_reasons_str) = attributes.get(attrs::GEN_AI_RESPONSE_FINISH_REASONS) {
            if finish_reasons_str.starts_with('[') {
                if let Ok(reasons) = serde_json::from_str::<Vec<String>>(finish_reasons_str) {
                    payload.finish_reasons = Some(reasons);
                }
            } else {
                payload.finish_reasons = Some(
                    finish_reasons_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect(),
                );
            }
        }

        // =========================================================================
        // REQUEST PARAMETERS / HYPERPARAMETERS
        // =========================================================================
        payload.temperature = attributes
            .get(attrs::GEN_AI_REQUEST_TEMPERATURE)
            .and_then(|s| s.parse().ok());
        payload.top_p = attributes
            .get(attrs::GEN_AI_REQUEST_TOP_P)
            .and_then(|s| s.parse().ok());
        payload.top_k = attributes
            .get(attrs::GEN_AI_REQUEST_TOP_K)
            .and_then(|s| s.parse().ok());
        payload.max_tokens = attributes
            .get(attrs::GEN_AI_REQUEST_MAX_TOKENS)
            .and_then(|s| s.parse().ok());
        payload.frequency_penalty = attributes
            .get(attrs::GEN_AI_REQUEST_FREQUENCY_PENALTY)
            .and_then(|s| s.parse().ok());
        payload.presence_penalty = attributes
            .get(attrs::GEN_AI_REQUEST_PRESENCE_PENALTY)
            .and_then(|s| s.parse().ok());
        payload.seed = attributes
            .get(attrs::GEN_AI_REQUEST_SEED)
            .and_then(|s| s.parse().ok());
        payload.choice_count = attributes
            .get(attrs::GEN_AI_REQUEST_CHOICE_COUNT)
            .and_then(|s| s.parse().ok());

        // Stop sequences (JSON array or comma-separated)
        if let Some(stop_str) = attributes.get(attrs::GEN_AI_REQUEST_STOP_SEQUENCES) {
            if stop_str.starts_with('[') {
                if let Ok(sequences) = serde_json::from_str::<Vec<String>>(stop_str) {
                    payload.stop_sequences = Some(sequences);
                }
            } else {
                payload.stop_sequences =
                    Some(stop_str.split(',').map(|s| s.trim().to_string()).collect());
            }
        }

        // =========================================================================
        // SERVER INFORMATION
        // =========================================================================
        payload.server_address = attributes.get(attrs::SERVER_ADDRESS).cloned();
        payload.server_port = attributes
            .get(attrs::SERVER_PORT)
            .and_then(|s| s.parse().ok());

        // =========================================================================
        // ERROR TRACKING
        // =========================================================================
        payload.error_type = attributes.get(attrs::ERROR_TYPE).cloned();

        // =========================================================================
        // AGENT ATTRIBUTES
        // =========================================================================
        payload.agent_id = attributes.get(attrs::GEN_AI_AGENT_ID).cloned();
        payload.agent_name = attributes.get(attrs::GEN_AI_AGENT_NAME).cloned();
        payload.agent_description = attributes.get(attrs::GEN_AI_AGENT_DESCRIPTION).cloned();
        payload.conversation_id = attributes.get(attrs::GEN_AI_CONVERSATION_ID).cloned();

        // =========================================================================
        // TOOL DEFINITIONS
        // =========================================================================
        if let Some(tools_str) = attributes.get(attrs::GEN_AI_TOOL_DEFINITIONS) {
            if let Ok(tools) = serde_json::from_str::<serde_json::Value>(tools_str) {
                payload.tool_definitions = Some(tools);
            }
        }

        // =========================================================================
        // ADDITIONAL ATTRIBUTES (store everything not explicitly extracted)
        // =========================================================================
        let extracted_keys: std::collections::HashSet<&str> = [
            attrs::GEN_AI_SYSTEM,
            attrs::GEN_AI_PROVIDER_NAME,
            attrs::GEN_AI_OPERATION_NAME,
            attrs::GEN_AI_REQUEST_MODEL,
            attrs::GEN_AI_RESPONSE_MODEL,
            attrs::GEN_AI_RESPONSE_ID,
            attrs::GEN_AI_USAGE_INPUT_TOKENS,
            attrs::GEN_AI_USAGE_OUTPUT_TOKENS,
            attrs::GEN_AI_USAGE_TOTAL_TOKENS,
            attrs::GEN_AI_USAGE_REASONING_TOKENS,
            attrs::GEN_AI_USAGE_CACHE_READ_TOKENS,
            attrs::GEN_AI_USAGE_CACHE_CREATION_TOKENS,
            attrs::GEN_AI_RESPONSE_FINISH_REASONS,
            attrs::GEN_AI_REQUEST_TEMPERATURE,
            attrs::GEN_AI_REQUEST_TOP_P,
            attrs::GEN_AI_REQUEST_TOP_K,
            attrs::GEN_AI_REQUEST_MAX_TOKENS,
            attrs::GEN_AI_REQUEST_FREQUENCY_PENALTY,
            attrs::GEN_AI_REQUEST_PRESENCE_PENALTY,
            attrs::GEN_AI_REQUEST_STOP_SEQUENCES,
            attrs::GEN_AI_REQUEST_SEED,
            attrs::GEN_AI_REQUEST_CHOICE_COUNT,
            attrs::SERVER_ADDRESS,
            attrs::SERVER_PORT,
            attrs::ERROR_TYPE,
            attrs::GEN_AI_AGENT_ID,
            attrs::GEN_AI_AGENT_NAME,
            attrs::GEN_AI_AGENT_DESCRIPTION,
            attrs::GEN_AI_CONVERSATION_ID,
            attrs::GEN_AI_TOOL_DEFINITIONS,
            attrs::LEGACY_MODEL_NAME,
            attrs::LEGACY_TOKENS,
            attrs::LEGACY_TOKEN_COUNT,
        ]
        .iter()
        .copied()
        .collect();

        for (key, value) in attributes {
            if extracted_keys.contains(key.as_str()) {
                continue;
            }

            // Store ALL other attributes including:
            // - gen_ai.tool.* (individual tool calls)
            // - gen_ai.prompt.* (prompts)
            // - gen_ai.completion.* (completions)
            // - otel.events (OTEL events)
            // - user.*, conversation.*, session.* (context)
            // - metadata.* (custom metadata)
            // - All other custom attributes

            // Try to parse as different types
            if let Ok(num) = value.parse::<i64>() {
                payload
                    .additional
                    .insert(key.clone(), serde_json::json!(num));
            } else if let Ok(float) = value.parse::<f64>() {
                payload
                    .additional
                    .insert(key.clone(), serde_json::json!(float));
            } else if value == "true" {
                payload
                    .additional
                    .insert(key.clone(), serde_json::json!(true));
            } else if value == "false" {
                payload
                    .additional
                    .insert(key.clone(), serde_json::json!(false));
            } else if value.starts_with('[') || value.starts_with('{') {
                // Try to parse as JSON
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
                    payload.additional.insert(key.clone(), json_val);
                } else {
                    payload
                        .additional
                        .insert(key.clone(), serde_json::json!(value));
                }
            } else {
                payload
                    .additional
                    .insert(key.clone(), serde_json::json!(value));
            }
        }

        payload
    }

    /// Calculate total tokens from input + output if not provided
    pub fn calculate_total_tokens(&mut self) {
        if self.total_tokens.is_none() {
            if let (Some(input), Some(output)) = (self.input_tokens, self.output_tokens) {
                self.total_tokens = Some(input + output);
            }
        }
    }

    /// Get effective token counts for cost calculation
    /// Returns (input_tokens, output_tokens, reasoning_tokens)
    pub fn get_token_counts(&self) -> (u32, u32, u32) {
        let input = self.input_tokens.unwrap_or(0);
        let output = self.output_tokens.unwrap_or(0);
        let reasoning = self.reasoning_tokens.unwrap_or(0);
        (input, output, reasoning)
    }

    /// Calculate accurate cost using actual token split
    pub fn calculate_cost(&self, model_pricing: &ModelPricing) -> f64 {
        let (input_tokens, output_tokens, reasoning_tokens) = self.get_token_counts();
        let cache_tokens = self.cache_read_tokens.unwrap_or(0);

        // Regular input tokens (excluding cached)
        let regular_input = input_tokens.saturating_sub(cache_tokens);

        let input_cost = (regular_input as f64 / 1_000_000.0) * model_pricing.input_price_per_1m;
        let cache_cost = (cache_tokens as f64 / 1_000_000.0) * model_pricing.cache_price_per_1m;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * model_pricing.output_price_per_1m;
        let reasoning_cost =
            (reasoning_tokens as f64 / 1_000_000.0) * model_pricing.reasoning_price_per_1m;

        input_cost + cache_cost + output_cost + reasoning_cost
    }
}

/// Model pricing information
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_price_per_1m: f64,
    pub output_price_per_1m: f64,
    pub cache_price_per_1m: f64, // Cache read tokens (90% discount for Anthropic)
    pub reasoning_price_per_1m: f64, // For OpenAI o1 models
}

impl ModelPricing {
    /// Get pricing for a model
    pub fn for_model(system: &str, model: &str) -> Self {
        match (system, model) {
            // OpenAI models
            ("openai", m) if m.contains("gpt-4o") => Self {
                input_price_per_1m: 2.50,
                output_price_per_1m: 10.0,
                cache_price_per_1m: 0.0,
                reasoning_price_per_1m: 0.0,
            },
            ("openai", m) if m.contains("gpt-4o-mini") => Self {
                input_price_per_1m: 0.15,
                output_price_per_1m: 0.60,
                cache_price_per_1m: 0.0,
                reasoning_price_per_1m: 0.0,
            },
            ("openai", m) if m.contains("gpt-4-turbo") => Self {
                input_price_per_1m: 10.0,
                output_price_per_1m: 30.0,
                cache_price_per_1m: 0.0,
                reasoning_price_per_1m: 0.0,
            },
            ("openai", m) if m.contains("o1-preview") => Self {
                input_price_per_1m: 15.0,
                output_price_per_1m: 60.0,
                cache_price_per_1m: 0.0,
                reasoning_price_per_1m: 15.0, // Reasoning tokens
            },
            ("openai", m) if m.contains("o1-mini") => Self {
                input_price_per_1m: 3.0,
                output_price_per_1m: 12.0,
                cache_price_per_1m: 0.0,
                reasoning_price_per_1m: 3.0,
            },

            // Anthropic models
            ("anthropic", m) if m.contains("claude-3-5-sonnet") => Self {
                input_price_per_1m: 3.0,
                output_price_per_1m: 15.0,
                cache_price_per_1m: 0.30, // 90% discount
                reasoning_price_per_1m: 0.0,
            },
            ("anthropic", m) if m.contains("claude-3-opus") => Self {
                input_price_per_1m: 15.0,
                output_price_per_1m: 75.0,
                cache_price_per_1m: 1.50,
                reasoning_price_per_1m: 0.0,
            },
            ("anthropic", m) if m.contains("claude-3-sonnet") => Self {
                input_price_per_1m: 3.0,
                output_price_per_1m: 15.0,
                cache_price_per_1m: 0.30,
                reasoning_price_per_1m: 0.0,
            },
            ("anthropic", m) if m.contains("claude-3-haiku") => Self {
                input_price_per_1m: 0.25,
                output_price_per_1m: 1.25,
                cache_price_per_1m: 0.03,
                reasoning_price_per_1m: 0.0,
            },

            // Default pricing
            _ => Self {
                input_price_per_1m: 10.0,
                output_price_per_1m: 30.0,
                cache_price_per_1m: 1.0,
                reasoning_price_per_1m: 0.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genai_payload_extraction() {
        let mut attributes = HashMap::new();
        attributes.insert("gen_ai.system".to_string(), "openai".to_string());
        attributes.insert(
            "gen_ai.request.model".to_string(),
            "gpt-4-turbo".to_string(),
        );
        attributes.insert("gen_ai.usage.input_tokens".to_string(), "1000".to_string());
        attributes.insert("gen_ai.usage.output_tokens".to_string(), "500".to_string());

        let payload = GenAIPayload::from_attributes(&attributes);

        assert_eq!(payload.system, Some("openai".to_string()));
        assert_eq!(payload.request_model, Some("gpt-4-turbo".to_string()));
        assert_eq!(payload.input_tokens, Some(1000));
        assert_eq!(payload.output_tokens, Some(500));
    }

    #[test]
    fn test_cost_calculation() {
        let payload = GenAIPayload {
            system: Some("openai".to_string()),
            request_model: Some("gpt-4-turbo".to_string()),
            input_tokens: Some(1000),
            output_tokens: Some(500),
            ..Default::default()
        };

        let pricing = ModelPricing::for_model("openai", "gpt-4-turbo");
        let cost = payload.calculate_cost(&pricing);

        // Expected: (1000/1M * $10) + (500/1M * $30) = $0.01 + $0.015 = $0.025
        assert!((cost - 0.025).abs() < 0.001);
    }

    #[test]
    fn test_cache_tokens_discount() {
        let payload = GenAIPayload {
            system: Some("anthropic".to_string()),
            request_model: Some("claude-3-5-sonnet".to_string()),
            input_tokens: Some(1000),
            output_tokens: Some(500),
            cache_read_tokens: Some(800), // 800 from cache
            ..Default::default()
        };

        let pricing = ModelPricing::for_model("anthropic", "claude-3-5-sonnet");
        let cost = payload.calculate_cost(&pricing);

        // Expected: (200/1M * $3) + (800/1M * $0.30) + (500/1M * $15)
        //         = $0.0006 + $0.00024 + $0.0075 = $0.00834
        assert!((cost - 0.00834).abs() < 0.001);
    }
}
