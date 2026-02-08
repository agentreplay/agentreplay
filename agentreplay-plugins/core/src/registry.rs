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

//! Plugin registry and discovery
//!
//! Manages plugin discovery from multiple sources.

use crate::error::{PluginError, PluginResult};
use crate::manifest::{PluginManifest, PluginType};
use crate::MANIFEST_FILENAME;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Source of a plugin
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginSource {
    /// Built-in plugin shipped with Agentreplay
    Builtin { path: PathBuf },
    /// User-installed plugin
    UserInstalled { path: PathBuf },
    /// Project-local plugin
    ProjectLocal { path: PathBuf },
    /// Development plugin (live reload)
    Development { path: PathBuf, watch: bool },
}

impl PluginSource {
    /// Get the path to the plugin
    pub fn path(&self) -> &Path {
        match self {
            PluginSource::Builtin { path } => path,
            PluginSource::UserInstalled { path } => path,
            PluginSource::ProjectLocal { path } => path,
            PluginSource::Development { path, .. } => path,
        }
    }

    /// Check if this is a development source
    pub fn is_development(&self) -> bool {
        matches!(self, PluginSource::Development { .. })
    }

    /// Get source priority (lower = higher priority)
    pub fn priority(&self) -> u8 {
        match self {
            PluginSource::Builtin { .. } => 0,
            PluginSource::UserInstalled { .. } => 1,
            PluginSource::ProjectLocal { .. } => 2,
            PluginSource::Development { .. } => 3,
        }
    }
}

/// Indexed plugin entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedPlugin {
    /// Plugin manifest
    pub manifest: PluginManifest,
    /// Source of the plugin
    pub source: PluginSource,
    /// Installation timestamp
    pub installed_at: DateTime<Utc>,
    /// Last modified timestamp
    pub modified_at: DateTime<Utc>,
    /// Content hash for integrity
    pub content_hash: String,
    /// Whether the plugin is valid
    pub is_valid: bool,
    /// Validation errors if any
    pub validation_errors: Vec<String>,
}

impl IndexedPlugin {
    /// Create new indexed plugin
    pub fn new(manifest: PluginManifest, source: PluginSource) -> Self {
        let content_hash = manifest.content_hash();
        Self {
            manifest,
            source,
            installed_at: Utc::now(),
            modified_at: Utc::now(),
            content_hash,
            is_valid: true,
            validation_errors: Vec::new(),
        }
    }

    /// Get plugin ID
    pub fn id(&self) -> &str {
        &self.manifest.plugin.id
    }

    /// Get plugin version
    pub fn version(&self) -> &str {
        &self.manifest.plugin.version
    }

    /// Get plugin type
    pub fn plugin_type(&self) -> PluginType {
        self.manifest.plugin.plugin_type
    }
}

