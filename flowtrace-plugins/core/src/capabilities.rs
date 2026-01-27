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

//! Plugin capabilities and access control
//!
//! Implements capability-based security for plugins.

use crate::error::{PluginError, PluginResult};
use crate::manifest::CapabilityRequirements;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Individual capability that can be granted to a plugin
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Read trace data
    ReadTraces,
    /// Write/modify trace data
    WriteTraces,
    /// Delete trace data
    DeleteTraces,
    /// Network access (general)
    Network,
    /// Network access to specific domain
    NetworkDomain(String),
    /// Read filesystem (general)
    FilesystemRead,
    /// Read specific path
    FilesystemReadPath(String),
    /// Write filesystem (general)
    FilesystemWrite,
    /// Write specific path
    FilesystemWritePath(String),
    /// Execute shell commands
    Shell,
    /// Access LLM provider
    LlmProvider(String),
    /// Send notifications
    Notifications,
    /// Access clipboard
    Clipboard,
    /// Create UI panels
    UiPanel,
    /// Modify UI theme
    UiTheme,
    /// Access system information
    SystemInfo,
}

impl Capability {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Capability::ReadTraces => "Read trace data",
            Capability::WriteTraces => "Write and modify trace data",
            Capability::DeleteTraces => "Delete trace data",
            Capability::Network => "Full network access",
            Capability::NetworkDomain(_) => "Network access to specific domain",
            Capability::FilesystemRead => "Read files",
            Capability::FilesystemReadPath(_) => "Read specific file path",
            Capability::FilesystemWrite => "Write files",
            Capability::FilesystemWritePath(_) => "Write to specific file path",
            Capability::Shell => "Execute shell commands",
            Capability::LlmProvider(_) => "Access LLM provider",
            Capability::Notifications => "Send desktop notifications",
            Capability::Clipboard => "Access clipboard",
            Capability::UiPanel => "Create UI panels",
            Capability::UiTheme => "Modify UI theme",
            Capability::SystemInfo => "Access system information",
        }
    }

    /// Check if this capability is considered dangerous
    pub fn is_dangerous(&self) -> bool {
        matches!(
            self,
            Capability::WriteTraces
                | Capability::DeleteTraces
                | Capability::Network
                | Capability::FilesystemWrite
                | Capability::FilesystemWritePath(_)
                | Capability::Shell
        )
    }
}

/// A set of capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    capabilities: HashSet<Capability>,
}

impl CapabilitySet {
    /// Create an empty capability set
    pub fn new() -> Self {
        Self {
            capabilities: HashSet::new(),
        }
    }

    /// Create a capability set from requirements
    pub fn from_requirements(reqs: &CapabilityRequirements) -> Self {
        let mut set = Self::new();

        if reqs.read_traces {
            set.add(Capability::ReadTraces);
        }
        if reqs.write_traces {
            set.add(Capability::WriteTraces);
        }
        if reqs.shell {
            set.add(Capability::Shell);
        }
        if reqs.notifications {
            set.add(Capability::Notifications);
        }
        if reqs.clipboard {
            set.add(Capability::Clipboard);
        }

        if let Some(net) = &reqs.network {
            if net.allowed_domains.is_empty() {
                set.add(Capability::Network);
            } else {
                for domain in &net.allowed_domains {
                    set.add(Capability::NetworkDomain(domain.clone()));
                }
            }
        }

        if let Some(fs) = &reqs.filesystem {
            for path in &fs.read {
                set.add(Capability::FilesystemReadPath(path.clone()));
            }
            for path in &fs.write {
                set.add(Capability::FilesystemWritePath(path.clone()));
            }
        }

        for provider in &reqs.llm_providers {
            set.add(Capability::LlmProvider(provider.clone()));
        }

        set
    }

    /// Add a capability
    pub fn add(&mut self, cap: Capability) {
        self.capabilities.insert(cap);
    }

    /// Remove a capability
    pub fn remove(&mut self, cap: &Capability) {
        self.capabilities.remove(cap);
    }

    /// Check if a capability is present
    pub fn has(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Get all capabilities
    pub fn all(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }

    /// Get dangerous capabilities
    pub fn dangerous(&self) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| c.is_dangerous())
            .collect()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Number of capabilities
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }
}

