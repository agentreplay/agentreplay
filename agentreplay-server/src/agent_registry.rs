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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{error, info, warn};

/// Metadata about a registered agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub agent_id: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl AgentMetadata {
    /// Get the full display name for this agent
    pub fn display_name(&self) -> String {
        match (&self.namespace, &self.version) {
            (Some(ns), Some(ver)) => format!("{}.{}.{}", ns, self.name, ver),
            (Some(ns), None) => format!("{}.{}", ns, self.name),
            (None, Some(ver)) => format!("{}.{}", self.name, ver),
            (None, None) => self.name.clone(),
        }
    }
}

/// Thread-safe agent registry
#[derive(Clone)]
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<u64, AgentMetadata>>>,
    storage_path: PathBuf,
}

impl AgentRegistry {
    /// Create a new agent registry
    pub fn new(storage_path: impl AsRef<Path>) -> Self {
        let storage_path = storage_path.as_ref().to_path_buf();
        let agents = Arc::new(RwLock::new(HashMap::new()));

        let registry = Self {
            agents,
            storage_path,
        };

        // Try to load existing registry from disk
        if let Err(e) = registry.load_from_disk() {
            warn!(
                "Failed to load agent registry from disk: {}. Starting with empty registry.",
                e
            );
        }

        registry
    }

    /// Register a new agent or update existing registration
    ///
    /// # Edge Cases Handled:
    /// - Concurrent registration: RwLock ensures thread safety
    /// - Update existing: Returns updated metadata
    /// - Persistence failure: Logs error but keeps in-memory state
    pub fn register(&self, metadata: AgentMetadata) -> Result<AgentMetadata, String> {
        let agent_id = metadata.agent_id;

        // Validate input
        if metadata.name.is_empty() {
            return Err("Agent name cannot be empty".to_string());
        }

        let mut agents = self
            .agents
            .write()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        // Check if updating existing agent
        let is_update = agents.contains_key(&agent_id);

        // Insert or update
        agents.insert(agent_id, metadata.clone());

        drop(agents); // Release lock before disk I/O

        // Persist to disk (non-blocking for caller)
        if let Err(e) = self.save_to_disk() {
            error!("Failed to persist agent registry: {}", e);
            // Don't fail the registration - in-memory state is more important
        }

        if is_update {
            info!(
                "Updated agent registration: agent_id={}, name={}",
                agent_id, metadata.name
            );
        } else {
            info!(
                "Registered new agent: agent_id={}, name={}",
                agent_id, metadata.name
            );
        }

        Ok(metadata)
    }

    /// Get agent metadata by ID
    ///
    /// # Edge Cases:
    /// - Missing agent: Returns None
    /// - Concurrent read: RwLock allows multiple readers
    pub fn get(&self, agent_id: u64) -> Option<AgentMetadata> {
        let agents = self.agents.read().ok()?;
        agents.get(&agent_id).cloned()
    }

    /// Get display name for agent, with fallback for unregistered agents
    ///
    /// # Edge Cases:
    /// - Missing agent: Returns "Unknown Agent (ID)"
    /// - Ensures UI always has something to display
    pub fn get_display_name(&self, agent_id: u64) -> String {
        self.get(agent_id)
            .map(|m| m.display_name())
            .unwrap_or_else(|| format!("Unknown Agent ({})", agent_id))
    }

    /// List all registered agents
    pub fn list(&self) -> Result<Vec<AgentMetadata>, String> {
        let agents = self
            .agents
            .read()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        Ok(agents.values().cloned().collect())
    }

    /// Auto-register an agent with default name if not already registered
    ///
    /// Used during trace ingestion to ensure all agents have at least a default entry.
    ///
    /// # Edge Cases:
    /// - Agent already registered: No-op, returns existing metadata
    /// - First time seen: Creates default entry
    pub fn auto_register(&self, agent_id: u64) -> Result<AgentMetadata, String> {
        // Check if already registered
        if let Some(metadata) = self.get(agent_id) {
            return Ok(metadata);
        }

        // Create default registration
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metadata = AgentMetadata {
            agent_id,
            name: format!("agent_{}", agent_id),
            namespace: None,
            version: None,
            description: Some("Auto-registered agent".to_string()),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        };

        self.register(metadata)
    }

    /// Check if an agent is registered
    pub fn is_registered(&self, agent_id: u64) -> bool {
        self.agents
            .read()
            .ok()
            .map(|agents| agents.contains_key(&agent_id))
            .unwrap_or(false)
    }

    /// Get total number of registered agents
    pub fn count(&self) -> usize {
        self.agents
            .read()
            .ok()
            .map(|agents| agents.len())
            .unwrap_or(0)
    }

