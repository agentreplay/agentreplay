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

//! Project Registry with Cache
//!
//! Manages project metadata with in-memory caching
//! - Fast lookups without filesystem scanning
//! - Persistent storage of project metadata
//! - Automatic cache invalidation

use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

/// Project metadata stored in registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub project_id: u16,
    pub name: String,
    pub description: Option<String>,
    pub created_at: u64,
    pub last_updated: u64,
    pub favorite: bool,
}

/// Cached project statistics
#[derive(Debug, Clone)]
pub struct ProjectStats {
    pub trace_count: usize,
    pub last_updated: u64,
}

/// Combined project info for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    #[serde(flatten)]
    pub metadata: ProjectMetadata,
    pub trace_count: usize,
}

/// Project registry with fast in-memory cache
pub struct ProjectRegistry {
    /// Base directory for projects
    base_dir: PathBuf,
    /// Registry file path
    registry_path: PathBuf,
    /// In-memory cache for project metadata
    metadata_cache: Arc<DashMap<u16, ProjectMetadata>>,
    /// In-memory cache for stats (shorter TTL)
    stats_cache: Arc<DashMap<u16, ProjectStats>>,
}

impl ProjectRegistry {
    /// Create new project registry
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        let registry_path = base_dir.join("projects_registry.json");

        let registry = Self {
            base_dir,
            registry_path,
            metadata_cache: Arc::new(DashMap::new()),
            stats_cache: Arc::new(DashMap::new()),
        };

        // Load existing registry from disk
        registry.load_from_disk()?;

        Ok(registry)
    }

    /// Load registry from disk into cache
    fn load_from_disk(&self) -> Result<()> {
        if !self.registry_path.exists() {
            info!("No existing project registry found, starting fresh");
            return Ok(());
        }

        let contents = std::fs::read_to_string(&self.registry_path)?;
        let projects: Vec<ProjectMetadata> = serde_json::from_str(&contents)?;

        for project in projects {
            self.metadata_cache.insert(project.project_id, project);
        }

        info!(
            "Loaded {} projects from registry",
            self.metadata_cache.len()
        );
        Ok(())
    }

    /// Save registry to disk
    fn save_to_disk(&self) -> Result<()> {
        let projects: Vec<ProjectMetadata> = self
            .metadata_cache
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        let contents = serde_json::to_string_pretty(&projects)?;
        std::fs::write(&self.registry_path, contents)?;

        info!("Saved {} projects to registry", projects.len());
        Ok(())
    }

    /// Register a new project
    pub fn register_project(
        &self,
        project_id: u16,
        name: String,
        description: Option<String>,
    ) -> Result<ProjectMetadata> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let metadata = ProjectMetadata {
            project_id,
            name,
            description,
            created_at: now,
            last_updated: now,
            favorite: false,
        };

        // Insert into cache
        self.metadata_cache.insert(project_id, metadata.clone());

        // Persist to disk
        self.save_to_disk()?;

        info!("Registered new project {}: {}", project_id, metadata.name);
        Ok(metadata)
    }

    /// Update project metadata
    pub fn update_project(
        &self,
        project_id: u16,
        name: Option<String>,
        description: Option<String>,
        favorite: Option<bool>,
    ) -> Result<ProjectMetadata> {
        let mut metadata = self
            .metadata_cache
            .get(&project_id)
            .ok_or_else(|| anyhow::anyhow!("Project {} not found", project_id))?
            .clone();

        if let Some(name) = name {
            metadata.name = name;
        }
        if let Some(desc) = description {
            metadata.description = Some(desc);
        }
        if let Some(fav) = favorite {
            metadata.favorite = fav;
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        metadata.last_updated = now;

        // Update cache
        self.metadata_cache.insert(project_id, metadata.clone());

        // Persist to disk
        self.save_to_disk()?;

        Ok(metadata)
    }

    /// Get project metadata from cache
    pub fn get_metadata(&self, project_id: u16) -> Option<ProjectMetadata> {
        self.metadata_cache.get(&project_id).map(|r| r.clone())
    }

    /// Get all projects (from cache)
    pub fn list_projects(&self) -> Vec<ProjectMetadata> {
        self.metadata_cache
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Cache project stats
    pub fn cache_stats(&self, project_id: u16, trace_count: usize) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let stats = ProjectStats {
            trace_count,
            last_updated: now,
        };

        self.stats_cache.insert(project_id, stats);
    }

    /// Get cached stats (returns None if cache miss or stale)
    pub fn get_cached_stats(&self, project_id: u16, max_age_secs: u64) -> Option<usize> {
        self.stats_cache.get(&project_id).and_then(|stats| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Return cached value only if fresh enough
            if now - stats.last_updated < max_age_secs {
                Some(stats.trace_count)
            } else {
                None
            }
        })
    }

    /// Discover projects from filesystem (expensive operation)
    pub fn discover_projects(&self) -> Result<Vec<u16>> {
        let mut project_ids = Vec::new();

        if !self.base_dir.exists() {
            return Ok(project_ids);
        }

        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(id_str) = dir_name.strip_prefix("project_") {
                        if let Ok(project_id) = id_str.parse::<u16>() {
                            project_ids.push(project_id);

                            // If not in cache, add default metadata
                            if self.metadata_cache.get(&project_id).is_none() {
                                warn!(
                                    "Discovered unregistered project {}, adding to registry",
                                    project_id
                                );
                                let _ = self.register_project(
                                    project_id,
                                    format!("Project {}", project_id),
                                    Some("Auto-discovered project".to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(project_ids)
    }

    /// Invalidate stats cache for a project
    pub fn invalidate_stats(&self, project_id: u16) {
        self.stats_cache.remove(&project_id);
    }

    /// Remove a project from the registry
    pub fn remove_project(&self, project_id: u16) {
        // Remove from metadata cache
        self.metadata_cache.remove(&project_id);
        // Remove from stats cache
        self.stats_cache.remove(&project_id);
        // Persist changes to disk
        if let Err(e) = self.save_to_disk() {
            warn!(
                "Failed to save registry after removing project {}: {}",
                project_id, e
            );
        } else {
            info!("Removed project {} from registry", project_id);
        }
    }

    /// Clear all projects from registry
    pub fn clear_all(&self) {
        self.metadata_cache.clear();
        self.stats_cache.clear();
        // Persist empty registry to disk
        if let Err(e) = self.save_to_disk() {
            warn!("Failed to save empty registry: {}", e);
        } else {
            info!("Cleared all projects from registry");
        }
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        self.metadata_cache.clear();
        self.stats_cache.clear();
    }
}
