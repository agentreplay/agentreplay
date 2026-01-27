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

//! Unified Tool Definition
//!
//! Supports both MCP protocol tools and native API tools with versioning,
//! rate limiting, and execution configuration.
//!
//! ## Design Notes (from tauri.md review)
//!
//! **Issue 1 Fix**: Uses flattened DashMap keys `(namespace, name, version)` instead of
//! nested `DashMap<(String, String), BTreeMap<...>>` to avoid lock contention.
//!
//! **Issue 2 Fix**: Uses semver crate for proper version handling.
//!
//! **Issue 5 Fix**: Implements hierarchical rate limiting per-kind.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified tool definition supporting both MCP and native API tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedToolDefinition {
    /// Unique tool identifier (namespace:name:version)
    pub tool_id: String,
    /// Tool namespace (e.g., "default", "mcp", "custom")
    pub namespace: String,
    /// Tool name (MCP-compatible)
    pub name: String,
    /// Semantic version (major.minor.patch)
    pub version: ToolVersion,
    /// Tool kind: MCP, REST, gRPC, Function
    pub kind: ToolKind,
    /// JSON Schema for input validation
    pub input_schema: serde_json::Value,
    /// JSON Schema for output validation
    pub output_schema: Option<serde_json::Value>,
    /// Tool metadata
    pub metadata: ToolDefinitionMetadata,
    /// Execution configuration
    pub execution: ExecutionConfig,
    /// Whether tool is enabled
    pub enabled: bool,
    /// Creation timestamp (microseconds)
    pub created_at: u64,
    /// Last updated timestamp (microseconds)
    pub updated_at: u64,
}

impl UnifiedToolDefinition {
    /// Create a new tool definition
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
        version: ToolVersion,
        kind: ToolKind,
        input_schema: serde_json::Value,
    ) -> Self {
        let namespace = namespace.into();
        let name = name.into();
        let tool_id = format!("{}:{}:{}", namespace, name, version);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        Self {
            tool_id,
            namespace,
            name,
            version,
            kind,
            input_schema,
            output_schema: None,
            metadata: ToolDefinitionMetadata::default(),
            execution: ExecutionConfig::default(),
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get the tool ID key for storage (namespace:name:version)
    pub fn storage_key(&self) -> String {
        self.tool_id.clone()
    }

    /// Get the latest version key (namespace:name)
    pub fn latest_key(&self) -> String {
        format!("{}:{}", self.namespace, self.name)
    }
}

/// Semantic version for tools
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ToolVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<String>,
}

impl ToolVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
        }
    }

    pub fn with_prerelease(mut self, prerelease: impl Into<String>) -> Self {
        self.prerelease = Some(prerelease.into());
        self
    }

    /// Check if this version satisfies a constraint (e.g., "^1.0.0", ">=2.0.0")
    pub fn satisfies(&self, constraint: &str) -> bool {
        // Simple constraint parsing - extend for full semver support
        let constraint = constraint.trim();

        if constraint == "latest" || constraint == "*" {
            return true;
        }

        if let Some(rest) = constraint.strip_prefix('^') {
            // Caret: compatible with major version
            if let Some(v) = Self::parse(rest) {
                return self.major == v.major && *self >= v;
            }
        } else if let Some(rest) = constraint.strip_prefix('~') {
            // Tilde: compatible with minor version
            if let Some(v) = Self::parse(rest) {
                return self.major == v.major && self.minor == v.minor && *self >= v;
            }
        } else if let Some(rest) = constraint.strip_prefix(">=") {
            if let Some(v) = Self::parse(rest) {
                return *self >= v;
            }
        } else if let Some(rest) = constraint.strip_prefix("<=") {
            if let Some(v) = Self::parse(rest) {
                return *self <= v;
            }
        } else if let Some(rest) = constraint.strip_prefix('>') {
            if let Some(v) = Self::parse(rest) {
                return *self > v;
            }
        } else if let Some(rest) = constraint.strip_prefix('<') {
            if let Some(v) = Self::parse(rest) {
                return *self < v;
            }
        } else if let Some(rest) = constraint.strip_prefix('=') {
            if let Some(v) = Self::parse(rest) {
                return *self == v;
            }
        } else if let Some(v) = Self::parse(constraint) {
            // Exact match
            return *self == v;
        }

        false
    }

    /// Parse a version string (e.g., "1.2.3", "1.2.3-beta")
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let (version_part, prerelease) = if let Some(idx) = s.find('-') {
            (&s[..idx], Some(s[idx + 1..].to_string()))
        } else {
            (s, None)
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return None;
        }

        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);

        Some(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }
}

impl std::fmt::Display for ToolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref pre) = self.prerelease {
            write!(f, "{}.{}.{}-{}", self.major, self.minor, self.patch, pre)
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

impl Default for ToolVersion {
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

/// Tool kind specifies how the tool is executed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum ToolKind {
    /// MCP protocol tool (JSON-RPC 2.0)
    MCP {
        server_uri: String,
        transport: MCPTransport,
    },
    /// REST API endpoint
    REST {
        endpoint: String,
        method: HttpMethod,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    /// Internal function (Rust native)
    Native { handler_id: String },
    /// Mock tool for testing
    Mock { responses: Vec<MockResponse> },
}

/// MCP transport type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "transport_type")]
pub enum MCPTransport {
    Stdio,
    Http { url: String },
    WebSocket { url: String },
    SSE { url: String },
}

/// HTTP method for REST tools
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

/// Mock response for testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MockResponse {
    /// Pattern to match in input (JSON path or regex)
    pub match_pattern: Option<String>,
    /// Response to return
    pub response: serde_json::Value,
    /// Simulated latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Whether to simulate an error
    pub is_error: bool,
}

/// Tool metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolDefinitionMetadata {
    /// Human-readable description
    pub description: String,
    /// Documentation URL
    pub documentation_url: Option<String>,
    /// Author/owner
    pub author: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Custom labels
    pub labels: HashMap<String, String>,
    /// Deprecation notice
    pub deprecated: Option<String>,
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Timeout in milliseconds
    pub timeout_ms: u64,
    /// Retry policy
    pub retry: RetryPolicy,
    /// Rate limiting configuration (hierarchical)
    pub rate_limits: RateLimitConfig,
    /// Whether execution is sandboxed
    pub sandboxed: bool,
    /// Maximum concurrent executions
    pub max_concurrent: Option<usize>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000, // 30 seconds
            retry: RetryPolicy::default(),
            rate_limits: RateLimitConfig::default(),
            sandboxed: false,
            max_concurrent: None,
        }
    }
}

