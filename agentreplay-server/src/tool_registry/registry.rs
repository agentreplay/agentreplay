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

//! Tool Registry - Core registration and lookup
//!
//! ## Key Design (Issue 1 Fix from tauri.md review)
//!
//! Uses flattened DashMap with composite key `(namespace, name, version_str)` instead of:
//! ```ignore
//! DashMap<(String, String), BTreeMap<SemVer, Tool>>  // BAD: lock contention on BTreeMap
//! ```
//!
//! This approach:
//! - Provides true concurrent access to individual version entries
//! - Avoids holding locks while iterating BTreeMap for version resolution
//! - Uses separate index for latest version tracking

use dashmap::DashMap;
use agentreplay_core::{ToolRegistration, ToolVersion, UnifiedToolDefinition};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;

/// Registry errors
#[derive(Debug, Error)]
pub enum ToolRegistryError {
    #[error("Tool not found: {namespace}:{name}")]
    ToolNotFound { namespace: String, name: String },

    #[error("Tool version not found: {namespace}:{name}@{version}")]
    VersionNotFound {
        namespace: String,
        name: String,
        version: String,
    },

    #[error("No version satisfies constraint: {namespace}:{name} @ {constraint}")]
    NoMatchingVersion {
        namespace: String,
        name: String,
        constraint: String,
    },

    #[error("Tool already exists: {tool_id}")]
    AlreadyExists { tool_id: String },

    #[error("Invalid tool definition: {reason}")]
    InvalidDefinition { reason: String },

    #[error("Namespace not found: {namespace}")]
    NamespaceNotFound { namespace: String },
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistryConfig {
    /// Default namespace for tools without explicit namespace
    pub default_namespace: String,
    /// Maximum versions to keep per tool (0 = unlimited)
    pub max_versions_per_tool: usize,
    /// Whether to auto-enable newly registered tools
    pub auto_enable_new_tools: bool,
    /// Allowed namespaces (empty = all allowed)
    pub allowed_namespaces: HashSet<String>,
}

impl Default for ToolRegistryConfig {
    fn default() -> Self {
        Self {
            default_namespace: "default".to_string(),
            max_versions_per_tool: 10,
            auto_enable_new_tools: true,
            allowed_namespaces: HashSet::new(),
        }
    }
}

/// Composite key for tool storage: (namespace, name, version_string)
type ToolKey = (String, String, String);

/// Latest version key: (namespace, name)
type LatestKey = (String, String);

/// Result from tool lookup
#[derive(Debug, Clone)]
pub struct ToolLookupResult {
    pub tool: UnifiedToolDefinition,
    pub is_latest: bool,
    pub available_versions: Vec<ToolVersion>,
}

/// Unified Tool Registry
///
/// Thread-safe registry for tool definitions with semantic versioning support.
/// Uses flattened DashMap for concurrent access without lock contention.
pub struct ToolRegistry {
    /// Tool storage: (namespace, name, version) -> Tool
    /// Using flattened key to avoid nested lock contention (Issue 1 fix)
    tools: DashMap<ToolKey, UnifiedToolDefinition>,

    /// Latest version index: (namespace, name) -> latest ToolVersion
    /// Separate from tools map for O(1) latest lookups
    latest_versions: DashMap<LatestKey, ToolVersion>,

    /// All versions for a tool: (namespace, name) -> sorted versions
    /// Protected by RwLock for atomic version list updates
    version_index: RwLock<HashMap<LatestKey, BTreeMap<ToolVersion, ()>>>,

    /// Registry configuration
    config: ToolRegistryConfig,

    /// Metrics
    metrics: RegistryMetrics,
}

/// Registry metrics
#[derive(Debug, Default)]
struct RegistryMetrics {
    total_registrations: std::sync::atomic::AtomicU64,
    total_lookups: std::sync::atomic::AtomicU64,
    cache_hits: std::sync::atomic::AtomicU64,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new(config: ToolRegistryConfig) -> Self {
        Self {
            tools: DashMap::new(),
            latest_versions: DashMap::new(),
            version_index: RwLock::new(HashMap::new()),
            config,
            metrics: RegistryMetrics::default(),
        }
    }

