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

//! Plugin manager - main entry point for plugin operations
//!
//! Orchestrates plugin lifecycle: discovery, installation, loading, and execution.

use crate::capabilities::{CapabilityEnforcer, CapabilitySet, GrantedCapabilities};
use crate::error::{PluginError, PluginResult};
use crate::installer::{
    InstallResult, PluginInstaller, PluginUpdater, UninstallMode, UninstallResult,
};
use crate::manifest::{PluginManifest, PluginType};
use crate::native::ExecutablePlugin;
use crate::registry::{IndexedPlugin, PluginRegistry, PluginSource};
use crate::resolver::DependencyResolver;
use crate::state::PluginStateStore;
use crate::PLUGINS_DIR_NAME;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Base directory for plugin data
    pub data_dir: PathBuf,
    /// Enable development mode features
    pub dev_mode: bool,
    /// Auto-enable newly installed plugins
    pub auto_enable: bool,
    /// Check for updates on startup
    pub check_updates_on_startup: bool,
    /// Maximum plugins to load
    pub max_plugins: usize,
}

impl Default for PluginConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agentreplay");

        Self {
            data_dir,
            dev_mode: cfg!(debug_assertions),
            auto_enable: true,
            check_updates_on_startup: true,
            max_plugins: 100,
        }
    }
}

/// Plugin state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Discovered but not loaded
    Discovered,
    /// Installed but not enabled
    Installed,
    /// Enabled and ready to load
    Enabled,
    /// Currently loading
    Loading,
    /// Fully loaded and active
    Active,
    /// Disabled by user
    Disabled,
    /// Failed to load
    Failed,
    /// Uninstalling
    Uninstalling,
}

/// Plugin information for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Version
    pub version: String,
    /// Description
    pub description: String,
    /// Plugin type
    pub plugin_type: PluginType,
    /// Authors
    pub authors: Vec<String>,
    /// Current state
    pub state: PluginState,
    /// Whether plugin is enabled
    pub enabled: bool,
    /// Installation path
    pub install_path: PathBuf,
    /// Installation time
    pub installed_at: DateTime<Utc>,
    /// Source type
    pub source: String,
    /// Required capabilities
    pub capabilities: Vec<String>,
    /// Tags
    pub tags: Vec<String>,
    /// Error message if failed
    pub error: Option<String>,
}

impl From<&IndexedPlugin> for PluginInfo {
    fn from(plugin: &IndexedPlugin) -> Self {
        let manifest = &plugin.manifest;
        let caps = CapabilitySet::from_requirements(&manifest.capabilities);

        Self {
            id: manifest.plugin.id.clone(),
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            description: manifest.plugin.description.clone(),
            plugin_type: manifest.plugin.plugin_type,
            authors: manifest.plugin.authors.clone(),
            state: PluginState::Installed,
            enabled: true,
            install_path: plugin.source.path().to_path_buf(),
            installed_at: plugin.installed_at,
            source: match &plugin.source {
                PluginSource::Builtin { .. } => "builtin",
                PluginSource::UserInstalled { .. } => "user",
                PluginSource::ProjectLocal { .. } => "project",
                PluginSource::Development { .. } => "development",
            }
            .to_string(),
            capabilities: caps.all().map(|c| format!("{:?}", c)).collect(),
            tags: manifest.plugin.tags.clone(),
            error: None,
        }
    }
}

/// Active plugin instance
#[allow(dead_code)]
struct ActivePlugin {
    info: PluginInfo,
    plugin: Box<dyn ExecutablePlugin>,
    capabilities: GrantedCapabilities,
    loaded_at: DateTime<Utc>,
}

/// Plugin manager - main interface for plugin operations
pub struct PluginManager {
    /// Configuration
    config: PluginConfig,
    /// Plugin registry
    registry: Arc<PluginRegistry>,
    /// Dependency resolver
    resolver: Arc<TokioRwLock<DependencyResolver>>,
    /// State store
    state_store: Arc<TokioRwLock<PluginStateStore>>,
    /// Plugin installer
    installer: Arc<PluginInstaller>,
    /// Plugin updater
    #[allow(dead_code)]
    updater: Arc<PluginUpdater>,
    /// Active plugins
    active_plugins: RwLock<HashMap<String, ActivePlugin>>,
    /// Plugin states
    states: RwLock<HashMap<String, PluginState>>,
    /// Capability enforcer
    #[allow(dead_code)]
    enforcer: Arc<CapabilityEnforcer>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub async fn new(config: PluginConfig) -> PluginResult<Self> {
        // Ensure directories exist
        let plugins_dir = config.data_dir.join(PLUGINS_DIR_NAME);
        std::fs::create_dir_all(&plugins_dir)?;

        // Load state store
        let state_path = config.data_dir.join("plugin-state.json");
        let state_store = PluginStateStore::load(&state_path)?;
        let state_store = Arc::new(TokioRwLock::new(state_store));

        // Create registry
        let mut registry = PluginRegistry::new();
        registry.add_scan_directory(plugins_dir.clone());
        let registry = Arc::new(registry);

        // Create resolver
        let resolver = Arc::new(TokioRwLock::new(DependencyResolver::new()));

        // Create installer
        let installer = Arc::new(PluginInstaller::new(
            plugins_dir,
            Arc::clone(&registry),
            Arc::clone(&resolver),
            Arc::clone(&state_store),
        ));

        // Create updater
        let updater = Arc::new(PluginUpdater::new(
            Arc::clone(&installer),
            Arc::clone(&state_store),
        ));

        let manager = Self {
            config,
            registry,
            resolver,
            state_store,
            installer,
            updater,
            active_plugins: RwLock::new(HashMap::new()),
            states: RwLock::new(HashMap::new()),
            enforcer: Arc::new(CapabilityEnforcer::new()),
        };

        // Scan for plugins
        manager.scan_plugins().await?;

        Ok(manager)
    }

