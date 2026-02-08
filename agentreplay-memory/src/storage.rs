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

//! Storage layer for memory system
//!
//! Uses agentreplay-storage's LSM tree for persistence with custom
//! key schemes for efficient queries.

use crate::error::{MemoryError, MemoryResult};
use crate::observation::{Observation, ObservationId, ObservationQuery};
use crate::session::{SessionId, SessionSummary};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Key prefix for observations
const OBS_PREFIX: &str = "obs";
/// Key prefix for sessions
const SESSION_PREFIX: &str = "session";
/// Key prefix for workspace metadata
const WORKSPACE_PREFIX: &str = "workspace";

/// In-memory store for development/testing
/// Production would use agentreplay-storage LSM
#[derive(Debug)]
pub struct MemoryStore {
    /// Storage path
    path: PathBuf,
    /// Observations by ID
    observations: RwLock<HashMap<String, Observation>>,
    /// Observations by workspace
    observations_by_workspace: RwLock<HashMap<String, Vec<String>>>,
    /// Session summaries by ID
    sessions: RwLock<HashMap<String, SessionSummary>>,
    /// Sessions by workspace
    sessions_by_workspace: RwLock<HashMap<String, Vec<String>>>,
}

impl MemoryStore {
    /// Create a new memory store
    pub async fn new(path: impl AsRef<Path>) -> MemoryResult<Self> {
        let path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;

        let store = Self {
            path,
            observations: RwLock::new(HashMap::new()),
            observations_by_workspace: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
            sessions_by_workspace: RwLock::new(HashMap::new()),
        };

        // Load existing data
        store.load_from_disk().await?;

        Ok(store)
    }

    /// Save an observation
    pub async fn save_observation(&self, observation: &Observation) -> MemoryResult<()> {
        let id = observation.id.0.clone();
        let workspace_id = observation.workspace_id.clone();

        {
            let mut obs = self.observations.write().await;
            obs.insert(id.clone(), observation.clone());
        }

        {
            let mut by_workspace = self.observations_by_workspace.write().await;
            by_workspace
                .entry(workspace_id)
                .or_insert_with(Vec::new)
                .push(id);
        }

        self.persist_observation(observation).await
    }

    /// Get an observation by ID
    pub async fn get_observation(&self, id: &ObservationId) -> MemoryResult<Option<Observation>> {
        let obs = self.observations.read().await;
        Ok(obs.get(&id.0).cloned())
    }

    /// Query observations
    pub async fn query_observations(
        &self,
        query: &ObservationQuery,
    ) -> MemoryResult<Vec<Observation>> {
        let obs = self.observations.read().await;
        let by_workspace = self.observations_by_workspace.read().await;

        let mut results: Vec<Observation> = if let Some(workspace_id) = &query.workspace_id {
            by_workspace
                .get(workspace_id)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| obs.get(id).cloned())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            obs.values().cloned().collect()
        };

        // Apply filters
        if let Some(session_id) = &query.session_id {
            results.retain(|o| o.session_id == *session_id);
        }

        if !query.categories.is_empty() {
            results.retain(|o| query.categories.contains(&o.category));
        }

        if !query.tags.is_empty() {
            results.retain(|o| o.tags.iter().any(|t| query.tags.contains(t)));
        }

        if let Some(from) = query.from_time {
            results.retain(|o| o.created_at >= from);
        }

        if let Some(to) = query.to_time {
            results.retain(|o| o.created_at <= to);
        }

        if let Some(text) = &query.text_query {
            let text_lower = text.to_lowercase();
            results.retain(|o| o.content.to_lowercase().contains(&text_lower));
        }

        // Sort
        match query.sort.as_deref() {
            Some("oldest") => results.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
            _ => results.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        }

        // Apply pagination
        if let Some(offset) = query.offset {
            results = results.into_iter().skip(offset).collect();
        }

        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Delete an observation
    pub async fn delete_observation(&self, id: &ObservationId) -> MemoryResult<bool> {
        let mut obs = self.observations.write().await;
        let removed = obs.remove(&id.0);

        if let Some(observation) = &removed {
            let mut by_workspace = self.observations_by_workspace.write().await;
            if let Some(ids) = by_workspace.get_mut(&observation.workspace_id) {
                ids.retain(|i| i != &id.0);
            }
        }

        Ok(removed.is_some())
    }

    /// Save a session summary
    pub async fn save_session(&self, summary: &SessionSummary) -> MemoryResult<()> {
        let id = summary.session_id.0.clone();
        let workspace_id = summary.workspace_id.clone();

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(id.clone(), summary.clone());
        }

        {
            let mut by_workspace = self.sessions_by_workspace.write().await;
            by_workspace
                .entry(workspace_id)
                .or_insert_with(Vec::new)
                .push(id);
        }

