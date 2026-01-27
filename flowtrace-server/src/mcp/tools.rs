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

//! MCP Tools Implementation
//!
//! Implements the core tools exposed via MCP:
//!
//! - `search_traces`: Semantic search with multi-signal relevance scoring
//! - `get_context`: Retrieve relevant context for error resolution
//! - `get_trace_details`: Get full details for a specific trace
//! - `get_related_traces`: Get causally related traces from the graph
//!
//! ## Project Isolation
//!
//! All MCP tools operate in an isolated context:
//! - **Tenant ID 2**: Dedicated tenant for MCP memory operations
//! - **Project ID 1000**: Auto-created "MCP Memory" project
//!
//! This ensures MCP's operations don't conflict with LLM observability tracing.

pub mod registry;

use crate::api::AppState;
use crate::mcp::context::{MCP_DEFAULT_PROJECT_ID, MCP_TENANT_ID};
use crate::mcp::protocol::*;
use crate::mcp::relevance::{BatchRelevanceScorer, RelevanceConfig};
use flowtrace_index::embedding::{EmbeddingProvider, LocalEmbeddingProvider};
use flowtrace_index::{CausalIndex, Embedding};
use serde_json::json;
use std::sync::Arc;

/// Tool registry - defines all available MCP tools
pub fn get_tool_definitions() -> Vec<Tool> {
    vec![
        Tool {
            name: "search_traces".to_string(),
            description: Some(
                "Search for relevant traces using natural language. Returns traces ranked by \
                 semantic similarity, temporal recency, and graph influence."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language search query (e.g., 'authentication errors', 'slow API calls')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 10, max: 100)",
                        "default": 10
                    },
                    "start_ts": {
                        "type": "integer",
                        "description": "Start timestamp in microseconds (optional)"
                    },
                    "end_ts": {
                        "type": "integer",
                        "description": "End timestamp in microseconds (optional)"
                    },
                    "span_types": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Filter by span types (e.g., ['llm', 'tool', 'error'])"
                    },
                    "include_payload": {
                        "type": "boolean",
                        "description": "Include payload content in results",
                        "default": false
                    },
                    "include_related": {
                        "type": "boolean",
                        "description": "Include related traces from causal graph",
                        "default": false
                    }
                },
                "required": ["query"]
            }),
        },
        Tool {
            name: "get_context".to_string(),
            description: Some(
                "Retrieve relevant context for an error or question. Searches historical traces \
                 to find similar past issues and their resolutions."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The error message or question to find context for"
                    },
                    "context_type": {
                        "type": "string",
                        "enum": ["error_resolution", "code_changes", "related_traces", "all"],
                        "description": "Type of context to retrieve",
                        "default": "error_resolution"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of context items (default: 5)",
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        },
        Tool {
            name: "get_trace_details".to_string(),
            description: Some(
                "Get full details for a specific trace by ID, including payload and metadata."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "edge_id": {
                        "type": "string",
                        "description": "The trace edge ID in hex format (e.g., '0x1234abcd')"
                    }
                },
                "required": ["edge_id"]
            }),
        },
        Tool {
            name: "get_related_traces".to_string(),
            description: Some(
                "Get traces that are causally related to a given trace (ancestors, descendants, \
                 or path between traces)."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "edge_id": {
                        "type": "string",
                        "description": "The trace edge ID in hex format"
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["ancestors", "descendants", "both"],
                        "description": "Direction to traverse the causal graph",
                        "default": "both"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum depth to traverse (default: 10)",
                        "default": 10
                    }
                },
                "required": ["edge_id"]
            }),
        },
        Tool {
            name: "get_trace_summary".to_string(),
            description: Some(
                "Get a high-level summary of traces matching certain criteria, including \
                 statistics on errors, latency, and token usage."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "time_range": {
                        "type": "string",
                        "enum": ["last_hour", "last_day", "last_week", "last_month"],
                        "description": "Time range for the summary",
                        "default": "last_day"
                    },
                    "group_by": {
                        "type": "string",
                        "enum": ["span_type", "model", "agent", "session"],
                        "description": "How to group the summary statistics"
                    }
                }
            }),
        },
    ]
}

