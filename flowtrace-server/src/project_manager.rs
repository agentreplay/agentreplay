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

//! Project Manager - manages multiple Flowtrace instances per project
//!
//! Each project gets its own isolated storage directory:
//! - flowtrace/project_0/wal/
//! - flowtrace/project_0/data/
//! - flowtrace/project_0/causal.index
//! - flowtrace/project_0/vector.index
//!
//! Benefits:
//! - Project isolation - one project's data doesn't affect others
//! - Independent backups per project
//! - Easier to delete/archive projects
//! - Better performance - smaller indexes per project

use flowtrace_core::{AgentFlowEdge, Result};
use flowtrace_query::Flowtrace;
use moka::sync::Cache;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

/// Manages multiple Flowtrace database instances, one per project
///
/// CRITICAL FIX: Now uses LRU cache with eviction to prevent file descriptor exhaustion.
/// The cache automatically closes databases that haven't been accessed recently.
pub struct ProjectManager {
    /// Base directory for all projects (e.g., "./flowtrace")
    base_dir: PathBuf,
    /// LRU cache of open Flowtrace instances: project_id -> Flowtrace
    /// Max 50 projects open at once to prevent FD exhaustion
    projects: Cache<u16, Arc<Flowtrace>>,
}

impl ProjectManager {
    /// Create a new ProjectManager
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for all project data (e.g., "./flowtrace")
    ///
    /// # Directory Structure
    /// ```text
    /// base_dir/
    ///   project_0/
    ///     wal/
    ///     data/
    ///     causal.index
    ///     vector.index
    ///   project_1/
    ///     wal/
    ///     data/
    ///     causal.index
    ///     vector.index
    /// ```
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();

        // Create base directory if it doesn't exist
        if !base_dir.exists() {
            std::fs::create_dir_all(&base_dir)?;
            info!("Created base directory: {:?}", base_dir);
        }

        // Create LRU cache with eviction listener
        let projects = Cache::builder()
            .max_capacity(50) // Keep max 50 projects open at once
            .eviction_listener(|key, _value: Arc<Flowtrace>, cause| {
                // Flowtrace::close() will be called when Arc drops
                info!("Closing project {} due to {:?}", key, cause);
            })
            .build();

