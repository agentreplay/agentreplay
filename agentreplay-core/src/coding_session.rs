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

//! Coding Session Schema for IDE/Coding Agent Traces
//!
//! This module captures interactions from coding assistants like Claude Code,
//! Cursor, VS Code Copilot, and other AI-powered coding tools.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      CodingSession                               │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │ Session Metadata: agent, directory, git_branch, etc.     │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │                              │                                   │
//! │              ┌───────────────┼───────────────┐                  │
//! │              ▼               ▼               ▼                  │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
//! │  │ Observation  │  │ Observation  │  │ Observation  │          │
//! │  │ (Read file)  │  │ (Edit file)  │  │ (Bash cmd)   │          │
//! │  └──────────────┘  └──────────────┘  └──────────────┘          │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Encoding
//!
//! - Sessions: `coding_sessions/{tenant_id}/{project_id}/{session_id:032x}`
//! - Observations: `coding_observations/{session_id:032x}/{timestamp:020}/{id:032x}`
//! - Summaries: `coding_summaries/{session_id:032x}`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Coding Agent Types
// ============================================================================

/// The type of coding agent/assistant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingAgent {
    /// Claude Code (Anthropic's terminal-based agent)
    ClaudeCode,
    /// Cursor AI IDE
    Cursor,
    /// VS Code GitHub Copilot
    Copilot,
    /// Continue.dev extension
    Continue,
    /// Windsurf AI
    Windsurf,
    /// Aider (command-line AI pair programmer)
    Aider,
    /// Cline (VS Code extension)
    Cline,
    /// Generic/unknown agent
    Other,
}

impl CodingAgent {
    /// Parse from string
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "claude-code" | "claude_code" | "claudecode" => CodingAgent::ClaudeCode,
            "cursor" => CodingAgent::Cursor,
            "copilot" | "github-copilot" | "github_copilot" => CodingAgent::Copilot,
            "continue" | "continue.dev" => CodingAgent::Continue,
            "windsurf" => CodingAgent::Windsurf,
            "aider" => CodingAgent::Aider,
            "cline" => CodingAgent::Cline,
            _ => CodingAgent::Other,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            CodingAgent::ClaudeCode => "claude-code",
            CodingAgent::Cursor => "cursor",
            CodingAgent::Copilot => "copilot",
            CodingAgent::Continue => "continue",
            CodingAgent::Windsurf => "windsurf",
            CodingAgent::Aider => "aider",
            CodingAgent::Cline => "cline",
            CodingAgent::Other => "other",
        }
    }
}

impl Default for CodingAgent {
    fn default() -> Self {
        CodingAgent::Other
    }
}

// ============================================================================
// Session Event Types
// ============================================================================

/// The type of session lifecycle event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventType {
    /// Session initialization
    Init,
    /// Session resumed after idle
    Resume,
    /// Session paused/idle
    Pause,
    /// Session ended
    End,
    /// Session summarization triggered
    Summarize,
}

// ============================================================================
// Tool Action Types
// ============================================================================

/// The type of tool action observed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAction {
    /// Read a file
    Read,
    /// Edit/write a file
    Edit,
    /// Create a new file
    Create,
    /// Delete a file
    Delete,
    /// Run a bash/shell command
    Bash,
    /// Search codebase (grep, semantic search)
    Search,
    /// List directory contents
    ListDir,
    /// Git operation
    Git,
    /// Web fetch/browse
    WebFetch,
    /// Agent thinking/reasoning (internal)
    Think,
    /// MCP tool call
    McpTool,
    /// User message/input
    UserMessage,
    /// Agent response/output
    AgentResponse,
    /// Task completion
    TaskComplete,
    /// Unknown/other action
    Other,
}

