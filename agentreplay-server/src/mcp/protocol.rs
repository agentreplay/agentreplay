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

//! MCP Protocol Types
//!
//! JSON-RPC 2.0 message types for the Model Context Protocol.
//! Reference: https://modelcontextprotocol.io/specification

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON-RPC 2.0 protocol version
pub const JSONRPC_VERSION: &str = "2.0";

/// MCP protocol version (2025-11-25 — latest spec)
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

/// Supported protocol versions for negotiation (newest first)
pub const MCP_SUPPORTED_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];

// =============================================================================
// Core JSON-RPC 2.0 Types
// =============================================================================

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub id: JsonRpcId,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: JsonRpcId,
}

/// JSON-RPC 2.0 Notification (no id, no response expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 ID (can be string, number, or null)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Number(i64),
    Null,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Parse error (-32700)
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: message.into(),
            data: None,
        }
    }

    /// Invalid request (-32600)
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
            data: None,
        }
    }

    /// Method not found (-32601)
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    /// Invalid params (-32602)
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    /// Internal error (-32603)
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
            data: None,
        }
    }
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: JsonRpcId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: JsonRpcId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

// =============================================================================
// MCP Protocol Types
// =============================================================================

/// Server capabilities advertised during initialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourcesCapability {
    pub subscribe: bool,
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoggingCapability {}

/// Server info returned during initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Initialize request params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

/// Client capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RootsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SamplingCapability {}

/// Client info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Initialize result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

// =============================================================================
// Resource Types
// =============================================================================

/// MCP Resource - represents a data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Unique URI for the resource
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Resource content with text or blob data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>, // Base64 encoded
}

/// List resources result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResourcesResult {
    pub resources: Vec<Resource>,
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Read resource params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResourceParams {
    pub uri: String,
}

/// Read resource result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResourceResult {
    pub contents: Vec<ResourceContent>,
}

// =============================================================================
// Tool Types
// =============================================================================

/// MCP Tool - an action that can be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Unique name for the tool
    pub name: String,
    /// Human-readable title for display (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for tool parameters
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
    /// JSON Schema for tool output (MCP 2025-06-18)
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    /// Behavioral annotations (MCP 2025-03-26)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

/// Tool behavioral annotations (MCP 2025-03-26)
///
/// Describes tool behavior so clients can make informed UI/safety decisions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolAnnotations {
    /// Tool does not modify any state
    #[serde(rename = "readOnlyHint", skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// Tool performs irreversible operations
    #[serde(rename = "destructiveHint", skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// Tool can safely be called multiple times with same params
    #[serde(rename = "idempotentHint", skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    /// Tool interacts with external services
    #[serde(rename = "openWorldHint", skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

/// List tools result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Call tool params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Tool content types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: ResourceContent },
}

/// Call tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
    /// Machine-parseable structured output (MCP 2025-06-18)
    #[serde(rename = "structuredContent", skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

// =============================================================================
// Prompt Types
// =============================================================================

/// MCP Prompt - a template for common operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Unique name for the prompt
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arguments the prompt accepts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Prompt argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the argument is required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// List prompts result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPromptsResult {
    pub prompts: Vec<Prompt>,
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Get prompt params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptParams {
    pub name: String,
    #[serde(default)]
    pub arguments: HashMap<String, String>,
}

/// Prompt message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: PromptRole,
    pub content: PromptContent,
}

/// Prompt role
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptRole {
    User,
    Assistant,
}

/// Prompt content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromptContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: ResourceContent },
}

/// Get prompt result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

// =============================================================================
// FlowtTrace-specific MCP Types
// =============================================================================

/// Trace search result with relevance scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSearchResult {
    /// Edge ID in hex format
    pub edge_id: String,
    /// Timestamp in microseconds
    pub timestamp_us: u64,
    /// Operation/span type
    pub operation: String,
    /// Duration in milliseconds
    pub duration_ms: f64,
    /// Token count
    pub tokens: u32,
    /// Estimated cost in USD
    pub cost: f64,
    /// Composite relevance score (0.0 - 1.0)
    pub relevance_score: f64,
    /// Semantic similarity score
    pub semantic_score: f64,
    /// Temporal recency score
    pub temporal_score: f64,
    /// Graph influence score (PageRank)
    pub graph_score: f64,
    /// Payload summary (first N chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_summary: Option<String>,
    /// Related trace IDs (causal graph)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_traces: Option<Vec<String>>,
}

/// Parameters for trace search tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSearchParams {
    /// Natural language query
    pub query: String,
    /// Maximum results to return
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Time range start (microseconds, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ts: Option<u64>,
    /// Time range end (microseconds, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ts: Option<u64>,
    /// Filter by span types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_types: Option<Vec<String>>,
    /// Include payload content
    #[serde(default)]
    pub include_payload: bool,
    /// Include related traces from causal graph
    #[serde(default)]
    pub include_related: bool,
}

