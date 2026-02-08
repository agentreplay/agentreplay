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

//! Observation Storage
//!
//! Storage backend for observations with efficient querying.
//!
//! # Key Encoding
//!
//! ```text
//! obs/{project_id}/{session_id:032x}/{timestamp:020}/{id:032x}
//! ```
//!
//! This encoding enables:
//! - Efficient project-scoped queries
//! - Chronological ordering
//! - Session grouping

use serde::{Deserialize, Serialize};
use parking_lot::RwLock;
use std::collections::BTreeMap;

/// Observation storage key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservationKey {
    /// Project ID.
    pub project_id: u128,
    /// Session ID.
    pub session_id: u128,
    /// Timestamp (HLC microseconds).
    pub timestamp: u64,
    /// Observation ID.
    pub observation_id: u128,
}

impl ObservationKey {
    /// Create a new observation key.
    pub fn new(
        project_id: u128,
        session_id: u128,
        timestamp: u64,
        observation_id: u128,
    ) -> Self {
        Self {
            project_id,
            session_id,
            timestamp,
            observation_id,
        }
    }

    /// Encode to storage key bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut key = Vec::with_capacity(64);
        key.extend_from_slice(b"obs/");
        key.extend_from_slice(&self.project_id.to_be_bytes());
        key.push(b'/');
        key.extend_from_slice(&self.session_id.to_be_bytes());
        key.push(b'/');
        key.extend_from_slice(&self.timestamp.to_be_bytes());
        key.push(b'/');
        key.extend_from_slice(&self.observation_id.to_be_bytes());
        key
    }

    /// Encode to human-readable string.
    pub fn to_string_key(&self) -> String {
        format!(
            "obs/{:032x}/{:032x}/{:020}/{:032x}",
            self.project_id, self.session_id, self.timestamp, self.observation_id
        )
    }

    /// Decode from storage key bytes.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        // Total: 4 ("obs/") + 16 (project) + 1 ("/") + 16 (session) + 1 ("/") + 8 (ts) + 1 ("/") + 16 (id) = 63
        if bytes.len() < 63 || !bytes.starts_with(b"obs/") {
            return None;
        }

        let mut pos = 4; // Skip "obs/"

        // Parse project_id (16 bytes)
        let project_id = u128::from_be_bytes(bytes[pos..pos + 16].try_into().ok()?);
        pos += 16;
        
        // Skip "/" separator
        if bytes.get(pos) != Some(&b'/') {
            return None;
        }
        pos += 1;

        // Parse session_id (16 bytes)
        let session_id = u128::from_be_bytes(bytes[pos..pos + 16].try_into().ok()?);
        pos += 16;
        
        // Skip "/" separator
        if bytes.get(pos) != Some(&b'/') {
            return None;
        }
        pos += 1;

        // Parse timestamp (8 bytes)
        let timestamp = u64::from_be_bytes(bytes[pos..pos + 8].try_into().ok()?);
        pos += 8;
        
        // Skip "/" separator
        if bytes.get(pos) != Some(&b'/') {
            return None;
        }
        pos += 1;

        // Parse observation_id (16 bytes)
        let observation_id = u128::from_be_bytes(bytes[pos..pos + 16].try_into().ok()?);

        Some(Self {
            project_id,
            session_id,
            timestamp,
            observation_id,
        })
    }

    /// Create prefix for project queries.
    pub fn project_prefix(project_id: u128) -> Vec<u8> {
        let mut key = Vec::with_capacity(21);
        key.extend_from_slice(b"obs/");
        key.extend_from_slice(&project_id.to_be_bytes());
        key.push(b'/');
        key
    }

    /// Create prefix for session queries.
    pub fn session_prefix(project_id: u128, session_id: u128) -> Vec<u8> {
        let mut key = Vec::with_capacity(38);
        key.extend_from_slice(b"obs/");
        key.extend_from_slice(&project_id.to_be_bytes());
        key.push(b'/');
        key.extend_from_slice(&session_id.to_be_bytes());
        key.push(b'/');
        key
    }
}

