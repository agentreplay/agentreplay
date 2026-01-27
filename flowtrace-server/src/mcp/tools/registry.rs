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

//! MCP tool registry with JSON schema validation.

use async_trait::async_trait;
use dashmap::DashMap;
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

/// Tool execution context.
pub struct ToolContext {
    pub request_id: Option<Value>,
}

/// Tool execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Value,
}

/// Trait for MCP tools.
#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> &Value;

    async fn execute(&self, params: Value, context: &ToolContext) -> Result<ToolResult, ToolError>;
}

/// Registry for MCP tools.
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn McpTool>>,
    validators: DashMap<String, JSONSchema>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
            validators: DashMap::new(),
        }
    }

    pub fn register(&self, tool: Arc<dyn McpTool>) -> Result<(), RegistrationError> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(RegistrationError::DuplicateName(name));
        }

        let schema = tool.input_schema().clone();
        let validator = JSONSchema::options()
            .compile(&schema)
            .map_err(|e| RegistrationError::Schema(e.to_string()))?;
        self.validators.insert(name.clone(), validator);
        self.tools.insert(name, tool);
        Ok(())
    }

    pub fn list(&self) -> Vec<ToolListEntry> {
        self.tools
            .iter()
            .map(|entry| {
                let tool = entry.value();
                ToolListEntry {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    input_schema: tool.input_schema().clone(),
                }
            })
            .collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        params: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let tool = self.tools.get(name).ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        let validator = self
            .validators
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        if let Err(errors) = validator.validate(&params) {
            let message: String = errors
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ToolError::InvalidParams(message));
        }

        tool.execute(params, context).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListEntry {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Invalid tool params: {0}")]
    InvalidParams(String),
    #[error("Execution error: {0}")]
    Execution(String),
}

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("Duplicate tool name: {0}")]
    DuplicateName(String),
    #[error("Invalid schema: {0}")]
    Schema(String),
}
