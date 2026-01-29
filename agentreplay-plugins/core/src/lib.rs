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

//! Agentreplay Plugin System
//!
//! A sandboxed, versioned plugin system for extending Agentreplay with custom
//! evaluators, providers, visualizations, and integrations.
//!
//! # Architecture
//!
//! The plugin system uses WASM (WebAssembly) as the universal runtime, enabling
//! plugins to be written in any language that compiles to WASM:
//! - Rust (native performance)
//! - Python (via componentize-py)
//! - TypeScript (via jco)
//! - Go (via TinyGo)
//! - C/C++, Zig, and more
//!
//! ## Bundle Plugins (Schema v2+)
//!
//! In addition to runtime plugins, Agentreplay supports "bundle" plugins that
//! install file-based integrations into external systems like Claude Code,
//! Cursor, and other editor/agent ecosystems. These bundles can include:
//! - Hook configurations (hooks.json, .mcp.json)
//! - Plugin manifests (.claude-plugin/plugin.json)
//! - Installation instructions (install.md)
//! - Template files with variable substitution
//!
//! # Features
//!
//! - **Plugin Types**: Evaluators, embedding providers, exporters, transformers, bundles
//! - **Sandboxing**: Capability-based access control for security
//! - **Versioning**: Semantic versioning with dependency resolution
//! - **Hot Reload**: Development mode with automatic reload on changes
//! - **Language Agnostic**: Universal WASM interface via WIT
//! - **Bundle Support**: File-based integrations for Claude Code, Cursor, etc.
//!
//! # Example
//!
//! ```rust,ignore
//! use agentreplay_plugins::{PluginManager, PluginConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = PluginConfig::default();
//!     let manager = PluginManager::new(config).await.unwrap();
//!     
//!     // List installed plugins
//!     let plugins = manager.list_plugins();
//!     
//!     // Install a plugin
//!     manager.install_from_file("./my-plugin").await.unwrap();
//!     
//!     // Enable a plugin
//!     manager.enable("my-plugin").await.unwrap();
//!     
//!     // For bundle plugins, get target info and install instructions
//!     if let Some(info) = manager.get_plugin("my-bundle") {
//!         if let Some(bundle_info) = bundle::get_bundle_info(&info.manifest) {
//!             for target in bundle_info.targets {
//!                 println!("Available target: {}", target.display_name);
//!             }
//!         }
//!     }
//! }
//! ```

pub mod bundle;
pub mod capabilities;
pub mod error;
pub mod hooks;
pub mod installer;
pub mod manager;
pub mod manifest;
pub mod native;
pub mod providers;
pub mod registry;
pub mod resolver;
pub mod state;
pub mod wasm;

// Re-exports
pub use bundle::{
    create_install_plan, detect_target, execute_install_plan, get_bundle_info,
    load_install_instructions, verify_installation, BundleInfo, BundleTargetInfo, 
    CreatedFile, DetectionResult, InstallExecutionResult, InstallPlan, InstallReceipt,
    JsonModification, PerformedOperation, PlannedCommand, PlannedInstallOp, PlannedOperation,
    ResolvedInstallOp, ReverseOp, VariableContext, VariableInfo, VerifyResult,
};
pub use capabilities::{Capability, CapabilitySet, GrantedCapabilities};
pub use error::{PluginError, PluginResult};
pub use installer::{
    InstallProgress, InstallResult, PluginInstaller, UninstallMode, UninstallResult,
};
pub use manager::{PluginConfig, PluginInfo, PluginManager, PluginState};
pub use manifest::{
    BundleManifest, BundleTarget, BundleVariable, CopyStrategy, DetectRule, EntryPoint,
    FileCopyStep, InstallCommand, InstallMode, InstallOp, InstallScope, JsonPatchOp,
    JsonPatchOpType, PluginManifest, PluginType, TargetKind, TemplateSpec, VariableKind,
};
pub use providers::{
    ProviderConfig, ProviderDriver, ProviderError, ProviderFactory, ProviderFilter, ProviderKind,
    ProviderRegistry,
};
pub use registry::{PluginRegistry, PluginSource};
pub use resolver::{DependencyResolver, ResolvedPlugin};
pub use state::PluginStateStore;
pub use wasm::{LoadedPlugin, PluginInstance, WasmExecutor};

/// Plugin API version - plugins must be compatible with this
pub const PLUGIN_API_VERSION: &str = "0.1.0";

/// Default plugins directory name
pub const PLUGINS_DIR_NAME: &str = "plugins";

/// Plugin manifest filename
pub const MANIFEST_FILENAME: &str = "agentreplay-plugin.toml";
