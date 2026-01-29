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

//! Plugin state persistence
//!
//! Manages persisted state for plugins across app restarts.

use crate::error::PluginResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Persisted state for all plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateStore {
    /// Path to the state file
    #[serde(skip)]
    path: Option<PathBuf>,

    /// Enabled/disabled state for each plugin
    #[serde(default)]
    pub enabled: HashMap<String, bool>,

    /// Pinned versions (prevent auto-update)
    #[serde(default)]
    pub pinned_versions: HashMap<String, String>,

    /// Last update check time
    #[serde(default)]
    pub last_update_check: HashMap<String, DateTime<Utc>>,

    /// Plugin-specific settings
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,

    /// Installation order for deterministic loading
    #[serde(default)]
    pub install_order: Vec<String>,

    /// Plugins marked for removal on next startup
    #[serde(default)]
    pub pending_removal: Vec<String>,

    /// Previous versions for rollback
    #[serde(default)]
    pub previous_versions: HashMap<String, String>,
}

impl Default for PluginStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginStateStore {
    /// Create a new empty state store
    pub fn new() -> Self {
        Self {
            path: None,
            enabled: HashMap::new(),
            pinned_versions: HashMap::new(),
            last_update_check: HashMap::new(),
            settings: HashMap::new(),
            install_order: Vec::new(),
            pending_removal: Vec::new(),
            previous_versions: HashMap::new(),
        }
    }

    /// Load state from file
    pub fn load(path: &Path) -> PluginResult<Self> {
        if !path.exists() {
            let mut store = Self::new();
            store.path = Some(path.to_path_buf());
            return Ok(store);
        }

        let content = std::fs::read_to_string(path)?;
        let mut store: Self = serde_json::from_str(&content)?;
        store.path = Some(path.to_path_buf());
        Ok(store)
    }

    /// Save state to file
    pub fn save(&self) -> PluginResult<()> {
        if let Some(path) = &self.path {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            std::fs::write(path, content)?;
        }
        Ok(())
    }

    /// Set the path for this store
    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    /// Check if a plugin is enabled
    pub fn is_enabled(&self, plugin_id: &str) -> bool {
        self.enabled.get(plugin_id).copied().unwrap_or(true)
    }

    /// Set plugin enabled state
    pub fn set_enabled(&mut self, plugin_id: &str, enabled: bool) -> PluginResult<()> {
        self.enabled.insert(plugin_id.to_string(), enabled);
        self.save()
    }

    /// Check if a plugin version is pinned
    pub fn is_pinned(&self, plugin_id: &str) -> bool {
        self.pinned_versions.contains_key(plugin_id)
    }

    /// Pin a plugin version
    pub fn pin_version(&mut self, plugin_id: &str, version: &str) -> PluginResult<()> {
        self.pinned_versions
            .insert(plugin_id.to_string(), version.to_string());
        self.save()
    }

    /// Unpin a plugin version
    pub fn unpin_version(&mut self, plugin_id: &str) -> PluginResult<()> {
        self.pinned_versions.remove(plugin_id);
        self.save()
    }

    /// Get plugin settings
    pub fn get_settings(&self, plugin_id: &str) -> Option<&serde_json::Value> {
        self.settings.get(plugin_id)
    }

    /// Set plugin settings
    pub fn set_settings(
        &mut self,
        plugin_id: &str,
        settings: serde_json::Value,
    ) -> PluginResult<()> {
        self.settings.insert(plugin_id.to_string(), settings);
        self.save()
    }

    /// Record plugin installation
    pub fn record_installation(&mut self, plugin_id: &str) -> PluginResult<()> {
        if !self.install_order.contains(&plugin_id.to_string()) {
            self.install_order.push(plugin_id.to_string());
        }
        self.enabled.insert(plugin_id.to_string(), true);
        self.save()
    }

    /// Record plugin uninstallation
    pub fn record_uninstallation(&mut self, plugin_id: &str) -> PluginResult<()> {
        self.install_order.retain(|id| id != plugin_id);
        self.enabled.remove(plugin_id);
        self.pinned_versions.remove(plugin_id);
        self.settings.remove(plugin_id);
        self.last_update_check.remove(plugin_id);
        self.previous_versions.remove(plugin_id);
        self.save()
    }

    /// Mark plugin for pending removal
    pub fn mark_for_removal(&mut self, plugin_id: &str) -> PluginResult<()> {
        if !self.pending_removal.contains(&plugin_id.to_string()) {
            self.pending_removal.push(plugin_id.to_string());
        }
        self.save()
    }

    /// Clear pending removal
    pub fn clear_pending_removal(&mut self, plugin_id: &str) -> PluginResult<()> {
        self.pending_removal.retain(|id| id != plugin_id);
        self.save()
    }

    /// Get plugins in installation order
    pub fn installed_plugins(&self) -> &[String] {
        &self.install_order
    }

    /// Record previous version for rollback
    pub fn record_previous_version(&mut self, plugin_id: &str, version: &str) -> PluginResult<()> {
        self.previous_versions
            .insert(plugin_id.to_string(), version.to_string());
        self.save()
    }

    /// Get previous version for rollback
    pub fn get_previous_version(&self, plugin_id: &str) -> Option<&str> {
        self.previous_versions.get(plugin_id).map(|s| s.as_str())
    }

    /// Update last update check time
    pub fn record_update_check(&mut self, plugin_id: &str) -> PluginResult<()> {
        self.last_update_check
            .insert(plugin_id.to_string(), Utc::now());
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_state_store_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        // Create and save state
        {
            let mut store = PluginStateStore::load(&state_path).unwrap();
            store.set_enabled("test-plugin", true).unwrap();
            store
                .set_settings("test-plugin", serde_json::json!({"key": "value"}))
                .unwrap();
        }

        // Load and verify
        {
            let store = PluginStateStore::load(&state_path).unwrap();
            assert!(store.is_enabled("test-plugin"));
            assert_eq!(
                store.get_settings("test-plugin"),
                Some(&serde_json::json!({"key": "value"}))
            );
        }
    }

    #[test]
    fn test_installation_tracking() {
        let mut store = PluginStateStore::new();

        store.record_installation("plugin-a").ok();
        store.record_installation("plugin-b").ok();
        store.record_installation("plugin-c").ok();

        assert_eq!(
            store.installed_plugins(),
            &["plugin-a", "plugin-b", "plugin-c"]
        );

        store.record_uninstallation("plugin-b").ok();
        assert_eq!(store.installed_plugins(), &["plugin-a", "plugin-c"]);
    }
}
