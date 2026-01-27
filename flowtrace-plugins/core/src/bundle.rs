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

//! Bundle installer for external integration plugins
//!
//! Handles installation of file-based bundles to external targets like
//! Claude Code, Cursor, VS Code, and other editor/agent ecosystems.

use crate::error::{PluginError, PluginResult};
use crate::manifest::{
    BundleTarget, ConditionalExpr, CopyStrategy, DetectRule,
    InstallScope, PluginManifest, TargetKind, InstallOp,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============================================================================
// Variable Resolution
// ============================================================================

/// Built-in variables available for path templates
#[derive(Debug, Clone)]
pub struct BuiltinVariables {
    /// User home directory
    pub home: PathBuf,
    /// User config directory (~/.config on Linux, ~/Library/Application Support on macOS)
    pub config_dir: PathBuf,
    /// User data directory
    pub data_dir: PathBuf,
    /// Current working directory
    pub cwd: PathBuf,
}

impl BuiltinVariables {
    /// Create builtin variables from system paths
    pub fn from_system() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_else(|| PathBuf::from("~")),
            config_dir: dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config")),
            data_dir: dirs::data_dir().unwrap_or_else(|| PathBuf::from("~/.local/share")),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// Convert to a HashMap for variable substitution
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("home".to_string(), self.home.to_string_lossy().to_string());
        map.insert(
            "config_dir".to_string(),
            self.config_dir.to_string_lossy().to_string(),
        );
        map.insert(
            "data_dir".to_string(),
            self.data_dir.to_string_lossy().to_string(),
        );
        map.insert("cwd".to_string(), self.cwd.to_string_lossy().to_string());
        map
    }
}

/// Variable context for template substitution
#[derive(Debug, Clone, Default)]
pub struct VariableContext {
    /// All resolved variables
    pub variables: HashMap<String, String>,
}

impl VariableContext {
    /// Create a new context with builtin variables
    pub fn new() -> Self {
        let builtins = BuiltinVariables::from_system();
        Self {
            variables: builtins.to_map(),
        }
    }

    /// Add user-provided variables
    pub fn with_user_variables(mut self, vars: HashMap<String, String>) -> Self {
        self.variables.extend(vars);
        self
    }

    /// Add target-specific computed variables
    pub fn with_target_variables(mut self, target: &BundleTarget, scope: InstallScope) -> Self {
        // Compute install_root based on target kind and scope
        let install_root = match (&target.kind, scope) {
            (TargetKind::ClaudePlugin, InstallScope::User) => {
                format!("{}/.claude/plugins/{}", self.get("home"), target.id)
            }
            (TargetKind::ClaudePlugin, InstallScope::Project) => {
                format!(
                    "{}/.claude/plugins/{}",
                    self.get("project_dir"),
                    target.id
                )
            }
            (TargetKind::ClaudePlugin, InstallScope::Local) => {
                format!(
                    "{}/.claude/plugins.local/{}",
                    self.get("project_dir"),
                    target.id
                )
            }
            (TargetKind::CursorMcp, InstallScope::User) => {
                format!("{}/.cursor", self.get("home"))
            }
            (TargetKind::CursorMcp, InstallScope::Project) => {
                format!("{}/.cursor", self.get("project_dir"))
            }
            _ => self.get("home"),
        };
        self.variables.insert("install_root".to_string(), install_root);

        // Cursor MCP path
        let cursor_mcp_path = match scope {
            InstallScope::User => format!("{}/.cursor/mcp.json", self.get("home")),
            InstallScope::Project => format!("{}/.cursor/mcp.json", self.get("project_dir")),
            _ => format!("{}/.cursor/mcp.json", self.get("home")),
        };
        self.variables
            .insert("cursor_mcp_path".to_string(), cursor_mcp_path);

        self
    }

    /// Get a variable value
    pub fn get(&self, name: &str) -> String {
        self.variables
            .get(name)
            .cloned()
            .unwrap_or_else(|| format!("${{{}}}", name))
    }

    /// Check if a variable is set
    pub fn is_set(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Substitute variables in a path template
    pub fn substitute(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (name, value) in &self.variables {
            result = result.replace(&format!("${{{}}}", name), value);
        }
        result
    }
}

// ============================================================================
// Detection
// ============================================================================

/// Result of running detection rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    /// Target ID
    pub target_id: String,
    /// Whether the target is detected/available
    pub detected: bool,
    /// Individual rule results
    pub rule_results: Vec<RuleResult>,
    /// Additional info about the detection
    pub info: HashMap<String, String>,
}

/// Result of a single detection rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleResult {
    /// Rule description
    pub rule: String,
    /// Whether the rule passed
    pub passed: bool,
    /// Additional details
    pub details: Option<String>,
}

/// Run detection rules for a target
pub fn detect_target(
    target: &BundleTarget,
    context: &VariableContext,
) -> PluginResult<DetectionResult> {
    let mut rule_results = Vec::new();
    let mut all_passed = true;

    for rule in &target.detect {
        let result = evaluate_detect_rule(rule, context)?;
        if !result.passed {
            all_passed = false;
        }
        rule_results.push(result);
    }

    Ok(DetectionResult {
        target_id: target.id.clone(),
        detected: all_passed,
        rule_results,
        info: HashMap::new(),
    })
}

