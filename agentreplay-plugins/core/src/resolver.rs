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

//! Dependency resolver with topological sort
//!
//! Resolves plugin dependencies using Kahn's algorithm.

use crate::error::{PluginError, PluginResult};
use crate::manifest::{DependencySpec, PluginManifest};
use semver::{Version, VersionReq};
use std::collections::{HashMap, HashSet, VecDeque};

/// Resolved plugin with dependency information
#[derive(Debug, Clone)]
pub struct ResolvedPlugin {
    /// Plugin manifest
    pub manifest: PluginManifest,
    /// Resolved dependencies
    pub resolved_deps: Vec<String>,
    /// Load order index (lower = load first)
    pub load_order: usize,
}

/// Dependency resolver
pub struct DependencyResolver {
    /// Available plugins by ID
    available: HashMap<String, PluginManifest>,
}

impl DependencyResolver {
    /// Create a new resolver
    pub fn new() -> Self {
        Self {
            available: HashMap::new(),
        }
    }

    /// Add an available plugin
    pub fn add_available(&mut self, manifest: PluginManifest) {
        self.available.insert(manifest.plugin.id.clone(), manifest);
    }

    /// Remove an available plugin
    pub fn remove_available(&mut self, plugin_id: &str) {
        self.available.remove(plugin_id);
    }

    /// Clear all available plugins
    pub fn clear(&mut self) {
        self.available.clear();
    }