/// Stored observation with serialized data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredObservation {
    /// Observation ID.
    pub id: u128,
    /// Session ID.
    pub session_id: u128,
    /// Project ID.
    pub project_id: u128,
    /// Observation type (e.g., "implementation", "debugging").
    pub observation_type: String,
    /// Title/summary.
    pub title: String,
    /// Optional subtitle.
    pub subtitle: Option<String>,
    /// Key facts learned.
    pub facts: Vec<String>,
    /// Narrative description.
    pub narrative: String,
    /// Extracted concepts.
    pub concepts: Vec<String>,
    /// Files read during the observation.
    pub files_read: Vec<String>,
    /// Files modified.
    pub files_modified: Vec<String>,
    /// Source edge ID.
    pub source_edge_id: Option<u128>,
    /// Creation timestamp (HLC microseconds).
    pub created_at: u64,
    /// Last updated timestamp.
    pub updated_at: u64,
}

impl StoredObservation {
    /// Create storage key for this observation.
    pub fn storage_key(&self) -> ObservationKey {
        ObservationKey::new(self.project_id, self.session_id, self.created_at, self.id)
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}

/// Query options for observation retrieval.
#[derive(Debug, Clone, Default)]
pub struct ObservationQuery {
    /// Filter by project ID.
    pub project_id: Option<u128>,
    /// Filter by session ID.
    pub session_id: Option<u128>,
    /// Filter by observation type.
    pub observation_type: Option<String>,
    /// Minimum timestamp.
    pub min_timestamp: Option<u64>,
    /// Maximum timestamp.
    pub max_timestamp: Option<u64>,
    /// Maximum number of results.
    pub limit: Option<usize>,
    /// Skip first N results.
    pub offset: Option<usize>,
    /// Order descending (newest first).
    pub descending: bool,
}

impl ObservationQuery {
    /// Create a new query builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by project.
    pub fn project(mut self, project_id: u128) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Filter by session.
    pub fn session(mut self, session_id: u128) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Filter by observation type.
    pub fn observation_type(mut self, t: impl Into<String>) -> Self {
        self.observation_type = Some(t.into());
        self
    }

    /// Set time range.
    pub fn time_range(mut self, min: u64, max: u64) -> Self {
        self.min_timestamp = Some(min);
        self.max_timestamp = Some(max);
        self
    }

    /// Set limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset.
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Order descending (newest first).
    pub fn descending(mut self) -> Self {
        self.descending = true;
        self
    }

    /// Check if an observation matches this query.
    pub fn matches(&self, obs: &StoredObservation) -> bool {
        if let Some(pid) = self.project_id {
            if obs.project_id != pid {
                return false;
            }
        }

        if let Some(sid) = self.session_id {
            if obs.session_id != sid {
                return false;
            }
        }

        if let Some(ref t) = self.observation_type {
            if obs.observation_type != *t {
                return false;
            }
        }

        if let Some(min) = self.min_timestamp {
            if obs.created_at < min {
                return false;
            }
        }

        if let Some(max) = self.max_timestamp {
            if obs.created_at > max {
                return false;
            }
        }

        true
    }
}

/// In-memory observation store for testing/development.
///
/// Production should use SochDB with AgentReplayStorage extension.
pub struct ObservationStore {
    observations: RwLock<BTreeMap<Vec<u8>, StoredObservation>>,
}

impl Default for ObservationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationStore {
    /// Create a new observation store.
    pub fn new() -> Self {
        Self {
            observations: RwLock::new(BTreeMap::new()),
        }
    }

    /// Store an observation.
    pub fn put(&self, observation: StoredObservation) -> Result<(), ObservationStoreError> {
        let key = observation.storage_key().encode();
        self.observations.write().insert(key, observation);
        Ok(())
    }

    /// Get an observation by key.
    pub fn get(&self, key: &ObservationKey) -> Result<Option<StoredObservation>, ObservationStoreError> {
        Ok(self.observations.read().get(&key.encode()).cloned())
    }

    /// Get an observation by ID.
    pub fn get_by_id(&self, observation_id: u128) -> Result<Option<StoredObservation>, ObservationStoreError> {
        Ok(self
            .observations
            .read()
            .values()
            .find(|o| o.id == observation_id)
            .cloned())
    }

    /// Delete an observation.
    pub fn delete(&self, key: &ObservationKey) -> Result<Option<StoredObservation>, ObservationStoreError> {
        Ok(self.observations.write().remove(&key.encode()))
    }

