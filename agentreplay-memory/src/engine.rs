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

//! Memory engine - main entry point for memory operations
//!
//! Orchestrates storage, indexing, and context packing.

use crate::config::MemoryConfig;
use crate::context::{ContextPacker, ContextSpec, PackedContext};
use crate::error::{MemoryError, MemoryResult};
use crate::observation::{Observation, ObservationId, ObservationQuery};
use crate::session::{SessionId, SessionMemory, SessionSummary};
use crate::storage::MemoryStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Main memory engine
pub struct MemoryEngine {
    /// Configuration
    config: MemoryConfig,
    /// Persistent storage
    store: Arc<MemoryStore>,
    /// Active sessions (in-memory)
    active_sessions: RwLock<HashMap<String, SessionMemory>>,
    /// Context packer
    context_packer: ContextPacker,
}

impl MemoryEngine {
    /// Create a new memory engine
    pub async fn new(config: MemoryConfig) -> MemoryResult<Self> {
        info!("Initializing memory engine at {:?}", config.data_dir);

        std::fs::create_dir_all(&config.data_dir)?;

        let store = MemoryStore::new(&config.data_dir).await?;
        let context_packer = ContextPacker::new(config.context_token_budget);

        Ok(Self {
            config,
            store: Arc::new(store),
            active_sessions: RwLock::new(HashMap::new()),
            context_packer,
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }

    // ========================================================================
    // Observation API
    // ========================================================================

    /// Write an observation to memory
    pub async fn write_observation(&self, observation: Observation) -> MemoryResult<ObservationId> {
        let id = observation.id.clone();
        debug!("Writing observation {} to workspace {}", id, observation.workspace_id);

        // Save to store
        self.store.save_observation(&observation).await?;

        // Add to active session if present
        if let Some(session) = self.active_sessions.write().await.get_mut(&observation.session_id) {
            session.add_observation(id.0.clone());
        }

        Ok(id)
    }

    /// Get an observation by ID
    pub async fn get_observation(&self, id: &ObservationId) -> MemoryResult<Option<Observation>> {
        self.store.get_observation(id).await
    }

    /// Query observations
    pub async fn query_observations(
        &self,
        query: ObservationQuery,
    ) -> MemoryResult<Vec<Observation>> {
        self.store.query_observations(&query).await
    }

    /// Delete an observation
    pub async fn delete_observation(&self, id: &ObservationId) -> MemoryResult<bool> {
        self.store.delete_observation(id).await
    }

    // ========================================================================
    // Session API
    // ========================================================================

    /// Start a new session
    pub async fn start_session(&self, workspace_id: &str) -> MemoryResult<SessionId> {
        let memory = SessionMemory::new(workspace_id);
        let session_id = memory.session_id.clone();

        info!("Starting session {} for workspace {}", session_id, workspace_id);

        self.active_sessions
            .write()
            .await
            .insert(session_id.0.clone(), memory);

        Ok(session_id)
    }

    /// Get active session for a workspace
    pub async fn get_active_session(&self, session_id: &str) -> Option<SessionMemory> {
        self.active_sessions.read().await.get(session_id).cloned()
    }

    /// Record a message in a session
    pub async fn record_message(&self, session_id: &str, tokens: u64) -> MemoryResult<()> {
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.record_message(tokens);
            Ok(())
        } else {
            Err(MemoryError::SessionNotFound(session_id.to_string()))
        }
    }

    /// Record a tool call in a session
    pub async fn record_tool_call(&self, session_id: &str) -> MemoryResult<()> {
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.record_tool_call();
            Ok(())
        } else {
            Err(MemoryError::SessionNotFound(session_id.to_string()))
        }
    }

    /// End a session and optionally create a summary
    pub async fn end_session(
        &self,
        session_id: &str,
        summary_text: Option<String>,
    ) -> MemoryResult<Option<SessionSummary>> {
        let mut sessions = self.active_sessions.write().await;

        let session = sessions
            .remove(session_id)
            .ok_or_else(|| MemoryError::SessionNotFound(session_id.to_string()))?;

        info!(
            "Ending session {} ({} messages, {} tool calls)",
            session_id, session.message_count, session.tool_call_count
        );

        let summary = if let Some(text) = summary_text {
            let summary = SessionSummary::from_memory(&session, text);
            self.store.save_session(&summary).await?;
            Some(summary)
        } else if self.config.auto_summarize_sessions {
            // Auto-generate a basic summary
            let text = format!(
                "Session with {} messages and {} tool calls",
                session.message_count, session.tool_call_count
            );
            let summary = SessionSummary::from_memory(&session, text);
            self.store.save_session(&summary).await?;
            Some(summary)
        } else {
            None
        };

        Ok(summary)
    }

    /// Get session summaries for a workspace
    pub async fn get_session_summaries(
        &self,
        workspace_id: &str,
        limit: Option<usize>,
    ) -> MemoryResult<Vec<SessionSummary>> {
        self.store.get_workspace_sessions(workspace_id, limit).await
    }

