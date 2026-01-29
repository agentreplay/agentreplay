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

//! Persistent memory agent storage backed by SochDB.

use agentreplay_core::memory_agent::MemorySession;
use moka::sync::Cache;
use sochdb::Connection;
use std::sync::Arc;
use thiserror::Error;

const SESSION_PREFIX: &str = "mem_session/";
const CONVERSATION_PREFIX: &str = "mem_conv/";
const PENDING_PREFIX: &str = "mem_pending/";

/// Statistics for session deletion operations
#[derive(Debug, Default)]
pub struct SessionDeleteStats {
    /// Number of session metadata records deleted
    pub sessions_deleted: u64,
    /// Number of conversation entries deleted
    pub conversations_deleted: u64,
    /// Number of pending messages deleted
    pub pending_deleted: u64,
    /// Total keys deleted
    pub total_keys_deleted: u64,
}

/// Errors for memory agent persistence.
#[derive(Debug, Error)]
pub enum MemoryAgentStoreError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Storage error: {0}")]
    Storage(String),
}

/// Persistent store for memory agent sessions.
pub struct PersistentMemoryStore {
    connection: Arc<Connection>,
    session_cache: Cache<u128, MemorySession>,
}

impl PersistentMemoryStore {
    /// Create a new persistent store with a bounded cache.
    pub fn new(connection: Arc<Connection>, cache_capacity: u64) -> Self {
        Self {
            connection,
            session_cache: Cache::new(cache_capacity),
        }
    }

    /// Persist a memory session to SochDB.
    pub fn persist_session(&self, session: &MemorySession) -> Result<(), MemoryAgentStoreError> {
        let key = format!("{}{:032x}", SESSION_PREFIX, session.content_session_id);
        let value = bincode::serialize(session)?;
        self.connection
            .put(key.as_bytes(), &value)
            .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;

        self.session_cache
            .insert(session.content_session_id, session.clone());
        Ok(())
    }

    /// Load a memory session by content session ID.
    pub fn load_session(&self, content_session_id: u128) -> Result<Option<MemorySession>, MemoryAgentStoreError> {
        if let Some(session) = self.session_cache.get(&content_session_id) {
            return Ok(Some(session));
        }

        let key = format!("{}{:032x}", SESSION_PREFIX, content_session_id);
        let data = self
            .connection
            .get(key.as_bytes())
            .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;

        if let Some(data) = data {
            let session: MemorySession = bincode::deserialize(&data)?;
            self.session_cache.insert(content_session_id, session.clone());
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    /// Delete a memory session with cascading cleanup of all related data.
    ///
    /// **Cascading Deletes:** This method removes:
    /// - Session metadata (`mem_session/{id}`)
    /// - All conversation entries (`mem_conv/{id}/*`)
    /// - All pending messages (`mem_pending/{id}/*`)
    ///
    /// This ensures no orphaned data remains after session deletion.
    pub fn delete_session(&self, content_session_id: u128) -> Result<SessionDeleteStats, MemoryAgentStoreError> {
        let mut stats = SessionDeleteStats::default();

        // 1. Delete session metadata
        let session_key = format!("{}{:032x}", SESSION_PREFIX, content_session_id);
        if self.connection.get(session_key.as_bytes())
            .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?
            .is_some()
        {
            self.connection
                .delete(session_key.as_bytes())
                .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;
            stats.sessions_deleted = 1;
            stats.total_keys_deleted += 1;
        }

        // 2. Delete all conversation entries (cascading)
        let conv_prefix = format!("{}{:032x}/", CONVERSATION_PREFIX, content_session_id);
        let conv_entries = self.connection
            .scan(conv_prefix.as_bytes())
            .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;
        
        for (key, _) in conv_entries {
            self.connection
                .delete(&key)  // key is already a String
                .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;
            stats.conversations_deleted += 1;
            stats.total_keys_deleted += 1;
        }

        // 3. Delete all pending messages (cascading)
        let pending_prefix = format!("{}{:032x}/", PENDING_PREFIX, content_session_id);
        let pending_entries = self.connection
            .scan(pending_prefix.as_bytes())
            .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;
        
        for (key, _) in pending_entries {
            self.connection
                .delete(&key)  // key is already a String
                .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;
            stats.pending_deleted += 1;
            stats.total_keys_deleted += 1;
        }

        // Invalidate cache
        self.session_cache.invalidate(&content_session_id);
        
        Ok(stats)
    }

    /// Delete a memory session (legacy API for backward compatibility).
    ///
    /// **Deprecated:** Use `delete_session` which returns deletion statistics.
    pub fn delete_session_simple(&self, content_session_id: u128) -> Result<(), MemoryAgentStoreError> {
        self.delete_session(content_session_id)?;
        Ok(())
    }

    /// Scan all persisted sessions.
    pub fn list_sessions(&self) -> Result<Vec<MemorySession>, MemoryAgentStoreError> {
        let prefix = SESSION_PREFIX.as_bytes();
        let results = self
            .connection
            .scan(prefix)
            .map_err(|e| MemoryAgentStoreError::Storage(e.to_string()))?;

        let mut sessions = Vec::with_capacity(results.len());
        for (_, value) in results {
            let session: MemorySession = bincode::deserialize(&value)?;
            sessions.push(session);
        }
        Ok(sessions)
    }

    /// Rebuild the cache from persisted sessions.
    pub fn rebuild_cache(&self) -> Result<(), MemoryAgentStoreError> {
        let sessions = self.list_sessions()?;
        for session in sessions {
            self.session_cache
                .insert(session.content_session_id, session);
        }
        Ok(())
    }

    /// Create a key for a conversation entry.
    pub fn conversation_key(content_session_id: u128, sequence: u64) -> String {
        format!("{}{:032x}/{:020}", CONVERSATION_PREFIX, content_session_id, sequence)
    }

    /// Create a key for pending message entries.
    pub fn pending_key(content_session_id: u128, message_id: u128) -> String {
        format!("{}{:032x}/{:032x}", PENDING_PREFIX, content_session_id, message_id)
    }
}
