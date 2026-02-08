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

//! Memory session management.

use super::message::ConversationHistory;
use serde::{Deserialize, Serialize};

/// State of a memory session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session is initializing.
    Initializing,
    /// Session is active and processing events.
    Active,
    /// Session is paused (idle timeout).
    Paused,
    /// Session is summarizing (Stop event received).
    Summarizing,
    /// Session has ended.
    Ended,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::Initializing
    }
}

/// A memory session for tracking agent interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySession {
    /// Unique session identifier (from primary agent).
    pub content_session_id: u128,
    /// Memory agent's session identifier (for LLM conversation continuity).
    pub memory_session_id: u128,
    /// Project this session belongs to.
    pub project_id: u128,
    /// Current prompt number in the session.
    pub prompt_number: u32,
    /// Last observation ID generated.
    pub last_observation_id: Option<u128>,
    /// Conversation history for the memory agent.
    #[serde(skip)]
    pub conversation_history: ConversationHistory,
    /// Current session state.
    pub state: SessionState,
    /// Session start time (microseconds since epoch).
    pub start_time_us: u64,
    /// Last activity time (microseconds since epoch).
    pub last_activity_us: u64,
    /// Session timeout configuration.
    pub timeout_secs: u64,
    /// Number of observations generated.
    pub observation_count: u32,
    /// Number of tool events processed.
    pub tool_event_count: u32,
    /// Total tokens used by memory agent.
    pub total_tokens_used: u64,
}

impl MemorySession {
    /// Create a new memory session.
    pub fn new(
        content_session_id: u128,
        project_id: u128,
        token_budget: usize,
        timeout_secs: u64,
    ) -> Self {
        let now = current_timestamp_us();
        let memory_session_id = generate_session_id();

        Self {
            content_session_id,
            memory_session_id,
            project_id,
            prompt_number: 0,
            last_observation_id: None,
            conversation_history: ConversationHistory::new(token_budget),
            state: SessionState::Initializing,
            start_time_us: now,
            last_activity_us: now,
            timeout_secs,
            observation_count: 0,
            tool_event_count: 0,
            total_tokens_used: 0,
        }
    }

    /// Check if the session should be resumed (multi-turn continuation).
    pub fn should_resume(&self) -> bool {
        self.prompt_number > 1 && self.state == SessionState::Active
    }

    /// Increment the prompt number.
    pub fn next_prompt(&mut self) {
        self.prompt_number += 1;
        self.touch();
    }

    /// Update the last activity time.
    pub fn touch(&mut self) {
        self.last_activity_us = current_timestamp_us();
    }

    /// Check if the session has timed out.
    pub fn is_timed_out(&self) -> bool {
        let now = current_timestamp_us();
        let elapsed_secs = (now - self.last_activity_us) / 1_000_000;
        elapsed_secs > self.timeout_secs
    }

    /// Get the session duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        let end = if self.state == SessionState::Ended {
            self.last_activity_us
        } else {
            current_timestamp_us()
        };
        (end - self.start_time_us) / 1000
    }

    /// Record an observation.
    pub fn record_observation(&mut self, observation_id: u128) {
        self.last_observation_id = Some(observation_id);
        self.observation_count += 1;
        self.touch();
    }

    /// Record a tool event.
    pub fn record_tool_event(&mut self) {
        self.tool_event_count += 1;
        self.touch();
    }

    /// Record token usage.
    pub fn record_tokens(&mut self, tokens: u64) {
        self.total_tokens_used += tokens;
    }

    /// Transition to active state.
    pub fn activate(&mut self) {
        self.state = SessionState::Active;
        self.touch();
    }

    /// Transition to summarizing state.
    pub fn start_summarizing(&mut self) {
        self.state = SessionState::Summarizing;
        self.touch();
    }

    /// End the session.
    pub fn end(&mut self) {
        self.state = SessionState::Ended;
        self.touch();
    }

    /// Pause the session (idle).
    pub fn pause(&mut self) {
        self.state = SessionState::Paused;
    }

    /// Resume from paused state.
    pub fn resume(&mut self) {
        if self.state == SessionState::Paused {
            self.state = SessionState::Active;
            self.touch();
        }
    }

    /// Get session statistics.
    pub fn stats(&self) -> SessionStats {
        SessionStats {
            content_session_id: self.content_session_id,
            memory_session_id: self.memory_session_id,
            project_id: self.project_id,
            prompt_count: self.prompt_number,
            observation_count: self.observation_count,
            tool_event_count: self.tool_event_count,
            total_tokens_used: self.total_tokens_used,
            duration_ms: self.duration_ms(),
            state: self.state,
        }
    }
}

/// Session statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub content_session_id: u128,
    pub memory_session_id: u128,
    pub project_id: u128,
    pub prompt_count: u32,
    pub observation_count: u32,
    pub tool_event_count: u32,
    pub total_tokens_used: u64,
    pub duration_ms: u64,
    pub state: SessionState,
}

/// Generate a unique session ID.
fn generate_session_id() -> u128 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);

    let hash = hasher.finish();
    let random_part = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    ((hash as u128) << 64) | (random_part as u128)
}

fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = MemorySession::new(1, 1, 8000, 1800);
        assert_eq!(session.content_session_id, 1);
        assert_eq!(session.state, SessionState::Initializing);
        assert_eq!(session.prompt_number, 0);
    }

    #[test]
    fn test_session_lifecycle() {
        let mut session = MemorySession::new(1, 1, 8000, 1800);

        session.activate();
        assert_eq!(session.state, SessionState::Active);

        session.next_prompt();
        assert_eq!(session.prompt_number, 1);

        session.record_observation(100);
        assert_eq!(session.observation_count, 1);
        assert_eq!(session.last_observation_id, Some(100));

        session.start_summarizing();
        assert_eq!(session.state, SessionState::Summarizing);

        session.end();
        assert_eq!(session.state, SessionState::Ended);
    }

    #[test]
    fn test_should_resume() {
        let mut session = MemorySession::new(1, 1, 8000, 1800);
        assert!(!session.should_resume());

        session.activate();
        session.next_prompt();
        assert!(!session.should_resume()); // prompt_number is 1

        session.next_prompt();
        assert!(session.should_resume()); // prompt_number is 2
    }

    #[test]
    fn test_session_stats() {
        let mut session = MemorySession::new(1, 2, 8000, 1800);
        session.activate();
        session.record_observation(100);
        session.record_tokens(500);

        let stats = session.stats();
        assert_eq!(stats.project_id, 2);
        assert_eq!(stats.observation_count, 1);
        assert_eq!(stats.total_tokens_used, 500);
    }
}
