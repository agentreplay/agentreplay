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

//! Plugin installer and uninstaller
//!
//! Handles plugin installation, uninstallation, and updates.

use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginManifest;
use crate::registry::{IndexedPlugin, PluginRegistry, PluginSource};
use crate::resolver::DependencyResolver;
use crate::state::PluginStateStore;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Installation progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallProgress {
    pub plugin_id: String,
    pub state: InstallState,
    pub progress_percent: u8,
    pub current_step: String,
    pub errors: Vec<String>,
}

/// Installation state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallState {
    Queued,
    Validating,
    ResolvingDependencies,
    Copying,
    Installing,
    Configuring,
    Activating,
    Complete,
    Failed,
    Cancelled,
}

/// Installation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub plugin_id: String,
    pub version: String,
    pub install_path: PathBuf,
    pub installed_at: DateTime<Utc>,
    pub dependencies_installed: Vec<String>,
}

/// Uninstallation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallResult {
    pub plugin_id: String,
    pub removed_files: usize,
    pub data_preserved: bool,
    pub broken_dependents: Vec<String>,
}

/// Uninstall mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UninstallMode {
    /// Remove plugin only if no dependents
    Safe,
    /// Remove plugin and all dependents
    Cascade,
    /// Remove plugin, mark dependents as broken
    Force,
}

/// Plugin installer
pub struct PluginInstaller {
    /// Base directory for installed plugins
    plugins_dir: PathBuf,
    /// Plugin registry
    registry: Arc<PluginRegistry>,
    /// Dependency resolver
    resolver: Arc<RwLock<DependencyResolver>>,
    /// State store
    state_store: Arc<RwLock<PluginStateStore>>,
    /// Active installations
    active_installations: RwLock<std::collections::HashMap<String, InstallProgress>>,
}