        Ok(Self { base_dir, projects })
    }

    /// Get the storage directory for a specific project
    fn project_dir(&self, project_id: u16) -> PathBuf {
        self.base_dir.join(format!("project_{}", project_id))
    }

    /// Get or open a Flowtrace instance for a project
    ///
    /// This method:
    /// 1. Checks if the project is already open (cached)
    /// 2. If not, opens the Flowtrace database for that project
    /// 3. Caches the instance for future use (LRU eviction policy)
    /// 4. Returns an Arc to the Flowtrace instance
    ///
    /// Thread-safe: Multiple threads can call this concurrently.
    /// The LRU cache automatically evicts least-recently-used projects
    /// when the capacity (50) is reached.
    pub fn get_or_open_project(&self, project_id: u16) -> Result<Arc<Flowtrace>> {
        // Try to get from cache using get_with method
        // This is thread-safe and handles concurrent access internally
        self.projects
            .try_get_with(project_id, || {
                // Open the Flowtrace instance for this project
                let project_dir = self.project_dir(project_id);
                info!(
                    "Opening Flowtrace for project {} at {:?}",
                    project_id, project_dir
                );

                // Always use high-performance mode for projects
                Flowtrace::open_high_performance(&project_dir).map(Arc::new)
            })
            .map_err(|e| match Arc::try_unwrap(e) {
                Ok(err) => err,
                Err(arc_err) => {
                    // If we can't unwrap, create a new error with the same message
                    use flowtrace_core::FlowtraceError;
                    FlowtraceError::Internal(format!("Failed to open project: {}", arc_err))
                }
            })
    }

    /// Insert an edge into the appropriate project
    pub async fn insert(&self, edge: AgentFlowEdge) -> Result<()> {
        let project_id = edge.project_id;
        let db = self.get_or_open_project(project_id)?;
        db.insert(edge).await
    }

    /// Insert an edge with its embedding vector into the appropriate project
    ///
    /// This stores both the edge AND the embedding in the vector index,
    /// enabling semantic search across all ingested traces.
    pub async fn insert_with_embedding(
        &self,
        edge: AgentFlowEdge,
        embedding: Vec<f32>,
    ) -> Result<()> {
        use flowtrace_index::Embedding;

        let project_id = edge.project_id;
        let db = self.get_or_open_project(project_id)?;
        let embedding_array = Embedding::from_vec(embedding);
        db.insert_with_vector(edge, embedding_array).await
    }

    /// Query edges from a specific project
    pub fn query_project(
        &self,
        project_id: u16,
        tenant_id: u64,
        start_ts: u64,
        end_ts: u64,
    ) -> Result<Vec<AgentFlowEdge>> {
        let db = self.get_or_open_project(project_id)?;
        db.query_temporal_range_for_tenant(start_ts, end_ts, tenant_id)
    }

    /// Query edges across all projects for a tenant
    pub fn query_all_projects(
        &self,
        tenant_id: u64,
        start_ts: u64,
        end_ts: u64,
    ) -> Result<Vec<AgentFlowEdge>> {
        let mut all_edges = Vec::new();

        // Get list of all project directories
        let projects = self.discover_projects()?;

        for project_id in projects {
            match self.query_project(project_id, tenant_id, start_ts, end_ts) {
                Ok(edges) => all_edges.extend(edges),
                Err(e) => {
                    warn!("Failed to query project {}: {}", project_id, e);
                }
            }
        }

        Ok(all_edges)
    }

    /// Get a specific edge by ID across all projects for a tenant
    ///
    /// This is used by the get_trace endpoint to find an edge when
    /// we don't know which project it belongs to.
    pub fn get_edge_for_tenant(
        &self,
        edge_id: u128,
        tenant_id: u64,
    ) -> Result<Option<AgentFlowEdge>> {
        // Get list of all project directories
        let projects = self.discover_projects()?;

        for project_id in projects {
            if let Ok(db) = self.get_or_open_project(project_id) {
                match db.get_for_tenant(edge_id, tenant_id) {
                    Ok(Some(edge)) => return Ok(Some(edge)),
                    Ok(None) => continue, // Not in this project, try next
                    Err(e) => {
                        warn!("Failed to query edge from project {}: {}", project_id, e);
                        continue;
                    }
                }
            }
        }

        // Not found in any project
        Ok(None)
    }

    /// Discover all existing projects by scanning the base directory
    pub fn discover_projects(&self) -> Result<Vec<u16>> {
        let mut project_ids = Vec::new();

        if !self.base_dir.exists() {
            return Ok(project_ids);
        }

        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(dir_name) = path.file_name() {
                    if let Some(name_str) = dir_name.to_str() {
                        if let Some(id_str) = name_str.strip_prefix("project_") {
                            if let Ok(project_id) = id_str.parse::<u16>() {
                                project_ids.push(project_id);
                            }
                        }
                    }
                }
            }
        }

        project_ids.sort();
        Ok(project_ids)
    }

    /// Close a specific project (flush and release resources)
    pub fn close_project(&self, project_id: u16) -> Result<()> {
        self.projects.invalidate(&project_id);
        info!("Invalidated project {} from cache", project_id);
        // Flowtrace::close() will be called when Arc drops
        Ok(())
    }

    /// Close all projects gracefully
    pub fn close_all(&self) -> Result<()> {
        let count = self.projects.entry_count();
        info!("Closing all {} projects", count);
        self.projects.invalidate_all();
        // Flowtrace::close() will be called when Arcs drop
        Ok(())
    }

    /// Delete a specific project and all its data
    pub fn delete_project(&self, project_id: u16) -> Result<()> {
        // First close the project to release all handles
        self.close_project(project_id)?;

        // Wait a moment for file handles to be released
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Delete the project directory
        let project_dir = self.project_dir(project_id);
        if project_dir.exists() {
            info!(
                "Deleting project {} directory: {:?}",
                project_id, project_dir
            );
            std::fs::remove_dir_all(&project_dir)?;
            info!("Successfully deleted project {}", project_id);
        } else {
            warn!(
                "Project {} directory does not exist: {:?}",
                project_id, project_dir
            );
        }

        Ok(())
    }

    /// Delete all projects and data
    pub fn delete_all_projects(&self) -> Result<usize> {
        let projects = self.discover_projects()?;
        let count = projects.len();

        // Close all projects first
        self.close_all()?;

        // Wait a moment for file handles to be released
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Delete all project directories
        for project_id in &projects {
            let project_dir = self.project_dir(*project_id);
            if project_dir.exists() {
                info!(
                    "Deleting project {} directory: {:?}",
                    project_id, project_dir
                );
                if let Err(e) = std::fs::remove_dir_all(&project_dir) {
                    warn!("Failed to delete project {} directory: {}", project_id, e);
                }
            }
        }

        info!("Deleted {} projects", count);
        Ok(count)
    }

    /// Get statistics about all projects
    pub fn get_project_stats(&self) -> Result<HashMap<u16, ProjectStats>> {
        let mut stats = HashMap::new();
        let projects = self.discover_projects()?;

        for project_id in projects {
            if let Ok(_db) = self.get_or_open_project(project_id) {
                // For now, just report basic info
                // TODO: Add LSM stats when available
                stats.insert(
                    project_id,
                    ProjectStats {
                        project_id,
                        total_edges: 0, // TODO: Get from stats
                        directory_size_bytes: Self::calculate_dir_size(
                            &self.project_dir(project_id),
                        ),
                    },
                );
            }
        }

        Ok(stats)
    }

    /// Calculate total size of a directory
    fn calculate_dir_size(path: &Path) -> u64 {
        let mut total_size = 0u64;

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        total_size += metadata.len();
                    } else if metadata.is_dir() {
                        total_size += Self::calculate_dir_size(&entry.path());
                    }
                }
            }
        }

        total_size
    }
}

/// Statistics for a project
#[derive(Debug, Clone)]
pub struct ProjectStats {
    pub project_id: u16,
    pub total_edges: usize,
    pub directory_size_bytes: u64,
}