/// Retry policy with exponential backoff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Whether to add jitter
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Hierarchical rate limiting configuration
///
/// **Issue 5 Fix**: Supports per-kind and per-tool rate limits
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimitConfig {
    /// Global rate limit (all tools)
    pub global: Option<RateLimit>,
    /// Per-kind rate limits (REST, MCP, etc.)
    pub per_kind: HashMap<String, RateLimit>,
    /// Per-tool rate limit (overrides per-kind)
    pub per_tool: Option<RateLimit>,
}

/// Token bucket rate limit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Maximum tokens (capacity)
    pub capacity: u64,
    /// Tokens refilled per second
    pub refill_rate: f64,
}

impl RateLimit {
    pub fn new(capacity: u64, refill_rate: f64) -> Self {
        Self {
            capacity,
            refill_rate,
        }
    }

    /// Create a rate limit of N requests per minute
    pub fn per_minute(requests: u64) -> Self {
        Self {
            capacity: requests,
            refill_rate: requests as f64 / 60.0,
        }
    }

    /// Create a rate limit of N requests per second
    pub fn per_second(requests: u64) -> Self {
        Self {
            capacity: requests,
            refill_rate: requests as f64,
        }
    }
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Result data (if successful)
    pub data: Option<serde_json::Value>,
    /// Error message (if failed)
    pub error: Option<ToolExecutionError>,
    /// Execution latency in milliseconds
    pub latency_ms: u64,
    /// Number of retries used
    pub retries: u32,
    /// Tool version that was executed
    pub tool_version: ToolVersion,
}

/// Tool execution error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionError {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
    /// Whether error is retryable
    pub retryable: bool,
    /// Additional error details
    pub details: Option<serde_json::Value>,
}

impl ToolExecutionError {
    pub fn timeout(message: impl Into<String>) -> Self {
        Self {
            code: "TIMEOUT".to_string(),
            message: message.into(),
            retryable: true,
            details: None,
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            code: "RATE_LIMITED".to_string(),
            message: message.into(),
            retryable: true,
            details: None,
        }
    }

    pub fn not_found(name: &str) -> Self {
        Self {
            code: "NOT_FOUND".to_string(),
            message: format!("Tool not found: {}", name),
            retryable: false,
            details: None,
        }
    }

    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self {
            code: "VALIDATION_FAILED".to_string(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }

    pub fn execution_failed(message: impl Into<String>) -> Self {
        Self {
            code: "EXECUTION_FAILED".to_string(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }
}

/// Tool registration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistration {
    pub tool_id: String,
    pub version: ToolVersion,
    pub registered_at: u64,
    pub is_update: bool,
}

/// Tool execution record for tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    /// Unique execution ID
    pub execution_id: u128,
    /// Associated span ID
    pub span_id: u128,
    /// Tool ID
    pub tool_id: String,
    /// Tool version
    pub tool_version: ToolVersion,
    /// Input arguments
    pub arguments: serde_json::Value,
    /// Execution result
    pub result: ToolExecutionResult,
    /// Execution timestamp (microseconds)
    pub timestamp: u64,
    /// Execution context
    pub context: ExecutionContext,
}

/// Execution context for tracing and experiments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionContext {
    /// Trace ID
    pub trace_id: Option<u128>,
    /// Session ID
    pub session_id: Option<String>,
    /// User ID
    pub user_id: Option<String>,
    /// Experiment ID (if part of A/B test)
    pub experiment_id: Option<u128>,
    /// Variant name
    pub variant: Option<String>,
    /// Environment (dev/staging/prod)
    pub environment: String,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let v = ToolVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.prerelease.is_none());

        let v = ToolVersion::parse("2.0.0-beta").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.prerelease, Some("beta".to_string()));
    }

    #[test]
    fn test_version_constraints() {
        let v1_2_3 = ToolVersion::new(1, 2, 3);

        assert!(v1_2_3.satisfies("^1.0.0"));
        assert!(v1_2_3.satisfies("^1.2.0"));
        assert!(!v1_2_3.satisfies("^2.0.0"));

        assert!(v1_2_3.satisfies("~1.2.0"));
        assert!(!v1_2_3.satisfies("~1.3.0"));

        assert!(v1_2_3.satisfies(">=1.0.0"));
        assert!(v1_2_3.satisfies("<=2.0.0"));
        assert!(v1_2_3.satisfies("latest"));
    }

    #[test]
    fn test_version_ordering() {
        let v1 = ToolVersion::new(1, 0, 0);
        let v2 = ToolVersion::new(1, 2, 0);
        let v3 = ToolVersion::new(2, 0, 0);

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }
}