fn evaluate_detect_rule(rule: &DetectRule, context: &VariableContext) -> PluginResult<RuleResult> {
    match rule {
        DetectRule::FileExists { path, when } => {
            // Check condition first
            if let Some(cond) = when {
                if !evaluate_condition(cond, context) {
                    return Ok(RuleResult {
                        rule: format!("file_exists: {}", path),
                        passed: true, // Skip if condition not met
                        details: Some("Condition not met, skipped".to_string()),
                    });
                }
            }

            let resolved_path = context.substitute(path);
            let exists = Path::new(&resolved_path).exists();
            Ok(RuleResult {
                rule: format!("file_exists: {}", path),
                passed: exists,
                details: Some(format!("Resolved to: {}", resolved_path)),
            })
        }
        DetectRule::DirectoryExists { path, when } => {
            if let Some(cond) = when {
                if !evaluate_condition(cond, context) {
                    return Ok(RuleResult {
                        rule: format!("directory_exists: {}", path),
                        passed: true,
                        details: Some("Condition not met, skipped".to_string()),
                    });
                }
            }

            let resolved_path = context.substitute(path);
            let exists = Path::new(&resolved_path).is_dir();
            Ok(RuleResult {
                rule: format!("directory_exists: {}", path),
                passed: exists,
                details: Some(format!("Resolved to: {}", resolved_path)),
            })
        }
        DetectRule::CommandExists { command } => {
            let exists = which::which(command).is_ok();
            Ok(RuleResult {
                rule: format!("command_exists: {}", command),
                passed: exists,
                details: None,
            })
        }
        DetectRule::EnvVarSet { name } => {
            let is_set = std::env::var(name).is_ok();
            Ok(RuleResult {
                rule: format!("env_var_set: {}", name),
                passed: is_set,
                details: None,
            })
        }
        DetectRule::JsonHasKey { path, key, value } => {
            let resolved_path = context.substitute(path);
            let result = check_json_key(&resolved_path, key, value.as_ref());
            Ok(RuleResult {
                rule: format!("json_has_key: {}[{}]", path, key),
                passed: result,
                details: Some(format!("Resolved to: {}", resolved_path)),
            })
        }
    }
}

fn evaluate_condition(cond: &ConditionalExpr, context: &VariableContext) -> bool {
    if let Some(is_set) = cond.is_set {
        return context.is_set(&cond.var) == is_set;
    }
    if let Some(equals) = &cond.equals {
        return context.get(&cond.var) == *equals;
    }
    true
}

fn check_json_key(path: &str, key: &str, expected_value: Option<&serde_json::Value>) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json): Result<serde_json::Value, _> = serde_json::from_str(&content) else {
        return false;
    };

    let Some(value) = json.get(key) else {
        return false;
    };

    match expected_value {
        Some(expected) => value == expected,
        None => true, // Just check key exists
    }
}

// ============================================================================
// Installation Plan
// ============================================================================

/// A planned installation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    /// Target ID
    pub target_id: String,
    /// Target display name
    pub display_name: String,
    /// Installation scope
    pub scope: InstallScope,
    /// Planned file operations
    pub operations: Vec<PlannedOperation>,
    /// Planned install ops (json_merge, json_patch, etc.)
    pub install_ops: Vec<PlannedInstallOp>,
    /// Commands to run
    pub commands: Vec<PlannedCommand>,
    /// Warnings or conflicts detected
    pub warnings: Vec<String>,
    /// Estimated total bytes to copy
    pub total_bytes: u64,
}

/// A planned file operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedOperation {
    /// Source file (relative to plugin assets)
    pub source: PathBuf,
    /// Destination path (resolved)
    pub destination: PathBuf,
    /// Operation type
    pub operation: CopyStrategy,
    /// Whether destination already exists
    pub exists: bool,
    /// File size in bytes (if known)
    pub size_bytes: Option<u64>,
    /// Description
    pub description: Option<String>,
}

/// A planned command to run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedCommand {
    /// Command label
    pub label: String,
    /// Resolved command
    pub command: String,
    /// Whether this runs automatically
    pub auto_run: bool,
    /// Confirmation message if required
    pub confirm: Option<String>,
}

/// A planned install op from the manifest ops array
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedInstallOp {
    /// Operation index (for ordering)
    pub index: usize,
    /// Operation type
    pub op_type: String,
    /// Target file(s) affected
    pub target_files: Vec<PathBuf>,
    /// Description
    pub description: Option<String>,
    /// The resolved operation
    pub resolved: ResolvedInstallOp,
}

/// Resolved install operation ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedInstallOp {
    JsonMerge {
        file: PathBuf,
        object: serde_json::Value,
        create_parents: bool,
    },
    JsonPatch {
        file: PathBuf,
        patch: Vec<crate::manifest::JsonPatchOp>,
        create_if_missing: bool,
        initial_content: Option<serde_json::Value>,
    },
    Copy {
        src: PathBuf,
        dst: PathBuf,
        overwrite: bool,
    },
    Mkdir {
        path: PathBuf,
    },
    Symlink {
        src: PathBuf,
        dst: PathBuf,
    },
    Exec {
        command: String,
        cwd: Option<PathBuf>,
        require_confirm: bool,
    },
    AppendText {
        file: PathBuf,
        content: String,
        delimiter: String,
    },
}

// ============================================================================
// Install Receipts (for rollback/verify/uninstall)
// ============================================================================

