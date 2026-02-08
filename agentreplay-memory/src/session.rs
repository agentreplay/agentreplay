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

//! Session memory and summarization
//!
//! Sessions represent individual coding/conversation sessions. At session end,
//! the memory system can automatically generate summaries that capture the
//! key outcomes, decisions, and learnings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique session identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    /// Generate a new unique ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Memory state for an active session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemory {
    /// Session ID
    pub session_id: SessionId,
    /// Workspace ID
    pub workspace_id: String,
    /// Session start time
    pub started_at: DateTime<Utc>,
    /// Session end time (if ended)
    pub ended_at: Option<DateTime<Utc>>,
    /// Number of messages in the session
    pub message_count: usize,
    /// Number of tool calls made
    pub tool_call_count: usize,
    /// Total tokens used
    pub total_tokens: u64,
    /// Observations collected during this session
    pub observation_ids: Vec<String>,
    /// Whether the session has been summarized
    pub is_summarized: bool,
    /// Associated trace ID (if tracing is enabled)
    pub trace_id: Option<String>,
}

impl SessionMemory {
    /// Create a new session memory
    pub fn new(workspace_id: impl Into<String>) -> Self {
        Self {
            session_id: SessionId::new(),
            workspace_id: workspace_id.into(),
            started_at: Utc::now(),
            ended_at: None,
            message_count: 0,
            tool_call_count: 0,
            total_tokens: 0,
            observation_ids: Vec::new(),
            is_summarized: false,
            trace_id: None,
        }
    }

    /// Mark the session as ended
    pub fn end(&mut self) {
        self.ended_at = Some(Utc::now());
    }

    /// Record a message
    pub fn record_message(&mut self, tokens: u64) {
        self.message_count += 1;
        self.total_tokens += tokens;
    }

    /// Record a tool call
    pub fn record_tool_call(&mut self) {
        self.tool_call_count += 1;
    }

    /// Add an observation
    pub fn add_observation(&mut self, observation_id: String) {
        self.observation_ids.push(observation_id);
    }

    /// Get session duration in seconds
    pub fn duration_secs(&self) -> Option<i64> {
        self.ended_at.map(|end| (end - self.started_at).num_seconds())
    }
}

/// Summary of a completed session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Session ID
    pub session_id: SessionId,
    /// Workspace ID
    pub workspace_id: String,
    /// When the session started
    pub started_at: DateTime<Utc>,
    /// When the session ended
    pub ended_at: DateTime<Utc>,
    /// High-level summary of what was accomplished
    pub summary: String,
    /// Key decisions made during the session
    pub decisions: Vec<String>,
    /// Main topics discussed
    pub topics: Vec<String>,
    /// Files that were created or modified
    pub files_touched: Vec<String>,
    /// Number of messages
    pub message_count: usize,
    /// Number of tool calls
    pub tool_call_count: usize,
    /// Total tokens used
    pub total_tokens: u64,
    /// Estimated cost in USD
    pub estimated_cost: Option<f64>,
    /// IDs of observations created during this session
    pub observation_ids: Vec<String>,
    /// Rolling summary this session contributes to
    pub rolling_summary_id: Option<String>,
}

impl SessionSummary {
    /// Create a new session summary from session memory
    pub fn from_memory(memory: &SessionMemory, summary: String) -> Self {
        Self {
            session_id: memory.session_id.clone(),
            workspace_id: memory.workspace_id.clone(),
            started_at: memory.started_at,
            ended_at: memory.ended_at.unwrap_or_else(Utc::now),
            summary,
            decisions: Vec::new(),
            topics: Vec::new(),
            files_touched: Vec::new(),
            message_count: memory.message_count,
            tool_call_count: memory.tool_call_count,
            total_tokens: memory.total_tokens,
            estimated_cost: None,
            observation_ids: memory.observation_ids.clone(),
            rolling_summary_id: None,
        }
    }

    /// Add decisions
    pub fn with_decisions(mut self, decisions: Vec<String>) -> Self {
        self.decisions = decisions;
        self
    }

    /// Add topics
    pub fn with_topics(mut self, topics: Vec<String>) -> Self {
        self.topics = topics;
        self
    }

    /// Add files touched
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files_touched = files;
        self
    }

    /// Set estimated cost
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.estimated_cost = Some(cost);
        self
    }
}

/// Request to summarize a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeRequest {
    /// Session ID to summarize
    pub session_id: SessionId,
    /// Include conversation transcript in context
    pub include_transcript: bool,
    /// Include tool call results in context
    pub include_tool_results: bool,
    /// Maximum tokens for the summary
    pub max_summary_tokens: usize,
}

impl SummarizeRequest {
    /// Create a default summarize request
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            include_transcript: true,
            include_tool_results: true,
            max_summary_tokens: 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_memory() {
        let mut memory = SessionMemory::new("my-workspace");
        
        memory.record_message(100);
        memory.record_message(150);
        memory.record_tool_call();
        memory.add_observation("obs-1".to_string());
        
        assert_eq!(memory.message_count, 2);
        assert_eq!(memory.total_tokens, 250);
        assert_eq!(memory.tool_call_count, 1);
        assert_eq!(memory.observation_ids.len(), 1);
        assert!(memory.ended_at.is_none());
        
        memory.end();
        assert!(memory.ended_at.is_some());
        assert!(memory.duration_secs().is_some());
    }

    #[test]
    fn test_session_summary() {
        let mut memory = SessionMemory::new("my-workspace");
        memory.record_message(1000);
        memory.end();
        
        let summary = SessionSummary::from_memory(&memory, "Implemented error handling".to_string())
            .with_decisions(vec!["Use Result type".to_string()])
            .with_topics(vec!["error-handling".to_string()])
            .with_files(vec!["src/error.rs".to_string()])
            .with_cost(0.05);
        
        assert_eq!(summary.summary, "Implemented error handling");
        assert_eq!(summary.decisions.len(), 1);
        assert_eq!(summary.estimated_cost, Some(0.05));
    }
}
