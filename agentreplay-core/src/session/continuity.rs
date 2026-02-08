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

//! Session continuity implementation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use parking_lot::RwLock;

/// Configuration for session continuity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuityConfig {
    /// Session timeout in seconds (default: 30 minutes).
    #[serde(default = "default_session_timeout")]
    pub session_timeout_secs: u64,

    /// Whether to enable automatic session recovery.
    #[serde(default = "default_auto_recovery")]
    pub auto_recovery: bool,

    /// Maximum sessions to track in memory.
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
}

fn default_session_timeout() -> u64 {
    1800 // 30 minutes
}

fn default_auto_recovery() -> bool {
    true
}

fn default_max_sessions() -> usize {
    100
}

impl Default for ContinuityConfig {
    fn default() -> Self {
        Self {
            session_timeout_secs: default_session_timeout(),
            auto_recovery: default_auto_recovery(),
            max_sessions: default_max_sessions(),
        }
    }
}

impl ContinuityConfig {
    /// Get session timeout as Duration.
    pub fn session_timeout(&self) -> Duration {
        Duration::from_secs(self.session_timeout_secs)
    }
}

/// Session continuity state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContinuity {
    /// Content session ID (from primary agent).
    pub content_session_id: u128,
    /// Memory session ID (for LLM conversation).
    pub memory_session_id: Option<u128>,
    /// Current prompt number in the session.
    pub prompt_number: u32,
    /// Last observation ID generated.
    pub last_observation_id: Option<u128>,
    /// Last activity timestamp (microseconds since epoch).
    pub last_activity_us: u64,
    /// Session creation timestamp (microseconds since epoch).
    pub created_at_us: u64,
    /// Whether the session was explicitly ended.
    pub ended: bool,
    /// Project ID this session belongs to.
    pub project_id: u128,
}

impl SessionContinuity {
    /// Create a new session continuity state.
    pub fn new(content_session_id: u128, project_id: u128) -> Self {
        let now = current_timestamp_us();
        Self {
            content_session_id,
            memory_session_id: None,
            prompt_number: 0,
            last_observation_id: None,
            last_activity_us: now,
            created_at_us: now,
            ended: false,
            project_id,
        }
    }

    /// Check if this session should be resumed (multi-turn continuation).
    pub fn should_resume(&self) -> bool {
        self.memory_session_id.is_some() && self.prompt_number > 1 && !self.ended
    }

    /// Check if the session has timed out.
    pub fn is_timed_out(&self, timeout_secs: u64) -> bool {
        let now = current_timestamp_us();
        let elapsed_secs = (now - self.last_activity_us) / 1_000_000;
        elapsed_secs > timeout_secs
    }

    /// Update activity timestamp.
    pub fn touch(&mut self) {
        self.last_activity_us = current_timestamp_us();
    }

    /// Increment prompt number.
    pub fn next_prompt(&mut self) {
        self.prompt_number += 1;
        self.touch();
    }

    /// Set the memory session ID.
    pub fn set_memory_session(&mut self, memory_session_id: u128) {
        self.memory_session_id = Some(memory_session_id);
        self.touch();
    }

    /// Record an observation.
    pub fn record_observation(&mut self, observation_id: u128) {
        self.last_observation_id = Some(observation_id);
        self.touch();
    }

    /// Mark the session as ended.
    pub fn end(&mut self) {
        self.ended = true;
        self.touch();
    }

    /// Get session duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        (self.last_activity_us - self.created_at_us) / 1000
    }

    /// Get storage key for this session.
    pub fn storage_key(&self) -> String {
        format!("session_state/{:032x}", self.content_session_id)
    }
}

/// Manager for session continuity across multiple sessions.
pub struct ContinuityManager {
    sessions: RwLock<HashMap<u128, SessionContinuity>>,
    config: ContinuityConfig,
}

impl Default for ContinuityManager {
    fn default() -> Self {
        Self::new(ContinuityConfig::default())
    }
}