/// Execute the search_traces tool
///
/// Uses the underlying semantic search which implements two-phase retrieval:
/// 1. Phase 1: HNSW approximate search for candidates
/// 2. Phase 2: Rerank with exact cosine similarity (in SemanticSearchEngine)
/// Then applies multi-signal relevance scoring (semantic + temporal + graph)
pub async fn execute_search_traces(
    state: &AppState,
    params: TraceSearchParams,
    causal_index: Arc<CausalIndex>,
) -> Result<CallToolResult, String> {
    let limit = params.limit.clamp(1, 100);

    // Initialize embedding provider
    let provider = LocalEmbeddingProvider::default_provider()
        .map_err(|e| format!("Failed to initialize embedding provider: {}", e))?;

    // Generate query embedding
    let query_vec = provider
        .embed(&params.query)
        .map_err(|e| format!("Failed to generate query embedding: {}", e))?;
    let query_embedding = Embedding::from_vec(query_vec);

    // Two-phase retrieval:
    // - Phase 1: HNSW approximate search fetches 3x candidates (handled internally)
    // - Phase 2: Rerank with exact distances (handled by SemanticSearchEngine)
    // We fetch 2x limit for additional multi-signal filtering
    let edges = state
        .db
        .semantic_search(&query_embedding, limit * 2)
        .map_err(|e| format!("Semantic search failed: {}", e))?;

    if edges.is_empty() {
        return Ok(CallToolResult {
            content: vec![ToolContent::Text {
                text: json!({
                    "results": [],
                    "count": 0,
                    "message": "No matching traces found"
                })
                .to_string(),
            }],
            is_error: None,
        });
    }

    // Create batch scorer with causal index
    let scorer = BatchRelevanceScorer::new(causal_index.clone());

    // Prepare traces for scoring (edge_id, semantic_score, timestamp_us)
    // Note: semantic_search returns edges without scores, so we use 1.0 as base
    let traces_to_score: Vec<(u128, f64, u64)> = edges
        .iter()
        .map(|edge| (edge.edge_id, 0.8, edge.timestamp_us)) // Base semantic score
        .collect();

    // Score and rank traces
    let scored = scorer.score_batch(traces_to_score);

    // Build results
    let mut results: Vec<TraceSearchResult> = Vec::new();

    for (edge_id, final_score, semantic_score, temporal_score, graph_score) in
        scored.iter().take(limit)
    {
        // Find the edge
        if let Some(edge) = edges.iter().find(|e| e.edge_id == *edge_id) {
            let mut result = TraceSearchResult {
                edge_id: format!("{:#x}", edge.edge_id),
                timestamp_us: edge.timestamp_us,
                operation: format!("{:?}", edge.get_span_type()),
                duration_ms: edge.duration_us as f64 / 1_000.0,
                tokens: edge.token_count,
                cost: estimate_cost(edge.token_count),
                relevance_score: *final_score,
                semantic_score: *semantic_score,
                temporal_score: *temporal_score,
                graph_score: *graph_score,
                payload_summary: None,
                related_traces: None,
            };

            // Include payload if requested
            if params.include_payload {
                if let Ok(Some(payload_bytes)) = state.db.get_payload(edge.edge_id) {
                    if let Ok(payload_str) = String::from_utf8(payload_bytes) {
                        // Truncate to first 500 chars
                        let summary = if payload_str.len() > 500 {
                            format!("{}...", &payload_str[..500])
                        } else {
                            payload_str
                        };
                        result.payload_summary = Some(summary);
                    }
                }
            }

            // Include related traces if requested
            if params.include_related {
                let children = causal_index.get_children(edge.edge_id);
                let parents = causal_index.get_parents(edge.edge_id);

                let mut related: Vec<String> = Vec::new();
                for id in parents.iter().take(3) {
                    related.push(format!("{:#x}", id));
                }
                for id in children.iter().take(3) {
                    related.push(format!("{:#x}", id));
                }

                if !related.is_empty() {
                    result.related_traces = Some(related);
                }
            }

            results.push(result);
        }
    }

    let response = json!({
        "results": results,
        "count": results.len(),
        "query": params.query,
        "mcp_context": {
            "tenant_id": MCP_TENANT_ID,
            "project_id": MCP_DEFAULT_PROJECT_ID,
            "isolation": "active"
        }
    });

    Ok(CallToolResult {
        content: vec![ToolContent::Text {
            text: response.to_string(),
        }],
        is_error: None,
    })
}

