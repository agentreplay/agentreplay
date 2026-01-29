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

//! Plugin management commands for Tauri IPC
//!
//! Exposes plugin operations to the frontend.

use agentreplay_plugins::{
    InstallResult, PluginConfig, PluginInfo, PluginManager, UninstallMode, UninstallResult,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// Plugin manager state
pub struct PluginState {
    pub manager: Arc<RwLock<Option<PluginManager>>>,
}

impl PluginState {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(None)),
        }
    }

    #[allow(dead_code)]
    pub async fn init(&self, data_dir: PathBuf) -> Result<(), String> {
        Self::init_with_arc(&self.manager, data_dir).await
    }

    pub async fn init_with_arc(manager: &Arc<RwLock<Option<PluginManager>>>, data_dir: PathBuf) -> Result<(), String> {
        let config = PluginConfig {
            data_dir,
            ..Default::default()
        };
        let plugin_manager = PluginManager::new(config)
            .await
            .map_err(|e| e.to_string())?;
        
        let mut guard = manager.write().await;
        *guard = Some(plugin_manager);
        Ok(())
    }

    pub async fn with_manager<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&PluginManager) -> Result<R, String>,
    {
        let guard = self.manager.read().await;
        let manager = guard
            .as_ref()
            .ok_or_else(|| "Plugin manager not initialized".to_string())?;
        f(manager)
    }
}

impl Default for PluginState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginListResponse {
    pub plugins: Vec<PluginInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallSource {
    Directory { path: String },
    File { path: String },
    Dev { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRequest {
    pub source: InstallSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallRequest {
    pub plugin_id: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub preserve_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettingsRequest {
    pub plugin_id: String,
    pub settings: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStats {
    pub total: usize,
    pub active: usize,
    pub evaluators: usize,
}

// ============================================================================
// Plugin Commands
// ============================================================================

/// List all installed plugins
#[tauri::command]
pub async fn plugin_list(state: State<'_, PluginState>) -> Result<PluginListResponse, String> {
    state
        .with_manager(|manager| {
            let plugins = manager.list_plugins();
            Ok(PluginListResponse {
                total: plugins.len(),
                plugins,
            })
        })
        .await
}

/// Get a specific plugin by ID
#[tauri::command]
pub async fn plugin_get(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<Option<PluginInfo>, String> {
    state
        .with_manager(|manager| Ok(manager.get_plugin(&plugin_id)))
        .await
}

/// Install a plugin from a source
#[tauri::command]
pub async fn plugin_install(
    request: InstallRequest,
    state: State<'_, PluginState>,
) -> Result<InstallResult, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    match request.source {
        InstallSource::Directory { path } => {
            manager
                .install_from_directory(&PathBuf::from(path))
                .await
                .map_err(|e| e.to_string())
        }
        InstallSource::File { path } => {
            manager
                .install_from_file(&PathBuf::from(path))
                .await
                .map_err(|e| e.to_string())
        }
        InstallSource::Dev { path } => {
            manager
                .install_dev(&PathBuf::from(path))
                .await
                .map_err(|e| e.to_string())
        }
    }
}

/// Uninstall a plugin
#[tauri::command]
pub async fn plugin_uninstall(
    request: UninstallRequest,
    state: State<'_, PluginState>,
) -> Result<UninstallResult, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let mode = match request.mode.as_str() {
        "cascade" => UninstallMode::Cascade,
        "force" => UninstallMode::Force,
        _ => UninstallMode::Safe,
    };

    manager
        .uninstall(&request.plugin_id, mode, request.preserve_data)
        .await
        .map_err(|e| e.to_string())
}

/// Enable a plugin
#[tauri::command]
pub async fn plugin_enable(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<(), String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    manager.enable(&plugin_id).await.map_err(|e| e.to_string())
}

/// Disable a plugin
#[tauri::command]
pub async fn plugin_disable(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<(), String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    manager.disable(&plugin_id).await.map_err(|e| e.to_string())
}

/// Get plugin settings
#[tauri::command]
pub async fn plugin_get_settings(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<serde_json::Value, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    manager
        .get_settings(&plugin_id)
        .await
        .map_err(|e| e.to_string())
}

/// Update plugin settings
#[tauri::command]
pub async fn plugin_update_settings(
    request: PluginSettingsRequest,
    state: State<'_, PluginState>,
) -> Result<(), String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    manager
        .update_settings(&request.plugin_id, request.settings)
        .await
        .map_err(|e| e.to_string())
}

/// Search plugins
#[tauri::command]
pub async fn plugin_search(
    query: String,
    state: State<'_, PluginState>,
) -> Result<Vec<PluginInfo>, String> {
    state
        .with_manager(|manager| Ok(manager.search(&query)))
        .await
}

/// Reload a development plugin
#[tauri::command]
pub async fn plugin_reload(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<(), String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    manager.reload(&plugin_id).await.map_err(|e| e.to_string())
}

/// Scan for new plugins
#[tauri::command]
pub async fn plugin_scan(state: State<'_, PluginState>) -> Result<Vec<PluginInfo>, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    manager.scan_plugins().await.map_err(|e| e.to_string())
}

/// Get plugins directory path
#[tauri::command]
pub async fn plugin_get_dir(state: State<'_, PluginState>) -> Result<String, String> {
    state
        .with_manager(|manager| {
            Ok(manager.plugins_dir().to_string_lossy().to_string())
        })
        .await
}

/// Get plugin statistics
#[tauri::command]
pub async fn plugin_stats(state: State<'_, PluginState>) -> Result<PluginStats, String> {
    state
        .with_manager(|manager| {
            Ok(PluginStats {
                total: manager.total_count(),
                active: manager.active_count(),
                evaluators: manager.get_evaluator_plugins().len(),
            })
        })
        .await
}

// ============================================================================
// Bundle Plugin Commands (Schema v2+) - EXPERIMENTAL
// ============================================================================

use agentreplay_plugins::{
    bundle, BundleInfo, BundleTargetInfo, DetectionResult, InstallExecutionResult, InstallPlan,
    InstallReceipt, VariableContext, VariableInfo, VerifyResult,
};
use std::collections::HashMap as StdHashMap;

/// Request to get bundle info for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInfoRequest {
    pub plugin_id: String,
}

/// Request to detect bundle target availability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleDetectRequest {
    pub plugin_id: String,
    pub target_id: String,
    #[serde(default)]
    pub user_variables: StdHashMap<String, String>,
}

/// Request to get install instructions for a bundle target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInstallMdRequest {
    pub plugin_id: String,
    pub target_id: String,
}

/// Request to create an installation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInstallPlanRequest {
    pub plugin_id: String,
    pub target_id: String,
    pub scope: String, // "user", "project", "local", "enterprise"
    #[serde(default)]
    pub user_variables: StdHashMap<String, String>,
}

