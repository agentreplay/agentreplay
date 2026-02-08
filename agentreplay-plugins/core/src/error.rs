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

//! Plugin error types

use thiserror::Error;

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Errors that can occur in the plugin system
#[derive(Debug, Error)]
pub enum PluginError {
    // Manifest errors
    #[error("Manifest not found: {0}")]
    ManifestNotFound(String),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Manifest parse error: {0}")]
    ManifestParseError(String),

    #[error("Schema version {0} not supported (max: {1})")]
    UnsupportedSchemaVersion(u32, u32),

    // Dependency errors
    #[error("Dependency not found: {0}")]
    DependencyNotFound(String),

    #[error("Dependency cycle detected: {0}")]
    DependencyCycle(String),

    #[error("Version conflict: {0}")]
    VersionConflict(String),

    #[error("Incompatible plugin API version: requires {0}, have {1}")]
    IncompatibleApiVersion(String, String),

    // Installation errors
    #[error("Plugin already installed: {0}")]
    AlreadyInstalled(String),

    #[error("Plugin not installed: {0}")]
    NotInstalled(String),

    #[error("Installation failed: {0}")]
    InstallationFailed(String),

    #[error("Install failed: {0}")]
    InstallFailed(String),

    #[error("Uninstallation failed: {0}")]
    UninstallationFailed(String),

    #[error("Plugin has dependents: {0:?}")]
    HasDependents(Vec<String>),

    // Bundle errors
    #[error("Bundle target not found: {0}")]
    BundleTargetNotFound(String),

    #[error("Bundle detection failed: {0}")]
    BundleDetectionFailed(String),

    #[error("Bundle variable required: {0}")]
    BundleVariableRequired(String),

    // Runtime errors
    #[error("Plugin not loaded: {0}")]
    NotLoaded(String),

    #[error("Plugin load failed: {0}")]
    LoadFailed(String),

    #[error("Plugin execution error: {0}")]
    ExecutionError(String),

    #[error("Plugin timeout")]
    Timeout,

    // Capability errors
    #[error("Capability denied: {0}")]
    CapabilityDenied(String),

    #[error("Capability not granted: {0}")]
    CapabilityNotGranted(String),

    // State errors
    #[error("Plugin already enabled: {0}")]
    AlreadyEnabled(String),

    #[error("Plugin already disabled: {0}")]
    AlreadyDisabled(String),

    #[error("Invalid state transition: {0} -> {1}")]
    InvalidStateTransition(String, String),

    // Signature/security errors
    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Integrity check failed: {0}")]
    IntegrityCheckFailed(String),

    // IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    // Registry errors
    #[error("Plugin not found in registry: {0}")]
    NotFoundInRegistry(String),

    #[error("Registry error: {0}")]
    RegistryError(String),

    // Update errors
    #[error("Update check failed: {0}")]
    UpdateCheckFailed(String),

    #[error("Update failed: {0}")]
    UpdateFailed(String),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    // Generic errors
    #[error("Plugin error: {0}")]
    Other(String),
}

impl From<serde_json::Error> for PluginError {
    fn from(e: serde_json::Error) -> Self {
        PluginError::SerializationError(e.to_string())
    }
}

impl From<toml::de::Error> for PluginError {
    fn from(e: toml::de::Error) -> Self {
        PluginError::ManifestParseError(e.to_string())
    }
}

impl From<semver::Error> for PluginError {
    fn from(e: semver::Error) -> Self {
        PluginError::InvalidManifest(format!("Invalid version: {}", e))
    }
}
