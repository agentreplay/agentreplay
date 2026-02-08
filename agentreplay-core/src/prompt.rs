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

//! Structured prompt and completion storage for GenAI traces
//!
//! This module provides strongly-typed schemas for storing prompts, completions,
//! and model parameters separately from the main edge structure.

use serde::{Deserialize, Serialize};

/// Prompt role per OpenTelemetry GenAI conventions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptRole {
    System,
    User,
    Assistant,
    Tool,
}

impl PromptRole {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "system" => PromptRole::System,
            "user" => PromptRole::User,
            "assistant" => PromptRole::Assistant,
            "tool" => PromptRole::Tool,
            _ => PromptRole::User, // Default
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PromptRole::System => "system",
            PromptRole::User => "user",
            PromptRole::Assistant => "assistant",
            PromptRole::Tool => "tool",
        }
    }
}

/// Individual prompt message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub role: PromptRole,
    pub content: String,
    pub index: u32,
    pub token_count: Option<u32>,
}

/// Individual completion message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub role: PromptRole,
    pub content: String,
    pub finish_reason: Option<String>,
    pub token_count: Option<u32>,
}

/// Combined prompt/completion data for a span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCompletion {
    pub edge_id: u128,
    pub prompts: Vec<Prompt>,
    pub completions: Vec<Completion>,
    pub model_params: ModelParameters,
}

/// Model parameters for LLM requests
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelParameters {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<u32>,
    pub model: Option<String>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
}

impl PromptCompletion {
    pub fn new(edge_id: u128) -> Self {
        Self {
            edge_id,
            prompts: Vec::new(),
            completions: Vec::new(),
            model_params: ModelParameters::default(),
        }
    }

    /// Get total input tokens
    pub fn input_tokens(&self) -> u32 {
        self.prompts.iter().filter_map(|p| p.token_count).sum()
    }

    /// Get total output tokens
    pub fn output_tokens(&self) -> u32 {
        self.completions.iter().filter_map(|c| c.token_count).sum()
    }

    /// Get total tokens
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens() + self.output_tokens()
    }
}
