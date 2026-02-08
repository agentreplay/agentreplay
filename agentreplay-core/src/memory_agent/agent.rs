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

//! Memory agent implementation.

use super::config::MemoryAgentConfig;
use super::message::Message;
use super::session::{MemorySession, SessionState};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during memory agent operations.
#[derive(Debug, Error)]
pub enum MemoryAgentError {
    #[error("LLM client error: {0}")]
    LlmError(String),

    #[error("Session not found: {0}")]
    SessionNotFound(u128),

    #[error("Session already exists: {0}")]
    SessionAlreadyExists(u128),

    #[error("Session in invalid state: expected {expected:?}, got {actual:?}")]
    InvalidSessionState {
        expected: SessionState,
        actual: SessionState,
    },

    #[error("Request timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Parsing error: {0}")]
    ParseError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Status of the memory agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryAgentStatus {
    /// Agent is idle.
    Idle,
    /// Agent is processing a request.
    Processing,
    /// Agent encountered an error.
    Error,
    /// Agent is shutting down.
    ShuttingDown,
}

/// Trait for LLM clients used by the memory agent.
#[async_trait]
pub trait MemoryLLMClient: Send + Sync {
    /// Send a conversation and get a response.
    async fn chat(
        &self,
        messages: &[Message],
        max_tokens: u32,
        temperature: f32,
    ) -> Result<LLMChatResponse, MemoryAgentError>;

    /// Get the model name.
    fn model_name(&self) -> &str;
}

/// Response from LLM chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMChatResponse {
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub model: String,
}

/// Tool event data for observation generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEvent {
    /// Unique identifier for this tool invocation.
    pub invocation_id: u128,
    /// Name of the tool.
    pub tool_name: String,
    /// Input provided to the tool.
    pub input: serde_json::Value,
    /// Output from the tool.
    pub output: serde_json::Value,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Whether the tool execution was successful.
    pub success: bool,
    /// Files read during execution.
    pub files_read: Vec<String>,
    /// Files modified during execution.
    pub files_modified: Vec<String>,
    /// Timestamp (microseconds since epoch).
    pub timestamp_us: u64,
}

/// Memory agent for observation generation.
///
/// The memory agent observes tool usage from the primary agent and generates
/// structured observations using an LLM. It maintains conversation continuity
/// for multi-turn observation generation.
pub struct MemoryAgent<C: MemoryLLMClient> {
    /// LLM client for observation generation.
    client: Arc<C>,
    /// Configuration.
    config: MemoryAgentConfig,
    /// Active sessions.
    sessions: RwLock<HashMap<u128, MemorySession>>,
    /// Current status.
    status: RwLock<MemoryAgentStatus>,
}

impl<C: MemoryLLMClient> MemoryAgent<C> {
    /// Create a new memory agent.
    pub fn new(client: Arc<C>, config: MemoryAgentConfig) -> Self {
        Self {
            client,
            config,
            sessions: RwLock::new(HashMap::new()),
            status: RwLock::new(MemoryAgentStatus::Idle),
        }
    }

    /// Get the current status.
    pub fn status(&self) -> MemoryAgentStatus {
        *self.status.read()
    }

    /// Initialize a new session.
    pub fn init_session(
        &self,
        content_session_id: u128,
        project_id: u128,
    ) -> Result<u128, MemoryAgentError> {
        let mut sessions = self.sessions.write();

        if sessions.contains_key(&content_session_id) {
            return Err(MemoryAgentError::SessionAlreadyExists(content_session_id));
        }

        let session = MemorySession::new(
            content_session_id,
            project_id,
            self.config.token_budget,
            self.config.session_timeout_secs,
        );

        let memory_session_id = session.memory_session_id;
        sessions.insert(content_session_id, session);

        tracing::info!(
            content_session_id = %content_session_id,
            memory_session_id = %memory_session_id,
            project_id = %project_id,
            "Memory session initialized"
        );

        Ok(memory_session_id)
    }