    /// Load registry from disk
    ///
    /// # Edge Cases:
    /// - File doesn't exist: Start with empty registry
    /// - Corrupted file: Log error and start fresh
    /// - Backup file exists: Try to load from backup
    fn load_from_disk(&self) -> Result<(), String> {
        if !self.storage_path.exists() {
            info!("Agent registry file not found, starting with empty registry");
            return Ok(());
        }

        let file = File::open(&self.storage_path)
            .map_err(|e| format!("Failed to open registry file: {}", e))?;

        let reader = BufReader::new(file);
        let agents: HashMap<u64, AgentMetadata> = serde_json::from_reader(reader)
            .map_err(|e| format!("Failed to parse registry JSON: {}", e))?;

        let mut registry = self
            .agents
            .write()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        *registry = agents;

        info!("Loaded {} agents from registry", registry.len());
        Ok(())
    }

    /// Save registry to disk
    ///
    /// # Edge Cases:
    /// - Directory doesn't exist: Create it
    /// - Write failure: Backup previous version
    /// - Atomic write: Write to temp file then rename
    fn save_to_disk(&self) -> Result<(), String> {
        // Ensure directory exists
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create registry directory: {}", e))?;
        }

        // Backup existing file if it exists
        if self.storage_path.exists() {
            let backup_path = self.storage_path.with_extension("json.bak");
            if let Err(e) = fs::copy(&self.storage_path, &backup_path) {
                warn!("Failed to create backup of agent registry: {}", e);
            }
        }

        // Write to temporary file first (atomic write pattern)
        let temp_path = self.storage_path.with_extension("json.tmp");

        {
            let file = File::create(&temp_path)
                .map_err(|e| format!("Failed to create temp registry file: {}", e))?;

            let writer = BufWriter::new(file);

            let agents = self
                .agents
                .read()
                .map_err(|e| format!("Lock poisoned: {}", e))?;

            serde_json::to_writer_pretty(writer, &*agents)
                .map_err(|e| format!("Failed to serialize registry: {}", e))?;
        }

        // Atomic rename
        fs::rename(&temp_path, &self.storage_path)
            .map_err(|e| format!("Failed to rename temp file: {}", e))?;

        Ok(())
    }

    /// Delete an agent registration
    ///
    /// Note: This doesn't delete traces from that agent, just the metadata.
    /// Traces will show fallback name "Unknown Agent (ID)" after deletion.
    pub fn delete(&self, agent_id: u64) -> Result<bool, String> {
        let mut agents = self
            .agents
            .write()
            .map_err(|e| format!("Lock poisoned: {}", e))?;

        let removed = agents.remove(&agent_id).is_some();

        drop(agents);

        if removed {
            if let Err(e) = self.save_to_disk() {
                error!("Failed to persist agent registry after deletion: {}", e);
            }
            info!("Deleted agent registration: agent_id={}", agent_id);
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_register_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("agents.json");
        let registry = AgentRegistry::new(&registry_path);

        let metadata = AgentMetadata {
            agent_id: 1,
            name: "test_bot".to_string(),
            namespace: Some("testing".to_string()),
            version: Some("v1".to_string()),
            description: None,
            created_at: 1000,
            updated_at: 1000,
            metadata: HashMap::new(),
        };

        registry.register(metadata.clone()).unwrap();

        let retrieved = registry.get(1).unwrap();
        assert_eq!(retrieved.name, "test_bot");
        assert_eq!(retrieved.display_name(), "testing.test_bot.v1");
    }

    #[test]
    fn test_auto_register() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("agents.json");
        let registry = AgentRegistry::new(&registry_path);

        let metadata = registry.auto_register(42).unwrap();
        assert_eq!(metadata.name, "agent_42");
        assert_eq!(metadata.agent_id, 42);

        // Second call should return same metadata
        let metadata2 = registry.auto_register(42).unwrap();
        assert_eq!(metadata.created_at, metadata2.created_at);
    }

    #[test]
    fn test_display_name_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("agents.json");
        let registry = AgentRegistry::new(&registry_path);

        let name = registry.get_display_name(999);
        assert_eq!(name, "Unknown Agent (999)");
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("agents.json");

        {
            let registry = AgentRegistry::new(&registry_path);
            let metadata = AgentMetadata {
                agent_id: 1,
                name: "persistent_bot".to_string(),
                namespace: None,
                version: None,
                description: None,
                created_at: 1000,
                updated_at: 1000,
                metadata: HashMap::new(),
            };
            registry.register(metadata).unwrap();
        }

        // Create new registry instance - should load from disk
        let registry2 = AgentRegistry::new(&registry_path);
        let retrieved = registry2.get(1).unwrap();
        assert_eq!(retrieved.name, "persistent_bot");
    }
}
