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

//! Agentreplay SDK Types
//!
//! Core type definitions for the Agentreplay observability platform.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent execution span types - represents the type of operation being traced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum SpanType {
    /// Root span - top-level agent execution
    #[default]
    Root = 0,
    /// Planning phase
    Planning = 1,
    /// Reasoning/thinking phase
    Reasoning = 2,
    /// Tool/function call
    ToolCall = 3,
    /// Tool response
    ToolResponse = 4,
    /// Result synthesis
    Synthesis = 5,
    /// Final response
    Response = 6,
    /// Error state
    Error = 7,
    /// Vector DB retrieval
    Retrieval = 8,
    /// Text embedding
    Embedding = 9,
    /// HTTP API call
    HttpCall = 10,
    /// Database query
    Database = 11,
    /// Generic function
    Function = 12,
    /// Result reranking
    Reranking = 13,
    /// Document parsing
    Parsing = 14,
    /// Content generation
    Generation = 15,
    /// Custom type (use values >= 16)
    Custom = 255,
}

impl SpanType {
    /// Get the string representation of the span type.
    pub fn as_str(&self) -> &'static str {
        match self {
            SpanType::Root => "Root",
            SpanType::Planning => "Planning",
            SpanType::Reasoning => "Reasoning",
            SpanType::ToolCall => "ToolCall",
            SpanType::ToolResponse => "ToolResponse",
            SpanType::Synthesis => "Synthesis",
            SpanType::Response => "Response",
            SpanType::Error => "Error",
            SpanType::Retrieval => "Retrieval",
            SpanType::Embedding => "Embedding",
            SpanType::HttpCall => "HttpCall",
            SpanType::Database => "Database",
            SpanType::Function => "Function",
            SpanType::Reranking => "Reranking",
            SpanType::Parsing => "Parsing",
            SpanType::Generation => "Generation",
            SpanType::Custom => "Custom",
        }
    }
}

impl std::fmt::Display for SpanType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Sensitivity flags for PII and redaction control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensitivityFlags(u8);

impl SensitivityFlags {
    /// No special sensitivity
    pub const NONE: Self = Self(0);
    /// Contains personally identifiable information
    pub const PII: Self = Self(1 << 0);
    /// Contains secrets/credentials
    pub const SECRET: Self = Self(1 << 1);
    /// Internal-only data
    pub const INTERNAL: Self = Self(1 << 2);
    /// Never embed in vector index
    pub const NO_EMBED: Self = Self(1 << 3);

    /// Create a new sensitivity flags value.
    pub fn new(flags: u8) -> Self {
        Self(flags)
    }

    /// Get the raw flags value.
    pub fn bits(&self) -> u8 {
        self.0
    }
}

/// Environment type for deployment context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }
}

/// Result of creating a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceResult {
    pub edge_id: String,
    pub tenant_id: i64,
    pub agent_id: i64,
    pub session_id: i64,
    pub span_type: String,
}

/// Result of creating a GenAI trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenAITraceResult {
    pub edge_id: String,
    pub tenant_id: i64,
    pub agent_id: i64,
    pub session_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Result of creating a tool trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTraceResult {
    pub edge_id: String,
    pub tenant_id: i64,
    pub agent_id: i64,
    pub session_id: i64,
    pub tool_name: String,
}

/// Trace view as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceView {
    pub edge_id: String,
    pub tenant_id: i64,
    pub project_id: i64,
    pub agent_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    pub session_id: i64,
    pub span_type: String,
    pub timestamp_us: i64,
    pub duration_us: i64,
    pub token_count: i32,
    pub confidence: f64,
    pub environment: String,
    pub has_payload: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Response from query operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub traces: Vec<TraceView>,
    pub total: i32,
    pub limit: i32,
    pub offset: i32,
}

/// Filters for querying traces.
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    pub tenant_id: Option<i64>,
    pub project_id: Option<i64>,
    pub agent_id: Option<i64>,
    pub session_id: Option<i64>,
    pub span_type: Option<SpanType>,
    pub min_confidence: Option<f64>,
    pub exclude_pii: bool,
    pub exclude_secrets: bool,
    pub environment: Option<Environment>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Span input for ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanInput {
    pub span_id: String,
    pub trace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    pub name: String,
    pub start_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    pub attributes: HashMap<String, String>,
}

/// Response from batch ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResponse {
    pub accepted: i32,
    pub rejected: i32,
    pub errors: Vec<String>,
}

/// Node in the trace hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceTreeNode {
    pub edge_id: String,
    pub span_type: String,
    pub duration_us: i64,
    pub children: Vec<TraceTreeNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Response from getting a trace tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceTreeResponse {
    pub root: TraceTreeNode,
}

/// Response from submitting feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResponse {
    pub success: bool,
    pub message: String,
}

/// Response from adding to a dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetResponse {
    pub success: bool,
    pub dataset_name: String,
}

/// Response from health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }
}

/// Options for creating a trace.
#[derive(Debug, Clone, Default)]
pub struct CreateTraceOptions {
    pub agent_id: i64,
    pub session_id: Option<i64>,
    pub span_type: SpanType,
    pub parent_id: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Options for creating a GenAI trace.
#[derive(Debug, Clone, Default)]
pub struct CreateGenAITraceOptions {
    pub agent_id: i64,
    pub session_id: Option<i64>,
    pub input_messages: Vec<Message>,
    pub output: Option<Message>,
    pub model: Option<String>,
    pub model_parameters: Option<HashMap<String, serde_json::Value>>,
    pub input_usage: Option<i32>,
    pub output_usage: Option<i32>,
    pub total_usage: Option<i32>,
    pub parent_id: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub operation_name: Option<String>,
    pub finish_reason: Option<String>,
    pub system: Option<String>,
}

/// Options for creating a tool trace.
#[derive(Debug, Clone, Default)]
pub struct CreateToolTraceOptions {
    pub agent_id: i64,
    pub session_id: Option<i64>,
    pub tool_name: String,
    pub tool_input: Option<HashMap<String, serde_json::Value>>,
    pub tool_output: Option<HashMap<String, serde_json::Value>>,
    pub tool_description: Option<String>,
    pub parent_id: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Options for updating a trace.
#[derive(Debug, Clone, Default)]
pub struct UpdateTraceOptions {
    pub edge_id: String,
    pub session_id: i64,
    pub token_count: Option<i32>,
    pub duration_us: Option<i64>,
    pub duration_ms: Option<i64>,
    pub payload: Option<HashMap<String, serde_json::Value>>,
}