/// Install receipt - records what was installed for rollback/verify/uninstall
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReceipt {
    /// Receipt schema version
    pub schema_version: u32,
    /// Plugin ID
    pub plugin_id: String,
    /// Target ID
    pub target_id: String,
    /// Installation scope
    pub scope: InstallScope,
    /// When the install was performed
    pub installed_at: chrono::DateTime<chrono::Utc>,
    /// User who performed the install (if known)
    #[serde(default)]
    pub installed_by: Option<String>,
    /// Files that were created
    pub created_files: Vec<CreatedFile>,
    /// JSON modifications that were made
    pub json_modifications: Vec<JsonModification>,
    /// Directories that were created
    pub created_dirs: Vec<PathBuf>,
    /// Install operations performed (for reverse order)
    pub operations_performed: Vec<PerformedOperation>,
}

/// Record of a file created during installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedFile {
    /// Path to the file
    pub path: PathBuf,
    /// SHA256 hash of the content
    pub content_hash: String,
    /// Whether the file existed before (was overwritten)
    pub was_overwrite: bool,
    /// Backup path if file was backed up
    #[serde(default)]
    pub backup_path: Option<PathBuf>,
}

/// Record of a JSON modification for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonModification {
    /// Target file
    pub file: PathBuf,
    /// Keys that were added
    pub added_keys: Vec<String>,
    /// Original values of keys that were modified
    pub original_values: HashMap<String, serde_json::Value>,
    /// Keys that were removed
    pub removed_keys: Vec<String>,
}

/// Record of an operation performed (for rollback)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformedOperation {
    /// Operation index
    pub index: usize,
    /// Operation type
    pub op_type: String,
    /// Reverse operation (for rollback)
    pub reverse: Option<ReverseOp>,
    /// Whether operation succeeded
    pub success: bool,
}

/// Reverse operation for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReverseOp {
    /// Delete a file
    DeleteFile { path: PathBuf },
    /// Restore a file from backup
    RestoreFile { path: PathBuf, backup: PathBuf },
    /// Remove directory (if empty)
    RemoveDir { path: PathBuf },
    /// Restore JSON to original state
    RestoreJson { file: PathBuf, original: serde_json::Value },
    /// Remove keys from JSON
    RemoveJsonKeys { file: PathBuf, keys: Vec<String> },
    /// No reverse possible
    None { reason: String },
}

impl InstallReceipt {
    /// Create a new empty receipt
    pub fn new(plugin_id: String, target_id: String, scope: InstallScope) -> Self {
        Self {
            schema_version: 1,
            plugin_id,
            target_id,
            scope,
            installed_at: chrono::Utc::now(),
            installed_by: None,
            created_files: Vec::new(),
            json_modifications: Vec::new(),
            created_dirs: Vec::new(),
            operations_performed: Vec::new(),
        }
    }

    /// Save receipt to file
    pub fn save(&self, receipt_path: &Path) -> PluginResult<()> {
        let content = serde_json::to_string_pretty(self).map_err(|e| {
            PluginError::InstallFailed(format!("Failed to serialize receipt: {}", e))
        })?;
        
        if let Some(parent) = receipt_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        
        std::fs::write(receipt_path, content).map_err(|e| {
            PluginError::InstallFailed(format!("Failed to save receipt: {}", e))
        })?;
        
        Ok(())
    }

    /// Load receipt from file
    pub fn load(receipt_path: &Path) -> PluginResult<Self> {
        let content = std::fs::read_to_string(receipt_path).map_err(|e| {
            PluginError::InstallFailed(format!("Failed to read receipt: {}", e))
        })?;
        
        serde_json::from_str(&content).map_err(|e| {
            PluginError::InstallFailed(format!("Failed to parse receipt: {}", e))
        })
    }

    /// Get receipt file path for a given install
    pub fn receipt_path(flowtrace_data_dir: &Path, plugin_id: &str, target_id: &str) -> PathBuf {
        flowtrace_data_dir
            .join("receipts")
            .join(format!("{}_{}.json", plugin_id, target_id))
    }
}

/// Verify an installation against its receipt
pub fn verify_installation(receipt: &InstallReceipt) -> VerifyResult {
    let mut issues = Vec::new();
    let mut verified_files = 0;
    let mut missing_files = 0;
    let mut modified_files = 0;

    for file in &receipt.created_files {
        if file.path.exists() {
            // Check hash
            if let Ok(content) = std::fs::read(&file.path) {
                let hash = sha256_hash(&content);
                if hash == file.content_hash {
                    verified_files += 1;
                } else {
                    modified_files += 1;
                    issues.push(format!(
                        "File {} has been modified since installation",
                        file.path.display()
                    ));
                }
            }
        } else {
            missing_files += 1;
            issues.push(format!("File {} is missing", file.path.display()));
        }
    }

    VerifyResult {
        valid: issues.is_empty(),
        verified_files,
        missing_files,
        modified_files,
        issues,
    }
}

/// Result of verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub valid: bool,
    pub verified_files: usize,
    pub missing_files: usize,
    pub modified_files: usize,
    pub issues: Vec<String>,
}

