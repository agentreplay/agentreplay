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

use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::State, Json};
use agentreplay_core::{AgentFlowEdge, SpanType};
use agentreplay_index::embedding::{EmbeddingProvider, LocalEmbeddingProvider};
use agentreplay_index::Embedding;
use serde::{Deserialize, Serialize};

use crate::{api::query::ApiError, api::AppState, auth::AuthContext};

const DEFAULT_LOOKBACK_US: u64 = 86_400_000_000; // 24 hours
const MAX_LIMIT: usize = 500;
const MAX_QUERY_LENGTH: usize = 2000;

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<TraceSearchView>,
    pub count: usize,
    pub query_interpretation: QueryInterpretation,
}

#[derive(Debug, Serialize)]
pub struct TraceSearchView {
    pub edge_id: String,
    pub timestamp_us: u64,
    pub operation: String,
    pub span_type: String,
    pub duration_ms: f64,
    pub tokens: u32,
    pub cost: f64,
    pub status: String,
    pub model: Option<String>,
    pub agent_id: u64,
    pub session_id: u64,
}

#[derive(Debug, Serialize)]
pub struct QueryInterpretation {
    pub model_filter: Option<String>,
    pub error_filter: bool,
    pub min_tokens: Option<u32>,
    pub time_range: String,
}

struct ParsedQuery {
    start_ts: u64,
    end_ts: u64,
    model: Option<String>,
    min_tokens: Option<u32>,
    only_errors: bool,
}