/// Capabilities that have been granted to a plugin
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GrantedCapabilities {
    /// Plugin ID
    plugin_id: String,
    /// Granted capabilities
    granted: CapabilitySet,
    /// Required capabilities that were not granted
    denied: CapabilitySet,
    /// Time when capabilities were granted
    granted_at: chrono::DateTime<chrono::Utc>,
}

impl GrantedCapabilities {
    /// Create new granted capabilities
    pub fn new(plugin_id: String, granted: CapabilitySet) -> Self {
        Self {
            plugin_id,
            granted,
            denied: CapabilitySet::new(),
            granted_at: chrono::Utc::now(),
        }
    }

    /// Grant all requested capabilities
    pub fn grant_all(plugin_id: String, requirements: &CapabilityRequirements) -> Self {
        let granted = CapabilitySet::from_requirements(requirements);
        Self::new(plugin_id, granted)
    }

    /// Check if a capability is granted
    pub fn check(&self, cap: &Capability) -> PluginResult<()> {
        if self.granted.has(cap) {
            Ok(())
        } else {
            Err(PluginError::CapabilityDenied(format!(
                "Plugin '{}' does not have capability: {:?}",
                self.plugin_id, cap
            )))
        }
    }

    /// Check if plugin can read traces
    pub fn can_read_traces(&self) -> bool {
        self.granted.has(&Capability::ReadTraces)
    }

    /// Check if plugin can write traces
    pub fn can_write_traces(&self) -> bool {
        self.granted.has(&Capability::WriteTraces)
    }

    /// Check if plugin has network access
    pub fn can_network(&self, domain: Option<&str>) -> bool {
        if self.granted.has(&Capability::Network) {
            return true;
        }
        if let Some(d) = domain {
            self.granted.has(&Capability::NetworkDomain(d.to_string()))
        } else {
            false
        }
    }

    /// Check if plugin can use shell
    pub fn can_shell(&self) -> bool {
        self.granted.has(&Capability::Shell)
    }

    /// Get all granted capabilities
    pub fn all_granted(&self) -> &CapabilitySet {
        &self.granted
    }
}

/// Capability enforcer for runtime checks
pub struct CapabilityEnforcer {
    /// Audit log for capability checks
    audit_log: parking_lot::RwLock<Vec<AuditEntry>>,
}

/// Audit entry for capability checks
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub plugin_id: String,
    pub capability: String,
    pub granted: bool,
    pub context: Option<String>,
}

impl CapabilityEnforcer {
    /// Create new enforcer
    pub fn new() -> Self {
        Self {
            audit_log: parking_lot::RwLock::new(Vec::new()),
        }
    }

    /// Check capability and log the access
    pub fn check_and_log(
        &self,
        plugin_id: &str,
        granted: &GrantedCapabilities,
        cap: &Capability,
        context: Option<&str>,
    ) -> PluginResult<()> {
        let result = granted.check(cap);

        let entry = AuditEntry {
            timestamp: chrono::Utc::now(),
            plugin_id: plugin_id.to_string(),
            capability: format!("{:?}", cap),
            granted: result.is_ok(),
            context: context.map(|s| s.to_string()),
        };

        {
            let mut log = self.audit_log.write();
            log.push(entry);
            // Keep only last 1000 entries
            if log.len() > 1000 {
                log.remove(0);
            }
        }

        result
    }

    /// Get recent audit entries
    pub fn recent_entries(&self, count: usize) -> Vec<AuditEntry> {
        let log = self.audit_log.read();
        log.iter().rev().take(count).cloned().collect()
    }
}

impl Default for CapabilityEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_set() {
        let mut set = CapabilitySet::new();
        set.add(Capability::ReadTraces);
        set.add(Capability::WriteTraces);

        assert!(set.has(&Capability::ReadTraces));
        assert!(set.has(&Capability::WriteTraces));
        assert!(!set.has(&Capability::Shell));
    }

    #[test]
    fn test_granted_capabilities() {
        let mut granted = CapabilitySet::new();
        granted.add(Capability::ReadTraces);

        let caps = GrantedCapabilities::new("test-plugin".to_string(), granted);

        assert!(caps.check(&Capability::ReadTraces).is_ok());
        assert!(caps.check(&Capability::WriteTraces).is_err());
    }

    #[test]
    fn test_dangerous_capabilities() {
        let mut set = CapabilitySet::new();
        set.add(Capability::ReadTraces);
        set.add(Capability::Shell);
        set.add(Capability::Network);

        let dangerous = set.dangerous();
        assert_eq!(dangerous.len(), 2);
    }
}
