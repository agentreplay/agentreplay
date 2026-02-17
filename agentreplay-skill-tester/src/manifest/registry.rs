// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Skill Registry — content-addressable skill storage with B-tree indexing
//!
//! The registry stores `SkillManifest` instances keyed by `(namespace, skill_name, version_hash)`.
//! BLAKE3 hashing ensures content-addressable versioning — identical skill content always
//! produces the same hash, enabling reliable regression attribution.

use super::parser::SkillManifest;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};

/// Snapshot of a skill version loaded during an eval run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    pub name: String,
    pub version: String,
    pub version_hash: String,
    pub loaded_at: DateTime<Utc>,
}

/// Registry key for skill lookups: (namespace, skill_name, version_hash)
type RegistryKey = (String, String, String);

/// Content-addressable skill registry backed by a B-tree index.
///
/// Provides O(log k) lookup on `(namespace, skill_name, version_hash)`.
/// Thread-safe via DashMap for the name→versions index.
pub struct SkillRegistry {
    /// Primary store: registry key → manifest
    manifests: BTreeMap<RegistryKey, SkillManifest>,

    /// Name index for fast lookup by skill name
    name_index: DashMap<String, Vec<String>>,  // name → version_hashes

    /// Snapshots for eval runs
    snapshots: Vec<SkillSnapshot>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            manifests: BTreeMap::new(),
            name_index: DashMap::new(),
            snapshots: Vec::new(),
        }
    }

    /// Register a skill manifest in the registry
    pub fn register(&mut self, namespace: &str, manifest: SkillManifest) -> SkillSnapshot {
        let key = (
            namespace.to_string(),
            manifest.name.clone(),
            manifest.version_hash.clone(),
        );

        let snapshot = SkillSnapshot {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            version_hash: manifest.version_hash.clone(),
            loaded_at: Utc::now(),
        };

        // Update name index
        self.name_index
            .entry(manifest.name.clone())
            .or_insert_with(Vec::new)
            .push(manifest.version_hash.clone());

        self.manifests.insert(key, manifest);
        self.snapshots.push(snapshot.clone());

        snapshot
    }

    /// Look up a specific skill version
    pub fn get(&self, namespace: &str, name: &str, version_hash: &str) -> Option<&SkillManifest> {
        let key = (
            namespace.to_string(),
            name.to_string(),
            version_hash.to_string(),
        );
        self.manifests.get(&key)
    }

    /// Get all versions of a skill by name
    pub fn get_versions(&self, name: &str) -> Vec<&SkillManifest> {
        if let Some(hashes) = self.name_index.get(name) {
            hashes.iter()
                .filter_map(|hash| {
                    // Search across all namespaces
                    self.manifests.iter()
                        .find(|(k, _)| k.1 == name && k.2 == *hash)
                        .map(|(_, v)| v)
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the latest version of a skill
    pub fn get_latest(&self, name: &str) -> Option<&SkillManifest> {
        self.get_versions(name).into_iter().last()
    }

    /// List all registered skills
    pub fn list_skills(&self) -> Vec<&SkillManifest> {
        self.manifests.values().collect()
    }

    /// Get skill snapshots for the current eval session
    pub fn snapshots(&self) -> &[SkillSnapshot] {
        &self.snapshots
    }

    /// Total number of registered skill versions
    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe shared registry handle
pub type SharedSkillRegistry = Arc<parking_lot::RwLock<SkillRegistry>>;

/// Create a new shared registry
pub fn shared_registry() -> SharedSkillRegistry {
    Arc::new(parking_lot::RwLock::new(SkillRegistry::new()))
}
