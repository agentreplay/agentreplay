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

//! MCP prompt registry with argument validation.

use async_trait::async_trait;
use dashmap::DashMap;
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use moka::sync::Cache;

/// Prompt argument definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}

/// MCP prompt message role.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptRole {
    User,
    Assistant,
}

/// Text content shape per MCP spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// MCP prompt content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PromptContent {
    Text(TextContent),
}

/// MCP prompt message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: PromptRole,
    pub content: PromptContent,
}

/// Context available to prompts.
#[derive(Clone)]
pub struct PromptContext {
    pub metadata: HashMap<String, String>,
}

/// Trait for MCP prompts.
#[async_trait]
pub trait McpPrompt: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn arguments(&self) -> &[PromptArgument];

    async fn get_messages(
        &self,
        arguments: HashMap<String, Value>,
        context: &PromptContext,
    ) -> Result<Vec<PromptMessage>, PromptError>;
}

/// Prompt registry.
pub struct PromptRegistry {
    prompts: DashMap<String, Arc<dyn McpPrompt>>,
    static_cache: Cache<(String, u64), Vec<PromptMessage>>,
}

impl PromptRegistry {
    pub fn new() -> Self {
        Self {
            prompts: DashMap::new(),
            static_cache: Cache::new(256),
        }
    }

    pub fn register(&self, prompt: Arc<dyn McpPrompt>) -> Result<(), PromptError> {
        let name = prompt.name().to_string();
        if self.prompts.contains_key(&name) {
            return Err(PromptError::DuplicateName(name));
        }
        self.prompts.insert(name, prompt);
        Ok(())
    }

    pub fn list(&self) -> Vec<PromptListEntry> {
        self.prompts
            .iter()
            .map(|entry| {
                let prompt = entry.value();
                PromptListEntry {
                    name: prompt.name().to_string(),
                    description: prompt.description().map(|d| d.to_string()),
                    arguments: prompt.arguments().to_vec(),
                }
            })
            .collect()
    }

    pub async fn get_messages(
        &self,
        name: &str,
        arguments: HashMap<String, Value>,
        context: &PromptContext,
    ) -> Result<Vec<PromptMessage>, PromptError> {
        let prompt = self
            .prompts
            .get(name)
            .ok_or_else(|| PromptError::NotFound(name.to_string()))?;

        for arg in prompt.arguments() {
            if arg.required && !arguments.contains_key(&arg.name) {
                return Err(PromptError::MissingArgument(arg.name.clone()));
            }
        }

        let cache_key = (name.to_string(), hash_arguments(&arguments));
        if let Some(cached) = self.static_cache.get(&cache_key) {
            return Ok(cached);
        }

        let messages = prompt.get_messages(arguments, context).await?;
        self.static_cache.insert(cache_key, messages.clone());
        Ok(messages)
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptListEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Error)]
pub enum PromptError {
    #[error("Prompt not found: {0}")]
    NotFound(String),
    #[error("Duplicate prompt name: {0}")]
    DuplicateName(String),
    #[error("Missing required argument: {0}")]
    MissingArgument(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Execution error: {0}")]
    Execution(String),
}

fn hash_arguments(arguments: &HashMap<String, Value>) -> u64 {
    let bytes = serde_json::to_vec(arguments).unwrap_or_default();
    let mut hasher = Hasher::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    u64::from_le_bytes(hash.as_bytes()[0..8].try_into().unwrap())
}