/// Execute the get_context tool
pub async fn execute_get_context(
    state: &AppState,
    params: GetContextParams,
    causal_index: Arc<CausalIndex>,
) -> Result<CallToolResult, String> {
    let limit = params.limit.clamp(1, 20);

    // Initialize embedding provider
    let provider = LocalEmbeddingProvider::default_provider()
        .map_err(|e| format!("Failed to initialize embedding provider: {}", e))?;

    // Generate query embedding
    let query_vec = provider
        .embed(&params.query)
        .map_err(|e| format!("Failed to generate query embedding: {}", e))?;
    let query_embedding = Embedding::from_vec(query_vec);

    // Search for relevant traces
    let edges = state
        .db
        .semantic_search(&query_embedding, limit * 3)
        .map_err(|e| format!("Search failed: {}", e))?;

    let mut items: Vec<ContextItem> = Vec::new();

    // Create relevance scorer
    let scorer = BatchRelevanceScorer::with_config(
        causal_index.clone(),
        RelevanceConfig::influence_focused(), // Prioritize influential traces
    );

    // Score traces
    let traces_to_score: Vec<(u128, f64, u64)> = edges
        .iter()
        .map(|edge| (edge.edge_id, 0.8, edge.timestamp_us))
        .collect();

    let scored = scorer.score_batch(traces_to_score);

    for (edge_id, final_score, _, _, _) in scored.iter().take(limit) {
        if let Some(edge) = edges.iter().find(|e| e.edge_id == *edge_id) {
            // Get payload content
            let content = if let Ok(Some(payload_bytes)) = state.db.get_payload(edge.edge_id) {
                if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
                    json_val
                } else if let Ok(text) = String::from_utf8(payload_bytes) {
                    json!({ "text": text })
                } else {
                    json!({})
                }
            } else {
                json!({})
            };

            let context_type = match edge.get_span_type() {
                flowtrace_core::SpanType::Error => "error_trace",
                flowtrace_core::SpanType::ToolCall => "tool_call",
                flowtrace_core::SpanType::ToolResponse => "tool_response",
                flowtrace_core::SpanType::Planning | flowtrace_core::SpanType::Reasoning => {
                    "llm_call"
                }
                _ => "trace",
            };

            items.push(ContextItem {
                context_type: context_type.to_string(),
                relevance_score: *final_score,
                description: format!(
                    "{} at {} (duration: {}ms, tokens: {})",
                    context_type,
                    edge.timestamp_us,
                    edge.duration_us / 1000,
                    edge.token_count
                ),
                content,
                source_trace_id: Some(format!("{:#x}", edge.edge_id)),
                timestamp_us: Some(edge.timestamp_us),
            });
        }
    }

    let result = GetContextResult {
        query: params.query,
        items,
        total_matches: edges.len(),
    };

    Ok(CallToolResult {
        content: vec![ToolContent::Text {
            text: serde_json::to_string(&result).unwrap_or_default(),
        }],
        is_error: None,
    })
}

