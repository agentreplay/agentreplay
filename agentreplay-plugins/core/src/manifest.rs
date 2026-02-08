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

//! Plugin manifest schema and parser
//!
//! Defines the structure of `agentreplay-plugin.toml` manifest files.

use crate::error::{PluginError, PluginResult};
use crate::MANIFEST_FILENAME;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

/// Current manifest schema version
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Maximum supported schema version (supports v1 and v2 manifests)
pub const MAX_SCHEMA_VERSION: u32 = 2;

/// Plugin manifest - the main configuration file for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Schema version for forward compatibility
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Plugin metadata
    pub plugin: PluginMetadata,

    /// Plugin dependencies
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,

    /// Required capabilities
    #[serde(default)]
    pub capabilities: CapabilityRequirements,

    /// Entry points for different plugin types
    #[serde(default)]
    pub entry: EntryPoints,

    /// Configuration schema for the plugin
    #[serde(default)]
    pub config: Option<ConfigSchema>,

    /// UI components (for frontend plugins)
    #[serde(default)]
    pub ui: Option<UiComponents>,

    /// Bundle configuration for external integrations (Claude Code, Cursor, etc.)
    /// Schema version 2+ only
    #[serde(default)]
    pub bundle: Option<BundleManifest>,
}

fn default_schema_version() -> u32 {
    1
}

/// Core plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin identifier (e.g., "agentreplay-hallucination-detector")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Plugin version (semver)
    pub version: String,

    /// Plugin description
    #[serde(default)]
    pub description: String,

    /// Plugin author(s)
    #[serde(default)]
    pub authors: Vec<String>,

    /// Plugin license
    #[serde(default)]
    pub license: Option<String>,

    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,

    /// Homepage URL
    #[serde(default)]
    pub homepage: Option<String>,

    /// Plugin type
    #[serde(rename = "type")]
    pub plugin_type: PluginType,

    /// Minimum Agentreplay version required
    #[serde(default = "default_min_agentreplay_version")]
    pub min_agentreplay_version: String,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Icon path (relative to plugin root)
    #[serde(default)]
    pub icon: Option<String>,
}

fn default_min_agentreplay_version() -> String {
    "0.1.0".to_string()
}

/// Plugin type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    /// Evaluator plugin - adds new evaluation capabilities
    Evaluator,
    /// Embedding provider - adds new embedding models
    EmbeddingProvider,
    /// LLM provider - adds new LLM integrations
    LlmProvider,
    /// Exporter - adds new export formats
    Exporter,
    /// Importer - adds new import formats
    Importer,
    /// UI widget - adds dashboard widgets
    UiWidget,
    /// Integration - external service integration
    Integration,
    /// Theme - custom UI themes
    Theme,
    /// Bundle - file-based integration bundle (Claude Code, Cursor, etc.)
    Bundle,
    /// Memory provider - adds memory/RAG capabilities
    MemoryProvider,
    /// Hook handler - processes editor/agent hook events
    HookHandler,
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginType::Evaluator => write!(f, "evaluator"),
            PluginType::EmbeddingProvider => write!(f, "embedding_provider"),
            PluginType::LlmProvider => write!(f, "llm_provider"),
            PluginType::Exporter => write!(f, "exporter"),
            PluginType::Importer => write!(f, "importer"),
            PluginType::UiWidget => write!(f, "ui_widget"),
            PluginType::Integration => write!(f, "integration"),
            PluginType::Theme => write!(f, "theme"),
            PluginType::Bundle => write!(f, "bundle"),
            PluginType::MemoryProvider => write!(f, "memory_provider"),
            PluginType::HookHandler => write!(f, "hook_handler"),
        }
    }
}

/// Dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencySpec {
    /// Simple version requirement (e.g., ">=1.0.0")
    Version(String),
    /// Detailed dependency specification
    Detailed(DetailedDependency),
}

/// Detailed dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedDependency {
    /// Version requirement
    pub version: String,
    /// Whether this dependency is optional
    #[serde(default)]
    pub optional: bool,
    /// Platform-specific (e.g., "macos", "linux", "windows")
    #[serde(default)]
    pub platform: Option<String>,
}