/// Compute SHA256 hash of content
fn sha256_hash(content: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

/// Create an installation plan for a target
pub fn create_install_plan(
    manifest: &PluginManifest,
    target_id: &str,
    scope: InstallScope,
    plugin_path: &Path,
    user_variables: HashMap<String, String>,
) -> PluginResult<InstallPlan> {
    let bundle = manifest
        .bundle
        .as_ref()
        .ok_or_else(|| PluginError::InvalidManifest("Plugin has no bundle configuration".into()))?;

    let target = bundle
        .targets
        .iter()
        .find(|t| t.id == target_id)
        .ok_or_else(|| {
            PluginError::InvalidManifest(format!("Bundle target '{}' not found", target_id))
        })?;

    // Build variable context
    let context = VariableContext::new()
        .with_user_variables(user_variables)
        .with_target_variables(target, scope);

    let assets_path = plugin_path.join(&bundle.assets_root);

    // Plan file operations
    let mut operations = Vec::new();
    let mut total_bytes = 0u64;
    let mut warnings = Vec::new();

    for step in &target.files_to_copy {
        let source = assets_path.join(&step.from);
        let destination = PathBuf::from(context.substitute(&step.to));
        let exists = destination.exists();

        // Get file size if source exists
        let size_bytes = std::fs::metadata(&source).ok().map(|m| m.len());
        if let Some(size) = size_bytes {
            total_bytes += size;
        }

        // Check for potential conflicts
        if exists && !step.overwrite && step.strategy == CopyStrategy::Copy {
            warnings.push(format!(
                "File {} exists and overwrite=false",
                destination.display()
            ));
        }

        operations.push(PlannedOperation {
            source,
            destination,
            operation: step.strategy,
            exists,
            size_bytes,
            description: step.description.clone(),
        });
    }

    // Plan commands
    let commands: Vec<PlannedCommand> = target
        .commands
        .iter()
        .map(|cmd| PlannedCommand {
            label: cmd.label.clone(),
            command: context.substitute(&cmd.command),
            auto_run: cmd.auto_run,
            confirm: cmd.confirm.clone(),
        })
        .collect();

    // Plan install ops
    let install_ops = plan_install_ops(&target.ops, &assets_path, &context)?;

    Ok(InstallPlan {
        target_id: target.id.clone(),
        display_name: target.display_name.clone(),
        scope,
        operations,
        install_ops,
        commands,
        warnings,
        total_bytes,
    })
}

/// Plan install ops from manifest
fn plan_install_ops(
    ops: &[InstallOp],
    assets_path: &Path,
    context: &VariableContext,
) -> PluginResult<Vec<PlannedInstallOp>> {
    let mut planned = Vec::new();

    for (index, op) in ops.iter().enumerate() {
        let (op_type, target_files, description, resolved) = match op {
            InstallOp::JsonMerge { file, object, description, create_parents } => {
                let resolved_file = PathBuf::from(context.substitute(file));
                (
                    "json_merge".to_string(),
                    vec![resolved_file.clone()],
                    description.clone(),
                    ResolvedInstallOp::JsonMerge {
                        file: resolved_file,
                        object: object.clone(),
                        create_parents: *create_parents,
                    },
                )
            }
            InstallOp::JsonPatch { file_candidates, patch, description, create_if_missing, initial_content } => {
                let resolved_files: Vec<PathBuf> = file_candidates
                    .iter()
                    .map(|f| PathBuf::from(context.substitute(f)))
                    .collect();
                
                // Find first existing file or use first candidate
                let target_file = resolved_files
                    .iter()
                    .find(|p| p.exists())
                    .cloned()
                    .unwrap_or_else(|| resolved_files.first().cloned().unwrap_or_default());

                (
                    "json_patch".to_string(),
                    resolved_files,
                    description.clone(),
                    ResolvedInstallOp::JsonPatch {
                        file: target_file,
                        patch: patch.clone(),
                        create_if_missing: *create_if_missing,
                        initial_content: initial_content.clone(),
                    },
                )
            }
            InstallOp::Copy { src, dst, overwrite, description } => {
                let src_path = assets_path.join(src);
                let dst_path = PathBuf::from(context.substitute(dst));
                (
                    "copy".to_string(),
                    vec![dst_path.clone()],
                    description.clone(),
                    ResolvedInstallOp::Copy {
                        src: src_path,
                        dst: dst_path,
                        overwrite: *overwrite,
                    },
                )
            }
            InstallOp::Mkdir { path, description } => {
                let resolved_path = PathBuf::from(context.substitute(path));
                (
                    "mkdir".to_string(),
                    vec![resolved_path.clone()],
                    description.clone(),
                    ResolvedInstallOp::Mkdir { path: resolved_path },
                )
            }
            InstallOp::Symlink { src, dst, description } => {
                let src_path = PathBuf::from(context.substitute(src));
                let dst_path = PathBuf::from(context.substitute(dst));
                (
                    "symlink".to_string(),
                    vec![dst_path.clone()],
                    description.clone(),
                    ResolvedInstallOp::Symlink { src: src_path, dst: dst_path },
                )
            }
            InstallOp::Exec { command, cwd, require_confirm, description } => {
                let resolved_command = context.substitute(command);
                let resolved_cwd = cwd.as_ref().map(|c| PathBuf::from(context.substitute(c)));
                (
                    "exec".to_string(),
                    Vec::new(),
                    description.clone(),
                    ResolvedInstallOp::Exec {
                        command: resolved_command,
                        cwd: resolved_cwd,
                        require_confirm: *require_confirm,
                    },
                )
            }
            InstallOp::AppendText { file, content, delimiter, description } => {
                let resolved_file = PathBuf::from(context.substitute(file));
                (
                    "append_text".to_string(),
                    vec![resolved_file.clone()],
                    description.clone(),
                    ResolvedInstallOp::AppendText {
                        file: resolved_file,
                        content: context.substitute(content),
                        delimiter: delimiter.clone(),
                    },
                )
            }
        };

        planned.push(PlannedInstallOp {
            index,
            op_type,
            target_files,
            description,
            resolved,
        });
    }

    Ok(planned)
}

// ============================================================================
// Execution
// ============================================================================

/// Result of executing an install plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallExecutionResult {
    /// Whether installation succeeded
    pub success: bool,
    /// Operations that completed successfully
    pub completed_operations: Vec<String>,
    /// Operations that failed
    pub failed_operations: Vec<OperationError>,
    /// Commands that were run
    pub commands_run: Vec<CommandResult>,
}

