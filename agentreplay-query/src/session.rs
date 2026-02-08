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

//! Session replay module for reconstructing conversation timelines
//!
//! Provides functionality to reconstruct full conversation history from traces.

use serde::{Deserialize, Serialize};

/// Message in a conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub timestamp_us: u64,
    pub role: String,
    pub content: String,
    pub message_type: MessageType,
    pub edge_id: u128,
    pub metadata: MessageMetadata,
}

/// Type of session message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageType {
    UserPrompt,
    AssistantResponse,
    SystemPrompt,
    ToolCall {
        tool_name: String,
        arguments: String,
    },
    ToolResult {
        result: String,
    },
    Error {
        error_message: String,
    },
}

/// Message metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub token_count: Option<u32>,
    pub duration_ms: Option<u32>,
    pub model: Option<String>,
    pub finish_reason: Option<String>,
}

/// Timeline view of a session
#[derive(Debug, Clone, Serialize)]
pub struct SessionTimeline {
    pub session_id: u64,
    pub total_duration_ms: u32,
    pub message_count: usize,
    pub events: Vec<TimelineEvent>,
}

/// Event in a timeline
#[derive(Debug, Clone, Serialize)]
pub struct TimelineEvent {
    pub message: SessionMessage,
    pub relative_time_ms: u32,
    pub gap_before_ms: u32,
}