/// Capability requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityRequirements {
    /// Read traces
    #[serde(default)]
    pub read_traces: bool,
    /// Write traces
    #[serde(default)]
    pub write_traces: bool,
    /// Network access
    #[serde(default)]
    pub network: Option<NetworkCapability>,
    /// Filesystem access
    #[serde(default)]
    pub filesystem: Option<FilesystemCapability>,
    /// Shell access (dangerous)
    #[serde(default)]
    pub shell: bool,
    /// Access to specific LLM providers
    #[serde(default)]
    pub llm_providers: Vec<String>,
    /// Send desktop notifications
    #[serde(default)]
    pub notifications: bool,
    /// Access clipboard
    #[serde(default)]
    pub clipboard: bool,
}

/// Network capability specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCapability {
    /// Allowed domains (empty = all)
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    /// Allow outbound HTTP
    #[serde(default)]
    pub http: bool,
    /// Allow outbound HTTPS
    #[serde(default)]
    pub https: bool,
}

/// Filesystem capability specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemCapability {
    /// Read-only paths (relative to data dir)
    #[serde(default)]
    pub read: Vec<String>,
    /// Read-write paths (relative to data dir)
    #[serde(default)]
    pub write: Vec<String>,
}

/// Entry points for the plugin
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntryPoints {
    /// Native library entry point
    #[serde(default)]
    pub native: Option<EntryPoint>,
    /// WASM entry point (future)
    #[serde(default)]
    pub wasm: Option<String>,
    /// Script entry point (Python, Node.js)
    #[serde(default)]
    pub script: Option<ScriptEntry>,
}

/// Native entry point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    /// Library path (relative to plugin root)
    pub path: String,
    /// Platform-specific library names
    #[serde(default)]
    pub platforms: HashMap<String, String>,
}

/// Script entry point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEntry {
    /// Script runtime (python, node)
    pub runtime: String,
    /// Script path
    pub path: String,
    /// Required runtime version
    #[serde(default)]
    pub runtime_version: Option<String>,
}

/// Configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// JSON Schema for configuration validation
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
    /// Default configuration values
    #[serde(default)]
    pub defaults: serde_json::Value,
}

/// UI component declarations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiComponents {
    /// Dashboard widgets
    #[serde(default)]
    pub widgets: Vec<WidgetDeclaration>,
    /// Settings panels
    #[serde(default)]
    pub settings_panel: Option<String>,
    /// Custom CSS
    #[serde(default)]
    pub styles: Option<String>,
}

/// Widget declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetDeclaration {
    /// Widget ID
    pub id: String,
    /// Widget name
    pub name: String,
    /// Where the widget can appear
    pub slots: Vec<String>,
    /// Widget component path
    pub component: String,
}

// ============================================================================
// Bundle Manifest (Schema Version 2+)
// ============================================================================
// Supports external integration bundles for Claude Code, Cursor, and other
// editor/agent ecosystems. Bundles contain file payloads, install instructions,
// and optional hooks that are installed to target-specific locations.

/// Bundle manifest for external integration plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    /// Bundle schema version (for bundle-specific evolution)
    #[serde(default = "default_bundle_version")]
    pub bundle_version: String,

    /// Default installation mode: "guided" shows instructions, "auto" runs silently, "manual" just shows docs
    #[serde(default = "default_install_mode")]
    pub default_install_mode: InstallMode,

    /// Root directory for integration assets within the plugin package
    #[serde(default = "default_assets_root")]
    pub assets_root: String,

    /// User-prompted variables (e.g., project directory for project-scoped installs)
    #[serde(default)]
    pub variables: Vec<BundleVariable>,

    /// Integration targets (Claude Code, Cursor, etc.)
    #[serde(default)]
    pub targets: Vec<BundleTarget>,
}

fn default_bundle_version() -> String {
    "1.0.0".to_string()
}

fn default_install_mode() -> InstallMode {
    InstallMode::Guided
}

fn default_assets_root() -> String {
    "integrations".to_string()
}

/// Installation mode for bundle targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    /// Show step-by-step instructions, user confirms each step
    #[default]
    Guided,
    /// Silently install without prompts (where allowed)
    Auto,
    /// Only show documentation, no automated install
    Manual,
}

