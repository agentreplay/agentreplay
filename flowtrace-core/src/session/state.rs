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

//! Session state persistence.

use super::continuity::SessionContinuity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use parking_lot::RwLock;

/// Persisted session state for recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSessionState {
    /// Content session ID.
    pub content_session_id: u128,
    /// Memory session ID.
    pub memory_session_id: Option<u128>,
    /// Project ID.
    pub project_id: u128,
    /// Prompt number.
    pub prompt_number: u32,
    /// Last observation ID.
    pub last_observation_id: Option<u128>,
    /// Created timestamp.
    pub created_at_us: u64,
    /// Last activity timestamp.
    pub last_activity_us: u64,
    /// Serialized conversation history.
    pub conversation_history: Option<Vec<u8>>,
}

impl From<&SessionContinuity> for PersistedSessionState {
    fn from(session: &SessionContinuity) -> Self {
        Self {
            content_session_id: session.content_session_id,
            memory_session_id: session.memory_session_id,
            project_id: session.project_id,
            prompt_number: session.prompt_number,
            last_observation_id: session.last_observation_id,
            created_at_us: session.created_at_us,
            last_activity_us: session.last_activity_us,
            conversation_history: None,
        }
    }
}

impl PersistedSessionState {
    /// Restore to SessionContinuity.
    pub fn to_continuity(&self) -> SessionContinuity {
        SessionContinuity {
            content_session_id: self.content_session_id,
            memory_session_id: self.memory_session_id,
            project_id: self.project_id,
            prompt_number: self.prompt_number,
            last_observation_id: self.last_observation_id,
            created_at_us: self.created_at_us,
            last_activity_us: self.last_activity_us,
            ended: false,
        }
    }

    /// Get storage key.
    pub fn storage_key(&self) -> String {
        format!("session_state/{:032x}", self.content_session_id)
    }
}

/// In-memory session state store (for testing/development).
///
/// Production implementations should use persistent storage (SochDB).
pub struct SessionStateStore {
    states: RwLock<HashMap<u128, PersistedSessionState>>,
}

impl Default for SessionStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStateStore {
    /// Create a new state store.
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// Save a session state.
    pub fn save(&self, state: PersistedSessionState) {
        self.states.write().insert(state.content_session_id, state);
    }

    /// Load a session state.
    pub fn load(&self, content_session_id: u128) -> Option<PersistedSessionState> {
        self.states.read().get(&content_session_id).cloned()
    }

    /// Delete a session state.
    pub fn delete(&self, content_session_id: u128) -> Option<PersistedSessionState> {
        self.states.write().remove(&content_session_id)
    }

    /// List all session states.
    pub fn list(&self) -> Vec<PersistedSessionState> {
        self.states.read().values().cloned().collect()
    }

    /// List session states for a project.
    pub fn list_for_project(&self, project_id: u128) -> Vec<PersistedSessionState> {
        self.states
            .read()
            .values()
            .filter(|s| s.project_id == project_id)
            .cloned()
            .collect()
    }

    /// Get count.
    pub fn count(&self) -> usize {
        self.states.read().len()
    }

    /// Clear all states.
    pub fn clear(&self) {
        self.states.write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persisted_state_from_continuity() {
        let continuity = SessionContinuity::new(1, 2);
        let persisted: PersistedSessionState = (&continuity).into();

        assert_eq!(persisted.content_session_id, 1);
        assert_eq!(persisted.project_id, 2);
    }

    #[test]
    fn test_state_store() {
        let store = SessionStateStore::new();

        let state = PersistedSessionState {
            content_session_id: 1,
            memory_session_id: Some(100),
            project_id: 2,
            prompt_number: 5,
            last_observation_id: None,
            created_at_us: 0,
            last_activity_us: 0,
            conversation_history: None,
        };

        store.save(state);
        assert_eq!(store.count(), 1);

        let loaded = store.load(1);
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().memory_session_id, Some(100));
    }

    #[test]
    fn test_list_for_project() {
        let store = SessionStateStore::new();

        for i in 0..5 {
            store.save(PersistedSessionState {
                content_session_id: i,
                memory_session_id: None,
                project_id: if i < 3 { 100 } else { 200 },
                prompt_number: 0,
                last_observation_id: None,
                created_at_us: 0,
                last_activity_us: 0,
                conversation_history: None,
            });
        }

        let project_100 = store.list_for_project(100);
        assert_eq!(project_100.len(), 3);

        let project_200 = store.list_for_project(200);
        assert_eq!(project_200.len(), 2);
    }
}