/// POST /api/v1/search
///
/// Performs semantic search using vector embeddings when possible,
/// falling back to content-based payload search for text queries.
pub async fn semantic_search(
    State(state): State<AppState>,
    axum::Extension(auth): axum::Extension<AuthContext>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, ApiError> {
    // Input validation
    if request.query.is_empty() {
        return Err(ApiError::BadRequest("Search query cannot be empty".into()));
    }
    if request.query.len() > MAX_QUERY_LENGTH {
        return Err(ApiError::BadRequest(format!(
            "Query too long: {} characters (max {})",
            request.query.len(),
            MAX_QUERY_LENGTH
        )));
    }

    let limit = request.limit.clamp(1, MAX_LIMIT);
    let parsed = parse_search_query(&request.query);
    let query_lower = request.query.to_lowercase();

    // First, try content-based search through payloads
    let edges = {
        // Get edges from temporal range
        let all_edges = state
            .db
            .query_temporal_range_for_tenant(parsed.start_ts, parsed.end_ts, auth.tenant_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Filter by content if it's a text search
        let mut matched_edges: Vec<AgentFlowEdge> = Vec::new();

        for edge in all_edges {
            // Apply filters
            if let Some(min_tokens) = parsed.min_tokens {
                if edge.token_count < min_tokens {
                    continue;
                }
            }

            // **FIX: Deleted vs Error Semantics**
            // Deleted edges should NEVER appear in search results regardless of filters
            // This fixes the bug where deleted edges could appear as "errors"
            if edge.is_deleted() {
                continue;
            }

            if parsed.only_errors {
                // Only include actual errors, not deleted edges
                if !matches!(edge.get_span_type(), SpanType::Error) {
                    continue;
                }
            }

            // Check payload content for the search query
            if let Ok(Some(payload_bytes)) = state.db.get_payload(edge.edge_id) {
                // Try to parse as JSON and search in string representation
                if let Ok(payload_str) = String::from_utf8(payload_bytes.clone()) {
                    if payload_str.to_lowercase().contains(&query_lower) {
                        matched_edges.push(edge);
                        continue;
                    }
                }

                // Also try to parse as JSON and check individual fields
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
                    if json_contains_text(&json, &query_lower) {
                        matched_edges.push(edge);
                    }
                }
            }
        }

        matched_edges
    };

    // If no results from content search and it looks like a semantic query, try embedding search
    let final_edges = if edges.is_empty() && is_natural_language_query(&request.query) {
        match perform_semantic_search(&state, &request.query, limit, auth.tenant_id).await {
            Ok(semantic_edges) => semantic_edges,
            Err(_) => edges, // Return empty if semantic search also fails
        }
    } else {
        edges
    };

    let mut filtered_edges = final_edges;
    filtered_edges.sort_by_key(|edge| std::cmp::Reverse(edge.timestamp_us));
    filtered_edges.truncate(limit);

    let results: Vec<TraceSearchView> = filtered_edges
        .into_iter()
        .map(TraceSearchView::from)
        .collect();
    let count = results.len();

    Ok(Json(SearchResponse {
        results,
        count,
        query_interpretation: QueryInterpretation {
            model_filter: parsed.model,
            error_filter: parsed.only_errors,
            min_tokens: parsed.min_tokens,
            time_range: format!("{} - {}", parsed.start_ts, parsed.end_ts),
        },
    }))
}

/// Recursively search JSON for text content
fn json_contains_text(json: &serde_json::Value, query: &str) -> bool {
    match json {
        serde_json::Value::String(s) => s.to_lowercase().contains(query),
        serde_json::Value::Array(arr) => arr.iter().any(|v| json_contains_text(v, query)),
        serde_json::Value::Object(obj) => obj.values().any(|v| json_contains_text(v, query)),
        _ => false,
    }
}

/// Check if query is natural language (for semantic search) vs structured
fn is_natural_language_query(query: &str) -> bool {
    let lower = query.to_lowercase();

    // Structured query indicators
    let structured_patterns = [
        "last hour",
        "last day",
        "last week",
        "past hour",
        "past day",
        "24h",
        "gpt-4",
        "gpt-3.5",
        "claude",
        "gemini",
        "more than",
        ">",
    ];

    // If it contains structured patterns, treat as structured query
    for pattern in structured_patterns {
        if lower.contains(pattern) {
            return false;
        }
    }

    // Natural language if it's more than 2 words
    query.split_whitespace().count() >= 2
}

/// Perform semantic search using embedding vectors
/// 
/// **Tenant Safety:** Results are filtered to only include edges belonging to the specified tenant.
/// This prevents cross-tenant data leakage in semantic search results.
async fn perform_semantic_search(
    state: &AppState,
    query: &str,
    limit: usize,
    tenant_id: u64,
) -> Result<Vec<AgentFlowEdge>, ApiError> {
    // Initialize embedding provider
    let provider = LocalEmbeddingProvider::default_provider().map_err(|e| {
        ApiError::Internal(format!("Failed to initialize embedding provider: {}", e))
    })?;

    // Generate query embedding
    let query_vec = provider
        .embed(query)
        .map_err(|e| ApiError::Internal(format!("Failed to generate query embedding: {}", e)))?;
    let query_embedding = Embedding::from_vec(query_vec);

    // Perform tenant-scoped semantic search
    // We request more results than needed to account for tenant filtering
    let expanded_limit = limit * 3; // Request 3x to ensure enough results after filtering
    let all_edges = state
        .db
        .semantic_search(&query_embedding, expanded_limit)
        .map_err(|e| ApiError::Internal(format!("Semantic search failed: {}", e)))?;

    // **TENANT ISOLATION**: Filter results to only include edges from the authenticated tenant
    // This is critical for multi-tenant security - prevents cross-tenant data exposure
    let tenant_edges: Vec<AgentFlowEdge> = all_edges
        .into_iter()
        .filter(|edge| edge.tenant_id == tenant_id)
        .take(limit)
        .collect();

    Ok(tenant_edges)
}

impl From<AgentFlowEdge> for TraceSearchView {
    fn from(edge: AgentFlowEdge) -> Self {
        let span = edge.get_span_type();
        Self {
            edge_id: format!("{:#x}", edge.edge_id),
            timestamp_us: edge.timestamp_us,
            operation: format!("{:?}", span),
            span_type: format!("{:?}", span),
            duration_ms: edge.duration_us as f64 / 1_000.0,
            tokens: edge.token_count,
            cost: estimate_edge_cost(&edge),
            status: if edge.is_deleted() || matches!(span, SpanType::Error) {
                "error".to_string()
            } else {
                "success".to_string()
            },
            model: None,
            agent_id: edge.agent_id,
            session_id: edge.session_id,
        }
    }
}

fn parse_search_query(query: &str) -> ParsedQuery {
    let lower = query.to_lowercase();

    let only_errors =
        lower.contains("failed") || lower.contains("error") || lower.contains("failure");

    let model = if lower.contains("gpt-4") {
        Some("gpt-4".to_string())
    } else if lower.contains("gpt-3.5") {
        Some("gpt-3.5".to_string())
    } else if lower.contains("claude") {
        Some("claude".to_string())
    } else if lower.contains("gemini") {
        Some("gemini".to_string())
    } else {
        None
    };

    let min_tokens = extract_min_tokens(&lower);
    let (start_ts, end_ts) = parse_time_range(&lower);

    ParsedQuery {
        start_ts,
        end_ts,
        model,
        min_tokens,
        only_errors,
    }
}

fn extract_min_tokens(query: &str) -> Option<u32> {
    if let Some(idx) = query.find(">") {
        let tail = &query[idx + 1..];
        return tail
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<u32>().ok());
    }

    if let Some(idx) = query.find("more than") {
        let tail = &query[idx + "more than".len()..];
        return tail
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<u32>().ok());
    }

    None
}

fn parse_time_range(query: &str) -> (u64, u64) {
    let now = current_timestamp_us();

    if query.contains("last hour") || query.contains("past hour") {
        (now.saturating_sub(3_600_000_000), now)
    } else if query.contains("last 6 hours") || query.contains("past 6 hours") {
        (now.saturating_sub(6 * 3_600_000_000), now)
    } else if query.contains("last day") || query.contains("past day") || query.contains("24h") {
        (now.saturating_sub(86_400_000_000), now)
    } else if query.contains("last week") || query.contains("past week") {
        (now.saturating_sub(7 * 86_400_000_000), now)
    } else {
        (now.saturating_sub(DEFAULT_LOOKBACK_US), now)
    }
}

fn estimate_edge_cost(edge: &AgentFlowEdge) -> f64 {
    const PRICE_PER_1K_TOKENS_USD: f64 = 0.002;
    (edge.token_count as f64 / 1_000.0) * PRICE_PER_1K_TOKENS_USD
}

fn current_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

const fn default_limit() -> usize {
    100
}