/// User-prompted variable for bundle installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleVariable {
    /// Variable name (used in path templates as ${name})
    pub name: String,
    /// Human-readable label for UI
    pub label: String,
    /// Variable type: "string", "directory", "file", "boolean"
    #[serde(default = "default_variable_kind")]
    pub kind: VariableKind,
    /// Whether this variable is required
    #[serde(default)]
    pub required: bool,
    /// Default value if not provided
    #[serde(default)]
    pub default: Option<String>,
    /// Description/help text
    #[serde(default)]
    pub description: Option<String>,
}

fn default_variable_kind() -> VariableKind {
    VariableKind::String
}

/// Variable type for bundle variables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VariableKind {
    #[default]
    String,
    Directory,
    File,
    Boolean,
}

/// Bundle integration target (e.g., Claude Code, Cursor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTarget {
    /// Unique target identifier (e.g., "claude-code", "cursor")
    pub id: String,
    /// Human-readable display name
    pub display_name: String,
    /// Target kind for specialized handling
    #[serde(default)]
    pub kind: TargetKind,
    /// Path to installation instructions markdown (relative to assets_root)
    #[serde(default)]
    pub install_md: Option<String>,
    /// Supported installation scopes
    #[serde(default)]
    pub scopes: Vec<InstallScope>,
    /// Default scope for this target
    #[serde(default)]
    pub default_scope: Option<InstallScope>,
    /// Detection rules to check if target is installed/configured
    #[serde(default)]
    pub detect: Vec<DetectRule>,
    /// Files to copy during installation
    #[serde(default)]
    pub files_to_copy: Vec<FileCopyStep>,
    /// Template files that need variable substitution
    #[serde(default)]
    pub templates: Vec<TemplateSpec>,
    /// Optional commands to show/run during installation
    #[serde(default)]
    pub commands: Vec<InstallCommand>,
    /// Required capabilities for this target installation
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    /// Declarative install operations (json_merge, json_patch, etc.)
    /// These are executed in order during installation
    #[serde(default)]
    pub ops: Vec<InstallOp>,
}

/// Install operation - declarative actions to perform during installation
/// These enable idempotent, reversible installation steps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallOp {
    /// Merge JSON object into existing file (RFC 7396 JSON Merge Patch)
    /// If file doesn't exist, creates it with the object content
    JsonMerge {
        /// Target file path (supports variable templates)
        file: String,
        /// JSON object to merge
        object: serde_json::Value,
        /// Description for this operation
        #[serde(default)]
        description: Option<String>,
        /// Create parent directories if needed
        #[serde(default = "default_true")]
        create_parents: bool,
    },
    /// Apply JSON Patch (RFC 6902) operations to a file
    /// More precise than merge for specific modifications
    JsonPatch {
        /// Possible file paths to patch (first existing is used)
        file_candidates: Vec<String>,
        /// JSON Patch operations
        patch: Vec<JsonPatchOp>,
        /// Description for this operation
        #[serde(default)]
        description: Option<String>,
        /// Create file if none of the candidates exist
        #[serde(default)]
        create_if_missing: bool,
        /// Initial content if creating new file
        #[serde(default)]
        initial_content: Option<serde_json::Value>,
    },
    /// Copy file(s) to destination
    Copy {
        /// Source path (relative to assets_root)
        src: String,
        /// Destination path (supports variables)
        dst: String,
        /// Overwrite existing files
        #[serde(default)]
        overwrite: bool,
        /// Description for this operation
        #[serde(default)]
        description: Option<String>,
    },
    /// Create a directory (with parents)
    Mkdir {
        /// Directory path to create (supports variables)
        path: String,
        /// Description for this operation
        #[serde(default)]
        description: Option<String>,
    },
    /// Create a symlink
    Symlink {
        /// Link source (what to link to)
        src: String,
        /// Link target (where to create the link)
        dst: String,
        /// Description for this operation
        #[serde(default)]
        description: Option<String>,
    },
    /// Execute a shell command (with caution)
    Exec {
        /// Command to execute
        command: String,
        /// Working directory (supports variables)
        #[serde(default)]
        cwd: Option<String>,
        /// Whether to require user confirmation
        #[serde(default = "default_true")]
        require_confirm: bool,
        /// Description for this operation
        #[serde(default)]
        description: Option<String>,
    },
    /// Append text to a file
    AppendText {
        /// Target file path
        file: String,
        /// Text content to append
        content: String,
        /// Delimiter to add before content (e.g., newline)
        #[serde(default = "default_newline")]
        delimiter: String,
        /// Description for this operation
        #[serde(default)]
        description: Option<String>,
    },
}

