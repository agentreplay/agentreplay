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

//! WASM Plugin Executor
//!
//! The core runtime that loads and executes WASM plugins using wasmtime.
//! Supports the WASM Component Model for polyglot plugin support.

use super::component::LoadedPlugin;
use super::host_functions::PluginHostState;
use crate::capabilities::GrantedCapabilities;
use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginManifest;
use std::path::PathBuf;
use wasmtime::component::{Component, Linker as ComponentLinker};
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

/// Configuration for the WASM runtime
#[derive(Clone, Debug)]
pub struct WasmRuntimeConfig {
    /// Maximum memory per plugin (default: 256MB)
    pub max_memory_bytes: u64,

    /// Maximum fuel (instruction count) per call
    pub max_fuel: u64,

    /// Enable async execution
    pub async_support: bool,

    /// Enable WASI support
    pub wasi_enabled: bool,

    /// Debug mode (verbose errors, no limits)
    pub debug_mode: bool,

    /// Allowed directories for WASI filesystem access
    pub allowed_dirs: Vec<PathBuf>,
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 256 * 1024 * 1024, // 256MB
            max_fuel: 10_000_000_000,            // ~10 seconds of execution
            async_support: true,
            wasi_enabled: true,
            debug_mode: false,
            allowed_dirs: vec![],
        }
    }
}

/// Universal WASM plugin executor
///
/// Loads and executes any plugin compiled to WASM, regardless of source language.
/// Uses the WASM Component Model for better interoperability.
pub struct WasmExecutor {
    engine: Engine,
    config: WasmRuntimeConfig,
}

impl WasmExecutor {
    /// Create a new WASM executor with the given configuration
    pub fn new(config: WasmRuntimeConfig) -> PluginResult<Self> {
        let mut engine_config = Config::new();

        // Enable fuel consumption tracking for resource limits
        engine_config.consume_fuel(true);

        // Enable async if configured
        if config.async_support {
            engine_config.async_support(true);
        }

        // Enable WASM Component Model
        engine_config.wasm_component_model(true);

        let engine = Engine::new(&engine_config)
            .map_err(|e| PluginError::LoadFailed(format!("Failed to create WASM engine: {}", e)))?;

        Ok(Self { engine, config })
    }

    /// Load a plugin from WASM bytes
    pub async fn load_plugin(
        &self,
        plugin_id: String,
        wasm_bytes: &[u8],
        manifest: PluginManifest,
        capabilities: GrantedCapabilities,
        config_json: serde_json::Value,
    ) -> PluginResult<LoadedPlugin> {
        // Compile the component
        let component = Component::new(&self.engine, wasm_bytes).map_err(|e| {
            PluginError::LoadFailed(format!("Failed to compile WASM component: {}", e))
        })?;

        // Create component linker
        let mut linker: ComponentLinker<PluginHostState> = ComponentLinker::new(&self.engine);

        // Add WASI if enabled
        if self.config.wasi_enabled {
            wasmtime_wasi::add_to_linker_async(&mut linker).map_err(|e| {
                PluginError::LoadFailed(format!("Failed to add WASI to linker: {}", e))
            })?;
        }

        // Create store with plugin state
        let host_state =
            self.create_host_state(plugin_id.clone(), capabilities.clone(), config_json)?;

        let mut store = Store::new(&self.engine, host_state);

        // Set resource limits
        if !self.config.debug_mode {
            store
                .set_fuel(self.config.max_fuel)
                .map_err(|e| PluginError::LoadFailed(format!("Failed to set fuel: {}", e)))?;
        }

        // Instantiate the component
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .map_err(|e| {
                PluginError::LoadFailed(format!("Failed to instantiate component: {}", e))
            })?;

        Ok(LoadedPlugin::new(
            plugin_id,
            manifest,
            capabilities,
            store,
            instance,
            component,
        ))
    }

    /// Load plugin from a file path
    pub async fn load_plugin_from_file(
        &self,
        plugin_id: String,
        wasm_path: &std::path::Path,
        manifest: PluginManifest,
        capabilities: GrantedCapabilities,
        config_json: serde_json::Value,
    ) -> PluginResult<LoadedPlugin> {
        let wasm_bytes = std::fs::read(wasm_path)?;

        self.load_plugin(plugin_id, &wasm_bytes, manifest, capabilities, config_json)
            .await
    }

    /// Create the host state for a plugin instance
    fn create_host_state(
        &self,
        plugin_id: String,
        capabilities: GrantedCapabilities,
        config_json: serde_json::Value,
    ) -> PluginResult<PluginHostState> {
        // Build WASI context
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build();

        let config_str = serde_json::to_string(&config_json).unwrap_or_default();
        Ok(PluginHostState::new(
            plugin_id,
            capabilities,
            config_str,
            wasi_ctx,
        ))
    }

    /// Get the engine reference
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WasmRuntimeConfig::default();
        assert_eq!(config.max_memory_bytes, 256 * 1024 * 1024);
        assert!(config.async_support);
        assert!(config.wasi_enabled);
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let config = WasmRuntimeConfig::default();
        let executor = WasmExecutor::new(config);
        assert!(executor.is_ok());
    }
}
