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
