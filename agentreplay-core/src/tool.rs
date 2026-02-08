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
