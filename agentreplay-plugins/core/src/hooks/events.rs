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

//! Agent lifecycle events and context data structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Agent lifecycle events that can be intercepted by hooks.
///
/// These events represent key points in an LLM agent's execution lifecycle
/// where memory capture and context injection can occur.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum AgentEvent {
    /// Agent session has started.
    /// Use this to initialize memory session and prepare context injection.
    SessionStart {
        session_id: u128,
        project_id: u128,
        #[serde(default)]
        metadata: HashMap<String, String>,
    },

    /// User has submitted a prompt to the agent.
    /// Use this to capture the original user request and start the memory agent.
    UserPromptSubmit {
        session_id: u128,
        prompt_number: u32,
        prompt_text: String,
        #[serde(default)]
        has_private_content: bool,
    },

    /// Agent is about to invoke a tool.
    /// Use this to capture tool inputs before execution.
    PreToolUse {
        session_id: u128,
        tool_name: String,
        tool_input: serde_json::Value,
        invocation_id: u128,
    },

    /// Agent has completed a tool invocation.
    /// Use this to observe tool outputs for compression into observations.
    PostToolUse {
        session_id: u128,
        tool_name: String,
        tool_input: serde_json::Value,
        tool_output: serde_json::Value,
        invocation_id: u128,
        duration_ms: u64,
        #[serde(default)]
        success: bool,
    },

    /// Agent has generated a response (partial or complete).
    AssistantResponse {
        session_id: u128,
        response_text: String,
        #[serde(default)]
        is_final: bool,
        token_count: Option<u32>,
    },

    /// Agent session is stopping.
    /// Use this to trigger session summarization.
    Stop {
        session_id: u128,
        #[serde(default)]
        reason: StopReason,
        final_message: Option<String>,
    },

    /// Session timeout occurred.
    SessionTimeout {
        session_id: u128,
        idle_duration_ms: u64,
    },

    /// Error occurred during agent execution.
    Error {
        session_id: u128,
        error_type: String,
        error_message: String,
        #[serde(default)]
        recoverable: bool,
    },

    /// Custom event for plugin-defined events.
    Custom {
        session_id: u128,
        event_name: String,
        payload: serde_json::Value,
    },
}

impl AgentEvent {
    /// Get the session ID associated with this event.
    pub fn session_id(&self) -> u128 {
        match self {
            AgentEvent::SessionStart { session_id, .. } => *session_id,
            AgentEvent::UserPromptSubmit { session_id, .. } => *session_id,
            AgentEvent::PreToolUse { session_id, .. } => *session_id,
            AgentEvent::PostToolUse { session_id, .. } => *session_id,
            AgentEvent::AssistantResponse { session_id, .. } => *session_id,
            AgentEvent::Stop { session_id, .. } => *session_id,
            AgentEvent::SessionTimeout { session_id, .. } => *session_id,
            AgentEvent::Error { session_id, .. } => *session_id,
            AgentEvent::Custom { session_id, .. } => *session_id,
        }
    }

    /// Get the event type name as a string.
    pub fn event_type(&self) -> &'static str {
        match self {
            AgentEvent::SessionStart { .. } => "SessionStart",
            AgentEvent::UserPromptSubmit { .. } => "UserPromptSubmit",
            AgentEvent::PreToolUse { .. } => "PreToolUse",
            AgentEvent::PostToolUse { .. } => "PostToolUse",
            AgentEvent::AssistantResponse { .. } => "AssistantResponse",
            AgentEvent::Stop { .. } => "Stop",
            AgentEvent::SessionTimeout { .. } => "SessionTimeout",
            AgentEvent::Error { .. } => "Error",
            AgentEvent::Custom { .. } => "Custom",
        }
    }

    /// Check if this is a session lifecycle event (start or stop).
    pub fn is_lifecycle_event(&self) -> bool {
        matches!(
            self,
            AgentEvent::SessionStart { .. }
                | AgentEvent::Stop { .. }
                | AgentEvent::SessionTimeout { .. }
        )
    }

    /// Check if this is a tool-related event.
    pub fn is_tool_event(&self) -> bool {
        matches!(
            self,
            AgentEvent::PreToolUse { .. } | AgentEvent::PostToolUse { .. }
        )
    }
}

/// Reason for session stop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// User explicitly ended the session.
    UserRequest,
    /// Agent completed its task.
    TaskComplete,
    /// Session timed out due to inactivity.
    Timeout,
    /// An error occurred.
    Error,
    /// Unknown or unspecified reason.
    #[default]
    Unknown,
}

/// Context provided to hook handlers during event processing.
#[derive(Debug, Clone)]
pub struct EventContext {
    /// The current project identifier.
    pub project_id: u128,
    /// The current session identifier.
    pub session_id: u128,
    /// Working directory for the project.
    pub working_directory: PathBuf,
    /// Additional metadata key-value pairs.
    pub metadata: HashMap<String, String>,
    /// Timestamp when the event was created (microseconds since epoch).
    pub timestamp_us: u64,
}

