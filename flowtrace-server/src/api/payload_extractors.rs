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

/// Helper functions to extract structured data from GenAI payloads
use crate::otel_genai::GenAIPayload;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hyperparameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBreakdown {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,
}

/// Extract prompts from GenAI payload
pub fn extract_prompts(payload: &GenAIPayload) -> Vec<PromptMessage> {
    let mut prompts = Vec::new();
    let mut i = 0;

    loop {
        let role_key = format!("gen_ai.prompt.{}.role", i);
        let content_key = format!("gen_ai.prompt.{}.content", i);

        if let (Some(role), Some(content)) = (
            payload.additional.get(&role_key),
            payload.additional.get(&content_key),
        ) {
            if let (Some(role_str), Some(content_str)) = (role.as_str(), content.as_str()) {
                prompts.push(PromptMessage {
                    role: role_str.to_string(),
                    content: content_str.to_string(),
                });
            }
            i += 1;
        } else {
            break;
        }
    }

    prompts
}

/// Extract completions from GenAI payload
pub fn extract_completions(payload: &GenAIPayload) -> Vec<CompletionMessage> {
    let mut completions = Vec::new();
    let mut i = 0;

    loop {
        let role_key = format!("gen_ai.completion.{}.role", i);
        let content_key = format!("gen_ai.completion.{}.content", i);
        let finish_reason_key = format!("gen_ai.completion.{}.finish_reason", i);

        if let (Some(role), Some(content)) = (
            payload.additional.get(&role_key),
            payload.additional.get(&content_key),
        ) {
            if let (Some(role_str), Some(content_str)) = (role.as_str(), content.as_str()) {
                completions.push(CompletionMessage {
                    role: role_str.to_string(),
                    content: content_str.to_string(),
                    finish_reason: payload
                        .additional
                        .get(&finish_reason_key)
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                });
            }
            i += 1;
        } else {
            break;
        }
    }

    completions
}

/// Extract tool calls from GenAI payload
pub fn extract_tool_calls(payload: &GenAIPayload) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();
    let mut i = 0;

    loop {
        let name_key = format!("gen_ai.tool.{}.name", i);
        let args_key = format!("gen_ai.tool.{}.arguments", i);
        let result_key = format!("gen_ai.tool.{}.result", i);

        if let Some(name) = payload.additional.get(&name_key).and_then(|v| v.as_str()) {
            let arguments = payload
                .additional
                .get(&args_key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            tool_calls.push(ToolCall {
                name: name.to_string(),
                arguments,
                result: payload
                    .additional
                    .get(&result_key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
            i += 1;
        } else {
            break;
        }
    }

    tool_calls
}

/// Extract hyperparameters from GenAI payload
pub fn extract_hyperparameters(payload: &GenAIPayload) -> Hyperparameters {
    Hyperparameters {
        temperature: payload.temperature,
        top_p: payload.top_p,
        max_tokens: payload.max_tokens,
        frequency_penalty: payload.frequency_penalty,
        presence_penalty: payload.presence_penalty,
    }
}

/// Extract token breakdown from GenAI payload
pub fn extract_token_breakdown(payload: &GenAIPayload) -> TokenBreakdown {
    TokenBreakdown {
        input_tokens: payload.input_tokens.unwrap_or(0),
        output_tokens: payload.output_tokens.unwrap_or(0),
        total_tokens: payload.total_tokens.unwrap_or(0),
        reasoning_tokens: payload.reasoning_tokens,
        cache_read_tokens: payload.cache_read_tokens,
    }
}

/// Get first 100 characters of first user prompt content
/// Checks multiple indices since index 0 might be system message
pub fn get_input_preview(payload: &GenAIPayload) -> Option<String> {
    // Try indices 0, 1, 2 to find first non-empty prompt (skipping system messages)
    for i in 0..3 {
        let key = format!("gen_ai.prompt.{}.content", i);
        if let Some(content) = payload.additional.get(&key).and_then(|v| v.as_str()) {
            // Check the role if available - prefer user messages
            let role_key = format!("gen_ai.prompt.{}.role", i);
            let role = payload.additional.get(&role_key).and_then(|v| v.as_str());

            // If role is system, skip and look for user
            if role == Some("system") {
                continue;
            }

            return Some(content.chars().take(100).collect());
        }
    }

    // Fallback to any prompt.0.content
    payload
        .additional
        .get("gen_ai.prompt.0.content")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(100).collect())
}

/// Get first 100 characters of first completion content
pub fn get_output_preview(payload: &GenAIPayload) -> Option<String> {
    payload
        .additional
        .get("gen_ai.completion.0.content")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(100).collect())
}