    /// Scan for plugins in configured directories
    pub async fn scan_plugins(&self) -> PluginResult<Vec<PluginInfo>> {
        let discovered = self.registry.scan()?;

        // Add to resolver
        {
            let mut resolver = self.resolver.write().await;
            resolver.clear();
            for plugin in &discovered {
                resolver.add_available(plugin.manifest.clone());
            }
        }

        // Update states
        {
            let state = self.state_store.read().await;
            let mut states = self.states.write();

            for plugin in &discovered {
                let plugin_id = plugin.id();
                let plugin_state = if state.is_enabled(plugin_id) {
                    PluginState::Enabled
                } else {
                    PluginState::Disabled
                };
                states.insert(plugin_id.to_string(), plugin_state);
            }
        }

        Ok(discovered.iter().map(PluginInfo::from).collect())
    }

    /// List all plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        let plugins = self.registry.list();
        let states = self.states.read();

        plugins
            .iter()
            .map(|p| {
                let mut info = PluginInfo::from(p);
                if let Some(state) = states.get(&info.id) {
                    info.state = *state;
                    info.enabled = *state != PluginState::Disabled;
                }
                info
            })
            .collect()
    }

    /// Get a specific plugin
    pub fn get_plugin(&self, plugin_id: &str) -> Option<PluginInfo> {
        self.registry.get(plugin_id).map(|p| {
            let mut info = PluginInfo::from(&p);
            if let Some(state) = self.states.read().get(plugin_id) {
                info.state = *state;
                info.enabled = *state != PluginState::Disabled;
            }
            info
        })
    }

    /// List plugins by type
    pub fn list_by_type(&self, plugin_type: PluginType) -> Vec<PluginInfo> {
        self.registry
            .list_by_type(plugin_type)
            .iter()
            .map(PluginInfo::from)
            .collect()
    }

    /// Install a plugin from directory
    pub async fn install_from_directory(&self, path: &Path) -> PluginResult<InstallResult> {
        self.installer.install_from_directory(path).await
    }

    /// Install a plugin from file
    pub async fn install_from_file(&self, path: &Path) -> PluginResult<InstallResult> {
        self.installer.install_from_file(path).await
    }

    /// Install a development plugin
    pub async fn install_dev(&self, path: &Path) -> PluginResult<InstallResult> {
        self.installer.install_dev(path).await
    }

    /// Uninstall a plugin
    pub async fn uninstall(
        &self,
        plugin_id: &str,
        mode: UninstallMode,
        preserve_data: bool,
    ) -> PluginResult<UninstallResult> {
        // Disable first if active
        if self.states.read().get(plugin_id) == Some(&PluginState::Active) {
            self.disable(plugin_id).await?;
        }

        // Remove from resolver
        {
            let mut resolver = self.resolver.write().await;
            resolver.remove_available(plugin_id);
        }

        // Uninstall
        self.installer
            .uninstall(plugin_id, mode, preserve_data)
            .await
    }

    /// Enable a plugin
    pub async fn enable(&self, plugin_id: &str) -> PluginResult<()> {
        // Check if plugin exists
        let _plugin = self
            .registry
            .get(plugin_id)
            .ok_or_else(|| PluginError::NotInstalled(plugin_id.to_string()))?;

        // Update state
        {
            let mut state = self.state_store.write().await;
            state.set_enabled(plugin_id, true)?;
        }

        {
            let mut states = self.states.write();
            states.insert(plugin_id.to_string(), PluginState::Enabled);
        }

        tracing::info!("Enabled plugin: {}", plugin_id);
        Ok(())
    }

    /// Disable a plugin
    pub async fn disable(&self, plugin_id: &str) -> PluginResult<()> {
        // Unload if active
        {
            let mut active = self.active_plugins.write();
            if active.remove(plugin_id).is_some() {
                tracing::info!("Unloaded plugin: {}", plugin_id);
            }
        }

        // Update state
        {
            let mut state = self.state_store.write().await;
            state.set_enabled(plugin_id, false)?;
        }

        {
            let mut states = self.states.write();
            states.insert(plugin_id.to_string(), PluginState::Disabled);
        }

        tracing::info!("Disabled plugin: {}", plugin_id);
        Ok(())
    }

    /// Get plugin settings
    pub async fn get_settings(&self, plugin_id: &str) -> PluginResult<serde_json::Value> {
        let state = self.state_store.read().await;
        Ok(state
            .get_settings(plugin_id)
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new())))
    }

    /// Update plugin settings
    pub async fn update_settings(
        &self,
        plugin_id: &str,
        settings: serde_json::Value,
    ) -> PluginResult<()> {
        let mut state = self.state_store.write().await;
        state.set_settings(plugin_id, settings)
    }

    /// Search plugins
    pub fn search(&self, query: &str) -> Vec<PluginInfo> {
        self.registry
            .search(query)
            .iter()
            .map(PluginInfo::from)
            .collect()
    }

    /// Get plugins directory
    pub fn plugins_dir(&self) -> PathBuf {
        self.config.data_dir.join(PLUGINS_DIR_NAME)
    }

    /// Check if plugin is enabled
    pub async fn is_enabled(&self, plugin_id: &str) -> bool {
        let state = self.state_store.read().await;
        state.is_enabled(plugin_id)
    }

    /// Get active plugin count
    pub fn active_count(&self) -> usize {
        self.active_plugins.read().len()
    }

    /// Get total plugin count
    pub fn total_count(&self) -> usize {
        self.registry.count()
    }

    /// Get evaluator plugins
    pub fn get_evaluator_plugins(&self) -> Vec<PluginInfo> {
        self.list_by_type(PluginType::Evaluator)
    }

    /// Reload a plugin (for development)
    pub async fn reload(&self, plugin_id: &str) -> PluginResult<()> {
        let plugin = self
            .registry
            .get(plugin_id)
            .ok_or_else(|| PluginError::NotInstalled(plugin_id.to_string()))?;

        // Only allow reload for development plugins
        if !plugin.source.is_development() {
            return Err(PluginError::Other(
                "Can only reload development plugins".to_string(),
            ));
        }

        // Unload
        {
            let mut active = self.active_plugins.write();
            active.remove(plugin_id);
        }

        // Re-parse manifest and re-register
        let path = plugin.source.path();
        let manifest = PluginManifest::from_directory(path)?;

        let _ = self.registry.unregister(plugin_id);
        let indexed = IndexedPlugin::new(
            manifest,
            PluginSource::Development {
                path: path.to_path_buf(),
                watch: true,
            },
        );
        self.registry.register(indexed)?;

        tracing::info!("Reloaded plugin: {}", plugin_id);
        Ok(())
    }

    /// Get plugin manifest
    pub fn get_manifest(&self, plugin_id: &str) -> PluginResult<PluginManifest> {
        self.registry
            .get(plugin_id)
            .map(|p| p.manifest.clone())
            .ok_or_else(|| PluginError::NotInstalled(plugin_id.to_string()))
    }

    /// Get plugin installation path
    pub fn get_plugin_path(&self, plugin_id: &str) -> PluginResult<PathBuf> {
        self.registry
            .get(plugin_id)
            .map(|p| p.source.path().to_path_buf())
            .ok_or_else(|| PluginError::NotInstalled(plugin_id.to_string()))
    }

    /// Get the data directory
    pub fn data_dir(&self) -> &Path {
        &self.config.data_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MANIFEST_FILENAME;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = PluginConfig {
            data_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let manager = PluginManager::new(config).await.unwrap();
        assert_eq!(manager.total_count(), 0);
    }

    #[tokio::test]
    async fn test_install_and_list() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("test-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = r#"
[plugin]
id = "test-plugin"
name = "Test Plugin"
version = "1.0.0"
type = "evaluator"
description = "A test plugin"

[capabilities]
read_traces = true
"#;
        std::fs::write(plugin_dir.join(MANIFEST_FILENAME), manifest).unwrap();

        let config = PluginConfig {
            data_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let manager = PluginManager::new(config).await.unwrap();
        manager.install_from_directory(&plugin_dir).await.unwrap();

        let plugins = manager.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "test-plugin");
    }

    #[tokio::test]
    async fn test_enable_disable() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("test-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = r#"
[plugin]
id = "test-plugin"
name = "Test Plugin"
version = "1.0.0"
type = "evaluator"
"#;
        std::fs::write(plugin_dir.join(MANIFEST_FILENAME), manifest).unwrap();

        let config = PluginConfig {
            data_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let manager = PluginManager::new(config).await.unwrap();
        manager.install_from_directory(&plugin_dir).await.unwrap();

        // Initially enabled
        assert!(manager.is_enabled("test-plugin").await);

        // Disable
        manager.disable("test-plugin").await.unwrap();
        assert!(!manager.is_enabled("test-plugin").await);

        // Enable
        manager.enable("test-plugin").await.unwrap();
        assert!(manager.is_enabled("test-plugin").await);
    }
}