impl EventContext {
    /// Create a new event context.
    pub fn new(project_id: u128, session_id: u128, working_directory: PathBuf) -> Self {
        Self {
            project_id,
            session_id,
            working_directory,
            metadata: HashMap::new(),
            timestamp_us: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
        }
    }

    /// Add metadata to the context.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Data captured from tool usage events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsageData {
    /// Name of the tool that was invoked.
    pub tool_name: String,
    /// Input provided to the tool.
    pub input: serde_json::Value,
    /// Output from the tool (if PostToolUse).
    pub output: Option<serde_json::Value>,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Whether the tool execution was successful.
    pub success: bool,
    /// Files read during tool execution.
    pub files_read: Vec<PathBuf>,
    /// Files modified during tool execution.
    pub files_modified: Vec<PathBuf>,
}

impl ToolUsageData {
    /// Create tool usage data from a PostToolUse event.
    pub fn from_post_tool_use(
        tool_name: String,
        input: serde_json::Value,
        output: serde_json::Value,
        duration_ms: u64,
        success: bool,
    ) -> Self {
        // Extract files from input/output if available
        let files_read = Self::extract_file_paths(&input, &["path", "file", "filepath"]);
        let files_modified = Self::extract_file_paths(&output, &["path", "file", "filepath"]);

        Self {
            tool_name,
            input,
            output: Some(output),
            duration_ms,
            success,
            files_read,
            files_modified,
        }
    }

    /// Extract file paths from JSON value based on common field names.
    fn extract_file_paths(value: &serde_json::Value, field_names: &[&str]) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let serde_json::Value::Object(obj) = value {
            for field_name in field_names {
                if let Some(path_value) = obj.get(*field_name) {
                    if let Some(path_str) = path_value.as_str() {
                        paths.push(PathBuf::from(path_str));
                    }
                }
            }

            // Also check for arrays of files
            if let Some(serde_json::Value::Array(files)) = obj.get("files") {
                for file in files {
                    if let Some(path_str) = file.as_str() {
                        paths.push(PathBuf::from(path_str));
                    }
                }
            }
        }

        paths
    }
}

/// Session data for lifecycle events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Unique session identifier.
    pub session_id: u128,
    /// Project this session belongs to.
    pub project_id: u128,
    /// Working directory.
    pub working_directory: PathBuf,
    /// Session start time (microseconds since epoch).
    pub start_time_us: u64,
    /// Session end time if ended (microseconds since epoch).
    pub end_time_us: Option<u64>,
    /// Number of prompts in this session.
    pub prompt_count: u32,
    /// Number of tool invocations in this session.
    pub tool_invocation_count: u32,
    /// Agent type (e.g., "claude_code", "cursor", "custom").
    pub agent_type: String,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl SessionData {
    /// Create a new session data instance.
    pub fn new(session_id: u128, project_id: u128, working_directory: PathBuf) -> Self {
        Self {
            session_id,
            project_id,
            working_directory,
            start_time_us: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
            end_time_us: None,
            prompt_count: 0,
            tool_invocation_count: 0,
            agent_type: "unknown".to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Calculate session duration in milliseconds.
    pub fn duration_ms(&self) -> Option<u64> {
        self.end_time_us.map(|end| (end - self.start_time_us) / 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_session_id() {
        let event = AgentEvent::SessionStart {
            session_id: 123,
            project_id: 456,
            metadata: HashMap::new(),
        };
        assert_eq!(event.session_id(), 123);
    }

    #[test]
    fn test_event_type_name() {
        let event = AgentEvent::PostToolUse {
            session_id: 1,
            tool_name: "read_file".to_string(),
            tool_input: serde_json::json!({}),
            tool_output: serde_json::json!({}),
            invocation_id: 1,
            duration_ms: 100,
            success: true,
        };
        assert_eq!(event.event_type(), "PostToolUse");
    }

    #[test]
    fn test_is_lifecycle_event() {
        let start = AgentEvent::SessionStart {
            session_id: 1,
            project_id: 1,
            metadata: HashMap::new(),
        };
        let tool = AgentEvent::PostToolUse {
            session_id: 1,
            tool_name: "test".to_string(),
            tool_input: serde_json::json!({}),
            tool_output: serde_json::json!({}),
            invocation_id: 1,
            duration_ms: 0,
            success: true,
        };

        assert!(start.is_lifecycle_event());
        assert!(!tool.is_lifecycle_event());
    }

    #[test]
    fn test_tool_usage_data_extraction() {
        let input = serde_json::json!({
            "path": "/src/main.rs",
            "content": "fn main() {}"
        });
        let output = serde_json::json!({
            "success": true
        });

        let data = ToolUsageData::from_post_tool_use(
            "write_file".to_string(),
            input,
            output,
            50,
            true,
        );

        assert_eq!(data.files_read.len(), 1);
        assert_eq!(data.files_read[0], PathBuf::from("/src/main.rs"));
    }
}