    /// Query observations.
    pub fn query(&self, query: &ObservationQuery) -> Result<Vec<StoredObservation>, ObservationStoreError> {
        let observations = self.observations.read();

        let mut results: Vec<_> = if let Some(pid) = query.project_id {
            let prefix = ObservationKey::project_prefix(pid);

            observations
                .range(prefix.clone()..)
                .take_while(|(k, _)| k.starts_with(&prefix))
                .filter(|(_, o)| query.matches(o))
                .map(|(_, o)| o.clone())
                .collect()
        } else {
            observations
                .values()
                .filter(|o| query.matches(o))
                .cloned()
                .collect()
        };

        // Apply ordering
        if query.descending {
            results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        } else {
            results.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        }

        // Apply offset
        if let Some(offset) = query.offset {
            results = results.into_iter().skip(offset).collect();
        }

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Get recent observations for a project.
    pub fn get_recent(
        &self,
        project_id: u128,
        limit: usize,
    ) -> Result<Vec<StoredObservation>, ObservationStoreError> {
        self.query(
            &ObservationQuery::new()
                .project(project_id)
                .descending()
                .limit(limit),
        )
    }

    /// Get session observations.
    pub fn get_session_observations(
        &self,
        project_id: u128,
        session_id: u128,
    ) -> Result<Vec<StoredObservation>, ObservationStoreError> {
        self.query(
            &ObservationQuery::new()
                .project(project_id)
                .session(session_id),
        )
    }

    /// Count observations matching query.
    pub fn count(&self, query: &ObservationQuery) -> Result<usize, ObservationStoreError> {
        Ok(self.query(query)?.len())
    }

    /// Get total observation count.
    pub fn total_count(&self) -> usize {
        self.observations.read().len()
    }

    /// Clear all observations.
    pub fn clear(&self) {
        self.observations.write().clear();
    }
}

/// Observation store errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ObservationStoreError {
    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Observation not found: {0}")]
    NotFound(u128),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_observation(id: u128, project_id: u128, session_id: u128, timestamp: u64) -> StoredObservation {
        StoredObservation {
            id,
            session_id,
            project_id,
            observation_type: "implementation".to_string(),
            title: "Test observation".to_string(),
            subtitle: None,
            facts: vec!["Fact 1".to_string()],
            narrative: "Test narrative".to_string(),
            concepts: vec!["concept1".to_string()],
            files_read: vec![],
            files_modified: vec![],
            source_edge_id: None,
            created_at: timestamp,
            updated_at: timestamp,
        }
    }

    #[test]
    fn test_observation_key_encoding() {
        let key = ObservationKey::new(1, 2, 12345, 100);
        let encoded = key.encode();
        let decoded = ObservationKey::decode(&encoded).unwrap();

        assert_eq!(key.project_id, decoded.project_id);
        assert_eq!(key.session_id, decoded.session_id);
        assert_eq!(key.timestamp, decoded.timestamp);
        assert_eq!(key.observation_id, decoded.observation_id);
    }

    #[test]
    fn test_store_and_retrieve() {
        let store = ObservationStore::new();
        let obs = create_test_observation(1, 100, 200, 1000);

        store.put(obs.clone()).unwrap();

        let retrieved = store.get_by_id(1).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test observation");
    }

    #[test]
    fn test_query() {
        let store = ObservationStore::new();

        // Insert observations for different projects
        for i in 0..10 {
            let project_id = if i < 5 { 100 } else { 200 };
            store
                .put(create_test_observation(i, project_id, 1, i as u64 * 1000))
                .unwrap();
        }

        // Query project 100
        let results = store
            .query(&ObservationQuery::new().project(100))
            .unwrap();
        assert_eq!(results.len(), 5);

        // Query with limit
        let results = store
            .query(&ObservationQuery::new().project(100).limit(3))
            .unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_descending_order() {
        let store = ObservationStore::new();

        for i in 0..5 {
            store
                .put(create_test_observation(i, 100, 1, i as u64 * 1000))
                .unwrap();
        }

        let results = store
            .query(&ObservationQuery::new().project(100).descending())
            .unwrap();

        assert_eq!(results[0].created_at, 4000);
        assert_eq!(results[4].created_at, 0);
    }
}