fn default_limit() -> usize {
    10
}

/// Parameters for context retrieval tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetContextParams {
    /// The error message or question
    pub query: String,
    /// Context type to retrieve
    #[serde(default)]
    pub context_type: ContextType,
    /// Maximum context items
    #[serde(default = "default_context_limit")]
    pub limit: usize,
}

fn default_context_limit() -> usize {
    5
}

/// Type of context to retrieve
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContextType {
    /// Find similar past errors and their resolutions
    #[default]
    ErrorResolution,
    /// Find related code changes
    CodeChanges,
    /// Find related traces
    RelatedTraces,
    /// Find all context types
    All,
}

/// Context result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    /// Type of context
    pub context_type: String,
    /// Relevance score
    pub relevance_score: f64,
    /// Content description
    pub description: String,
    /// Full content (JSON or text)
    pub content: serde_json::Value,
    /// Source trace ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_trace_id: Option<String>,
    /// Timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_us: Option<u64>,
}

/// Result from context retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetContextResult {
    /// Query that was processed
    pub query: String,
    /// Context items found
    pub items: Vec<ContextItem>,
    /// Total matching items (before limit)
    pub total_matches: usize,
}

// =============================================================================
// Session Management (MCP 2025-03-26)
// =============================================================================

/// Session ID header name
pub const MCP_SESSION_ID_HEADER: &str = "Mcp-Session-Id";

// =============================================================================
// Resource Templates (MCP spec)
// =============================================================================

/// Resource template with URI pattern (RFC 6570)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplate {
    /// URI template (RFC 6570)
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    /// Human-readable name
    pub name: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// List resource templates result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResourceTemplatesResult {
    #[serde(rename = "resourceTemplates")]
    pub resource_templates: Vec<ResourceTemplate>,
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// =============================================================================
// Progress Notifications (MCP spec)
// =============================================================================

/// Progress notification for long-running operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressNotification {
    /// Progress token from the original request
    #[serde(rename = "progressToken")]
    pub progress_token: JsonRpcId,
    /// Current progress value
    pub progress: f64,
    /// Total expected value (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    /// Human-readable progress message (MCP 2025-03-26)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// =============================================================================
// Logging (MCP spec)
// =============================================================================

/// Log levels per MCP specification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum McpLogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl Default for McpLogLevel {
    fn default() -> Self {
        Self::Info
    }
}

/// Set logging level params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLevelParams {
    pub level: McpLogLevel,
}

/// Log message notification (notifications/message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessageNotification {
    pub level: McpLogLevel,
    pub logger: String,
    pub data: serde_json::Value,
}

// =============================================================================
// Elicitation (MCP 2025-06-18)
// =============================================================================

/// Elicitation request sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitRequest {
    /// Human-readable message explaining what is needed
    pub message: String,
    /// JSON Schema describing required input for form-mode
    #[serde(rename = "requestedSchema", skip_serializing_if = "Option::is_none")]
    pub requested_schema: Option<serde_json::Value>,
    /// URL for url-mode elicitation (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Elicitation response from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitResult {
    /// Outcome of the elicitation
    pub action: ElicitAction,
    /// Collected data (for form-mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
}

/// Elicitation action outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElicitAction {
    Accept,
    Decline,
    Cancel,
}

// =============================================================================
// Sampling / CreateMessage (MCP spec)
// =============================================================================

/// Sampling request (server → client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageParams {
    pub messages: Vec<SamplingMessage>,
    #[serde(rename = "modelPreferences", skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<serde_json::Value>,
    #[serde(rename = "systemPrompt", skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Include tool definitions for tool-calling (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

/// Sampling message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: PromptRole,
    pub content: PromptContent,
}

/// Sampling result (client → server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageResult {
    pub role: PromptRole,
    pub content: PromptContent,
    pub model: String,
    #[serde(rename = "stopReason", skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

// =============================================================================
// Resource Subscriptions (MCP spec)
// =============================================================================

/// Subscribe to resource changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeParams {
    pub uri: String,
}

/// Unsubscribe from resource changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeParams {
    pub uri: String,
}

/// Resource updated notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUpdatedNotification {
    pub uri: String,
}

// =============================================================================
// Pagination helpers
// =============================================================================

/// Encode pagination cursor from offset
pub fn encode_cursor(offset: usize) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(offset.to_string())
}

/// Decode pagination cursor to offset
pub fn decode_cursor(cursor: &str) -> Option<usize> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(cursor)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|s| s.parse().ok())
}

/// Generic list params with optional cursor
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}