fn default_true() -> bool {
    true
}

fn default_newline() -> String {
    "\n".to_string()
}

/// JSON Patch operation (RFC 6902)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPatchOp {
    /// Operation type: add, remove, replace, move, copy, test
    pub op: JsonPatchOpType,
    /// JSON Pointer path
    pub path: String,
    /// Value for add/replace/test operations
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// Source path for move/copy operations
    #[serde(default)]
    pub from: Option<String>,
}

/// JSON Patch operation types (RFC 6902)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonPatchOpType {
    Add,
    Remove,
    Replace,
    Move,
    Copy,
    Test,
}

/// Target kind for specialized handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// Claude Code plugin ecosystem
    ClaudePlugin,
    /// Cursor MCP configuration
    CursorMcp,
    /// VS Code extension/config
    VsCodeExtension,
    /// Generic file bundle
    #[default]
    Generic,
}

/// Installation scope for targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallScope {
    /// User-level installation (~/.claude/, ~/.cursor/)
    User,
    /// Project-level installation (.claude/, .cursor/)
    Project,
    /// Local/workspace-only installation (not synced)
    Local,
    /// Enterprise/organization-level
    Enterprise,
}

/// Detection rule to check if a target is installed/available
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetectRule {
    /// Check if a file exists at the given path
    FileExists {
        path: String,
        #[serde(default)]
        when: Option<ConditionalExpr>,
    },
    /// Check if a directory exists
    DirectoryExists {
        path: String,
        #[serde(default)]
        when: Option<ConditionalExpr>,
    },
    /// Check if a command is available in PATH
    CommandExists {
        command: String,
    },
    /// Check if an environment variable is set
    EnvVarSet {
        name: String,
    },
    /// Check JSON file for a specific key/value
    JsonHasKey {
        path: String,
        key: String,
        #[serde(default)]
        value: Option<serde_json::Value>,
    },
}

/// Conditional expression for detection rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalExpr {
    /// Variable name to check
    pub var: String,
    /// Condition: "is_set", "is_not_set", "equals"
    #[serde(default)]
    pub is_set: Option<bool>,
    /// Value to compare against (for "equals" condition)
    #[serde(default)]
    pub equals: Option<String>,
}

/// File copy operation during installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCopyStep {
    /// Source path (relative to assets_root in plugin package)
    pub from: String,
    /// Destination path (supports variable templates like ${home}, ${project_dir})
    pub to: String,
    /// Copy strategy
    #[serde(default)]
    pub strategy: CopyStrategy,
    /// Whether to overwrite existing files (for "copy" strategy)
    #[serde(default)]
    pub overwrite: bool,
    /// For merge_json strategy: which key to merge under
    #[serde(default)]
    pub merge_key: Option<String>,
    /// Optional description for this step
    #[serde(default)]
    pub description: Option<String>,
}

/// Strategy for file copy/install operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CopyStrategy {
    /// Simple file copy (replace if overwrite=true)
    #[default]
    Copy,
    /// Copy entire directory recursively
    CopyDir,
    /// Merge into existing JSON file using RFC 7396 JSON Merge Patch
    MergeJson,
    /// Append content to existing text file
    AppendText,
    /// Create symlink instead of copying
    Symlink,
}

/// Template file specification for variable substitution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSpec {
    /// Path to template file (relative to assets_root)
    pub file: String,
    /// Variables used in this template (for validation)
    #[serde(default)]
    pub vars: Vec<String>,
    /// Output path (if different from source)
    #[serde(default)]
    pub output: Option<String>,
}