impl ToolAction {
    /// Parse from string (common patterns from various agents)
    pub fn parse(s: &str) -> Self {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "read" | "read_file" | "readfile" | "view" => ToolAction::Read,
            "edit" | "edit_file" | "editfile" | "write" | "str_replace_editor" | "str_replace" => {
                ToolAction::Edit
            }
            "create" | "create_file" | "createfile" | "new" => ToolAction::Create,
            "delete" | "delete_file" | "deletefile" | "remove" => ToolAction::Delete,
            "bash" | "shell" | "terminal" | "execute" | "run_terminal_command" => ToolAction::Bash,
            "search" | "grep" | "find" | "codebase_search" | "grep_search" | "semantic_search" => {
                ToolAction::Search
            }
            "ls" | "list" | "list_dir" | "listdir" | "directory" => ToolAction::ListDir,
            "git" | "git_diff" | "git_status" | "git_commit" => ToolAction::Git,
            "web" | "fetch" | "browse" | "web_fetch" | "http" => ToolAction::WebFetch,
            "think" | "thinking" | "reason" | "reasoning" => ToolAction::Think,
            "mcp" | "mcp_tool" | "mcptool" => ToolAction::McpTool,
            "user" | "user_message" | "human" | "input" => ToolAction::UserMessage,
            "response" | "agent_response" | "assistant" | "output" => ToolAction::AgentResponse,
            "complete" | "task_complete" | "done" | "finish" => ToolAction::TaskComplete,
            _ => ToolAction::Other,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolAction::Read => "read",
            ToolAction::Edit => "edit",
            ToolAction::Create => "create",
            ToolAction::Delete => "delete",
            ToolAction::Bash => "bash",
            ToolAction::Search => "search",
            ToolAction::ListDir => "list_dir",
            ToolAction::Git => "git",
            ToolAction::WebFetch => "web_fetch",
            ToolAction::Think => "think",
            ToolAction::McpTool => "mcp_tool",
            ToolAction::UserMessage => "user_message",
            ToolAction::AgentResponse => "agent_response",
            ToolAction::TaskComplete => "task_complete",
            ToolAction::Other => "other",
        }
    }

    /// Check if this is a file operation
    pub fn is_file_operation(&self) -> bool {
        matches!(
            self,
            ToolAction::Read | ToolAction::Edit | ToolAction::Create | ToolAction::Delete
        )
    }

    /// Check if this is a terminal operation
    pub fn is_terminal_operation(&self) -> bool {
        matches!(self, ToolAction::Bash | ToolAction::Git)
    }
}

impl Default for ToolAction {
    fn default() -> Self {
        ToolAction::Other
    }
}

// ============================================================================
// Coding Session
// ============================================================================

/// A coding session represents a continuous period of interaction with a coding agent.
///
/// Sessions are initiated when a user starts working with an AI coding assistant
/// and end when the session is explicitly closed or times out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingSession {
    /// Unique session identifier (UUID as u128)
    pub session_id: u128,

    /// Tenant ID for multi-tenancy
    pub tenant_id: u64,

    /// Project ID
    pub project_id: u16,

    /// The coding agent type
    pub agent: CodingAgent,

    /// Raw agent name string (for custom agents)
    pub agent_name: String,

    /// Working directory for this session
    pub working_directory: String,

    /// Git repository URL (if detected)
    pub git_repo: Option<String>,

    /// Git branch name
    pub git_branch: Option<String>,

    /// Session start timestamp (microseconds since epoch)
    pub start_time_us: u64,

    /// Session end timestamp (microseconds since epoch)
    pub end_time_us: Option<u64>,

    /// Current session state
    pub state: SessionState,

    /// Total token count for the session
    pub total_tokens: u64,

    /// Total cost for the session (in USD cents)
    pub total_cost_cents: u32,

    /// Number of observations in this session
    pub observation_count: u32,

    /// Number of file reads
    pub file_reads: u32,

    /// Number of file edits
    pub file_edits: u32,

    /// Number of bash commands
    pub bash_commands: u32,

    /// Session-level metadata
    pub metadata: HashMap<String, String>,

    /// Session summary (generated on summarize)
    pub summary: Option<SessionSummary>,
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    #[default]
    Active,
    Paused,
    Completed,
    Summarized,
}

impl CodingSession {
    /// Create a new coding session
    pub fn new(
        session_id: u128,
        tenant_id: u64,
        project_id: u16,
        agent: CodingAgent,
        agent_name: impl Into<String>,
        working_directory: impl Into<String>,
    ) -> Self {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        CodingSession {
            session_id,
            tenant_id,
            project_id,
            agent,
            agent_name: agent_name.into(),
            working_directory: working_directory.into(),
            git_repo: None,
            git_branch: None,
            start_time_us: now_us,
            end_time_us: None,
            state: SessionState::Active,
            total_tokens: 0,
            total_cost_cents: 0,
            observation_count: 0,
            file_reads: 0,
            file_edits: 0,
            bash_commands: 0,
            metadata: HashMap::new(),
            summary: None,
        }
    }