    /// Start a session (initialize the conversation with system prompt).
    pub async fn start_session(
        &self,
        content_session_id: u128,
        system_prompt: &str,
    ) -> Result<(), MemoryAgentError> {
        let mut sessions = self.sessions.write();
        let session = sessions
            .get_mut(&content_session_id)
            .ok_or(MemoryAgentError::SessionNotFound(content_session_id))?;

        // Add system prompt to conversation
        session.conversation_history.add_system(system_prompt);
        session.activate();

        tracing::debug!(
            content_session_id = %content_session_id,
            "Memory session started with system prompt"
        );

        Ok(())
    }

    /// Process a tool event and generate an observation.
    pub async fn process_tool_event(
        &self,
        content_session_id: u128,
        event: &ToolEvent,
        observation_prompt: &str,
    ) -> Result<String, MemoryAgentError> {
        *self.status.write() = MemoryAgentStatus::Processing;

        let result = self
            .process_tool_event_internal(content_session_id, event, observation_prompt)
            .await;

        *self.status.write() = MemoryAgentStatus::Idle;
        result
    }

    async fn process_tool_event_internal(
        &self,
        content_session_id: u128,
        event: &ToolEvent,
        observation_prompt: &str,
    ) -> Result<String, MemoryAgentError> {
        // Get messages for the request
        let messages = {
            let mut sessions = self.sessions.write();
            let session = sessions
                .get_mut(&content_session_id)
                .ok_or(MemoryAgentError::SessionNotFound(content_session_id))?;

            if session.state != SessionState::Active {
                return Err(MemoryAgentError::InvalidSessionState {
                    expected: SessionState::Active,
                    actual: session.state,
                });
            }

            // Add user message with the observation prompt
            session.conversation_history.add_user(observation_prompt);
            session.record_tool_event();

            // Get messages for the API call
            session.conversation_history.messages().to_vec()
        };

        // Call the LLM
        let response = self
            .client
            .chat(&messages, self.config.max_tokens, self.config.temperature)
            .await?;

        // Update session with response
        {
            let mut sessions = self.sessions.write();
            if let Some(session) = sessions.get_mut(&content_session_id) {
                session
                    .conversation_history
                    .add_assistant(&response.content);
                session.record_tokens((response.input_tokens + response.output_tokens) as u64);

                // Check if we need to summarize history
                if session.conversation_history.exceeds_budget() {
                    session
                        .conversation_history
                        .summarize_if_needed(self.config.max_history_messages);
                }
            }
        }

        tracing::debug!(
            content_session_id = %content_session_id,
            tool_name = %event.tool_name,
            input_tokens = response.input_tokens,
            output_tokens = response.output_tokens,
            "Generated observation for tool event"
        );

        Ok(response.content)
    }

    /// Generate a session summary.
    pub async fn generate_summary(
        &self,
        content_session_id: u128,
        summary_prompt: &str,
    ) -> Result<String, MemoryAgentError> {
        *self.status.write() = MemoryAgentStatus::Processing;

        let result = self
            .generate_summary_internal(content_session_id, summary_prompt)
            .await;

        *self.status.write() = MemoryAgentStatus::Idle;
        result
    }

    async fn generate_summary_internal(
        &self,
        content_session_id: u128,
        summary_prompt: &str,
    ) -> Result<String, MemoryAgentError> {
        let messages = {
            let mut sessions = self.sessions.write();
            let session = sessions
                .get_mut(&content_session_id)
                .ok_or(MemoryAgentError::SessionNotFound(content_session_id))?;

            session.start_summarizing();
            session.conversation_history.add_user(summary_prompt);
            session.conversation_history.messages().to_vec()
        };

        let response = self
            .client
            .chat(&messages, self.config.max_tokens, self.config.temperature)
            .await?;

        // End the session
        {
            let mut sessions = self.sessions.write();
            if let Some(session) = sessions.get_mut(&content_session_id) {
                session
                    .conversation_history
                    .add_assistant(&response.content);
                session.record_tokens((response.input_tokens + response.output_tokens) as u64);
                session.end();
            }
        }

        tracing::info!(
            content_session_id = %content_session_id,
            "Generated session summary"
        );

        Ok(response.content)
    }

