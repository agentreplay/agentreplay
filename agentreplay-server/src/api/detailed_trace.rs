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

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::api::payload_extractors::*;
use crate::api::build_eval_trace_v1;
use crate::api::query::{find_edge_by_id_or_session, ApiError, AppState};
use crate::auth::AuthContext;
use crate::otel_genai::ModelPricing;

/// Detailed trace response with structured prompts, completions, and tool calls
#[derive(Debug, Serialize)]
pub struct DetailedTraceResponse {
    // Basic trace info
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub session_id: u64,
    pub span_type: String,
    pub timestamp_us: u64,
    pub duration_us: u32,
    pub duration_ms: f64,
    pub status: String,

    // Agent and project info
    pub agent_id: u64,
    pub agent_name: String,
    pub project_id: u16,
    pub tenant_id: u64,

    // Model and provider info
    pub provider: Option<String>,
    pub model: Option<String>,
    pub route: Option<String>,

    // Token and cost info
    pub tokens: u32,
    pub cost: Option<f64>,
    pub confidence: Option<f32>,

    // Structured data
    pub prompts: Vec<PromptMessage>,
    pub completions: Vec<CompletionMessage>,
    pub tool_calls: Vec<ToolCall>,
    pub hyperparameters: Hyperparameters,
    pub token_breakdown: TokenBreakdown,

    // Previews
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,

    // Raw attributes (for backwards compatibility)
    pub attributes: Option<serde_json::Value>,

    // Canonical eval trace (EvalTraceV1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_trace: Option<agentreplay_core::EvalTraceV1>,
}

/// GET /api/v1/traces/:trace_id/detailed
/// Get detailed trace data with structured prompts, completions, and tool calls
pub async fn get_detailed_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
    auth: axum::Extension<AuthContext>,
) -> Result<Json<DetailedTraceResponse>, ApiError> {
    // Parse trace ID
    let trace_id_u128 = u128::from_str_radix(trace_id.trim_start_matches("0x"), 16)
        .map_err(|_| ApiError::BadRequest("Invalid trace ID format".into()))?;

    // Find edge (handling edge_id vs session_id mismatch)
    let edge = find_edge_by_id_or_session(&state, trace_id_u128, auth.tenant_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Trace not found".into()))?;

    // Fetch payload
    let payload = if edge.has_payload == 0 {
        None
    } else {
        let payload_bytes = state.db.get_payload(edge.edge_id).ok().flatten();
        payload_bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok())
    };

    // Extract structured data
    let prompts = payload.as_ref().map(extract_prompts).unwrap_or_default();

    let completions = payload
        .as_ref()
        .map(extract_completions)
        .unwrap_or_default();

    let tool_calls = payload.as_ref().map(extract_tool_calls).unwrap_or_default();

    let hyperparameters = payload
        .as_ref()
        .map(extract_hyperparameters)
        .unwrap_or_else(|| Hyperparameters {
            temperature: None,
            top_p: None,
            max_tokens: None,
            frequency_penalty: None,
            presence_penalty: None,
        });

    let token_breakdown = payload
        .as_ref()
        .map(extract_token_breakdown)
        .unwrap_or_else(|| TokenBreakdown {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            reasoning_tokens: None,
            cache_read_tokens: None,
        });

    // Calculate cost
    let cost = if let Some(payload) = payload.as_ref() {
        if let Some(model) = payload
            .request_model
            .as_ref()
            .or(payload.response_model.as_ref())
        {
            let pricing =
                ModelPricing::for_model(payload.system.as_deref().unwrap_or("openai"), model);
            Some(payload.calculate_cost(&pricing))
        } else {
            None
        }
    } else {
        None
    };

    // Determine status (use error_type field and span type)
    let status = if payload
        .as_ref()
        .and_then(|p| p.error_type.as_ref())
        .is_some()
        || edge.get_span_type() == agentreplay_core::SpanType::Error
    {
        "error".to_string()
    } else {
        "completed".to_string()
    };

    // Get agent name
    let agent_name = state.agent_registry.get_display_name(edge.agent_id);

    let response = DetailedTraceResponse {
        // Basic trace info
        trace_id: format!("{:#x}", edge.session_id),
        span_id: format!("{:#x}", edge.edge_id),
        parent_span_id: if edge.causal_parent != 0 {
            Some(format!("{:#x}", edge.causal_parent))
        } else {
            None
        },
        session_id: edge.session_id,
        span_type: format!("{:?}", edge.span_type),
        timestamp_us: edge.timestamp_us,
        duration_us: edge.duration_us,
        duration_ms: edge.duration_us as f64 / 1000.0,
        status,

        // Agent and project info
        agent_id: edge.agent_id,
        agent_name,
        project_id: edge.project_id,
        tenant_id: edge.tenant_id,

        // Model and provider info
        provider: payload.as_ref().and_then(|p| p.system.clone()),
        model: payload
            .as_ref()
            .and_then(|p| p.request_model.clone())
            .or_else(|| payload.as_ref().and_then(|p| p.response_model.clone())),
        route: payload.as_ref().and_then(|p| p.operation_name.clone()),

        // Token and cost info
        tokens: edge.token_count,
        cost,
        confidence: if edge.confidence > 0.0 && edge.confidence <= 1.0 {
            Some(edge.confidence)
        } else {
            None
        },

        // Structured data
        prompts,
        completions,
        tool_calls,
        hyperparameters,
        token_breakdown,

        // Previews
        input_preview: payload.as_ref().and_then(get_input_preview),
        output_preview: payload.as_ref().and_then(get_output_preview),

        // Raw attributes
        attributes: payload.and_then(|p| serde_json::to_value(p).ok()),

        // Canonical eval trace
        eval_trace: Some(build_eval_trace_v1(&state, &edge)),
    };

    Ok(Json(response))
}