/// A failed operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationError {
    pub operation: String,
    pub error: String,
}

/// Result of running a command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub label: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

/// Execute an installation plan
pub async fn execute_install_plan(plan: &InstallPlan) -> PluginResult<InstallExecutionResult> {
    let mut completed = Vec::new();
    let mut failed = Vec::new();
    let mut commands_run = Vec::new();

    // Execute file operations
    for op in &plan.operations {
        let result = execute_operation(op).await;
        match result {
            Ok(_) => {
                completed.push(format!(
                    "{:?}: {} -> {}",
                    op.operation,
                    op.source.display(),
                    op.destination.display()
                ));
            }
            Err(e) => {
                failed.push(OperationError {
                    operation: format!("{} -> {}", op.source.display(), op.destination.display()),
                    error: e.to_string(),
                });
            }
        }
    }

    // Execute install ops (json_merge, json_patch, etc.)
    for install_op in &plan.install_ops {
        let result = execute_install_op(&install_op.resolved).await;
        match result {
            Ok(_) => {
                let desc = install_op.description.as_deref().unwrap_or(&install_op.op_type);
                completed.push(format!("{}: {}", install_op.op_type, desc));
            }
            Err(e) => {
                failed.push(OperationError {
                    operation: format!("{}[{}]", install_op.op_type, install_op.index),
                    error: e.to_string(),
                });
            }
        }
    }

    // Execute auto-run commands
    for cmd in &plan.commands {
        if cmd.auto_run {
            let result = execute_command(cmd).await;
            commands_run.push(result);
        }
    }

    let success = failed.is_empty();

    Ok(InstallExecutionResult {
        success,
        completed_operations: completed,
        failed_operations: failed,
        commands_run,
    })
}

async fn execute_operation(op: &PlannedOperation) -> PluginResult<()> {
    // Ensure parent directory exists
    if let Some(parent) = op.destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PluginError::InstallFailed(format!(
                "Failed to create directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    match op.operation {
        CopyStrategy::Copy => {
            std::fs::copy(&op.source, &op.destination).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to copy {} to {}: {}",
                    op.source.display(),
                    op.destination.display(),
                    e
                ))
            })?;
        }
        CopyStrategy::CopyDir => {
            copy_dir_recursive(&op.source, &op.destination)?;
        }
        CopyStrategy::MergeJson => {
            merge_json_file(&op.source, &op.destination)?;
        }
        CopyStrategy::AppendText => {
            append_text_file(&op.source, &op.destination)?;
        }
        CopyStrategy::Symlink => {
            #[cfg(unix)]
            std::os::unix::fs::symlink(&op.source, &op.destination).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to create symlink {} -> {}: {}",
                    op.destination.display(),
                    op.source.display(),
                    e
                ))
            })?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(&op.source, &op.destination).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to create symlink {} -> {}: {}",
                    op.destination.display(),
                    op.source.display(),
                    e
                ))
            })?;
        }
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> PluginResult<()> {
    std::fs::create_dir_all(dst).map_err(|e| {
        PluginError::InstallFailed(format!(
            "Failed to create directory {}: {}",
            dst.display(),
            e
        ))
    })?;

    for entry in std::fs::read_dir(src).map_err(|e| {
        PluginError::InstallFailed(format!("Failed to read directory {}: {}", src.display(), e))
    })? {
        let entry = entry.map_err(|e| {
            PluginError::InstallFailed(format!("Failed to read entry in {}: {}", src.display(), e))
        })?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to copy {} to {}: {}",
                    src_path.display(),
                    dst_path.display(),
                    e
                ))
            })?;
        }
    }

    Ok(())
}