    /// Get session duration in seconds
    pub fn duration_seconds(&self) -> f64 {
        let end = self
            .end_time_us
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_micros() as u64)
                    .unwrap_or(0)
            });
        (end - self.start_time_us) as f64 / 1_000_000.0
    }

    /// Add an observation to this session
    pub fn add_observation(&mut self, obs: &CodingObservation) {
        self.observation_count += 1;
        self.total_tokens += obs.tokens_used as u64;
        self.total_cost_cents += obs.cost_cents;

        match obs.action {
            ToolAction::Read => self.file_reads += 1,
            ToolAction::Edit | ToolAction::Create => self.file_edits += 1,
            ToolAction::Bash => self.bash_commands += 1,
            _ => {}
        }
    }
}

// ============================================================================
// Coding Observation
// ============================================================================

/// A single observation from a coding session.
///
/// Observations are tool uses captured from the coding agent's actions,
/// such as file reads, edits, bash commands, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingObservation {
    /// Unique observation identifier
    pub observation_id: u128,

    /// Parent session ID
    pub session_id: u128,

    /// Observation timestamp (microseconds since epoch)
    pub timestamp_us: u64,

    /// Sequence number within the session (for ordering)
    pub sequence: u32,

    /// The tool action type
    pub action: ToolAction,

    /// Raw tool name (for custom tools)
    pub tool_name: String,

    /// Affected file path (if applicable)
    pub file_path: Option<String>,

    /// Directory path (for list_dir, bash with cwd)
    pub directory: Option<String>,

    /// Command executed (for bash)
    pub command: Option<String>,

    /// Exit code (for bash)
    pub exit_code: Option<i32>,

    /// Search query (for search operations)
    pub search_query: Option<String>,

    /// Input content/arguments (truncated)
    pub input_content: Option<String>,

    /// Output/result content (truncated)
    pub output_content: Option<String>,

    /// Duration in milliseconds
    pub duration_ms: u32,

    /// Tokens used for this operation
    pub tokens_used: u32,

    /// Cost in USD cents (hundredths of a cent)
    pub cost_cents: u32,

    /// Success/failure status
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,

    /// Line range for file operations (start_line, end_line)
    pub line_range: Option<(u32, u32)>,

    /// Number of lines changed (for edits)
    pub lines_changed: Option<u32>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl CodingObservation {
    /// Create a new observation
    pub fn new(
        observation_id: u128,
        session_id: u128,
        sequence: u32,
        action: ToolAction,
        tool_name: impl Into<String>,
    ) -> Self {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        CodingObservation {
            observation_id,
            session_id,
            timestamp_us: now_us,
            sequence,
            action,
            tool_name: tool_name.into(),
            file_path: None,
            directory: None,
            command: None,
            exit_code: None,
            search_query: None,
            input_content: None,
            output_content: None,
            duration_ms: 0,
            tokens_used: 0,
            cost_cents: 0,
            success: true,
            error: None,
            line_range: None,
            lines_changed: None,
            metadata: HashMap::new(),
        }
    }

    /// Set file information
    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Set bash command
    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Set duration
    pub fn with_duration(mut self, ms: u32) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Set result status
    pub fn with_result(mut self, success: bool, error: Option<String>) -> Self {
        self.success = success;
        self.error = error;
        self
    }
}

// ============================================================================
// Session Summary
// ============================================================================

/// Summary of a coding session, generated after session ends or on-demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Summary title
    pub title: String,

    /// Brief description of what was accomplished
    pub description: String,

    /// List of key accomplishments
    pub accomplishments: Vec<String>,

    /// Files that were modified
    pub files_modified: Vec<String>,

    /// Files that were read (for context)
    pub files_read: Vec<String>,

    /// Technologies/concepts encountered
    pub concepts: Vec<String>,

    /// Key decisions made
    pub decisions: Vec<String>,

    /// Open questions or follow-ups
    pub follow_ups: Vec<String>,

    /// Summary generated timestamp
    pub generated_at_us: u64,
}