    /// Get session statistics.
    pub fn get_session_stats(
        &self,
        content_session_id: u128,
    ) -> Option<super::session::SessionStats> {
        self.sessions
            .read()
            .get(&content_session_id)
            .map(|s| s.stats())
    }

    /// End a session.
    pub fn end_session(&self, content_session_id: u128) -> Result<(), MemoryAgentError> {
        let mut sessions = self.sessions.write();
        let session = sessions
            .get_mut(&content_session_id)
            .ok_or(MemoryAgentError::SessionNotFound(content_session_id))?;

        session.end();

        tracing::info!(
            content_session_id = %content_session_id,
            observations = session.observation_count,
            tokens_used = session.total_tokens_used,
            "Memory session ended"
        );

        Ok(())
    }

    /// Remove a session.
    pub fn remove_session(&self, content_session_id: u128) -> Option<MemorySession> {
        self.sessions.write().remove(&content_session_id)
    }

    /// Get all active session IDs.
    pub fn active_sessions(&self) -> Vec<u128> {
        self.sessions
            .read()
            .iter()
            .filter(|(_, s)| s.state == SessionState::Active)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Clean up timed-out sessions.
    pub fn cleanup_timed_out_sessions(&self) -> Vec<u128> {
        let mut sessions = self.sessions.write();
        let timed_out: Vec<_> = sessions
            .iter()
            .filter(|(_, s)| s.is_timed_out())
            .map(|(id, _)| *id)
            .collect();

        for id in &timed_out {
            if let Some(mut session) = sessions.remove(id) {
                session.end();
                tracing::warn!(
                    content_session_id = %id,
                    "Memory session timed out"
                );
            }
        }

        timed_out
    }
}

/// Mock LLM client for testing.
#[cfg(test)]
pub struct MockLLMClient {
    pub responses: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl MockLLMClient {
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: std::sync::Mutex::new(responses),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl MemoryLLMClient for MockLLMClient {
    async fn chat(
        &self,
        _messages: &[Message],
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<LLMChatResponse, MemoryAgentError> {
        let content = self
            .responses
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| "Mock response".to_string());

        Ok(LLMChatResponse {
            content,
            input_tokens: 100,
            output_tokens: 50,
            model: "mock-model".to_string(),
        })
    }

    fn model_name(&self) -> &str {
        "mock-model"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_lifecycle() {
        let client = Arc::new(MockLLMClient::new(vec![
            "Summary".to_string(),
            "<observation>...</observation>".to_string(),
        ]));
        let config = MemoryAgentConfig::default();
        let agent = MemoryAgent::new(client, config);

        // Initialize session
        let memory_id = agent.init_session(1, 1).unwrap();
        assert!(memory_id > 0);

        // Start session
        agent
            .start_session(1, "You are a memory agent.")
            .await
            .unwrap();

        // Process tool event
        let event = ToolEvent {
            invocation_id: 1,
            tool_name: "read_file".to_string(),
            input: serde_json::json!({"path": "/test.rs"}),
            output: serde_json::json!({"content": "fn main() {}"}),
            duration_ms: 50,
            success: true,
            files_read: vec!["/test.rs".to_string()],
            files_modified: vec![],
            timestamp_us: 0,
        };

        let observation = agent
            .process_tool_event(1, &event, "Generate observation")
            .await
            .unwrap();
        assert!(!observation.is_empty());

        // Get stats
        let stats = agent.get_session_stats(1).unwrap();
        assert_eq!(stats.tool_event_count, 1);

        // Generate summary
        let summary = agent.generate_summary(1, "Generate summary").await.unwrap();
        assert!(!summary.is_empty());

        // Session should be ended
        let stats = agent.get_session_stats(1).unwrap();
        assert_eq!(stats.state, SessionState::Ended);
    }

    #[test]
    fn test_session_not_found() {
        let client = Arc::new(MockLLMClient::new(vec![]));
        let config = MemoryAgentConfig::default();
        let agent = MemoryAgent::new(client, config);

        assert!(matches!(
            agent.get_session_stats(999),
            None
        ));
    }
}