/// Merge JSON files using RFC 7396 JSON Merge Patch semantics
fn merge_json_file(src: &Path, dst: &Path) -> PluginResult<()> {
    let src_content = std::fs::read_to_string(src).map_err(|e| {
        PluginError::InstallFailed(format!("Failed to read source {}: {}", src.display(), e))
    })?;

    let src_json: serde_json::Value = serde_json::from_str(&src_content).map_err(|e| {
        PluginError::InstallFailed(format!("Invalid JSON in {}: {}", src.display(), e))
    })?;

    let dst_json = if dst.exists() {
        let dst_content = std::fs::read_to_string(dst).map_err(|e| {
            PluginError::InstallFailed(format!(
                "Failed to read destination {}: {}",
                dst.display(),
                e
            ))
        })?;
        serde_json::from_str(&dst_content).map_err(|e| {
            PluginError::InstallFailed(format!("Invalid JSON in {}: {}", dst.display(), e))
        })?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let merged = json_merge_patch(dst_json, src_json);

    let merged_content = serde_json::to_string_pretty(&merged).map_err(|e| {
        PluginError::InstallFailed(format!("Failed to serialize merged JSON: {}", e))
    })?;

    std::fs::write(dst, merged_content).map_err(|e| {
        PluginError::InstallFailed(format!(
            "Failed to write merged JSON to {}: {}",
            dst.display(),
            e
        ))
    })?;

    Ok(())
}

/// RFC 7396 JSON Merge Patch implementation
fn json_merge_patch(target: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    match (target, patch) {
        (serde_json::Value::Object(mut target_obj), serde_json::Value::Object(patch_obj)) => {
            for (key, patch_value) in patch_obj {
                if patch_value.is_null() {
                    target_obj.remove(&key);
                } else if let Some(target_value) = target_obj.remove(&key) {
                    target_obj.insert(key, json_merge_patch(target_value, patch_value));
                } else {
                    target_obj.insert(key, patch_value);
                }
            }
            serde_json::Value::Object(target_obj)
        }
        (_, patch) => patch,
    }
}

fn append_text_file(src: &Path, dst: &Path) -> PluginResult<()> {
    let src_content = std::fs::read_to_string(src).map_err(|e| {
        PluginError::InstallFailed(format!("Failed to read source {}: {}", src.display(), e))
    })?;

    let existing = if dst.exists() {
        std::fs::read_to_string(dst).unwrap_or_default()
    } else {
        String::new()
    };

    let combined = if existing.is_empty() {
        src_content
    } else {
        format!("{}\n{}", existing.trim_end(), src_content)
    };

    std::fs::write(dst, combined).map_err(|e| {
        PluginError::InstallFailed(format!("Failed to write to {}: {}", dst.display(), e))
    })?;

    Ok(())
}

async fn execute_command(cmd: &PlannedCommand) -> CommandResult {
    use tokio::process::Command;

    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd.command)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            CommandResult {
                label: cmd.label.clone(),
                success: output.status.success(),
                output: if stdout.is_empty() {
                    None
                } else {
                    Some(stdout)
                },
                error: if stderr.is_empty() {
                    None
                } else {
                    Some(stderr)
                },
            }
        }
        Err(e) => CommandResult {
            label: cmd.label.clone(),
            success: false,
            output: None,
            error: Some(e.to_string()),
        },
    }
}

/// Execute a single install operation
async fn execute_install_op(op: &ResolvedInstallOp) -> PluginResult<()> {
    match op {
        ResolvedInstallOp::JsonMerge { file, object, create_parents } => {
            if *create_parents {
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
            }
            
            let existing = if file.exists() {
                let content = std::fs::read_to_string(file).map_err(|e| {
                    PluginError::InstallFailed(format!("Failed to read {}: {}", file.display(), e))
                })?;
                serde_json::from_str(&content).map_err(|e| {
                    PluginError::InstallFailed(format!("Invalid JSON in {}: {}", file.display(), e))
                })?
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            };

            let merged = json_merge_patch(existing, object.clone());
            let content = serde_json::to_string_pretty(&merged).map_err(|e| {
                PluginError::InstallFailed(format!("Failed to serialize JSON: {}", e))
            })?;

            std::fs::write(file, content).map_err(|e| {
                PluginError::InstallFailed(format!("Failed to write {}: {}", file.display(), e))
            })?;
        }
        ResolvedInstallOp::JsonPatch { file, patch, create_if_missing, initial_content } => {
            let mut json = if file.exists() {
                let content = std::fs::read_to_string(file).map_err(|e| {
                    PluginError::InstallFailed(format!("Failed to read {}: {}", file.display(), e))
                })?;
                serde_json::from_str(&content).map_err(|e| {
                    PluginError::InstallFailed(format!("Invalid JSON in {}: {}", file.display(), e))
                })?
            } else if *create_if_missing {
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                initial_content.clone().unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
            } else {
                return Err(PluginError::InstallFailed(format!(
                    "File {} does not exist and create_if_missing=false",
                    file.display()
                )));
            };

            // Apply JSON Patch operations
            for patch_op in patch {
                json = apply_json_patch_op(json, patch_op)?;
            }

            let content = serde_json::to_string_pretty(&json).map_err(|e| {
                PluginError::InstallFailed(format!("Failed to serialize JSON: {}", e))
            })?;

            std::fs::write(file, content).map_err(|e| {
                PluginError::InstallFailed(format!("Failed to write {}: {}", file.display(), e))
            })?;
        }
        ResolvedInstallOp::Copy { src, dst, overwrite } => {
            if dst.exists() && !*overwrite {
                return Err(PluginError::InstallFailed(format!(
                    "Destination {} exists and overwrite=false",
                    dst.display()
                )));
            }
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::copy(src, dst).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to copy {} to {}: {}",
                    src.display(),
                    dst.display(),
                    e
                ))
            })?;
        }
        ResolvedInstallOp::Mkdir { path } => {
            std::fs::create_dir_all(path).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to create directory {}: {}",
                    path.display(),
                    e
                ))
            })?;
        }
        ResolvedInstallOp::Symlink { src, dst } => {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(src, dst).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to create symlink {} -> {}: {}",
                    dst.display(),
                    src.display(),
                    e
                ))
            })?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(src, dst).map_err(|e| {
                PluginError::InstallFailed(format!(
                    "Failed to create symlink {} -> {}: {}",
                    dst.display(),
                    src.display(),
                    e
                ))
            })?;
        }
        ResolvedInstallOp::Exec { command, cwd, require_confirm: _ } => {
            // Note: require_confirm should be handled by the UI before calling execute
            use tokio::process::Command;
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(command);
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }
            let output = cmd.output().await.map_err(|e| {
                PluginError::InstallFailed(format!("Failed to execute command: {}", e))
            })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(PluginError::InstallFailed(format!(
                    "Command failed: {}",
                    stderr
                )));
            }
        }
        ResolvedInstallOp::AppendText { file, content, delimiter } => {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let existing = if file.exists() {
                std::fs::read_to_string(file).unwrap_or_default()
            } else {
                String::new()
            };
            let new_content = if existing.is_empty() {
                content.clone()
            } else {
                format!("{}{}{}", existing.trim_end(), delimiter, content)
            };
            std::fs::write(file, new_content).map_err(|e| {
                PluginError::InstallFailed(format!("Failed to write {}: {}", file.display(), e))
            })?;
        }
    }
    Ok(())
}