/// Execute the get_trace_details tool
pub async fn execute_get_trace_details(
    state: &AppState,
    edge_id_str: &str,
) -> Result<CallToolResult, String> {
    // Parse edge ID (remove 0x prefix if present)
    let edge_id_clean = edge_id_str.trim_start_matches("0x");
    let edge_id = u128::from_str_radix(edge_id_clean, 16)
        .map_err(|_| format!("Invalid edge ID format: {}", edge_id_str))?;

    // Get the edge
    let edge = state
        .db
        .get(edge_id)
        .map_err(|e| format!("Failed to get trace: {}", e))?
        .ok_or_else(|| format!("Trace not found: {}", edge_id_str))?;

    // Get payload
    let payload = if let Ok(Some(payload_bytes)) = state.db.get_payload(edge_id) {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
            Some(json_val)
        } else if let Ok(text) = String::from_utf8(payload_bytes) {
            Some(json!({ "raw_text": text }))
        } else {
            None
        }
    } else {
        None
    };

    let result = json!({
        "edge_id": format!("{:#x}", edge.edge_id),
        "timestamp_us": edge.timestamp_us,
        "duration_us": edge.duration_us,
        "duration_ms": edge.duration_us as f64 / 1000.0,
        "span_type": format!("{:?}", edge.get_span_type()),
        "token_count": edge.token_count,
        "estimated_cost": estimate_cost(edge.token_count),
        "agent_id": edge.agent_id,
        "session_id": edge.session_id,
        "tenant_id": edge.tenant_id,
        "causal_parent": format!("{:#x}", edge.causal_parent),
        "payload": payload,
    });

    Ok(CallToolResult {
        content: vec![ToolContent::Text {
            text: result.to_string(),
        }],
        is_error: None,
    })
}

/// Execute the get_related_traces tool
pub async fn execute_get_related_traces(
    _state: &AppState,
    causal_index: Arc<CausalIndex>,
    edge_id_str: &str,
    direction: &str,
    max_depth: usize,
) -> Result<CallToolResult, String> {
    // Parse edge ID
    let edge_id_clean = edge_id_str.trim_start_matches("0x");
    let edge_id = u128::from_str_radix(edge_id_clean, 16)
        .map_err(|_| format!("Invalid edge ID format: {}", edge_id_str))?;

    let mut ancestors: Vec<String> = Vec::new();
    let mut descendants: Vec<String> = Vec::new();

    match direction {
        "ancestors" | "both" => {
            for id in causal_index.get_ancestors(edge_id).iter().take(max_depth) {
                ancestors.push(format!("{:#x}", id));
            }
        }
        _ => {}
    }

    match direction {
        "descendants" | "both" => {
            let desc_with_depth = causal_index.get_descendants_with_depth(edge_id, max_depth, 100);
            for (id, depth) in desc_with_depth {
                if id != edge_id {
                    descendants.push(format!("{:#x} (depth: {})", id, depth));
                }
            }
        }
        _ => {}
    }

    // Get path to root if we have ancestors
    let path_to_root: Option<Vec<String>> = if !ancestors.is_empty() {
        if let Some(root_id) = ancestors.last() {
            let root_id_clean = root_id.trim_start_matches("0x");
            if let Ok(root) = u128::from_str_radix(root_id_clean, 16) {
                causal_index
                    .get_path(root, edge_id)
                    .map(|path| path.iter().map(|id| format!("{:#x}", id)).collect())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let result = json!({
        "edge_id": edge_id_str,
        "ancestors": ancestors,
        "descendants": descendants,
        "path_to_root": path_to_root,
        "total_ancestors": ancestors.len(),
        "total_descendants": descendants.len(),
    });

    Ok(CallToolResult {
        content: vec![ToolContent::Text {
            text: result.to_string(),
        }],
        is_error: None,
    })
}

/// Estimate cost based on token count (simple heuristic)
fn estimate_cost(tokens: u32) -> f64 {
    const PRICE_PER_1K_TOKENS: f64 = 0.002; // ~$0.002 per 1K tokens average
    (tokens as f64 / 1000.0) * PRICE_PER_1K_TOKENS
}