impl Default for SessionSummary {
    fn default() -> Self {
        SessionSummary {
            title: String::new(),
            description: String::new(),
            accomplishments: Vec::new(),
            files_modified: Vec::new(),
            files_read: Vec::new(),
            concepts: Vec::new(),
            decisions: Vec::new(),
            follow_ups: Vec::new(),
            generated_at_us: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
        }
    }
}

// ============================================================================
// Key Encoding Functions
// ============================================================================

/// Key prefix for coding sessions
pub const CODING_SESSION_PREFIX: &str = "coding_sessions";
/// Key prefix for coding observations
pub const CODING_OBSERVATION_PREFIX: &str = "coding_observations";
/// Key prefix for coding summaries
pub const CODING_SUMMARY_PREFIX: &str = "coding_summaries";

/// Encode a coding session key
pub fn encode_session_key(tenant_id: u64, project_id: u16, session_id: u128) -> String {
    format!(
        "{}/{}/{}/{:032x}",
        CODING_SESSION_PREFIX, tenant_id, project_id, session_id
    )
}

/// Encode a coding observation key
pub fn encode_observation_key(session_id: u128, timestamp_us: u64, observation_id: u128) -> String {
    format!(
        "{}/{:032x}/{:020}/{:032x}",
        CODING_OBSERVATION_PREFIX, session_id, timestamp_us, observation_id
    )
}

/// Encode a session summary key
pub fn encode_summary_key(session_id: u128) -> String {
    format!("{}/{:032x}", CODING_SUMMARY_PREFIX, session_id)
}

/// Generate a unique session ID
pub fn generate_session_id() -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.as_nanos().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    
    // Use the hash as the lower 64 bits and timestamp nanos as upper 64 bits
    let lower = hasher.finish() as u128;
    let upper = (now.as_nanos() as u128) << 64;
    upper | lower
}

/// Generate a unique observation ID
pub fn generate_observation_id() -> u128 {
    generate_session_id() // Same algorithm works
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coding_agent_parse() {
        assert_eq!(CodingAgent::parse("claude-code"), CodingAgent::ClaudeCode);
        assert_eq!(CodingAgent::parse("Cursor"), CodingAgent::Cursor);
        assert_eq!(CodingAgent::parse("copilot"), CodingAgent::Copilot);
        assert_eq!(CodingAgent::parse("unknown"), CodingAgent::Other);
    }

    #[test]
    fn test_tool_action_parse() {
        assert_eq!(ToolAction::parse("read_file"), ToolAction::Read);
        assert_eq!(ToolAction::parse("str_replace_editor"), ToolAction::Edit);
        assert_eq!(ToolAction::parse("bash"), ToolAction::Bash);
        assert_eq!(ToolAction::parse("grep_search"), ToolAction::Search);
    }

    #[test]
    fn test_session_key_encoding() {
        let key = encode_session_key(1, 42, 0x123456789abcdef0);
        assert!(key.starts_with("coding_sessions/"));
        assert!(key.contains("/1/42/"));
    }

    #[test]
    fn test_observation_key_encoding() {
        let key = encode_observation_key(
            0x123456789abcdef0,
            1704067200000000, // 2024-01-01
            0xfedcba987654321,
        );
        assert!(key.starts_with("coding_observations/"));
    }

    #[test]
    fn test_session_duration() {
        let mut session = CodingSession::new(
            1,
            1,
            1,
            CodingAgent::ClaudeCode,
            "claude-code",
            "/home/user/project",
        );
        
        // Set fixed times for testing
        session.start_time_us = 1704067200000000; // 2024-01-01 00:00:00
        session.end_time_us = Some(1704067260000000); // 60 seconds later
        
        assert_eq!(session.duration_seconds(), 60.0);
    }

    #[test]
    fn test_observation_builder() {
        let obs = CodingObservation::new(
            1,
            1,
            0,
            ToolAction::Edit,
            "str_replace_editor",
        )
        .with_file("src/main.rs")
        .with_duration(150)
        .with_result(true, None);

        assert_eq!(obs.file_path, Some("src/main.rs".to_string()));
        assert_eq!(obs.duration_ms, 150);
        assert!(obs.success);
    }
}