/// Apply a single JSON Patch operation (RFC 6902)
fn apply_json_patch_op(
    mut doc: serde_json::Value,
    op: &crate::manifest::JsonPatchOp,
) -> PluginResult<serde_json::Value> {
    use crate::manifest::JsonPatchOpType;
    
    // Parse JSON pointer path
    let path_parts: Vec<&str> = op.path.trim_start_matches('/').split('/').collect();
    
    match op.op {
        JsonPatchOpType::Add => {
            let value = op.value.clone().ok_or_else(|| {
                PluginError::InstallFailed("JSON Patch 'add' requires a value".into())
            })?;
            json_pointer_set(&mut doc, &path_parts, value)?;
        }
        JsonPatchOpType::Remove => {
            json_pointer_remove(&mut doc, &path_parts)?;
        }
        JsonPatchOpType::Replace => {
            let value = op.value.clone().ok_or_else(|| {
                PluginError::InstallFailed("JSON Patch 'replace' requires a value".into())
            })?;
            json_pointer_remove(&mut doc, &path_parts)?;
            json_pointer_set(&mut doc, &path_parts, value)?;
        }
        JsonPatchOpType::Move => {
            let from = op.from.as_ref().ok_or_else(|| {
                PluginError::InstallFailed("JSON Patch 'move' requires a 'from' path".into())
            })?;
            let from_parts: Vec<&str> = from.trim_start_matches('/').split('/').collect();
            let value = json_pointer_get(&doc, &from_parts)?;
            json_pointer_remove(&mut doc, &from_parts)?;
            json_pointer_set(&mut doc, &path_parts, value)?;
        }
        JsonPatchOpType::Copy => {
            let from = op.from.as_ref().ok_or_else(|| {
                PluginError::InstallFailed("JSON Patch 'copy' requires a 'from' path".into())
            })?;
            let from_parts: Vec<&str> = from.trim_start_matches('/').split('/').collect();
            let value = json_pointer_get(&doc, &from_parts)?;
            json_pointer_set(&mut doc, &path_parts, value)?;
        }
        JsonPatchOpType::Test => {
            let expected = op.value.clone().ok_or_else(|| {
                PluginError::InstallFailed("JSON Patch 'test' requires a value".into())
            })?;
            let actual = json_pointer_get(&doc, &path_parts)?;
            if actual != expected {
                return Err(PluginError::InstallFailed(format!(
                    "JSON Patch test failed at {}: expected {:?}, got {:?}",
                    op.path, expected, actual
                )));
            }
        }
    }
    
    Ok(doc)
}

/// Get a value from a JSON document using a pointer path
fn json_pointer_get(doc: &serde_json::Value, parts: &[&str]) -> PluginResult<serde_json::Value> {
    let mut current = doc;
    for part in parts {
        if part.is_empty() {
            continue;
        }
        current = current.get(*part).ok_or_else(|| {
            PluginError::InstallFailed(format!("Path not found: {}", parts.join("/")))
        })?;
    }
    Ok(current.clone())
}

/// Set a value in a JSON document using a pointer path
fn json_pointer_set(doc: &mut serde_json::Value, parts: &[&str], value: serde_json::Value) -> PluginResult<()> {
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
        *doc = value;
        return Ok(());
    }

    let mut current = doc;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == parts.len() - 1 {
            // Last part - set the value
            if let serde_json::Value::Object(obj) = current {
                obj.insert(part.to_string(), value);
                return Ok(());
            } else if let serde_json::Value::Array(arr) = current {
                if *part == "-" {
                    arr.push(value);
                    return Ok(());
                }
                let idx: usize = part.parse().map_err(|_| {
                    PluginError::InstallFailed(format!("Invalid array index: {}", part))
                })?;
                if idx <= arr.len() {
                    arr.insert(idx, value);
                    return Ok(());
                }
            }
            return Err(PluginError::InstallFailed(format!(
                "Cannot set value at path: {}",
                parts.join("/")
            )));
        } else {
            // Navigate deeper
            if !current.get(*part).is_some() {
                if let serde_json::Value::Object(obj) = current {
                    obj.insert(part.to_string(), serde_json::Value::Object(serde_json::Map::new()));
                }
            }
            current = current.get_mut(*part).ok_or_else(|| {
                PluginError::InstallFailed(format!("Cannot navigate to: {}", parts.join("/")))
            })?;
        }
    }
    Ok(())
}

