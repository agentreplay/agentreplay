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

//! Native plugin execution
//!
//! Executes plugins as native dynamic libraries or scripts.

use crate::capabilities::GrantedCapabilities;
use crate::error::{PluginError, PluginResult};
use crate::manifest::{PluginManifest, PluginType};
use async_trait::async_trait;
use agentreplay_evals::{EvalResult, Evaluator, EvaluatorMetadata, TraceContext};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Trait for executable plugins
#[async_trait]
pub trait ExecutablePlugin: Send + Sync {
    /// Get plugin ID
    fn id(&self) -> &str;

    /// Get plugin manifest
    fn manifest(&self) -> &PluginManifest;

    /// Initialize the plugin
    async fn initialize(&mut self, config: serde_json::Value) -> PluginResult<()>;

    /// Shutdown the plugin
    async fn shutdown(&mut self) -> PluginResult<()>;

    /// Check if plugin is healthy
    fn is_healthy(&self) -> bool;
}

/// Script-based plugin executor
#[allow(dead_code)]
pub struct ScriptPlugin {
    manifest: PluginManifest,
    install_path: PathBuf,
    capabilities: GrantedCapabilities,
    config: serde_json::Value,
    initialized: bool,
}

impl ScriptPlugin {
    /// Create a new script plugin
    pub fn new(
        manifest: PluginManifest,
        install_path: PathBuf,
        capabilities: GrantedCapabilities,
    ) -> Self {
        Self {
            manifest,
            install_path,
            capabilities,
            config: serde_json::Value::Null,
            initialized: false,
        }
    }

    /// Execute a Python script
    #[cfg(feature = "python")]
    async fn execute_python(&self, script: &str, input: &str) -> PluginResult<String> {
        use tokio::process::Command;

        let script_path = self.install_path.join(script);
        let output = Command::new("python3")
            .arg(&script_path)
            .arg(input)
            .output()
            .await
            .map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(PluginError::ExecutionError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}

#[async_trait]
impl ExecutablePlugin for ScriptPlugin {
    fn id(&self) -> &str {
        &self.manifest.plugin.id
    }

    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn initialize(&mut self, config: serde_json::Value) -> PluginResult<()> {
        self.config = config;
        self.initialized = true;
        Ok(())
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        self.initialized = false;
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        self.initialized
    }
}

/// Built-in evaluator plugin adapter
/// Wraps a native evaluator as a plugin
pub struct BuiltinEvaluatorPlugin {
    manifest: PluginManifest,
    evaluator: Arc<dyn Evaluator>,
}

impl BuiltinEvaluatorPlugin {
    /// Create from an existing evaluator
    pub fn from_evaluator(evaluator: Arc<dyn Evaluator>) -> Self {
        let metadata = evaluator.metadata();

        let manifest = PluginManifest {
            schema_version: 1,
            plugin: crate::manifest::PluginMetadata {
                id: evaluator.id().to_string(),
                name: metadata.name.clone(),
                version: metadata.version.clone(),
                description: metadata.description.clone(),
                authors: metadata.author.map(|a| vec![a]).unwrap_or_default(),
                license: None,
                repository: None,
                homepage: None,
                plugin_type: PluginType::Evaluator,
                min_agentreplay_version: "0.1.0".to_string(),
                tags: metadata.tags.clone(),
                icon: None,
            },
            dependencies: HashMap::new(),
            bundle: None,
            capabilities: crate::manifest::CapabilityRequirements {
                read_traces: true,
                ..Default::default()
            },
            entry: Default::default(),
            config: None,
            ui: None,
        };

        Self {
            manifest,
            evaluator,
        }
    }

    /// Get the underlying evaluator
    pub fn evaluator(&self) -> Arc<dyn Evaluator> {
        Arc::clone(&self.evaluator)
    }
}

#[async_trait]
impl ExecutablePlugin for BuiltinEvaluatorPlugin {
    fn id(&self) -> &str {
        &self.manifest.plugin.id
    }

    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn initialize(&mut self, _config: serde_json::Value) -> PluginResult<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        true
    }
}

#[async_trait]
impl Evaluator for BuiltinEvaluatorPlugin {
    fn id(&self) -> &str {
        self.evaluator.id()
    }

    async fn evaluate(
        &self,
        trace: &TraceContext,
    ) -> Result<EvalResult, agentreplay_evals::EvalError> {
        self.evaluator.evaluate(trace).await
    }

    fn metadata(&self) -> EvaluatorMetadata {
        self.evaluator.metadata()
    }

    fn is_parallelizable(&self) -> bool {
        self.evaluator.is_parallelizable()
    }