/// Request to execute a bundle installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleExecuteRequest {
    pub plugin_id: String,
    pub target_id: String,
    pub scope: String,
    #[serde(default)]
    pub user_variables: StdHashMap<String, String>,
}

/// Get bundle info for a plugin (if it has bundle configuration)
#[tauri::command]
pub async fn plugin_bundle_info(
    request: BundleInfoRequest,
    state: State<'_, PluginState>,
) -> Result<Option<BundleInfo>, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let plugin = manager
        .get_plugin(&request.plugin_id)
        .ok_or_else(|| format!("Plugin '{}' not found", request.plugin_id))?;

    // Get the manifest from the plugin
    let manifest = manager
        .get_manifest(&request.plugin_id)
        .map_err(|e| e.to_string())?;

    Ok(bundle::get_bundle_info(&manifest))
}

/// List all bundle targets for a plugin
#[tauri::command]
pub async fn plugin_bundle_targets(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<Vec<BundleTargetInfo>, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let manifest = manager
        .get_manifest(&plugin_id)
        .map_err(|e| e.to_string())?;

    let bundle_info = bundle::get_bundle_info(&manifest);
    Ok(bundle_info.map(|b| b.targets).unwrap_or_default())
}

/// Detect if a bundle target is available/installed
#[tauri::command]
pub async fn plugin_bundle_detect(
    request: BundleDetectRequest,
    state: State<'_, PluginState>,
) -> Result<DetectionResult, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let manifest = manager
        .get_manifest(&request.plugin_id)
        .map_err(|e| e.to_string())?;

    let target = manifest
        .get_bundle_target(&request.target_id)
        .ok_or_else(|| format!("Bundle target '{}' not found", request.target_id))?;

    let context = VariableContext::new().with_user_variables(request.user_variables);

    bundle::detect_target(target, &context).map_err(|e| e.to_string())
}

/// Get install instructions markdown for a bundle target
#[tauri::command]
pub async fn plugin_bundle_install_md(
    request: BundleInstallMdRequest,
    state: State<'_, PluginState>,
) -> Result<Option<String>, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let manifest = manager
        .get_manifest(&request.plugin_id)
        .map_err(|e| e.to_string())?;

    let plugin_path = manager.get_plugin_path(&request.plugin_id).map_err(|e| e.to_string())?;

    bundle::load_install_instructions(&manifest, &request.target_id, &plugin_path)
        .map_err(|e| e.to_string())
}