        self.persist_session(summary).await
    }

    /// Get a session summary by ID
    pub async fn get_session(&self, id: &SessionId) -> MemoryResult<Option<SessionSummary>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(&id.0).cloned())
    }

    /// Get sessions for a workspace
    pub async fn get_workspace_sessions(
        &self,
        workspace_id: &str,
        limit: Option<usize>,
    ) -> MemoryResult<Vec<SessionSummary>> {
        let sessions = self.sessions.read().await;
        let by_workspace = self.sessions_by_workspace.read().await;

        let mut results: Vec<SessionSummary> = by_workspace
            .get(workspace_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| sessions.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default();

        // Sort by start time, newest first
        results.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        if let Some(limit) = limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Get storage statistics
    pub async fn stats(&self) -> StoreStats {
        let obs = self.observations.read().await;
        let sessions = self.sessions.read().await;
        let workspaces = self.observations_by_workspace.read().await;

        StoreStats {
            observation_count: obs.len(),
            session_count: sessions.len(),
            workspace_count: workspaces.len(),
        }
    }

    async fn load_from_disk(&self) -> MemoryResult<()> {
        // Load observations
        let obs_path = self.path.join("observations");
        if obs_path.exists() {
            for entry in std::fs::read_dir(&obs_path)? {
                let entry = entry?;
                if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                    let content = std::fs::read_to_string(entry.path())?;
                    if let Ok(obs) = serde_json::from_str::<Observation>(&content) {
                        let id = obs.id.0.clone();
                        let workspace_id = obs.workspace_id.clone();

                        self.observations.write().await.insert(id.clone(), obs);
                        self.observations_by_workspace
                            .write()
                            .await
                            .entry(workspace_id)
                            .or_insert_with(Vec::new)
                            .push(id);
                    }
                }
            }
        }

        // Load sessions
        let sessions_path = self.path.join("sessions");
        if sessions_path.exists() {
            for entry in std::fs::read_dir(&sessions_path)? {
                let entry = entry?;
                if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                    let content = std::fs::read_to_string(entry.path())?;
                    if let Ok(session) = serde_json::from_str::<SessionSummary>(&content) {
                        let id = session.session_id.0.clone();
                        let workspace_id = session.workspace_id.clone();

                        self.sessions.write().await.insert(id.clone(), session);
                        self.sessions_by_workspace
                            .write()
                            .await
                            .entry(workspace_id)
                            .or_insert_with(Vec::new)
                            .push(id);
                    }
                }
            }
        }

        Ok(())
    }

    async fn persist_observation(&self, observation: &Observation) -> MemoryResult<()> {
        let obs_path = self.path.join("observations");
        std::fs::create_dir_all(&obs_path)?;

        let file_path = obs_path.join(format!("{}.json", observation.id.0));
        let content = serde_json::to_string_pretty(observation)?;
        std::fs::write(file_path, content)?;

        Ok(())
    }

    async fn persist_session(&self, summary: &SessionSummary) -> MemoryResult<()> {
        let sessions_path = self.path.join("sessions");
        std::fs::create_dir_all(&sessions_path)?;

        let file_path = sessions_path.join(format!("{}.json", summary.session_id.0));
        let content = serde_json::to_string_pretty(summary)?;
        std::fs::write(file_path, content)?;

        Ok(())
    }
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStats {
    pub observation_count: usize,
    pub session_count: usize,
    pub workspace_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::ObservationCategory;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_observation_storage() {
        let dir = tempdir().unwrap();
        let store = MemoryStore::new(dir.path()).await.unwrap();

        let obs = Observation::new("workspace-1", "session-1")
            .content("Test observation")
            .category(ObservationCategory::Note);

        store.save_observation(&obs).await.unwrap();

        let loaded = store.get_observation(&obs.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().content, "Test observation");
    }

    #[tokio::test]
    async fn test_query_observations() {
        let dir = tempdir().unwrap();
        let store = MemoryStore::new(dir.path()).await.unwrap();

        let obs1 = Observation::new("ws", "s1")
            .content("First")
            .category(ObservationCategory::Decision);
        let obs2 = Observation::new("ws", "s1")
            .content("Second")
            .category(ObservationCategory::Note);
        let obs3 = Observation::new("ws", "s2")
            .content("Third")
            .category(ObservationCategory::Decision);

        store.save_observation(&obs1).await.unwrap();
        store.save_observation(&obs2).await.unwrap();
        store.save_observation(&obs3).await.unwrap();

        // Query all for workspace
        let query = ObservationQuery::for_workspace("ws");
        let results = store.query_observations(&query).await.unwrap();
        assert_eq!(results.len(), 3);

        // Query by category
        let query = ObservationQuery::for_workspace("ws").category(ObservationCategory::Decision);
        let results = store.query_observations(&query).await.unwrap();
        assert_eq!(results.len(), 2);

        // Query with limit
        let query = ObservationQuery::for_workspace("ws").limit(2);
        let results = store.query_observations(&query).await.unwrap();
        assert_eq!(results.len(), 2);
    }
}
