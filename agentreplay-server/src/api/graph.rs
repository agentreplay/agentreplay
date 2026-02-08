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
use agentreplay_core::AgentFlowEdge;
use serde::Serialize;
use std::collections::HashMap;
use tracing::debug;

use crate::api::query::{find_edge_by_id_or_session, ApiError, AppState};
use crate::auth::AuthContext;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub layout: Option<GraphLayout>,
}

#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub node_id: String,
    pub span_id: String,
    pub label: String,
    pub span_type: String,
    pub duration_ms: f64,
    pub start_offset_ms: f64,
    pub tokens: u32,
    pub cost: f64,
    pub confidence: Option<f32>,
    pub status: String,
    pub position: Option<Position>,
}

#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: String,
}

#[derive(Debug, Serialize)]
pub struct GraphLayout {
    pub algorithm: String,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

// ============================================================================
// API Handlers
// ============================================================================

/// GET /api/v1/traces/:trace_id/graph
/// Get graph representation of a trace for Canvas View
pub async fn get_trace_graph(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
    auth: axum::Extension<AuthContext>,
) -> Result<Json<GraphResponse>, ApiError> {
    // Parse trace ID
    let trace_id_u128 = u128::from_str_radix(trace_id.trim_start_matches("0x"), 16)
        .map_err(|_| ApiError::BadRequest("Invalid trace ID format".into()))?;

    debug!("Generating graph for trace_id={:#x}", trace_id_u128);

    // 1. Fetch all spans for the trace
    // We reuse the logic from get_trace_observations to get the full tree

    // Get root span
    let root = match find_edge_by_id_or_session(&state, trace_id_u128, auth.tenant_id).await {
        Ok(Some(span)) => span,
        Ok(None) => return Err(ApiError::NotFound("Trace not found".into())),
        Err(e) => return Err(e),
    };

    // Get the correct database (project-specific or fallback)
    let db = if let Some(ref pm) = state.project_manager {
        match pm.get_or_open_project(root.project_id) {
            Ok(project_db) => project_db,
            Err(_) => state.db.clone(),
        }
    } else {
        state.db.clone()
    };

    // OPTIMIZED: Single call to get all descendants with depth information
    // This replaces the O(D) BFS loop with O(1) database round-trips
    const MAX_SPANS: usize = 10_000;
    const MAX_DEPTH: usize = 1000;

    let all_spans_with_depth = db
        .get_descendants_with_depth_for_tenant(root.edge_id, auth.tenant_id, MAX_DEPTH, MAX_SPANS)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    if all_spans_with_depth.len() >= MAX_SPANS {
        return Err(ApiError::BadRequest("Trace too large".into()));
    }

    // Extract spans and build parent map
    let all_spans: Vec<_> = all_spans_with_depth.iter().map(|(span, _)| *span).collect();

    // 2. Build Graph Nodes and Edges
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let start_time_us = root.timestamp_us;

    // Use depths from the optimized traversal
    let mut levels: HashMap<usize, Vec<String>> = HashMap::new();
    let mut node_depths: HashMap<u128, usize> = HashMap::new();

    for (span, depth) in &all_spans_with_depth {
        node_depths.insert(span.edge_id, *depth);
        levels
            .entry(*depth)
            .or_default()
            .push(format!("{:#x}", span.edge_id));
    }

    let mut parent_map: HashMap<u128, u128> = HashMap::new();
    for span in &all_spans {
        if span.causal_parent != 0 {
            parent_map.insert(span.edge_id, span.causal_parent);
        }
    }

    // Helper to fetch payload for a span
    let fetch_payload = |edge: &AgentFlowEdge| -> Option<crate::otel_genai::GenAIPayload> {
        if edge.has_payload == 0 {
            return None;
        }

        // Try to get from the appropriate database (project-specific or main)
        let payload_bytes = if let Some(ref pm) = state.project_manager {
            match pm.get_or_open_project(edge.project_id) {
                Ok(db) => db.get_payload(edge.edge_id).ok().flatten(),
                Err(_) => state.db.get_payload(edge.edge_id).ok().flatten(),
            }
        } else {
            state.db.get_payload(edge.edge_id).ok().flatten()
        };

        payload_bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok())
    };

    // Create Nodes
    for span in &all_spans {
        let node_id = format!("{:#x}", span.edge_id);
        let duration_ms = span.duration_us as f64 / 1000.0;
        let start_offset_ms = (span.timestamp_us.saturating_sub(start_time_us)) as f64 / 1000.0;

        let status = if span.get_span_type() == agentreplay_core::SpanType::Error {
            "error"
        } else {
            "completed"
        };

        // Calculate position
        let depth = *node_depths.get(&span.edge_id).unwrap_or(&0);
        let level_nodes = levels.get(&depth).unwrap();
        let index_in_level = level_nodes
            .iter()
            .position(|id| id == &node_id)
            .unwrap_or(0);

        // Simple grid layout
        let x = start_offset_ms * 0.1; // Time-based X
        let y = depth as f64 * 100.0 + (index_in_level as f64 * 50.0); // Depth-based Y

        // Fetch payload to enrich node
        let mut node = GraphNode {
            node_id: node_id.clone(),
            span_id: node_id.clone(),
            label: format!("{:?}", span.span_type),
            span_type: format!("{:?}", span.span_type).to_lowercase(),
            duration_ms,
            start_offset_ms,
            tokens: span.token_count,
            cost: 0.0,
            confidence: if span.confidence > 0.0 && span.confidence <= 1.0 {
                Some(span.confidence)
            } else {
                Some(0.5)
            },
            status: status.to_string(),
            position: Some(Position { x, y }),
        };

        // Enrich with payload data
        if let Some(payload) = fetch_payload(span) {
            // Update label with operation name
            node.label = payload
                .operation_name
                .clone()
                .or(payload.request_model.clone())
                .unwrap_or_else(|| format!("{:?}", span.span_type));

            // Update status based on payload
            if payload.error_type.is_some() {
                node.status = "error".to_string();
            }

            // Calculate cost
            if let Some(ref model) = payload.request_model {
                let pricing = crate::otel_genai::ModelPricing::for_model(
                    payload.system.as_deref().unwrap_or("openai"),
                    model,
                );
                node.cost = payload.calculate_cost(&pricing);
            }

            // We're not adding metadata to GraphNode itself to keep it lean
            // But the detail view could fetch /api/v1/traces/:id/attributes
        }

        nodes.push(node);

        // Create Edge if parent exists and is in our dataset
        if span.causal_parent != 0 {
            // Check if parent is in our list by checking node_depths (all nodes are there)
            if node_depths.contains_key(&span.causal_parent) {
                edges.push(GraphEdge {
                    source: format!("{:#x}", span.causal_parent),
                    target: node_id,
                    edge_type: "causal".to_string(),
                });
            }
        }
    }

    Ok(Json(GraphResponse {
        nodes,
        edges,
        layout: Some(GraphLayout {
            algorithm: "temporal-hierarchical".to_string(),
            width: 1000.0, // Dynamic based on content
            height: 1000.0,
        }),
    }))
}