    /// Resolve dependencies for a set of plugins to be loaded
    pub fn resolve(&self, plugin_ids: &[String]) -> PluginResult<Vec<ResolvedPlugin>> {
        // Build dependency graph
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize
        for id in plugin_ids {
            graph.entry(id.clone()).or_default();
            in_degree.entry(id.clone()).or_insert(0);
        }

        // Build edges
        for id in plugin_ids {
            if let Some(manifest) = self.available.get(id) {
                for (dep_id, spec) in &manifest.dependencies {
                    // Check if dependency is available
                    if !self.available.contains_key(dep_id) {
                        // Check if it's optional
                        if let DependencySpec::Detailed(d) = spec {
                            if d.optional {
                                continue;
                            }
                        }
                        return Err(PluginError::DependencyNotFound(dep_id.clone()));
                    }

                    // Verify version compatibility
                    self.check_version_compatibility(dep_id, spec)?;

                    // Add edge: dep_id -> id (dep must load before id)
                    graph.entry(dep_id.clone()).or_default().push(id.clone());
                    *in_degree.entry(id.clone()).or_insert(0) += 1;

                    // Ensure dependency is in the graph
                    graph.entry(dep_id.clone()).or_default();
                    in_degree.entry(dep_id.clone()).or_insert(0);
                }
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut result: Vec<String> = Vec::new();

        // Start with nodes that have no dependencies
        for (id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(id.clone());
            }
        }

        while let Some(id) = queue.pop_front() {
            result.push(id.clone());

            if let Some(dependents) = graph.get(&id) {
                for dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != in_degree.len() {
            let remaining: Vec<_> = in_degree
                .keys()
                .filter(|id| !result.contains(id))
                .cloned()
                .collect();
            return Err(PluginError::DependencyCycle(remaining.join(" -> ")));
        }

        // Build resolved plugins
        let mut resolved = Vec::new();
        for (order, id) in result.iter().enumerate() {
            if let Some(manifest) = self.available.get(id) {
                let deps: Vec<String> = manifest
                    .dependencies
                    .keys()
                    .filter(|d| self.available.contains_key(*d))
                    .cloned()
                    .collect();

                resolved.push(ResolvedPlugin {
                    manifest: manifest.clone(),
                    resolved_deps: deps,
                    load_order: order,
                });
            }
        }

        Ok(resolved)
    }

    /// Check version compatibility
    fn check_version_compatibility(&self, dep_id: &str, spec: &DependencySpec) -> PluginResult<()> {
        let version_req = match spec {
            DependencySpec::Version(v) => VersionReq::parse(v)?,
            DependencySpec::Detailed(d) => VersionReq::parse(&d.version)?,
        };

        if let Some(manifest) = self.available.get(dep_id) {
            let version = Version::parse(&manifest.plugin.version)?;
            if !version_req.matches(&version) {
                return Err(PluginError::VersionConflict(format!(
                    "Plugin '{}' requires {} {}, but {} is available",
                    dep_id, dep_id, version_req, version
                )));
            }
        }

        Ok(())
    }

    /// Get reverse dependencies (plugins that depend on this one)
    pub fn reverse_dependencies(&self, plugin_id: &str) -> Vec<String> {
        self.available
            .iter()
            .filter(|(id, manifest)| {
                *id != plugin_id && manifest.dependencies.contains_key(plugin_id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check if removing a plugin would break others
    pub fn check_removal(&self, plugin_id: &str) -> Result<(), Vec<String>> {
        let dependents = self.reverse_dependencies(plugin_id);
        if dependents.is_empty() {
            Ok(())
        } else {
            Err(dependents)
        }
    }

    /// Get all transitive dependencies
    pub fn transitive_dependencies(&self, plugin_id: &str) -> HashSet<String> {
        let mut deps = HashSet::new();
        let mut to_process = vec![plugin_id.to_string()];

        while let Some(id) = to_process.pop() {
            if let Some(manifest) = self.available.get(&id) {
                for dep_id in manifest.dependencies.keys() {
                    if !deps.contains(dep_id) {
                        deps.insert(dep_id.clone());
                        to_process.push(dep_id.clone());
                    }
                }
            }
        }

        deps
    }
}

impl Default for DependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PluginMetadata, PluginType};

    fn create_manifest(id: &str, deps: HashMap<String, DependencySpec>) -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMetadata {
                id: id.to_string(),
                name: id.to_string(),
                version: "1.0.0".to_string(),
                description: "".to_string(),
                authors: vec![],
                license: None,
                repository: None,
                homepage: None,
                plugin_type: PluginType::Evaluator,
                min_agentreplay_version: "0.1.0".to_string(),
                tags: vec![],
                icon: None,
            },
            dependencies: deps,
            capabilities: Default::default(),
            entry: Default::default(),
            config: None,
            ui: None,
            bundle: None,
        }
    }

    #[test]
    fn test_simple_resolution() {
        let mut resolver = DependencyResolver::new();

        resolver.add_available(create_manifest("a", HashMap::new()));
        resolver.add_available(create_manifest("b", HashMap::new()));

        let resolved = resolver
            .resolve(&["a".to_string(), "b".to_string()])
            .unwrap();
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn test_dependency_order() {
        let mut resolver = DependencyResolver::new();

        // b depends on a
        resolver.add_available(create_manifest("a", HashMap::new()));

        let mut deps = HashMap::new();
        deps.insert(
            "a".to_string(),
            DependencySpec::Version(">=1.0.0".to_string()),
        );
        resolver.add_available(create_manifest("b", deps));

        let resolved = resolver
            .resolve(&["a".to_string(), "b".to_string()])
            .unwrap();

        // a should come before b
        let a_order = resolved
            .iter()
            .find(|p| p.manifest.plugin.id == "a")
            .unwrap()
            .load_order;
        let b_order = resolved
            .iter()
            .find(|p| p.manifest.plugin.id == "b")
            .unwrap()
            .load_order;
        assert!(a_order < b_order);
    }

    #[test]
    fn test_cycle_detection() {
        let mut resolver = DependencyResolver::new();

        // a depends on b, b depends on a
        let mut deps_a = HashMap::new();
        deps_a.insert(
            "b".to_string(),
            DependencySpec::Version(">=1.0.0".to_string()),
        );
        resolver.add_available(create_manifest("a", deps_a));

        let mut deps_b = HashMap::new();
        deps_b.insert(
            "a".to_string(),
            DependencySpec::Version(">=1.0.0".to_string()),
        );
        resolver.add_available(create_manifest("b", deps_b));

        let result = resolver.resolve(&["a".to_string(), "b".to_string()]);
        assert!(matches!(result, Err(PluginError::DependencyCycle(_))));
    }

    #[test]
    fn test_missing_dependency() {
        let mut resolver = DependencyResolver::new();

        let mut deps = HashMap::new();
        deps.insert(
            "nonexistent".to_string(),
            DependencySpec::Version(">=1.0.0".to_string()),
        );
        resolver.add_available(create_manifest("a", deps));

        let result = resolver.resolve(&["a".to_string()]);
        assert!(matches!(result, Err(PluginError::DependencyNotFound(_))));
    }
}