    fn cost_per_eval(&self) -> Option<f64> {
        self.evaluator.cost_per_eval()
    }
}

/// JSON-based evaluator plugin
/// Evaluators defined via configuration files
pub struct JsonEvaluatorPlugin {
    id: String,
    manifest: PluginManifest,
    config: serde_json::Value,
    rules: Vec<EvaluationRule>,
}

/// A rule for JSON-based evaluation
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvaluationRule {
    pub name: String,
    pub condition: String,
    pub metric: String,
    pub weight: f64,
}

impl JsonEvaluatorPlugin {
    /// Create a new JSON evaluator
    pub fn new(
        id: String,
        manifest: PluginManifest,
        config: serde_json::Value,
    ) -> PluginResult<Self> {
        let rules = if let Some(rules_value) = config.get("rules") {
            serde_json::from_value(rules_value.clone())
                .map_err(|e| PluginError::InvalidManifest(format!("Invalid rules: {}", e)))?
        } else {
            Vec::new()
        };

        Ok(Self {
            id,
            manifest,
            config,
            rules,
        })
    }
}

#[async_trait]
impl ExecutablePlugin for JsonEvaluatorPlugin {
    fn id(&self) -> &str {
        &self.id
    }

    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn initialize(&mut self, config: serde_json::Value) -> PluginResult<()> {
        self.config = config;
        Ok(())
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        true
    }
}

#[async_trait]
impl Evaluator for JsonEvaluatorPlugin {
    fn id(&self) -> &str {
        &self.id
    }

    async fn evaluate(
        &self,
        trace: &TraceContext,
    ) -> Result<EvalResult, agentreplay_evals::EvalError> {
        let mut metrics = std::collections::HashMap::new();
        let mut passed = true;
        let mut explanations = Vec::new();

        // Simple rule-based evaluation
        for rule in &self.rules {
            let result = self.evaluate_rule(rule, trace);
            metrics.insert(
                rule.metric.clone(),
                agentreplay_evals::MetricValue::Float(result),
            );

            if result < 0.5 {
                passed = false;
                explanations.push(format!("Rule '{}' failed", rule.name));
            }
        }

        Ok(EvalResult {
            evaluator_id: self.id.clone(),
            evaluator_type: Some("rule-based".to_string()),
            metrics,
            passed,
            explanation: if explanations.is_empty() {
                Some("All rules passed".to_string())
            } else {
                Some(explanations.join("; "))
            },
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 0.9,
            cost: None,
            duration_ms: None,
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: self.manifest.plugin.name.clone(),
            version: self.manifest.plugin.version.clone(),
            description: self.manifest.plugin.description.clone(),
            cost_per_eval: None,
            avg_latency_ms: None,
            tags: self.manifest.plugin.tags.clone(),
            author: self.manifest.plugin.authors.first().cloned(),
        }
    }
}

impl JsonEvaluatorPlugin {
    fn evaluate_rule(&self, rule: &EvaluationRule, trace: &TraceContext) -> f64 {
        // Simple rule evaluation based on condition
        match rule.condition.as_str() {
            "has_output" => {
                if trace.output.is_some() {
                    1.0
                } else {
                    0.0
                }
            }
            "has_input" => {
                if trace.input.is_some() {
                    1.0
                } else {
                    0.0
                }
            }
            "not_empty_output" => trace
                .output
                .as_ref()
                .map(|o| if o.is_empty() { 0.0 } else { 1.0 })
                .unwrap_or(0.0),
            "output_length_reasonable" => trace
                .output
                .as_ref()
                .map(|o| {
                    let len = o.len();
                    if len > 10 && len < 10000 {
                        1.0
                    } else {
                        0.5
                    }
                })
                .unwrap_or(0.0),
            _ => 0.5, // Unknown condition
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PluginMetadata, PluginType};
    use std::collections::HashMap;

    fn create_test_manifest() -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMetadata {
                id: "test-evaluator".to_string(),
                name: "Test Evaluator".to_string(),
                version: "1.0.0".to_string(),
                description: "A test evaluator".to_string(),
                authors: vec![],
                license: None,
                repository: None,
                homepage: None,
                plugin_type: PluginType::Evaluator,
                min_agentreplay_version: "0.1.0".to_string(),
                tags: vec![],
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

    #[tokio::test]
    async fn test_json_evaluator() {
        let manifest = create_test_manifest();
        let config = serde_json::json!({
            "rules": [
                {
                    "name": "has_output",
                    "condition": "has_output",
                    "metric": "output_present",
                    "weight": 1.0
                }
            ]
        });

        let plugin =
            JsonEvaluatorPlugin::new("test-evaluator".to_string(), manifest, config).unwrap();

        let trace = TraceContext {
            trace_id: 1,
            edges: vec![],
            input: Some("test input".to_string()),
            output: Some("test output".to_string()),
            context: None,
            metadata: HashMap::new(),
            timestamp_us: 0,
            eval_trace: None,
        };

        let result = plugin.evaluate(&trace).await.unwrap();
        assert!(result.passed);
    }
}