impl ContinuityManager {
    /// Create a new continuity manager.
    pub fn new(config: ContinuityConfig) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Get or create a session.
    pub fn get_or_create(&self, content_session_id: u128, project_id: u128) -> SessionContinuity {
        let mut sessions = self.sessions.write();

        if let Some(session) = sessions.get_mut(&content_session_id) {
            // Check for timeout
            if session.is_timed_out(self.config.session_timeout_secs) {
                // Session timed out, create new one
                let new_session = SessionContinuity::new(content_session_id, project_id);
                sessions.insert(content_session_id, new_session.clone());
                return new_session;
            }
            session.touch();
            return session.clone();
        }

        // Create new session
        let session = SessionContinuity::new(content_session_id, project_id);
        sessions.insert(content_session_id, session.clone());

        // Enforce max sessions limit
        self.enforce_max_sessions(&mut sessions);

        session
    }

    /// Get an existing session.
    pub fn get(&self, content_session_id: u128) -> Option<SessionContinuity> {
        self.sessions.read().get(&content_session_id).cloned()
    }

    /// Update a session.
    pub fn update(&self, session: SessionContinuity) {
        self.sessions
            .write()
            .insert(session.content_session_id, session);
    }

    /// End a session.
    pub fn end_session(&self, content_session_id: u128) -> Option<SessionContinuity> {
        let mut sessions = self.sessions.write();
        if let Some(session) = sessions.get_mut(&content_session_id) {
            session.end();
            return Some(session.clone());
        }
        None
    }

    /// Remove a session.
    pub fn remove(&self, content_session_id: u128) -> Option<SessionContinuity> {
        self.sessions.write().remove(&content_session_id)
    }

    /// Get all active sessions for a project.
    pub fn get_project_sessions(&self, project_id: u128) -> Vec<SessionContinuity> {
        self.sessions
            .read()
            .values()
            .filter(|s| s.project_id == project_id && !s.ended)
            .cloned()
            .collect()
    }

    /// Cleanup timed-out sessions.
    pub fn cleanup_timed_out(&self) -> Vec<u128> {
        let mut sessions = self.sessions.write();
        let timeout = self.config.session_timeout_secs;

        let timed_out: Vec<_> = sessions
            .iter()
            .filter(|(_, s)| s.is_timed_out(timeout))
            .map(|(id, _)| *id)
            .collect();

        for id in &timed_out {
            sessions.remove(id);
        }

        timed_out
    }

    /// Get session count.
    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    fn enforce_max_sessions(&self, sessions: &mut HashMap<u128, SessionContinuity>) {
        if sessions.len() <= self.config.max_sessions {
            return;
        }

        // Find oldest ended sessions to remove
        let mut candidates: Vec<_> = sessions
            .iter()
            .filter(|(_, s)| s.ended)
            .map(|(id, s)| (*id, s.last_activity_us))
            .collect();

        candidates.sort_by_key(|(_, ts)| *ts);

        let to_remove = sessions.len() - self.config.max_sessions;
        for (id, _) in candidates.iter().take(to_remove) {
            sessions.remove(id);
        }
    }
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
    fn test_session_continuity_new() {
        let session = SessionContinuity::new(1, 2);
        assert_eq!(session.content_session_id, 1);
        assert_eq!(session.project_id, 2);
        assert_eq!(session.prompt_number, 0);
        assert!(!session.ended);
    }

    #[test]
    fn test_should_resume() {
        let mut session = SessionContinuity::new(1, 1);
        assert!(!session.should_resume()); // No memory session

        session.set_memory_session(100);
        session.next_prompt();
        assert!(!session.should_resume()); // prompt_number is 1

        session.next_prompt();
        assert!(session.should_resume()); // prompt_number is 2

        session.end();
        assert!(!session.should_resume()); // ended
    }

    #[test]
    fn test_continuity_manager() {
        let manager = ContinuityManager::default();

        let session = manager.get_or_create(1, 1);
        assert_eq!(session.content_session_id, 1);

        // Get same session again
        let session2 = manager.get_or_create(1, 1);
        assert_eq!(session2.content_session_id, 1);

        assert_eq!(manager.session_count(), 1);
    }

    #[test]
    fn test_end_session() {
        let manager = ContinuityManager::default();
        manager.get_or_create(1, 1);

        let ended = manager.end_session(1);
        assert!(ended.is_some());
        assert!(ended.unwrap().ended);
    }

    #[test]
    fn test_project_sessions() {
        let manager = ContinuityManager::default();
        manager.get_or_create(1, 100);
        manager.get_or_create(2, 100);
        manager.get_or_create(3, 200);

        let project_sessions = manager.get_project_sessions(100);
        assert_eq!(project_sessions.len(), 2);
    }
}
