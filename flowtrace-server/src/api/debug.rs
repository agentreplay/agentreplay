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

//! Fork & Debug API for Trace Replay
//!
//! This module provides the ability to "fork" a trace and replay parts of it:
//!
//! ## Supported Operations
//!
//! 1. **LLM Replay**: Re-execute LLM calls with modified inputs (fully supported)
//! 2. **Tool Mocking**: For tool calls, user provides mock output (no RCE needed)
//! 3. **Trace Forking**: Create a new trace branching from an existing one
//!
//! ## Architecture
//!
//! Flowtrace is a **passive observer** - it stores logs, not code.
//! For tools, we cannot execute the actual function (it lives in the user's app).
//! Instead, we provide three options:
//!
//! - **LLM spans**: Can be replayed using server's LLM credentials
//! - **Tool spans**: User provides mock output, trace continues with mocked data
//! - **Dev Mode**: WebSocket connection to user's running app for live execution
//!
//! ## Security Note
//!
//! Remote Code Execution (RCE) for tools is intentionally NOT supported.
//! This is a security feature, not a limitation.

use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use crate::llm::LLMProviderManager;

/// The type of span being debugged
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    /// LLM generation span - can be replayed
    Llm,
    /// Tool call span - requires mocking
    Tool,
    /// Retrieval span - can be replayed
    Retrieval,
    /// Other span types
    Other,
}

/// Capability of a span for fork & debug
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanDebugCapability {
    /// Whether this span can be replayed
    pub can_replay: bool,
    /// Whether this span requires mocking
    pub requires_mock: bool,
    /// Reason if replay is not possible
    pub reason: Option<String>,
    /// Suggested action for the user
    pub suggested_action: String,
}

