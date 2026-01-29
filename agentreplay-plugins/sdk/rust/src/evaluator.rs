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

//! Evaluator plugin interface
//!
//! Implement this trait to create an evaluator plugin.

use crate::types::{EvalResult, PluginMetadata, TraceContext};

/// Trait for evaluator plugins
///
/// Evaluators analyze traces and return evaluation results.
///
/// # Example
///
/// ```rust,ignore
/// use agentreplay_plugin_sdk::prelude::*;
///
/// #[derive(Default)]
/// struct LengthChecker;
///
/// impl Evaluator for LengthChecker {
///     fn evaluate(&self, trace: TraceContext) -> Result<EvalResult, String> {
///         let output_len = trace.output.as_ref().map(|s| s.len()).unwrap_or(0);
///         let passed = output_len >= 10 && output_len <= 5000;
///         
///         Ok(EvalResult {
///             evaluator_id: "length-checker".into(),
///             passed,
///             confidence: 1.0,
///             explanation: Some(format!("Output length: {} chars", output_len)),
///             ..Default::default()
///         })
///     }
///     
///     fn get_metadata(&self) -> PluginMetadata {
///         PluginMetadata {
///             id: "length-checker".into(),
///             name: "Output Length Checker".into(),
///             version: "1.0.0".into(),
///             description: "Checks if output length is within acceptable bounds".into(),
///             ..Default::default()
///         }
///     }
/// }
///
/// export_evaluator!(LengthChecker);
/// ```
pub trait Evaluator: Send + Sync {
    /// Evaluate a single trace
    ///
    /// Returns an EvalResult with pass/fail status, confidence score,
    /// and optional explanation and metrics.
    fn evaluate(&self, trace: TraceContext) -> Result<EvalResult, String>;

    /// Get plugin metadata
    ///
    /// Returns information about the plugin including ID, name, version,
    /// and description.
    fn get_metadata(&self) -> PluginMetadata;

    /// Evaluate multiple traces (batch)
    ///
    /// Default implementation calls evaluate() for each trace.
    /// Override for more efficient batch processing.
    fn evaluate_batch(&self, traces: Vec<TraceContext>) -> Result<Vec<EvalResult>, String> {
        traces.into_iter().map(|t| self.evaluate(t)).collect()
    }

    /// Get configuration schema (JSON Schema)
    ///
    /// Returns the JSON Schema for plugin configuration.
    /// Override to provide custom configuration options.
    fn get_config_schema(&self) -> Option<String> {
        None
    }
}