/// Remove a value from a JSON document using a pointer path
fn json_pointer_remove(doc: &mut serde_json::Value, parts: &[&str]) -> PluginResult<()> {
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
        return Err(PluginError::InstallFailed("Cannot remove document root".into()));
    }

    let mut current = doc;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == parts.len() - 1 {
            if let serde_json::Value::Object(obj) = current {
                obj.remove(*part);
                return Ok(());
            } else if let serde_json::Value::Array(arr) = current {
                let idx: usize = part.parse().map_err(|_| {
                    PluginError::InstallFailed(format!("Invalid array index: {}", part))
                })?;
                if idx < arr.len() {
                    arr.remove(idx);
                    return Ok(());
                }
            }
            return Err(PluginError::InstallFailed(format!(
                "Cannot remove value at path: {}",
                parts.join("/")
            )));
        } else {
            current = current.get_mut(*part).ok_or_else(|| {
                PluginError::InstallFailed(format!("Path not found: {}", parts.join("/")))
            })?;
        }
    }
    Ok(())
}

// ============================================================================
// Install Instructions
// ============================================================================

/// Load installation instructions markdown for a target
pub fn load_install_instructions(
    manifest: &PluginManifest,
    target_id: &str,
    plugin_path: &Path,
) -> PluginResult<Option<String>> {
    let bundle = manifest.bundle.as_ref().ok_or_else(|| {
        PluginError::InvalidManifest("Plugin has no bundle configuration".into())
    })?;

    let target = bundle
        .targets
        .iter()
        .find(|t| t.id == target_id)
        .ok_or_else(|| {
            PluginError::InvalidManifest(format!("Bundle target '{}' not found", target_id))
        })?;

    let Some(install_md) = &target.install_md else {
        return Ok(None);
    };

    let assets_path = plugin_path.join(&bundle.assets_root);
    let md_path = assets_path.join(install_md);

    if md_path.exists() {
        let content = std::fs::read_to_string(&md_path).map_err(|e| {
            PluginError::InvalidManifest(format!(
                "Failed to read install instructions at {}: {}",
                md_path.display(),
                e
            ))
        })?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

// ============================================================================
// Bundle Info for UI
// ============================================================================

/// Summary of a bundle for display in the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInfo {
    /// Bundle version
    pub bundle_version: String,
    /// Default install mode
    pub default_install_mode: String,
    /// Number of targets
    pub target_count: usize,
    /// Target summaries
    pub targets: Vec<BundleTargetInfo>,
    /// Required user variables
    pub required_variables: Vec<VariableInfo>,
}

/// Summary of a bundle target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTargetInfo {
    /// Target ID
    pub id: String,
    /// Display name
    pub display_name: String,
    /// Target kind
    pub kind: String,
    /// Available scopes
    pub scopes: Vec<String>,
    /// Whether install instructions are available
    pub has_install_md: bool,
    /// Number of files to copy
    pub file_count: usize,
    /// Number of commands
    pub command_count: usize,
}

/// Variable info for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInfo {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub required: bool,
    pub default: Option<String>,
    pub description: Option<String>,
}

/// Get bundle info for UI display
pub fn get_bundle_info(manifest: &PluginManifest) -> Option<BundleInfo> {
    let bundle = manifest.bundle.as_ref()?;

    let targets = bundle
        .targets
        .iter()
        .map(|t| BundleTargetInfo {
            id: t.id.clone(),
            display_name: t.display_name.clone(),
            kind: format!("{:?}", t.kind),
            scopes: t.scopes.iter().map(|s| format!("{:?}", s)).collect(),
            has_install_md: t.install_md.is_some(),
            file_count: t.files_to_copy.len(),
            command_count: t.commands.len(),
        })
        .collect();

    let required_variables = bundle
        .variables
        .iter()
        .map(|v| VariableInfo {
            name: v.name.clone(),
            label: v.label.clone(),
            kind: format!("{:?}", v.kind),
            required: v.required,
            default: v.default.clone(),
            description: v.description.clone(),
        })
        .collect();

    Some(BundleInfo {
        bundle_version: bundle.bundle_version.clone(),
        default_install_mode: format!("{:?}", bundle.default_install_mode),
        target_count: bundle.targets.len(),
        targets,
        required_variables,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_substitution() {
        let mut context = VariableContext::new();
        context
            .variables
            .insert("project_dir".to_string(), "/my/project".to_string());

        let result = context.substitute("${home}/.config/${project_dir}/test");
        assert!(result.contains("/my/project"));
        assert!(!result.contains("${project_dir}"));
    }

    #[test]
    fn test_json_merge_patch() {
        let target = serde_json::json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            }
        });

        let patch = serde_json::json!({
            "b": {
                "c": 10,
                "e": 4
            },
            "f": 5
        });

        let result = json_merge_patch(target, patch);

        assert_eq!(result["a"], 1);
        assert_eq!(result["b"]["c"], 10);
        assert_eq!(result["b"]["d"], 3);
        assert_eq!(result["b"]["e"], 4);
        assert_eq!(result["f"], 5);
    }

    #[test]
    fn test_json_merge_patch_null_removes() {
        let target = serde_json::json!({
            "a": 1,
            "b": 2
        });

        let patch = serde_json::json!({
            "a": null
        });

        let result = json_merge_patch(target, patch);

        assert!(result.get("a").is_none());
        assert_eq!(result["b"], 2);
    }
}