impl SpanDebugCapability {
    pub fn for_span_kind(kind: &SpanKind) -> Self {
        match kind {
            SpanKind::Llm => Self {
                can_replay: true,
                requires_mock: false,
                reason: None,
                suggested_action: "Click 'Run' to re-execute with modified input".to_string(),
            },
            SpanKind::Tool => Self {
                can_replay: false,
                requires_mock: true,
                reason: Some(
                    "Tool execution requires your application runtime. \
                     Flowtrace stores traces, not code."
                        .to_string(),
                ),
                suggested_action: "Provide mock output to continue the trace simulation"
                    .to_string(),
            },
            SpanKind::Retrieval => Self {
                can_replay: true,
                requires_mock: false,
                reason: None,
                suggested_action: "Click 'Run' to re-execute retrieval".to_string(),
            },
            SpanKind::Other => Self {
                can_replay: false,
                requires_mock: true,
                reason: Some("Unknown span type cannot be replayed".to_string()),
                suggested_action: "Provide mock output or skip this span".to_string(),
            },
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request to check if a span can be replayed
#[derive(Debug, Deserialize)]
pub struct CheckReplayableRequest {
    pub trace_id: String,
    pub span_id: String,
}

/// Response for replay capability check
#[derive(Debug, Serialize)]
pub struct CheckReplayableResponse {
    pub span_kind: SpanKind,
    pub capability: SpanDebugCapability,
}

/// Request to replay an LLM span
#[derive(Debug, Deserialize)]
pub struct ReplayLLMRequest {
    /// Original trace ID
    pub trace_id: String,
    /// Span ID to replay
    pub span_id: String,
    /// Modified input (if None, uses original)
    pub modified_input: Option<String>,
    /// Model override (if None, uses original)
    pub model: Option<String>,
    /// Temperature override
    pub temperature: Option<f64>,
    /// Max tokens override
    pub max_tokens: Option<u32>,
    /// Additional context to prepend
    pub additional_context: Option<String>,
}

/// Response from LLM replay
#[derive(Debug, Serialize)]
pub struct ReplayLLMResponse {
    /// New output from LLM
    pub output: String,
    /// New span ID created
    pub new_span_id: String,
    /// Forked trace ID
    pub forked_trace_id: String,
    /// Execution metadata
    pub metadata: ReplayMetadata,
    /// Diff from original output
    pub diff_summary: Option<String>,
}

/// Metadata about the replay execution
#[derive(Debug, Serialize)]
pub struct ReplayMetadata {
    pub latency_ms: u64,
    pub tokens_used: TokenUsage,
    pub cost_usd: f64,
    pub model_used: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Request to provide mock output for a tool span
#[derive(Debug, Deserialize)]
pub struct MockToolOutputRequest {
    /// Original trace ID
    pub trace_id: String,
    /// Tool span ID
    pub span_id: String,
    /// User-provided mock output
    pub mock_output: serde_json::Value,
    /// Optional error to simulate
    pub simulate_error: Option<String>,
}

/// Response from mocking a tool
#[derive(Debug, Serialize)]
pub struct MockToolOutputResponse {
    /// New span ID with mocked output
    pub new_span_id: String,
    /// Forked trace ID
    pub forked_trace_id: String,
    /// Whether there are more spans to process
    pub has_next_span: bool,
    /// Next span info if exists
    pub next_span: Option<NextSpanInfo>,
}

/// Info about the next span in the trace
#[derive(Debug, Serialize)]
pub struct NextSpanInfo {
    pub span_id: String,
    pub span_kind: SpanKind,
    pub capability: SpanDebugCapability,
}

/// Request to fork an entire trace
#[derive(Debug, Deserialize)]
pub struct ForkTraceRequest {
    /// Original trace ID
    pub trace_id: String,
    /// Optional: Fork from a specific span (else from start)
    pub from_span_id: Option<String>,
    /// Name for the forked trace
    pub fork_name: Option<String>,
}

/// Response from forking a trace
#[derive(Debug, Serialize)]
pub struct ForkTraceResponse {
    /// New forked trace ID
    pub forked_trace_id: String,
    /// Spans in the forked trace
    pub spans: Vec<ForkedSpanInfo>,
    /// Total spans copied
    pub spans_copied: usize,
}

/// Info about a span in the forked trace
#[derive(Debug, Serialize)]
pub struct ForkedSpanInfo {
    pub original_span_id: String,
    pub new_span_id: String,
    pub span_kind: SpanKind,
    pub can_replay: bool,
}

// ============================================================================
// App State
// ============================================================================

/// Application state for debug endpoints
#[derive(Clone)]
pub struct DebugState {
    pub llm_provider: Arc<LLMProviderManager>,
    // In real implementation: add trace storage, etc.
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/v1/debug/check-replayable
/// Check if a span can be replayed or requires mocking
pub async fn check_replayable(
    State(_state): State<DebugState>,
    Json(req): Json<CheckReplayableRequest>,
) -> Result<Json<CheckReplayableResponse>, (StatusCode, String)> {
    info!(
        trace_id = %req.trace_id,
        span_id = %req.span_id,
        "Checking replay capability"
    );

    // In real implementation: fetch span from storage and determine kind
    // For now, mock based on span_id pattern
    let span_kind = if req.span_id.contains("llm") || req.span_id.contains("gen") {
        SpanKind::Llm
    } else if req.span_id.contains("tool") {
        SpanKind::Tool
    } else if req.span_id.contains("retrieval") || req.span_id.contains("rag") {
        SpanKind::Retrieval
    } else {
        SpanKind::Other
    };

    let capability = SpanDebugCapability::for_span_kind(&span_kind);

    Ok(Json(CheckReplayableResponse {
        span_kind,
        capability,
    }))
}

/// POST /api/v1/debug/replay-llm
/// Replay an LLM span with optional modifications
pub async fn replay_llm(
    State(state): State<DebugState>,
    Json(req): Json<ReplayLLMRequest>,
) -> Result<Json<ReplayLLMResponse>, (StatusCode, String)> {
    info!(
        trace_id = %req.trace_id,
        span_id = %req.span_id,
        "Replaying LLM span"
    );

    // Determine provider and model from request
    let model = req.model.as_deref().unwrap_or("gpt-4");
    let (provider_id, model_name) = if model.starts_with("claude") {
        ("anthropic", model)
    } else if model.starts_with("gpt") || model.starts_with("o1") {
        ("openai", model)
    } else if model.starts_with("deepseek") {
        ("deepseek", model)
    } else {
        ("ollama", model)
    };

    // Build the prompt
    let input = req.modified_input.unwrap_or_else(|| {
        // In real implementation: fetch original input from trace storage
        "Original prompt would be fetched here".to_string()
    });

    let full_prompt = if let Some(ctx) = &req.additional_context {
        format!("{}\n\n{}", ctx, input)
    } else {
        input
    };

    // Build messages for chat
    let messages = vec![crate::llm::ChatMessage {
        role: "user".to_string(),
        content: full_prompt,
    }];

    // Execute LLM call using provider manager
    let start = std::time::Instant::now();

    let result = state
        .llm_provider
        .chat(
            provider_id,
            Some(model_name.to_string()),
            messages,
            1, // tenant_id - would come from auth in real impl
            1, // session_id
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("LLM execution failed: {}", e),
            )
        })?;

    let latency_ms = start.elapsed().as_millis() as u64;

    // Generate new IDs
    let new_span_id = uuid::Uuid::new_v4().to_string();
    let forked_trace_id = uuid::Uuid::new_v4().to_string();

    Ok(Json(ReplayLLMResponse {
        output: result.content,
        new_span_id,
        forked_trace_id,
        metadata: ReplayMetadata {
            latency_ms,
            tokens_used: TokenUsage {
                prompt_tokens: result.input_tokens.unwrap_or(0),
                completion_tokens: result.output_tokens.unwrap_or(0),
                total_tokens: result.tokens_used.unwrap_or(0),
            },
            cost_usd: 0.0, // Would calculate from token usage
            model_used: result.model,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        },
        diff_summary: None, // Would compute diff with original in real implementation
    }))
}

/// POST /api/v1/debug/mock-tool
/// Provide mock output for a tool span that cannot be replayed
pub async fn mock_tool_output(
    State(_state): State<DebugState>,
    Json(req): Json<MockToolOutputRequest>,
) -> Result<Json<MockToolOutputResponse>, (StatusCode, String)> {
    info!(
        trace_id = %req.trace_id,
        span_id = %req.span_id,
        "Mocking tool output"
    );

    // Generate new IDs
    let new_span_id = uuid::Uuid::new_v4().to_string();
    let forked_trace_id = uuid::Uuid::new_v4().to_string();

    // In real implementation:
    // 1. Store the mock output as the span's output
    // 2. Check if there are subsequent spans
    // 3. Return next span info if exists

    Ok(Json(MockToolOutputResponse {
        new_span_id,
        forked_trace_id,
        has_next_span: true, // Would check trace in real implementation
        next_span: Some(NextSpanInfo {
            span_id: "next-llm-span-id".to_string(),
            span_kind: SpanKind::Llm,
            capability: SpanDebugCapability::for_span_kind(&SpanKind::Llm),
        }),
    }))
}

/// POST /api/v1/debug/fork-trace
/// Fork an entire trace for debugging
pub async fn fork_trace(
    State(_state): State<DebugState>,
    Json(req): Json<ForkTraceRequest>,
) -> Result<Json<ForkTraceResponse>, (StatusCode, String)> {
    info!(
        trace_id = %req.trace_id,
        from_span = ?req.from_span_id,
        "Forking trace"
    );

    // Generate new trace ID
    let forked_trace_id = uuid::Uuid::new_v4().to_string();

    // In real implementation:
    // 1. Copy all spans from original trace (or from specified span)
    // 2. Assign new IDs
    // 3. Mark as "forked" in metadata

    // Mock response
    let spans = vec![
        ForkedSpanInfo {
            original_span_id: "span-1".to_string(),
            new_span_id: uuid::Uuid::new_v4().to_string(),
            span_kind: SpanKind::Llm,
            can_replay: true,
        },
        ForkedSpanInfo {
            original_span_id: "span-2".to_string(),
            new_span_id: uuid::Uuid::new_v4().to_string(),
            span_kind: SpanKind::Tool,
            can_replay: false,
        },
    ];

    Ok(Json(ForkTraceResponse {
        forked_trace_id,
        spans_copied: spans.len(),
        spans,
    }))
}

/// GET /api/v1/debug/capabilities
/// Get debug capabilities for all span types
pub async fn get_capabilities() -> Json<HashMap<String, SpanDebugCapability>> {
    let mut caps = HashMap::new();
    caps.insert(
        "llm".to_string(),
        SpanDebugCapability::for_span_kind(&SpanKind::Llm),
    );
    caps.insert(
        "tool".to_string(),
        SpanDebugCapability::for_span_kind(&SpanKind::Tool),
    );
    caps.insert(
        "retrieval".to_string(),
        SpanDebugCapability::for_span_kind(&SpanKind::Retrieval),
    );
    caps.insert(
        "other".to_string(),
        SpanDebugCapability::for_span_kind(&SpanKind::Other),
    );
    Json(caps)
}

// ============================================================================
// Router
// ============================================================================

use axum::routing::{get, post};
use axum::Router;

/// Create the debug router
pub fn router(state: DebugState) -> Router {
    Router::new()
        .route("/check-replayable", post(check_replayable))
        .route("/replay-llm", post(replay_llm))
        .route("/mock-tool", post(mock_tool_output))
        .route("/fork-trace", post(fork_trace))
        .route("/capabilities", get(get_capabilities))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_debug_capability_llm() {
        let cap = SpanDebugCapability::for_span_kind(&SpanKind::Llm);
        assert!(cap.can_replay);
        assert!(!cap.requires_mock);
        assert!(cap.reason.is_none());
    }

    #[test]
    fn test_span_debug_capability_tool() {
        let cap = SpanDebugCapability::for_span_kind(&SpanKind::Tool);
        assert!(!cap.can_replay);
        assert!(cap.requires_mock);
        assert!(cap.reason.is_some());
        assert!(cap.reason.unwrap().contains("runtime"));
    }

    #[test]
    fn test_span_kind_serialization() {
        let kind = SpanKind::Tool;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"tool\"");
    }
}
