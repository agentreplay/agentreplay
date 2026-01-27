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

//! Tool metadata for agentic workflow tracking
//!
//! Stores tool call information separately from edges for efficient querying.

use serde::{Deserialize, Serialize};

/// Tool call metadata per OpenTelemetry GenAI conventions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub edge_id: u128,
    pub tool_name: String,
    pub call_id: String,
    pub arguments: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub tool_type: String, // "function", "retrieval", "code_interpreter"
    pub success: bool,
    pub latency_ms: u32,
}

impl ToolMetadata {
    pub fn new(edge_id: u128, tool_name: String, call_id: String) -> Self {
        Self {
            edge_id,
            tool_name,
            call_id,
            arguments: serde_json::Value::Null,
            result: None,
            tool_type: "function".to_string(),
            success: true,
            latency_ms: 0,
        }
    }
}

/// Agent metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub agent_id: u64,
    pub agent_name: String,
    pub agent_description: Option<String>,
}
