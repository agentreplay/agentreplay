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

//! Host Functions for WASM Plugins
//!
//! These are the functions that plugins can call from within WASM to interact
//! with the Agentreplay host system.
//!
//! ## Gap #7 Fix: Proper WASI Implementation
//!
//! Implements WasiView trait properly with ResourceTable for WASI resource
//! management. This enables plugins to use WASI features like file I/O,
//! environment variables, and stdio.

use crate::capabilities::GrantedCapabilities;
use std::sync::Arc;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiView};

/// Trait for accessing traces (injected by host)
pub trait TraceAccessor: Send + Sync {
    fn query(&self, filter_json: &str, limit: u32) -> Result<Vec<serde_json::Value>, String>;
    fn get_trace(&self, trace_id: &str) -> Result<Option<serde_json::Value>, String>;
}

/// State passed to plugins via host functions
///
/// Contains all the state needed by a WASM plugin including WASI context,
/// resource table, and Agentreplay-specific extensions.
pub struct PluginHostState {
    /// Plugin identifier
    pub plugin_id: String,

    /// Granted capabilities
    pub capabilities: GrantedCapabilities,

    /// Plugin configuration (JSON string)
    pub config_json: String,

    /// WASI context for file/env access
    wasi_ctx: WasiCtx,

    /// Resource table for WASI resources (file handles, etc.)
    /// Gap #7 fix: Required by WasiView trait
    resource_table: ResourceTable,

    /// HTTP client (if network capability granted)
    pub http_client: Option<reqwest::Client>,

    /// Trace accessor for database queries
    pub trace_accessor: Option<Arc<dyn TraceAccessor>>,

    /// Plugin metrics
    pub metrics: PluginMetrics,
}

impl PluginHostState {
    /// Create a new plugin host state
    ///
    /// # Arguments
    /// - `plugin_id`: Unique identifier for the plugin
    /// - `capabilities`: Capabilities granted to this plugin
    /// - `config_json`: Plugin configuration as JSON
    /// - `wasi_ctx`: Pre-configured WASI context
    pub fn new(
        plugin_id: String,
        capabilities: GrantedCapabilities,
        config_json: String,
        wasi_ctx: WasiCtx,
    ) -> Self {
        Self {
            plugin_id: plugin_id.clone(),
            capabilities,
            config_json,
            wasi_ctx,
            resource_table: ResourceTable::new(),
            http_client: None,
            trace_accessor: None,
            metrics: PluginMetrics::new(&plugin_id),
        }
    }

    /// Set the HTTP client (requires network capability)
    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Set the trace accessor
    pub fn with_trace_accessor(mut self, accessor: Arc<dyn TraceAccessor>) -> Self {
        self.trace_accessor = Some(accessor);
        self
    }

    /// Check if a capability is granted
    pub fn has_capability(&self, cap: &str) -> bool {
        match cap {
            "trace_read" => self.capabilities.can_read_traces(),
            "trace_write" => self.capabilities.can_write_traces(),
            "network" => self.capabilities.can_network(None),
            "shell" => self.capabilities.can_shell(),
            // TODO: Add filesystem capability check when GrantedCapabilities supports it
            _ => false,
        }
    }
}

/// Gap #7 fix: Implement WasiView properly with ResourceTable
impl WasiView for PluginHostState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.resource_table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

/// Metrics collected during plugin execution
#[derive(Debug, Clone, Default)]
pub struct PluginMetrics {
    /// Plugin ID
    pub plugin_id: String,

    /// Number of evaluations performed
    pub eval_count: u64,

    /// Total time spent in evaluation (microseconds)
    pub total_eval_time_us: u64,

    /// Number of host function calls
    pub host_call_count: u64,

    /// Fuel consumed
    pub fuel_consumed: u64,

    /// Memory high water mark (bytes)
    pub memory_peak_bytes: u64,

    /// Number of errors
    pub error_count: u64,
}

impl PluginMetrics {
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            ..Default::default()
        }
    }

    pub fn record_eval(&mut self, duration_us: u64) {
        self.eval_count += 1;
        self.total_eval_time_us += duration_us;
    }

    pub fn record_host_call(&mut self) {
        self.host_call_count += 1;
    }

    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    pub fn average_eval_time_us(&self) -> f64 {
        if self.eval_count == 0 {
            0.0
        } else {
            self.total_eval_time_us as f64 / self.eval_count as f64
        }
    }
}

/// Log level enum matching the WIT definition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl From<i32> for LogLevel {
    fn from(val: i32) -> Self {
        match val {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            _ => LogLevel::Error,
        }
    }
}

impl LogLevel {
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}
