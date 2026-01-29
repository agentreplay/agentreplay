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

//! Exporter plugin interface
//!
//! Implement this trait to create an exporter plugin.

use crate::types::{PluginMetadata, TraceContext};

/// Trait for exporter plugins
///
/// Exporters convert traces to external formats.
///
/// # Example
///
/// ```rust,ignore
/// use agentreplay_plugin_sdk::prelude::*;
///
/// #[derive(Default)]
/// struct JsonExporter;
///
/// impl Exporter for JsonExporter {
///     fn export(
///         &self,
///         traces: Vec<TraceContext>,
///         format: &str,
///         options: &str
///     ) -> Result<Vec<u8>, String> {
///         match format {
///             "json" => {
///                 serde_json::to_vec(&traces).map_err(|e| e.to_string())
///             }
///             "jsonl" => {
///                 let lines: Vec<String> = traces.iter()
///                     .map(|t| serde_json::to_string(t).unwrap())
///                     .collect();
///                 Ok(lines.join("\n").into_bytes())
///             }
///             _ => Err(format!("Unsupported format: {}", format))
///         }
///     }
///     
///     fn supported_formats(&self) -> Vec<String> {
///         vec!["json".into(), "jsonl".into()]
///     }
///     
///     fn get_metadata(&self) -> PluginMetadata {
///         PluginMetadata {
///             id: "json-exporter".into(),
///             name: "JSON Exporter".into(),
///             version: "1.0.0".into(),
///             description: "Exports traces to JSON and JSONL formats".into(),
///             ..Default::default()
///         }
///     }
/// }
/// ```
pub trait Exporter: Send + Sync {
    /// Export traces to the specified format
    ///
    /// # Arguments
    /// * `traces` - The traces to export
    /// * `format` - The output format (must be one from supported_formats)
    /// * `options` - JSON string with format-specific options
    ///
    /// # Returns
    /// The exported data as bytes
    fn export(
        &self,
        traces: Vec<TraceContext>,
        format: &str,
        options: &str,
    ) -> Result<Vec<u8>, String>;

    /// Get list of supported export formats
    fn supported_formats(&self) -> Vec<String>;

    /// Get plugin metadata
    fn get_metadata(&self) -> PluginMetadata;
}