    /// Register a new tool definition
    pub fn register(
        &self,
        tool: UnifiedToolDefinition,
    ) -> Result<ToolRegistration, ToolRegistryError> {
        // Validate namespace
        if !self.config.allowed_namespaces.is_empty()
            && !self.config.allowed_namespaces.contains(&tool.namespace)
        {
            return Err(ToolRegistryError::NamespaceNotFound {
                namespace: tool.namespace.clone(),
            });
        }

        let key: ToolKey = (
            tool.namespace.clone(),
            tool.name.clone(),
            tool.version.to_string(),
        );
        let latest_key: LatestKey = (tool.namespace.clone(), tool.name.clone());

        // Check if this exact version already exists
        let is_update = self.tools.contains_key(&key);

        // Insert/update the tool
        self.tools.insert(key, tool.clone());

        // Update version index
        {
            let mut index = self.version_index.write();
            let versions = index
                .entry(latest_key.clone())
                .or_insert_with(BTreeMap::new);
            versions.insert(tool.version.clone(), ());

            // Enforce max versions limit
            if self.config.max_versions_per_tool > 0
                && versions.len() > self.config.max_versions_per_tool
            {
                // Remove oldest versions
                while versions.len() > self.config.max_versions_per_tool {
                    if let Some((oldest, _)) = versions.pop_first() {
                        let old_key = (
                            tool.namespace.clone(),
                            tool.name.clone(),
                            oldest.to_string(),
                        );
                        self.tools.remove(&old_key);
                    }
                }
            }
        }

        // Update latest version if this is newer
        self.latest_versions
            .entry(latest_key)
            .and_modify(|current| {
                if tool.version > *current {
                    *current = tool.version.clone();
                }
            })
            .or_insert(tool.version.clone());

        // Update metrics
        self.metrics
            .total_registrations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        Ok(ToolRegistration {
            tool_id: tool.tool_id,
            version: tool.version,
            registered_at: now,
            is_update,
        })
    }

    /// Look up a tool by name with optional version constraint
    ///
    /// # Arguments
    /// - `namespace`: Tool namespace (uses default if None)
    /// - `name`: Tool name
    /// - `version_constraint`: Optional version constraint (e.g., "^1.0.0", ">=2.0", "latest")
    pub fn lookup(
        &self,
        namespace: Option<&str>,
        name: &str,
        version_constraint: Option<&str>,
    ) -> Result<ToolLookupResult, ToolRegistryError> {
        let namespace = namespace.unwrap_or(&self.config.default_namespace);
        let latest_key: LatestKey = (namespace.to_string(), name.to_string());

        self.metrics
            .total_lookups
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Get available versions
        let available_versions: Vec<ToolVersion> = {
            let index = self.version_index.read();
            index
                .get(&latest_key)
                .map(|versions| versions.keys().cloned().collect())
                .unwrap_or_default()
        };

        if available_versions.is_empty() {
            return Err(ToolRegistryError::ToolNotFound {
                namespace: namespace.to_string(),
                name: name.to_string(),
            });
        }

        // Resolve version
        let constraint = version_constraint.unwrap_or("latest");
        let resolved_version = if constraint == "latest" || constraint == "*" {
            // Fast path: use latest version index
            self.metrics
                .cache_hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            self.latest_versions
                .get(&latest_key)
                .map(|v| v.clone())
                .ok_or_else(|| ToolRegistryError::ToolNotFound {
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                })?
        } else {
            // Find best matching version
            available_versions
                .iter()
                .filter(|v| v.satisfies(constraint))
                .max()
                .cloned()
                .ok_or_else(|| ToolRegistryError::NoMatchingVersion {
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                    constraint: constraint.to_string(),
                })?
        };

        let key: ToolKey = (
            namespace.to_string(),
            name.to_string(),
            resolved_version.to_string(),
        );

        let tool = self.tools.get(&key).map(|t| t.clone()).ok_or_else(|| {
            ToolRegistryError::VersionNotFound {
                namespace: namespace.to_string(),
                name: name.to_string(),
                version: resolved_version.to_string(),
            }
        })?;

        let is_latest = self
            .latest_versions
            .get(&latest_key)
            .map(|v| *v == resolved_version)
            .unwrap_or(false);

        Ok(ToolLookupResult {
            tool,
            is_latest,
            available_versions,
        })
    }

    /// Get a tool by exact version
    pub fn get_exact(
        &self,
        namespace: &str,
        name: &str,
        version: &ToolVersion,
    ) -> Option<UnifiedToolDefinition> {
        let key: ToolKey = (namespace.to_string(), name.to_string(), version.to_string());
        self.tools.get(&key).map(|t| t.clone())
    }