/// Plugin registry for managing discovered plugins
pub struct PluginRegistry {
    /// Indexed plugins by ID
    plugins: RwLock<HashMap<String, IndexedPlugin>>,
    /// Plugins by type
    by_type: RwLock<HashMap<PluginType, Vec<String>>>,
    /// Plugin directories to scan
    scan_directories: Vec<PathBuf>,
    /// Last scan time
    last_scan: RwLock<Option<DateTime<Utc>>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            by_type: RwLock::new(HashMap::new()),
            scan_directories: Vec::new(),
            last_scan: RwLock::new(None),
        }
    }

    /// Add a directory to scan for plugins
    pub fn add_scan_directory(&mut self, path: PathBuf) {
        if !self.scan_directories.contains(&path) {
            self.scan_directories.push(path);
        }
    }

    /// Get all scan directories
    pub fn scan_directories(&self) -> &[PathBuf] {
        &self.scan_directories
    }

    /// Scan all configured directories for plugins
    pub fn scan(&self) -> PluginResult<Vec<IndexedPlugin>> {
        let mut discovered = Vec::new();

        for dir in &self.scan_directories {
            if !dir.exists() {
                continue;
            }

            let plugins = self.scan_directory(dir)?;
            discovered.extend(plugins);
        }

        // Update last scan time
        *self.last_scan.write() = Some(Utc::now());

        Ok(discovered)
    }

    /// Scan a single directory for plugins
    fn scan_directory(&self, dir: &Path) -> PluginResult<Vec<IndexedPlugin>> {
        let mut plugins = Vec::new();

        for entry in WalkDir::new(dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path
                .file_name()
                .map(|n| n == MANIFEST_FILENAME)
                .unwrap_or(false)
            {
                if let Some(plugin_dir) = path.parent() {
                    match PluginManifest::from_directory(plugin_dir) {
                        Ok(manifest) => {
                            let source = self.determine_source(plugin_dir);
                            let indexed = IndexedPlugin::new(manifest, source);
                            plugins.push(indexed);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse plugin at {:?}: {}", plugin_dir, e);
                        }
                    }
                }
            }
        }

        Ok(plugins)
    }

    /// Determine the source type based on path
    fn determine_source(&self, path: &Path) -> PluginSource {
        // Check if it's in the user plugins directory
        if let Some(data_dir) = dirs::data_dir() {
            let user_plugins = data_dir.join("agentreplay").join("plugins");
            if path.starts_with(&user_plugins) {
                return PluginSource::UserInstalled {
                    path: path.to_path_buf(),
                };
            }
        }

        // Check if it's a project-local plugin
        if path.components().any(|c| c.as_os_str() == ".agentreplay") {
            return PluginSource::ProjectLocal {
                path: path.to_path_buf(),
            };
        }

        // Default to user installed
        PluginSource::UserInstalled {
            path: path.to_path_buf(),
        }
    }

    /// Register a plugin
    pub fn register(&self, plugin: IndexedPlugin) -> PluginResult<()> {
        let id = plugin.id().to_string();
        let plugin_type = plugin.plugin_type();

        {
            let mut plugins = self.plugins.write();
            if plugins.contains_key(&id) {
                return Err(PluginError::AlreadyInstalled(id));
            }
            plugins.insert(id.clone(), plugin);
        }

        {
            let mut by_type = self.by_type.write();
            by_type.entry(plugin_type).or_default().push(id);
        }

        Ok(())
    }

    /// Unregister a plugin
    pub fn unregister(&self, plugin_id: &str) -> PluginResult<IndexedPlugin> {
        let plugin = {
            let mut plugins = self.plugins.write();
            plugins
                .remove(plugin_id)
                .ok_or_else(|| PluginError::NotInstalled(plugin_id.to_string()))?
        };

        {
            let mut by_type = self.by_type.write();
            if let Some(ids) = by_type.get_mut(&plugin.plugin_type()) {
                ids.retain(|id| id != plugin_id);
            }
        }

        Ok(plugin)
    }

    /// Get a plugin by ID
    pub fn get(&self, plugin_id: &str) -> Option<IndexedPlugin> {
        self.plugins.read().get(plugin_id).cloned()
    }

    /// List all plugins
    pub fn list(&self) -> Vec<IndexedPlugin> {
        self.plugins.read().values().cloned().collect()
    }

    /// List plugins by type
    pub fn list_by_type(&self, plugin_type: PluginType) -> Vec<IndexedPlugin> {
        let by_type = self.by_type.read();
        let plugins = self.plugins.read();

        by_type
            .get(&plugin_type)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| plugins.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if a plugin is registered
    pub fn contains(&self, plugin_id: &str) -> bool {
        self.plugins.read().contains_key(plugin_id)
    }

    /// Get plugin count
    pub fn count(&self) -> usize {
        self.plugins.read().len()
    }

    /// Search plugins by name or description
    pub fn search(&self, query: &str) -> Vec<IndexedPlugin> {
        let query_lower = query.to_lowercase();
        self.plugins
            .read()
            .values()
            .filter(|p| {
                p.manifest.plugin.name.to_lowercase().contains(&query_lower)
                    || p.manifest
                        .plugin
                        .description
                        .to_lowercase()
                        .contains(&query_lower)
                    || p.manifest
                        .plugin
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
    }

    /// Clear all plugins
    pub fn clear(&self) {
        self.plugins.write().clear();
        self.by_type.write().clear();
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PluginMetadata;

    fn create_test_manifest(id: &str) -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMetadata {
                id: id.to_string(),
                name: format!("Test Plugin {}", id),
                version: "1.0.0".to_string(),
                description: "A test plugin".to_string(),
                authors: vec!["Test Author".to_string()],
                license: Some("MIT".to_string()),
                repository: None,
                homepage: None,
                plugin_type: PluginType::Evaluator,
                min_agentreplay_version: "0.1.0".to_string(),
                tags: vec!["test".to_string()],
                icon: None,
            },
            dependencies: HashMap::new(),
            capabilities: Default::default(),
            entry: Default::default(),
            config: None,
            ui: None,
            bundle: None,
        }
    }

    #[test]
    fn test_registry_operations() {
        let registry = PluginRegistry::new();

        let manifest = create_test_manifest("test-plugin");
        let source = PluginSource::UserInstalled {
            path: PathBuf::from("/test"),
        };
        let plugin = IndexedPlugin::new(manifest, source);

        // Register
        registry.register(plugin.clone()).unwrap();
        assert!(registry.contains("test-plugin"));
        assert_eq!(registry.count(), 1);

        // Get
        let retrieved = registry.get("test-plugin").unwrap();
        assert_eq!(retrieved.id(), "test-plugin");

        // Unregister
        registry.unregister("test-plugin").unwrap();
        assert!(!registry.contains("test-plugin"));
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_search() {
        let registry = PluginRegistry::new();

        let manifest = create_test_manifest("search-test");
        let source = PluginSource::UserInstalled {
            path: PathBuf::from("/test"),
        };
        let plugin = IndexedPlugin::new(manifest, source);

        registry.register(plugin).unwrap();

        let results = registry.search("search");
        assert_eq!(results.len(), 1);

        let results = registry.search("nonexistent");
        assert_eq!(results.len(), 0);
    }
}