impl PluginInstaller {
    /// Create a new installer
    pub fn new(
        plugins_dir: PathBuf,
        registry: Arc<PluginRegistry>,
        resolver: Arc<RwLock<DependencyResolver>>,
        state_store: Arc<RwLock<PluginStateStore>>,
    ) -> Self {
        Self {
            plugins_dir,
            registry,
            resolver,
            state_store,
            active_installations: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Get the plugins directory
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Install from a local directory
    pub async fn install_from_directory(&self, source_dir: &Path) -> PluginResult<InstallResult> {
        // Parse manifest
        let manifest = PluginManifest::from_directory(source_dir)?;
        let plugin_id = manifest.plugin.id.clone();
        let version = manifest.plugin.version.clone();

        // Check if already installed
        if self.registry.contains(&plugin_id) {
            return Err(PluginError::AlreadyInstalled(plugin_id));
        }

        // Update progress
        self.update_progress(
            &plugin_id,
            InstallState::Validating,
            10,
            "Validating manifest",
        )
        .await;

        // Validate manifest
        manifest.validate()?;

        // Check dependencies
        self.update_progress(
            &plugin_id,
            InstallState::ResolvingDependencies,
            30,
            "Resolving dependencies",
        )
        .await;

        {
            let mut resolver = self.resolver.write().await;
            resolver.add_available(manifest.clone());
            resolver.resolve(&[plugin_id.clone()])?;
        }

        // Create install directory
        let install_path = self.plugins_dir.join(&plugin_id);
        self.update_progress(&plugin_id, InstallState::Copying, 50, "Copying files")
            .await;

        if install_path.exists() {
            std::fs::remove_dir_all(&install_path)?;
        }
        self.copy_directory(source_dir, &install_path)?;

        // Register plugin
        self.update_progress(
            &plugin_id,
            InstallState::Installing,
            70,
            "Registering plugin",
        )
        .await;

        let indexed = IndexedPlugin::new(
            manifest,
            PluginSource::UserInstalled {
                path: install_path.clone(),
            },
        );
        self.registry.register(indexed)?;

        // Update state store
        self.update_progress(
            &plugin_id,
            InstallState::Configuring,
            90,
            "Saving configuration",
        )
        .await;

        {
            let mut state = self.state_store.write().await;
            state.record_installation(&plugin_id)?;
        }

        // Complete
        self.update_progress(
            &plugin_id,
            InstallState::Complete,
            100,
            "Installation complete",
        )
        .await;

        Ok(InstallResult {
            plugin_id,
            version,
            install_path,
            installed_at: Utc::now(),
            dependencies_installed: Vec::new(),
        })
    }

    /// Install from a file (archive)
    pub async fn install_from_file(&self, archive_path: &Path) -> PluginResult<InstallResult> {
        // For now, assume it's a directory
        // In a full implementation, we'd extract the archive first
        if archive_path.is_dir() {
            return self.install_from_directory(archive_path).await;
        }

        Err(PluginError::InstallationFailed(
            "Archive installation not yet implemented".to_string(),
        ))
    }

    /// Create a development plugin (symlink for hot reload)
    pub async fn install_dev(&self, source_dir: &Path) -> PluginResult<InstallResult> {
        let manifest = PluginManifest::from_directory(source_dir)?;
        let plugin_id = manifest.plugin.id.clone();
        let version = manifest.plugin.version.clone();

        // Create symlink to source directory
        let install_path = self.plugins_dir.join(&plugin_id);

        if install_path.exists() {
            if install_path.is_symlink() {
                std::fs::remove_file(&install_path)?;
            } else {
                std::fs::remove_dir_all(&install_path)?;
            }
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(source_dir, &install_path)?;

        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(source_dir, &install_path)?;

        // Register as development plugin
        let indexed = IndexedPlugin::new(
            manifest,
            PluginSource::Development {
                path: source_dir.to_path_buf(),
                watch: true,
            },
        );

        // Unregister if exists, then register
        let _ = self.registry.unregister(&plugin_id);
        self.registry.register(indexed)?;

        {
            let mut state = self.state_store.write().await;
            state.record_installation(&plugin_id)?;
        }

        Ok(InstallResult {
            plugin_id,
            version,
            install_path,
            installed_at: Utc::now(),
            dependencies_installed: Vec::new(),
        })
    }

    /// Uninstall a plugin
    pub async fn uninstall(
        &self,
        plugin_id: &str,
        mode: UninstallMode,
        preserve_data: bool,
    ) -> PluginResult<UninstallResult> {
        // Check if plugin exists
        let plugin = self
            .registry
            .get(plugin_id)
            .ok_or_else(|| PluginError::NotInstalled(plugin_id.to_string()))?;

        // Check dependents
        let dependents = {
            let resolver = self.resolver.read().await;
            resolver.reverse_dependencies(plugin_id)
        };

        if !dependents.is_empty() {
            match mode {
                UninstallMode::Safe => {
                    return Err(PluginError::HasDependents(dependents));
                }
                UninstallMode::Cascade => {
                    // Uninstall dependents first
                    for dep_id in &dependents {
                        Box::pin(self.uninstall(dep_id, UninstallMode::Cascade, preserve_data))
                            .await?;
                    }
                }
                UninstallMode::Force => {
                    // Just proceed, dependents will be broken
                }
            }
        }

        // Get install path
        let install_path = plugin.source.path().to_path_buf();

        // Unregister from registry
        self.registry.unregister(plugin_id)?;

        // Remove from resolver
        {
            let mut resolver = self.resolver.write().await;
            resolver.remove_available(plugin_id);
        }

        // Remove files
        let mut removed_files = 0;
        if install_path.exists() {
            if install_path.is_symlink() {
                std::fs::remove_file(&install_path)?;
                removed_files = 1;
            } else if !preserve_data {
                removed_files = count_files(&install_path);
                std::fs::remove_dir_all(&install_path)?;
            }
        }

        // Update state store
        {
            let mut state = self.state_store.write().await;
            state.record_uninstallation(plugin_id)?;
        }

        Ok(UninstallResult {
            plugin_id: plugin_id.to_string(),
            removed_files,
            data_preserved: preserve_data,
            broken_dependents: if matches!(mode, UninstallMode::Force) {
                dependents
            } else {
                Vec::new()
            },
        })
    }

    /// Get installation progress
    pub async fn get_progress(&self, plugin_id: &str) -> Option<InstallProgress> {
        self.active_installations
            .read()
            .await
            .get(plugin_id)
            .cloned()
    }

    /// Update installation progress
    async fn update_progress(&self, plugin_id: &str, state: InstallState, percent: u8, step: &str) {
        let mut installations = self.active_installations.write().await;
        let progress = installations
            .entry(plugin_id.to_string())
            .or_insert_with(|| InstallProgress {
                plugin_id: plugin_id.to_string(),
                state: InstallState::Queued,
                progress_percent: 0,
                current_step: String::new(),
                errors: Vec::new(),
            });
        progress.state = state;
        progress.progress_percent = percent;
        progress.current_step = step.to_string();
    }

    /// Copy directory recursively
    fn copy_directory(&self, src: &Path, dst: &Path) -> PluginResult<()> {
        std::fs::create_dir_all(dst)?;

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                self.copy_directory(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }
}

/// Count files in a directory
fn count_files(path: &Path) -> usize {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count()
}

/// Available update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableUpdate {
    pub plugin_id: String,
    pub current_version: String,
    pub available_version: String,
    pub changelog: Option<String>,
    pub breaking_changes: bool,
    pub size_bytes: Option<u64>,
}

/// Plugin update manager
pub struct PluginUpdater {
    installer: Arc<PluginInstaller>,
    state_store: Arc<RwLock<PluginStateStore>>,
}

impl PluginUpdater {
    /// Create a new updater
    pub fn new(
        installer: Arc<PluginInstaller>,
        state_store: Arc<RwLock<PluginStateStore>>,
    ) -> Self {
        Self {
            installer,
            state_store,
        }
    }

    /// Check for updates (stub - would connect to update server in production)
    pub async fn check_updates(&self) -> Vec<AvailableUpdate> {
        // In a real implementation, this would query an update server
        Vec::new()
    }

    /// Apply an update
    pub async fn apply_update(
        &self,
        update: &AvailableUpdate,
        source_path: &Path,
    ) -> PluginResult<()> {
        // Record previous version for rollback
        {
            let mut state = self.state_store.write().await;
            state.record_previous_version(&update.plugin_id, &update.current_version)?;
        }

        // Uninstall old version
        self.installer
            .uninstall(&update.plugin_id, UninstallMode::Force, true)
            .await?;

        // Install new version
        self.installer.install_from_directory(source_path).await?;

        Ok(())
    }

    /// Rollback to previous version
    pub async fn rollback(&self, plugin_id: &str) -> PluginResult<()> {
        let previous_version = {
            let state = self.state_store.read().await;
            state
                .get_previous_version(plugin_id)
                .map(|v| v.to_string())
                .ok_or_else(|| {
                    PluginError::RollbackFailed("No previous version available".to_string())
                })?
        };

        tracing::info!("Rolling back {} to version {}", plugin_id, previous_version);

        // In a real implementation, we'd restore from backup
        Err(PluginError::RollbackFailed(
            "Rollback from backup not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MANIFEST_FILENAME;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_install_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        let source_dir = temp_dir.path().join("source");

        std::fs::create_dir_all(&source_dir).unwrap();

        // Create manifest
        let manifest_content = r#"
[plugin]
id = "test-plugin"
name = "Test Plugin"
version = "1.0.0"
type = "evaluator"

[capabilities]
read_traces = true
"#;
        std::fs::write(source_dir.join(MANIFEST_FILENAME), manifest_content).unwrap();

        // Setup installer
        let registry = Arc::new(PluginRegistry::new());
        let resolver = Arc::new(RwLock::new(DependencyResolver::new()));
        let state_store = Arc::new(RwLock::new(PluginStateStore::new()));

        let installer =
            PluginInstaller::new(plugins_dir.clone(), registry.clone(), resolver, state_store);

        // Install
        let result = installer.install_from_directory(&source_dir).await.unwrap();

        assert_eq!(result.plugin_id, "test-plugin");
        assert_eq!(result.version, "1.0.0");
        assert!(result.install_path.exists());
        assert!(registry.contains("test-plugin"));
    }

    #[tokio::test]
    async fn test_uninstall() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        let source_dir = temp_dir.path().join("source");

        std::fs::create_dir_all(&source_dir).unwrap();

        let manifest_content = r#"
[plugin]
id = "test-plugin"
name = "Test Plugin"
version = "1.0.0"
type = "evaluator"
"#;
        std::fs::write(source_dir.join(MANIFEST_FILENAME), manifest_content).unwrap();

        let registry = Arc::new(PluginRegistry::new());
        let resolver = Arc::new(RwLock::new(DependencyResolver::new()));
        let state_store = Arc::new(RwLock::new(PluginStateStore::new()));

        let installer =
            PluginInstaller::new(plugins_dir.clone(), registry.clone(), resolver, state_store);

        // Install then uninstall
        installer.install_from_directory(&source_dir).await.unwrap();
        assert!(registry.contains("test-plugin"));

        let result = installer
            .uninstall("test-plugin", UninstallMode::Safe, false)
            .await
            .unwrap();
        assert_eq!(result.plugin_id, "test-plugin");
        assert!(!registry.contains("test-plugin"));
    }
}