/// Command to show or execute during installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallCommand {
    /// Human-readable label for this command
    pub label: String,
    /// The actual command to run
    pub command: String,
    /// Whether to use shell execution
    #[serde(default)]
    pub shell: bool,
    /// Run automatically or just display to user
    #[serde(default)]
    pub auto_run: bool,
    /// Working directory (supports variables)
    #[serde(default)]
    pub cwd: Option<String>,
    /// Required confirmation message before running
    #[serde(default)]
    pub confirm: Option<String>,
}

impl PluginManifest {
    /// Load manifest from a file
    pub fn from_file(path: &Path) -> PluginResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| PluginError::ManifestNotFound(format!("{}: {}", path.display(), e)))?;
        Self::from_str(&content)
    }

    /// Load manifest from a plugin directory
    pub fn from_directory(dir: &Path) -> PluginResult<Self> {
        let manifest_path = dir.join(MANIFEST_FILENAME);
        Self::from_file(&manifest_path)
    }

    /// Parse manifest from string
    pub fn from_str(content: &str) -> PluginResult<Self> {
        let manifest: PluginManifest = toml::from_str(content)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate the manifest
    pub fn validate(&self) -> PluginResult<()> {
        // Check schema version
        if self.schema_version > MAX_SCHEMA_VERSION {
            return Err(PluginError::UnsupportedSchemaVersion(
                self.schema_version,
                MAX_SCHEMA_VERSION,
            ));
        }

        // Validate plugin ID
        if self.plugin.id.is_empty() {
            return Err(PluginError::InvalidManifest(
                "Plugin ID cannot be empty".into(),
            ));
        }

        if !self
            .plugin
            .id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(PluginError::InvalidManifest(
                "Plugin ID can only contain alphanumeric characters, hyphens, and underscores"
                    .into(),
            ));
        }

        // Validate version
        semver::Version::parse(&self.plugin.version)?;

        // Validate min agentreplay version
        semver::Version::parse(&self.plugin.min_agentreplay_version)?;

        // Validate dependencies
        for (name, spec) in &self.dependencies {
            match spec {
                DependencySpec::Version(v) => {
                    semver::VersionReq::parse(v).map_err(|e| {
                        PluginError::InvalidManifest(format!(
                            "Invalid dependency version for '{}': {}",
                            name, e
                        ))
                    })?;
                }
                DependencySpec::Detailed(d) => {
                    semver::VersionReq::parse(&d.version).map_err(|e| {
                        PluginError::InvalidManifest(format!(
                            "Invalid dependency version for '{}': {}",
                            name, e
                        ))
                    })?;
                }
            }
        }

        // Validate bundle configuration (schema v2+)
        if let Some(bundle) = &self.bundle {
            self.validate_bundle(bundle)?;
        }

        Ok(())
    }

    /// Validate bundle configuration
    fn validate_bundle(&self, bundle: &BundleManifest) -> PluginResult<()> {
        // Validate bundle version is valid semver
        semver::Version::parse(&bundle.bundle_version).map_err(|e| {
            PluginError::InvalidManifest(format!("Invalid bundle_version: {}", e))
        })?;

        // Validate target IDs are unique
        let mut target_ids = HashSet::new();
        for target in &bundle.targets {
            if !target_ids.insert(&target.id) {
                return Err(PluginError::InvalidManifest(format!(
                    "Duplicate bundle target ID: '{}'",
                    target.id
                )));
            }

            // Validate target ID format
            if target.id.is_empty() {
                return Err(PluginError::InvalidManifest(
                    "Bundle target ID cannot be empty".into(),
                ));
            }

            // Validate install_md path if provided
            if let Some(install_md) = &target.install_md {
                if install_md.contains("..") {
                    return Err(PluginError::InvalidManifest(format!(
                        "install_md path cannot contain '..': {}",
                        install_md
                    )));
                }
            }

            // Validate files_to_copy paths
            for step in &target.files_to_copy {
                if step.from.contains("..") {
                    return Err(PluginError::InvalidManifest(format!(
                        "files_to_copy 'from' path cannot contain '..': {}",
                        step.from
                    )));
                }
            }
        }

        // Validate variable names are valid identifiers
        for var in &bundle.variables {
            if var.name.is_empty() {
                return Err(PluginError::InvalidManifest(
                    "Bundle variable name cannot be empty".into(),
                ));
            }
            if !var.name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(PluginError::InvalidManifest(format!(
                    "Bundle variable name '{}' can only contain alphanumeric characters and underscores",
                    var.name
                )));
            }
        }

        Ok(())
    }

    /// Get the plugin ID
    pub fn id(&self) -> &str {
        &self.plugin.id
    }

    /// Get the plugin version
    pub fn version(&self) -> semver::Version {
        semver::Version::parse(&self.plugin.version).unwrap()
    }

    /// Compute content hash for integrity verification
    pub fn content_hash(&self) -> String {
        let content = toml::to_string(self).unwrap_or_default();
        let hash = blake3::hash(content.as_bytes());
        hex::encode(hash.as_bytes())
    }

    /// Check if this plugin has bundle integrations
    pub fn has_bundle(&self) -> bool {
        self.bundle.is_some()
    }

    /// Get bundle targets if available
    pub fn bundle_targets(&self) -> Vec<&BundleTarget> {
        self.bundle
            .as_ref()
            .map(|b| b.targets.iter().collect())
            .unwrap_or_default()
    }

    /// Get a specific bundle target by ID
    pub fn get_bundle_target(&self, target_id: &str) -> Option<&BundleTarget> {
        self.bundle
            .as_ref()
            .and_then(|b| b.targets.iter().find(|t| t.id == target_id))
    }

    /// Check if this is a bundle-only plugin (no runtime entry points)
    pub fn is_bundle_only(&self) -> bool {
        self.has_bundle()
            && self.entry.native.is_none()
            && self.entry.wasm.is_none()
            && self.entry.script.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r#"
[plugin]
id = "sample-evaluator"
name = "Sample Evaluator"
version = "1.0.0"
description = "A sample evaluator plugin"
type = "evaluator"

[capabilities]
read_traces = true

[entry]
[entry.native]
path = "lib/sample_evaluator"
"#;

    const BUNDLE_MANIFEST: &str = r#"
schema_version = 2

[plugin]
id = "agentreplay-suite"
name = "Agentreplay Suite"
version = "1.0.0"
description = "Agent tracing + memory + Claude/Cursor integrations"
type = "bundle"

[capabilities]
read_traces = true

[bundle]
bundle_version = "1.0.0"
default_install_mode = "guided"
assets_root = "integrations"

[[bundle.variables]]
name = "project_dir"
label = "Project folder (for project-scoped installs)"
kind = "directory"
required = false

[[bundle.targets]]
id = "claude-code"
display_name = "Claude Code"
kind = "claude_plugin"
install_md = "claude/install.md"
scopes = ["user", "project"]
default_scope = "user"

[[bundle.targets.detect]]
type = "file_exists"
path = "${home}/.claude/settings.json"

[[bundle.targets.files_to_copy]]
from = "claude/plugin/.claude-plugin/plugin.json"
to = "${install_root}/.claude-plugin/plugin.json"
strategy = "copy"
overwrite = true

[[bundle.targets]]
id = "cursor"
display_name = "Cursor"
kind = "cursor_mcp"
install_md = "cursor/install.md"
scopes = ["user", "project"]

[[bundle.targets.files_to_copy]]
from = "cursor/mcp.agentreplay.json"
to = "${cursor_mcp_path}"
strategy = "merge_json"
merge_key = "mcpServers"
"#;

    #[test]
    fn test_parse_manifest() {
        let manifest = PluginManifest::from_str(SAMPLE_MANIFEST).unwrap();
        assert_eq!(manifest.plugin.id, "sample-evaluator");
        assert_eq!(manifest.plugin.name, "Sample Evaluator");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert_eq!(manifest.plugin.plugin_type, PluginType::Evaluator);
        assert!(manifest.capabilities.read_traces);
        assert!(!manifest.has_bundle());
    }

    #[test]
    fn test_validate_invalid_id() {
        let manifest_str = r#"
[plugin]
id = "invalid id with spaces"
name = "Test"
version = "1.0.0"
type = "evaluator"
"#;
        let result = PluginManifest::from_str(manifest_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_bundle_manifest() {
        let manifest = PluginManifest::from_str(BUNDLE_MANIFEST).unwrap();
        assert_eq!(manifest.schema_version, 2);
        assert_eq!(manifest.plugin.id, "agentreplay-suite");
        assert_eq!(manifest.plugin.plugin_type, PluginType::Bundle);
        assert!(manifest.has_bundle());
        
        let bundle = manifest.bundle.as_ref().unwrap();
        assert_eq!(bundle.bundle_version, "1.0.0");
        assert_eq!(bundle.default_install_mode, InstallMode::Guided);
        assert_eq!(bundle.assets_root, "integrations");
        assert_eq!(bundle.variables.len(), 1);
        assert_eq!(bundle.variables[0].name, "project_dir");
        assert_eq!(bundle.targets.len(), 2);
    }

    #[test]
    fn test_bundle_targets() {
        let manifest = PluginManifest::from_str(BUNDLE_MANIFEST).unwrap();
        let targets = manifest.bundle_targets();
        assert_eq!(targets.len(), 2);
        
        let claude_target = manifest.get_bundle_target("claude-code").unwrap();
        assert_eq!(claude_target.display_name, "Claude Code");
        assert_eq!(claude_target.kind, TargetKind::ClaudePlugin);
        assert_eq!(claude_target.scopes.len(), 2);
        assert!(claude_target.scopes.contains(&InstallScope::User));
        assert!(claude_target.scopes.contains(&InstallScope::Project));
        
        let cursor_target = manifest.get_bundle_target("cursor").unwrap();
        assert_eq!(cursor_target.display_name, "Cursor");
        assert_eq!(cursor_target.kind, TargetKind::CursorMcp);
    }

    #[test]
    fn test_bundle_files_to_copy() {
        let manifest = PluginManifest::from_str(BUNDLE_MANIFEST).unwrap();
        let claude_target = manifest.get_bundle_target("claude-code").unwrap();
        
        assert_eq!(claude_target.files_to_copy.len(), 1);
        let step = &claude_target.files_to_copy[0];
        assert_eq!(step.from, "claude/plugin/.claude-plugin/plugin.json");
        assert_eq!(step.strategy, CopyStrategy::Copy);
        assert!(step.overwrite);
    }

    #[test]
    fn test_bundle_merge_json_strategy() {
        let manifest = PluginManifest::from_str(BUNDLE_MANIFEST).unwrap();
        let cursor_target = manifest.get_bundle_target("cursor").unwrap();
        
        let step = &cursor_target.files_to_copy[0];
        assert_eq!(step.strategy, CopyStrategy::MergeJson);
        assert_eq!(step.merge_key.as_deref(), Some("mcpServers"));
    }

    #[test]
    fn test_is_bundle_only() {
        let manifest = PluginManifest::from_str(BUNDLE_MANIFEST).unwrap();
        assert!(manifest.is_bundle_only());
        
        let regular_manifest = PluginManifest::from_str(SAMPLE_MANIFEST).unwrap();
        assert!(!regular_manifest.is_bundle_only());
    }

    #[test]
    fn test_duplicate_target_id_fails() {
        let bad_manifest = r#"
schema_version = 2

[plugin]
id = "test-plugin"
name = "Test"
version = "1.0.0"
type = "bundle"

[bundle]
bundle_version = "1.0.0"

[[bundle.targets]]
id = "same-id"
display_name = "Target 1"

[[bundle.targets]]
id = "same-id"
display_name = "Target 2"
"#;
        let result = PluginManifest::from_str(bad_manifest);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Duplicate bundle target ID"));
    }

    #[test]
    fn test_path_traversal_blocked() {
        let bad_manifest = r#"
schema_version = 2

[plugin]
id = "test-plugin"
name = "Test"
version = "1.0.0"
type = "bundle"

[bundle]
bundle_version = "1.0.0"

[[bundle.targets]]
id = "test"
display_name = "Test"

[[bundle.targets.files_to_copy]]
from = "../../../etc/passwd"
to = "${home}/test"
"#;
        let result = PluginManifest::from_str(bad_manifest);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot contain '..'"));
    }
}