    /// List all tools in a namespace
    pub fn list_namespace(&self, namespace: &str) -> Vec<UnifiedToolDefinition> {
        self.tools
            .iter()
            .filter(|entry| entry.key().0 == namespace)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// List all unique tools (latest versions only)
    pub fn list_latest(&self) -> Vec<UnifiedToolDefinition> {
        self.latest_versions
            .iter()
            .filter_map(|entry| {
                let (namespace, name) = entry.key();
                let version = entry.value();
                let key: ToolKey = (namespace.clone(), name.clone(), version.to_string());
                self.tools.get(&key).map(|t| t.clone())
            })
            .collect()
    }

    /// Unregister a specific tool version
    pub fn unregister(
        &self,
        namespace: &str,
        name: &str,
        version: &ToolVersion,
    ) -> Result<UnifiedToolDefinition, ToolRegistryError> {
        let key: ToolKey = (namespace.to_string(), name.to_string(), version.to_string());
        let latest_key: LatestKey = (namespace.to_string(), name.to_string());

        let (_, tool) =
            self.tools
                .remove(&key)
                .ok_or_else(|| ToolRegistryError::VersionNotFound {
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                    version: version.to_string(),
                })?;

        // Update version index
        {
            let mut index = self.version_index.write();
            if let Some(versions) = index.get_mut(&latest_key) {
                versions.remove(version);

                // Update latest version if we removed it
                if self
                    .latest_versions
                    .get(&latest_key)
                    .map(|v| *v == *version)
                    .unwrap_or(false)
                {
                    if let Some((new_latest, _)) = versions.last_key_value() {
                        self.latest_versions
                            .insert(latest_key.clone(), new_latest.clone());
                    } else {
                        self.latest_versions.remove(&latest_key);
                    }
                }

                if versions.is_empty() {
                    index.remove(&latest_key);
                }
            }
        }

        Ok(tool)
    }

    /// Enable or disable a tool
    pub fn set_enabled(
        &self,
        namespace: &str,
        name: &str,
        version: &ToolVersion,
        enabled: bool,
    ) -> Result<(), ToolRegistryError> {
        let key: ToolKey = (namespace.to_string(), name.to_string(), version.to_string());

        let mut entry =
            self.tools
                .get_mut(&key)
                .ok_or_else(|| ToolRegistryError::VersionNotFound {
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                    version: version.to_string(),
                })?;

        entry.enabled = enabled;
        entry.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        Ok(())
    }

    /// Get registry statistics
    pub fn stats(&self) -> RegistryStats {
        RegistryStats {
            total_tools: self.tools.len(),
            unique_tools: self.latest_versions.len(),
            total_registrations: self
                .metrics
                .total_registrations
                .load(std::sync::atomic::Ordering::Relaxed),
            total_lookups: self
                .metrics
                .total_lookups
                .load(std::sync::atomic::Ordering::Relaxed),
            cache_hits: self
                .metrics
                .cache_hits
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Get all namespaces
    pub fn namespaces(&self) -> HashSet<String> {
        self.tools
            .iter()
            .map(|entry| entry.key().0.clone())
            .collect()
    }
}

/// Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_tools: usize,
    pub unique_tools: usize,
    pub total_registrations: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new(ToolRegistryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentreplay_core::ToolKind;

    fn make_test_tool(name: &str, version: ToolVersion) -> UnifiedToolDefinition {
        UnifiedToolDefinition::new(
            "default",
            name,
            version,
            ToolKind::Native {
                handler_id: "test_handler".to_string(),
            },
            serde_json::json!({ "type": "object" }),
        )
    }

    #[test]
    fn test_register_and_lookup() {
        let registry = ToolRegistry::default();

        let tool = make_test_tool("search", ToolVersion::new(1, 0, 0));
        let result = registry.register(tool.clone()).unwrap();

        assert!(!result.is_update);
        assert_eq!(result.version, ToolVersion::new(1, 0, 0));

        let lookup = registry.lookup(Some("default"), "search", None).unwrap();
        assert_eq!(lookup.tool.name, "search");
        assert!(lookup.is_latest);
    }

    #[test]
    fn test_version_constraints() {
        let registry = ToolRegistry::default();

        registry
            .register(make_test_tool("api", ToolVersion::new(1, 0, 0)))
            .unwrap();
        registry
            .register(make_test_tool("api", ToolVersion::new(1, 2, 0)))
            .unwrap();
        registry
            .register(make_test_tool("api", ToolVersion::new(2, 0, 0)))
            .unwrap();

        // Latest should be 2.0.0
        let latest = registry
            .lookup(Some("default"), "api", Some("latest"))
            .unwrap();
        assert_eq!(latest.tool.version, ToolVersion::new(2, 0, 0));

        // ^1.0.0 should match 1.2.0 (highest 1.x)
        let caret = registry
            .lookup(Some("default"), "api", Some("^1.0.0"))
            .unwrap();
        assert_eq!(caret.tool.version, ToolVersion::new(1, 2, 0));

        // >=1.2.0 should match 2.0.0
        let gte = registry
            .lookup(Some("default"), "api", Some(">=1.2.0"))
            .unwrap();
        assert_eq!(gte.tool.version, ToolVersion::new(2, 0, 0));
    }

    #[test]
    fn test_unregister() {
        let registry = ToolRegistry::default();

        registry
            .register(make_test_tool("temp", ToolVersion::new(1, 0, 0)))
            .unwrap();
        registry
            .register(make_test_tool("temp", ToolVersion::new(2, 0, 0)))
            .unwrap();

        // Unregister latest
        registry
            .unregister("default", "temp", &ToolVersion::new(2, 0, 0))
            .unwrap();

        // Latest should now be 1.0.0
        let lookup = registry.lookup(Some("default"), "temp", None).unwrap();
        assert_eq!(lookup.tool.version, ToolVersion::new(1, 0, 0));
    }
}