    // ========================================================================
    // Context API
    // ========================================================================

    /// Retrieve context for injection
    pub async fn retrieve_context(
        &self,
        workspace_id: &str,
        query: Option<&str>,
        k: usize,
    ) -> MemoryResult<Vec<Observation>> {
        let mut obs_query = ObservationQuery::for_workspace(workspace_id).limit(k);

        if let Some(q) = query {
            if self.config.enable_semantic_search {
                obs_query.semantic_query = Some(q.to_string());
            } else {
                obs_query.text_query = Some(q.to_string());
            }
        }

        self.store.query_observations(&obs_query).await
    }

    /// Pack context into a formatted output
    pub async fn pack_context(&self, spec: ContextSpec) -> MemoryResult<PackedContext> {
        let observations = self
            .store
            .query_observations(&ObservationQuery::for_workspace(&spec.workspace_id))
            .await?;

        let sessions = self
            .store
            .get_workspace_sessions(&spec.workspace_id, Some(10))
            .await?;

        Ok(self.context_packer.pack(&spec, &observations, &sessions))
    }

    /// Export context as MDC file
    pub async fn export_mdc(&self, workspace_id: &str) -> MemoryResult<String> {
        let spec = ContextSpec::for_workspace(workspace_id)
            .token_budget(self.config.context_token_budget);

        let packed = self.pack_context(spec).await?;
        Ok(packed.content)
    }

    /// Export context to a file
    pub async fn export_mdc_to_file(
        &self,
        workspace_id: &str,
        path: &PathBuf,
    ) -> MemoryResult<()> {
        let content = self.export_mdc(workspace_id).await?;
        std::fs::write(path, content)?;
        info!("Exported MDC context to {:?}", path);
        Ok(())
    }

    // ========================================================================
    // Maintenance API
    // ========================================================================

    /// Get storage statistics
    pub async fn stats(&self) -> MemoryResult<MemoryStats> {
        let store_stats = self.store.stats().await;
        let active_sessions = self.active_sessions.read().await.len();

        Ok(MemoryStats {
            observation_count: store_stats.observation_count,
            session_count: store_stats.session_count,
            workspace_count: store_stats.workspace_count,
            active_session_count: active_sessions,
        })
    }

    /// Run cleanup based on retention policy
    pub async fn cleanup(&self) -> MemoryResult<CleanupResult> {
        // TODO: Implement cleanup based on retention policy
        Ok(CleanupResult {
            observations_deleted: 0,
            sessions_deleted: 0,
        })
    }
}

/// Memory system statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub observation_count: usize,
    pub session_count: usize,
    pub workspace_count: usize,
    pub active_session_count: usize,
}

/// Result of cleanup operation
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub observations_deleted: usize,
    pub sessions_deleted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::ObservationCategory;
    use tempfile::tempdir;

    async fn create_test_engine() -> MemoryEngine {
        let dir = tempdir().unwrap();
        let config = MemoryConfig {
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        MemoryEngine::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_observation_lifecycle() {
        let engine = create_test_engine().await;

        let obs = Observation::new("ws-1", "s-1")
            .content("Test observation")
            .category(ObservationCategory::Note);

        let id = engine.write_observation(obs).await.unwrap();

        let loaded = engine.get_observation(&id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().content, "Test observation");

        let deleted = engine.delete_observation(&id).await.unwrap();
        assert!(deleted);

        let loaded = engine.get_observation(&id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let engine = create_test_engine().await;

        // Start session
        let session_id = engine.start_session("my-workspace").await.unwrap();

        // Record activity
        engine.record_message(&session_id.0, 100).await.unwrap();
        engine.record_message(&session_id.0, 150).await.unwrap();
        engine.record_tool_call(&session_id.0).await.unwrap();

        // Check active session
        let session = engine.get_active_session(&session_id.0).await.unwrap();
        assert_eq!(session.message_count, 2);
        assert_eq!(session.total_tokens, 250);
        assert_eq!(session.tool_call_count, 1);

        // End session
        let summary = engine
            .end_session(&session_id.0, Some("Completed task X".to_string()))
            .await
            .unwrap();

        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert_eq!(summary.summary, "Completed task X");
        assert_eq!(summary.message_count, 2);
    }

    #[tokio::test]
    async fn test_context_export() {
        let engine = create_test_engine().await;

        // Add some observations
        let obs1 = Observation::new("ws-1", "s-1")
            .content("Use explicit error handling")
            .category(ObservationCategory::Decision);
        let obs2 = Observation::new("ws-1", "s-1")
            .content("User prefers short variable names")
            .category(ObservationCategory::Preference);

        engine.write_observation(obs1).await.unwrap();
        engine.write_observation(obs2).await.unwrap();

        // Export MDC
        let mdc = engine.export_mdc("ws-1").await.unwrap();

        assert!(mdc.contains("---")); // Frontmatter
        assert!(mdc.contains("explicit error handling"));
    }
}
