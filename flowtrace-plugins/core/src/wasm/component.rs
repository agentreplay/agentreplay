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

//! WASM Component Types and Loaded Plugin
//!
//! Represents a loaded and instantiated WASM plugin component.

use super::host_functions::PluginHostState;
use crate::capabilities::GrantedCapabilities;
use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginManifest;
use wasmtime::component::{Component, Instance};
use wasmtime::Store;

/// Plugin type detection result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedPluginType {
    Evaluator,
    EmbeddingProvider,
    Exporter,
    Transformer,
    Multi(Vec<DetectedPluginType>),
    Unknown,
}

/// A loaded and ready-to-execute WASM plugin
#[allow(dead_code)]
pub struct LoadedPlugin {
    /// Plugin identifier
    id: String,

    /// Plugin manifest
    manifest: PluginManifest,

    /// Granted capabilities
    capabilities: GrantedCapabilities,

    /// Wasmtime store with plugin state
    store: Store<PluginHostState>,

    /// Instantiated component instance
    instance: Instance,

    /// Compiled component
    component: Component,
}

impl LoadedPlugin {
    /// Create a new loaded plugin
    pub fn new(
        id: String,
        manifest: PluginManifest,
        capabilities: GrantedCapabilities,
        store: Store<PluginHostState>,
        instance: Instance,
        component: Component,
    ) -> Self {
        Self {
            id,
            manifest,
            capabilities,
            store,
            instance,
            component,
        }
    }

    /// Get the plugin ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the plugin manifest
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Get granted capabilities
    pub fn capabilities(&self) -> &GrantedCapabilities {
        &self.capabilities
    }

    /// Get the store
    pub fn store(&self) -> &Store<PluginHostState> {
        &self.store
    }

    /// Get mutable store
    pub fn store_mut(&mut self) -> &mut Store<PluginHostState> {
        &mut self.store
    }

    /// Get the instance
    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    /// Detect what interfaces the plugin implements based on exports
    pub fn detect_type(&self) -> DetectedPluginType {
        // For component model, we'd inspect the component's exports
        // This is a simplified version - real implementation would
        // check the component's type exports
        use crate::manifest::PluginType;

        match self.manifest.plugin.plugin_type {
            PluginType::Evaluator => DetectedPluginType::Evaluator,
            PluginType::EmbeddingProvider => DetectedPluginType::EmbeddingProvider,
            PluginType::Exporter => DetectedPluginType::Exporter,
            _ => DetectedPluginType::Unknown,
        }
    }
}

/// Plugin instance wrapper for executing plugin functions
pub struct PluginInstance {
    plugin: LoadedPlugin,
}

impl PluginInstance {
    /// Create a new plugin instance
    pub fn new(plugin: LoadedPlugin) -> Self {
        Self { plugin }
    }

    /// Get plugin ID
    pub fn id(&self) -> &str {
        self.plugin.id()
    }

    /// Evaluate a trace (for evaluator plugins)
    pub async fn evaluate(&mut self, _trace_json: &str) -> PluginResult<EvalResult> {
        // For component model, we'd use wit-bindgen generated types
        // This is a placeholder implementation

        // Real implementation would:
        // 1. Use the generated bindings to call the evaluate export
        // 2. Pass the trace data through the component interface
        // 3. Return the result

        Err(PluginError::ExecutionError(
            "Evaluation not yet fully implemented - requires WIT bindings".to_string(),
        ))
    }

    /// Get plugin metadata
    pub fn get_metadata(&self) -> PluginResult<PluginMetadata> {
        // Extract metadata from manifest
        let manifest = &self.plugin.manifest;

        Ok(PluginMetadata {
            id: manifest.plugin.id.clone(),
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            description: manifest.plugin.description.clone(),
            author: manifest.plugin.authors.first().cloned(),
            tags: manifest.plugin.tags.clone(),
            cost_per_eval: None,
        })
    }

    /// Get the underlying plugin
    pub fn plugin(&self) -> &LoadedPlugin {
        &self.plugin
    }

    /// Get mutable access to the underlying plugin
    pub fn plugin_mut(&mut self) -> &mut LoadedPlugin {
        &mut self.plugin
    }
}

/// Evaluation result from a plugin
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvalResult {
    pub evaluator_id: String,
    pub passed: bool,
    pub confidence: f64,
    pub explanation: Option<String>,
    pub metrics: std::collections::HashMap<String, MetricValue>,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u32>,
}

/// Metric value variant
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
}

/// Plugin metadata from WIT
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub cost_per_eval: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detected_plugin_type() {
        assert_eq!(
            DetectedPluginType::Multi(vec![
                DetectedPluginType::Evaluator,
                DetectedPluginType::EmbeddingProvider
            ]),
            DetectedPluginType::Multi(vec![
                DetectedPluginType::Evaluator,
                DetectedPluginType::EmbeddingProvider
            ])
        );
    }
}