/// Get required variables for a bundle
#[tauri::command]
pub async fn plugin_bundle_variables(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<Vec<VariableInfo>, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let manifest = manager
        .get_manifest(&plugin_id)
        .map_err(|e| e.to_string())?;

    let bundle_info = bundle::get_bundle_info(&manifest);
    Ok(bundle_info.map(|b| b.required_variables).unwrap_or_default())
}

/// Create an installation plan (preview what will be installed)
#[tauri::command]
pub async fn plugin_bundle_plan(
    request: BundleInstallPlanRequest,
    state: State<'_, PluginState>,
) -> Result<InstallPlan, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let manifest = manager
        .get_manifest(&request.plugin_id)
        .map_err(|e| e.to_string())?;

    let plugin_path = manager.get_plugin_path(&request.plugin_id).map_err(|e| e.to_string())?;

    let scope = parse_install_scope(&request.scope)?;

    bundle::create_install_plan(
        &manifest,
        &request.target_id,
        scope,
        &plugin_path,
        request.user_variables,
    )
    .map_err(|e| e.to_string())
}

/// Execute a bundle installation
#[tauri::command]
pub async fn plugin_bundle_execute(
    request: BundleExecuteRequest,
    state: State<'_, PluginState>,
) -> Result<InstallExecutionResult, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let manifest = manager
        .get_manifest(&request.plugin_id)
        .map_err(|e| e.to_string())?;

    let plugin_path = manager.get_plugin_path(&request.plugin_id).map_err(|e| e.to_string())?;

    let scope = parse_install_scope(&request.scope)?;

    // Create the plan
    let plan = bundle::create_install_plan(
        &manifest,
        &request.target_id,
        scope,
        &plugin_path,
        request.user_variables,
    )
    .map_err(|e| e.to_string())?;

    // Execute the plan
    bundle::execute_install_plan(&plan)
        .await
        .map_err(|e| e.to_string())
}

/// Helper to parse install scope from string
fn parse_install_scope(scope: &str) -> Result<agentreplay_plugins::manifest::InstallScope, String> {
    use agentreplay_plugins::manifest::InstallScope;
    match scope.to_lowercase().as_str() {
        "user" => Ok(InstallScope::User),
        "project" => Ok(InstallScope::Project),
        "local" => Ok(InstallScope::Local),
        "enterprise" => Ok(InstallScope::Enterprise),
        _ => Err(format!("Invalid install scope: '{}'. Use: user, project, local, enterprise", scope)),
    }
}

/// Request to verify a bundle installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleVerifyRequest {
    pub plugin_id: String,
    pub target_id: String,
}

/// Verify a bundle installation against its receipt
#[tauri::command]
pub async fn plugin_bundle_verify(
    request: BundleVerifyRequest,
    state: State<'_, PluginState>,
) -> Result<VerifyResult, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    // Get the receipts directory
    let data_dir = manager.data_dir();
    let receipt_path = InstallReceipt::receipt_path(data_dir, &request.plugin_id, &request.target_id);

    if !receipt_path.exists() {
        return Err(format!(
            "No installation receipt found for plugin '{}' target '{}'",
            request.plugin_id, request.target_id
        ));
    }

    let receipt = InstallReceipt::load(&receipt_path).map_err(|e| e.to_string())?;
    Ok(bundle::verify_installation(&receipt))
}

/// Get installation receipt for a bundle target
#[tauri::command]
pub async fn plugin_bundle_receipt(
    request: BundleVerifyRequest,
    state: State<'_, PluginState>,
) -> Result<Option<InstallReceipt>, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let data_dir = manager.data_dir();
    let receipt_path = InstallReceipt::receipt_path(data_dir, &request.plugin_id, &request.target_id);

    if !receipt_path.exists() {
        return Ok(None);
    }

    let receipt = InstallReceipt::load(&receipt_path).map_err(|e| e.to_string())?;
    Ok(Some(receipt))
}

/// List all bundle target installations (receipts)
#[tauri::command]
pub async fn plugin_bundle_list_receipts(
    plugin_id: String,
    state: State<'_, PluginState>,
) -> Result<Vec<InstallReceipt>, String> {
    let guard = state.manager.read().await;
    let manager = guard
        .as_ref()
        .ok_or_else(|| "Plugin manager not initialized".to_string())?;

    let data_dir = manager.data_dir();
    let receipts_dir = data_dir.join("receipts");

    if !receipts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut receipts = Vec::new();
    let pattern = format!("{}_*.json", plugin_id);
    
    if let Ok(entries) = std::fs::read_dir(&receipts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with(&format!("{}_", plugin_id)) && name.ends_with(".json") {
                    if let Ok(receipt) = InstallReceipt::load(&path) {
                        receipts.push(receipt);
                    }
                }
            }
        }
    }

    Ok(receipts)
}
