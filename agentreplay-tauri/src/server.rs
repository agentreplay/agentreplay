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

// agentreplay-tauri/src/server.rs
//! HTTP Server Integration for Tauri Desktop App
//!
//! This module embeds the agentreplay-server HTTP server into the Tauri application,
//! allowing SDKs to send traces to the desktop app via HTTP.

use crate::AppState;
use anyhow::Result;
use axum::{
    extract::State as AxumState,
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use agentreplay_core::{
    AgentFlowEdge, ModelComparisonRequest, ModelPricingRegistry, ModelSelection,
    EvalDataset, TestCase, EvalRun, RunResult, SpanType, PromptTemplate,
    CodingAgent, CodingObservation, CodingSession, SessionState, SessionSummary, ToolAction,
    generate_observation_id, generate_session_id,
};
use agentreplay_storage::VersionStore;
use governor::{Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

/// Rate limiter configuration
/// Default: 10,000 spans per minute with burst of 50,000
const RATE_LIMIT_SPANS_PER_MINUTE: u32 = 10_000;
const RATE_LIMIT_BURST_SIZE: u32 = 50_000;

/// Deterministic project ID for "Claude Code" (hash-based)
/// Legacy traces from older plugins have project_id=0 and should only
/// appear under this project, not leak into other projects.
const CLAUDE_CODE_PROJECT_ID: u16 = 49455;

/// Shared state for the embedded HTTP server
#[derive(Clone)]
pub struct ServerState {
    pub tauri_state: AppState,
    pub start_time: u64,
    pub pricing_registry: Arc<ModelPricingRegistry>,
    /// Rate limiter for ingestion endpoints (token bucket algorithm)
    pub rate_limiter: Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
    /// Version store for trace references
    pub version_store: Arc<VersionStore>,
}

/// Simplified span structure for ingestion (matches agentreplay-observability)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentreplaySpan {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub attributes: HashMap<String, String>,
}

/// Convert span_type number to human-readable string
fn span_type_to_string(span_type: u32) -> &'static str {
    match SpanType::from_u64(span_type as u64) {
        SpanType::Root => "Root",
        SpanType::Planning => "Planning",
        SpanType::Reasoning => "Reasoning",
        SpanType::ToolCall => "Tool Call",
        SpanType::ToolResponse => "Tool Response",
        SpanType::Synthesis => "Synthesis",
        SpanType::Response => "LLM Response",
        SpanType::Error => "Error",
        SpanType::Retrieval => "Retrieval",
        SpanType::Embedding => "Embedding",
        SpanType::HttpCall => "HTTP Call",
        SpanType::Database => "Database",
        SpanType::Function => "Function",
        SpanType::Reranking => "Reranking",
        SpanType::Parsing => "Parsing",
        SpanType::Generation => "Generation",
        SpanType::Custom => "Custom",
    }
}

/// Request body for POST /api/v1/traces
#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub spans: Vec<AgentreplaySpan>,
}

/// Response for POST /api/v1/traces
#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub accepted: usize,
    pub rejected: usize,
    pub errors: Vec<String>,
}

/// Starts the embedded HTTP server
pub async fn start_embedded_server(host: String, port: u16, tauri_state: AppState) -> Result<()> {
    let start_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    // Clone shutdown token before moving tauri_state
    let shutdown_token = tauri_state.shutdown_token.clone();

    // Get data directory for pricing cache
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("agentreplay");
    
    // Initialize pricing registry with LiteLLM sync
    let pricing_registry = Arc::new(ModelPricingRegistry::new(data_dir.clone()));
    
    // Start background sync of LiteLLM pricing (non-blocking, with retry)
    let registry_clone = Arc::clone(&pricing_registry);
    tokio::spawn(async move {
        // Try up to 3 times with increasing delays
        for attempt in 1..=3 {
            match registry_clone.sync_from_litellm().await {
                Ok(_) => {
                    info!("Successfully synced LiteLLM model pricing");
                    return;
                }
                Err(e) => {
                    if attempt < 3 {
                        // Silent retry - just debug log
                        tracing::debug!("LiteLLM pricing sync attempt {} failed: {}, retrying...", attempt, e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5 * attempt as u64)).await;
                    } else {
                        // Final attempt - log at info level (not warn) since defaults work fine
                        info!("Using default model pricing (LiteLLM sync unavailable: {})", e);
                    }
                }
            }
        }
    });

    // Create rate limiter for ingestion endpoints
    // Token bucket: refills at RATE_LIMIT_SPANS_PER_MINUTE per minute, burst up to RATE_LIMIT_BURST_SIZE
    let rate_limiter = Arc::new(RateLimiter::direct(Quota::per_minute(
        NonZeroU32::new(RATE_LIMIT_SPANS_PER_MINUTE).unwrap()
    ).allow_burst(NonZeroU32::new(RATE_LIMIT_BURST_SIZE).unwrap())));

    // Create version store for trace references (lightweight, no data duplication)
    let _versions_dir = data_dir.join("versions");
    let version_store = Arc::new(
        VersionStore::new("agentreplay")
    );

    let server_state = ServerState {
        tauri_state,
        start_time,
        pricing_registry,
        rate_limiter,
        version_store,
    };

    // Build router with ingestion endpoints + analytics/sessions/playground
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/live", get(liveness_check))
        .route("/ready", get(readiness_check))
        .route("/api/v1/health", get(health_check_detailed))
        .route("/api/v1/traces", post(ingest_traces).get(query_traces))
        .route("/api/v1/traces/:trace_id", get(get_trace_by_id).delete(delete_trace_handler))
        .route("/api/v1/traces/:trace_id/observations", get(get_trace_observations))
        .route("/api/v1/traces/:trace_id/tree", get(get_trace_tree))
        .route("/api/v1/projects", post(create_project).get(list_projects_http))
        .route("/api/v1/projects/:project_id", delete(delete_project_handler))
        .route("/api/v1/projects/:project_id/metrics", get(get_project_metrics_handler)) // Added route
        .route("/api/v1/stats", get(get_stats))
        // Sessions endpoints
        .route("/api/v1/sessions", get(list_sessions_handler))
        .route("/api/v1/sessions/:session_id", get(get_session_handler).delete(delete_session_handler))
        // Analytics endpoints
        .route("/api/v1/analytics/timeseries", get(analytics_timeseries_handler))
        // Storage debug endpoint
        .route("/api/v1/storage/dump", get(storage_dump_handler))
        // Playground endpoint (now with real LLM support)
        .route("/api/v1/playground/run", post(playground_run_handler))
        // LLM model management endpoints
        .route("/api/v1/llm/models", get(list_llm_models_handler))
        .route("/api/v1/llm/config", get(get_llm_config_handler).post(update_llm_config_handler))
        .route("/api/v1/llm/check", get(check_llm_health_handler))
        // Model Comparison endpoints
        .route("/api/v1/comparison/run", post(run_comparison_handler))
        .route("/api/v1/comparison/models", get(list_comparison_models_handler))
        // Pricing endpoints
        .route("/api/v1/pricing/models", get(list_pricing_handler).post(import_pricing_handler))
        .route("/api/v1/pricing/models/all", get(list_all_pricing_handler))
        .route("/api/v1/pricing/sync", post(sync_pricing_handler))
        .route("/api/v1/pricing/calculate", post(calculate_pricing_handler))
        .route("/api/v1/pricing/custom", get(list_custom_pricing_handler).post(add_custom_pricing_handler).put(update_custom_pricing_handler))
        .route("/api/v1/pricing/custom/:model_id", delete(delete_custom_pricing_handler))
        // Cost Analytics endpoint (aggregates real trace data with pricing)
        .route("/api/v1/analytics/costs", get(analytics_costs_handler))
        // RAG/Memory endpoints
        .route("/api/v1/memory/info", get(crate::memory::get_memory_info))
        .route("/api/v1/memory/health", get(crate::memory::mcp_health_check))
        .route("/api/v1/memory/ingest", post(crate::memory::ingest_memory))
        .route("/api/v1/memory/retrieve", post(crate::memory::retrieve_memory))
        .route("/api/v1/memory/collections", get(crate::memory::list_collections))
        .route("/api/v1/memory/list", get(crate::memory::list_memories))
        // MCP JSON-RPC endpoint (for ping health checks)
        .route("/api/v1/mcp", post(crate::memory::handle_mcp_jsonrpc))
        // SSE endpoint for real-time trace streaming
        .route("/api/v1/traces/stream", get(crate::sse::sse_traces_handler))
        // SSE endpoint for real-time evaluation progress streaming
        .route("/api/v1/evals/stream", get(crate::sse::sse_evals_handler))
        // Search endpoint
        .route("/api/v1/search", post(search_traces_handler))
        // Insights endpoints (anomaly detection)
        .route("/api/v1/insights", get(get_insights_handler))
        .route("/api/v1/insights/summary", get(get_insights_summary_handler))
        // AI-powered trace analysis
        .route("/api/v1/traces/:trace_id/analyze", post(analyze_trace_handler))
        // Admin endpoints for data management
        .route("/api/v1/admin/reset", delete(reset_all_data_handler))
        .route("/api/v1/admin/backup", post(create_backup_handler))
        .route("/api/v1/admin/backups", get(list_backups_handler))
        .route("/api/v1/admin/backups/import", post(import_backup_handler))
        .route("/api/v1/admin/backups/:backup_id", delete(delete_backup_handler))
        .route("/api/v1/admin/backups/:backup_id/export", get(export_backup_handler))
        .route("/api/v1/admin/backups/:backup_id/restore", post(restore_backup_handler))
        // Evals endpoints
        .route("/api/v1/evals/datasets", get(list_datasets_handler).post(create_dataset_handler))
        .route("/api/v1/evals/datasets/:id", get(get_dataset_handler).delete(delete_dataset_handler))
        .route("/api/v1/evals/datasets/:id/examples", post(add_examples_handler))
        .route("/api/v1/evals/datasets/:dataset_id/examples/:example_id", delete(delete_example_handler))
        .route("/api/v1/evals/runs", get(list_eval_runs_handler).post(create_eval_run_handler))
        .route("/api/v1/evals/runs/:id", get(get_eval_run_handler).delete(delete_eval_run_handler))
        .route("/api/v1/evals/runs/:id/results", post(add_run_result_handler))
        .route("/api/v1/evals/runs/:id/complete", post(complete_eval_run_handler))
        .route("/api/v1/evals/compare", post(compare_eval_runs_handler))
        // G-Eval endpoint for LLM-as-judge evaluation
        .route("/api/v1/evals/geval", post(run_geval_handler))
        // Eval Pipeline endpoints (5-phase comprehensive evaluation)
        .route("/api/v1/evals/pipeline/collect", post(eval_pipeline_collect_handler))
        .route("/api/v1/evals/pipeline/process", post(eval_pipeline_process_handler))
        .route("/api/v1/evals/pipeline/annotate", post(eval_pipeline_annotate_handler))
        .route("/api/v1/evals/pipeline/golden", post(eval_pipeline_golden_handler))
        .route("/api/v1/evals/pipeline/evaluate", post(eval_pipeline_evaluate_handler))
        .route("/api/v1/evals/pipeline/recommendations", get(eval_pipeline_recommendations_handler))
        .route("/api/v1/evals/pipeline/metrics/definitions", get(eval_pipeline_metric_definitions_handler))
        .route("/api/v1/evals/pipeline/history", get(eval_pipeline_history_handler))
        // Prompt Registry endpoints
        .route("/api/v1/prompts", get(list_prompts_handler).post(store_prompt_handler))
        // Git-like Response Versioning endpoints
        .route("/api/v1/git/commit", post(git_commit_handler))
        .route("/api/v1/git/log", get(git_log_handler))
        .route("/api/v1/git/show/:ref", get(git_show_handler))
        .route("/api/v1/git/branches", get(git_list_branches_handler).post(git_create_branch_handler))
        .route("/api/v1/git/branches/:name", delete(git_delete_branch_handler))
        .route("/api/v1/git/tags", get(git_list_tags_handler).post(git_create_tag_handler))
        .route("/api/v1/git/diff", post(git_diff_handler))
        .route("/api/v1/git/stats", get(git_stats_handler))
        // Tool Registry endpoints (stub implementations)
        .route("/api/v1/tools", get(list_tools_handler).post(register_tool_handler))
        .route("/api/v1/tools/:tool_id", get(get_tool_handler).put(update_tool_handler).delete(unregister_tool_handler))
        .route("/api/v1/tools/:tool_id/execute", post(execute_tool_handler))
        .route("/api/v1/tools/:tool_id/executions", get(get_tool_executions_handler))
        .route("/api/v1/tools/mcp/servers", get(list_mcp_servers_handler).post(connect_mcp_server_handler))
        .route("/api/v1/tools/mcp/servers/:server_id/sync", post(sync_mcp_server_handler))
        .route("/api/v1/tools/mcp/servers/:server_id", delete(disconnect_mcp_server_handler))
        // Coding Sessions endpoints (IDE/coding agent traces)
        .route("/api/v1/coding-sessions", get(list_coding_sessions_handler).post(init_coding_session_handler))
        .route("/api/v1/coding-sessions/:session_id", get(get_coding_session_handler).delete(delete_coding_session_handler))
        .route("/api/v1/coding-sessions/:session_id/observations", get(list_observations_handler).post(add_observation_handler))
        .route("/api/v1/coding-sessions/:session_id/summarize", post(summarize_coding_session_handler))
        // Context injection endpoint for coding agents
        .route("/api/v1/context", get(get_context_handler))
        .with_state(server_state.clone());

    // MCP Router is now handled by start_mcp_server on a separate port
    let app = app.layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let addr = format!("{}:{}", host, port);
    info!("Starting embedded HTTP server on {}", addr);

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => {
            info!("Successfully bound to {}", addr);
            l
        }
        Err(e) => {
            error!("Failed to bind to {}: {}", addr, e);
            return Err(e.into());
        }
    };

    info!(
        "HTTP server listening and ready to accept connections on {}",
        addr
    );

    // Use graceful shutdown with cancellation token (already cloned at function start)
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
            info!("HTTP server received shutdown signal");
        })
        .await?;

    info!("HTTP server shutdown complete");
    Ok(())
}

/// GET /api/v1/traces - Query traces with filters
async fn query_traces(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Extract query parameters - support both naming conventions
    let start_ts = params
        .get("start_time")
        .or_else(|| params.get("start_ts"))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let end_ts = params
        .get("end_time")
        .or_else(|| params.get("end_ts"))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(u64::MAX);

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);

    let offset = params
        .get("offset")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let session_id = params.get("session_id").and_then(|s| s.parse::<u64>().ok());
    let project_id = params.get("project_id").and_then(|s| s.parse::<u16>().ok());
    
    // Additional filters
    let model_filter = params.get("model").or_else(|| params.get("models")).cloned();
    let status_filter = params.get("status").cloned();
    let min_latency_ms = params.get("min_latency_ms").and_then(|s| s.parse::<u64>().ok());
    let max_latency_ms = params.get("max_latency_ms").and_then(|s| s.parse::<u64>().ok());
    let full_text_search = params.get("full_text_search").cloned();

    // Query the database
    // For the Claude Code project (49455), also include legacy traces with project_id=0
    // since older plugin versions didn't set project_id correctly
    let edges = if let Some(pid) = project_id {
        let mut project_edges = match state.tauri_state.db.query_filtered(start_ts, end_ts, None, Some(pid)) {
            Ok(edges) => edges,
            Err(e) => {
                tracing::error!("Failed to query traces for project {}: {}", pid, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to query traces"})),
                )
                    .into_response();
            }
        };
        
        // Include project_id=0 traces ONLY for Claude Code project (49455)
        // Legacy plugin versions stored traces with project_id=0; these belong to Claude Code
        if pid == CLAUDE_CODE_PROJECT_ID {
            if let Ok(legacy_edges) = state.tauri_state.db.query_filtered(start_ts, end_ts, None, Some(0)) {
                project_edges.extend(legacy_edges);
            }
        }
        
        project_edges
    } else {
        // No project filter - return all traces (only when explicitly not filtering by project)
        match state.tauri_state.db.query_temporal_range(start_ts, end_ts) {
            Ok(edges) => edges,
            Err(e) => {
                tracing::error!("Failed to query traces: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to query traces"})),
                )
                    .into_response();
            }
        }
    };

    // Filter by session_id if provided
    // Also filter to only show root spans (causal_parent == 0) to avoid counting child spans
    let mut filtered_edges: Vec<_> = edges
        .into_iter()
        .filter(|e| {
            // Only include root spans (no parent) for trace listing
            // Child spans are shown in trace detail view
            let is_root = e.causal_parent == 0;
            let session_matches = session_id.map_or(true, |sid| e.session_id == sid);
            is_root && session_matches
        })
        .collect();
    
    // Apply latency filters
    if let Some(min_lat) = min_latency_ms {
        filtered_edges.retain(|e| (e.duration_us as u64) / 1000 >= min_lat);
    }
    if let Some(max_lat) = max_latency_ms {
        filtered_edges.retain(|e| (e.duration_us as u64) / 1000 <= max_lat);
    }
    
    // For model and full-text filters, we need to check payloads
    // This is done after initial filtering for efficiency
    if model_filter.is_some() || full_text_search.is_some() || status_filter.is_some() {
        let model_filter_lower = model_filter.as_ref().map(|m| m.to_lowercase());
        let search_lower = full_text_search.as_ref().map(|s| s.to_lowercase());
        let status_lower = status_filter.as_ref().map(|s| s.to_lowercase());
        
        filtered_edges.retain(|e| {
            let attributes = state.tauri_state.db
                .get_payload(e.edge_id)
                .ok()
                .flatten()
                .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok());
            
            // Check model filter
            if let Some(ref model_q) = model_filter_lower {
                let model = attributes.as_ref()
                    .and_then(|a| a.get("model").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_lowercase();
                if !model.contains(model_q) {
                    return false;
                }
            }
            
            // Check status filter
            if let Some(ref status_q) = status_lower {
                let status = attributes.as_ref()
                    .and_then(|a| a.get("status").and_then(|v| v.as_str()))
                    .unwrap_or("completed")
                    .to_lowercase();
                if !status.contains(status_q) {
                    return false;
                }
            }
            
            // Check full-text search (search in prompts, completions, name, model)
            if let Some(ref search_q) = search_lower {
                if let Some(ref attrs) = attributes {
                    let searchable_text = format!(
                        "{} {} {} {} {} {}",
                        attrs.get("model").and_then(|v| v.as_str()).unwrap_or(""),
                        attrs.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        attrs.get("operation_name").and_then(|v| v.as_str()).unwrap_or(""),
                        attrs.get("gen_ai.prompt.0.content").and_then(|v| v.as_str()).unwrap_or(""),
                        attrs.get("gen_ai.completion.0.content").and_then(|v| v.as_str()).unwrap_or(""),
                        attrs.get("input").and_then(|v| v.as_str()).unwrap_or(""),
                    ).to_lowercase();
                    
                    if !searchable_text.contains(search_q) {
                        return false;
                    }
                } else {
                    return false; // No attributes to search
                }
            }
            
            true
        });
    }

    // Sort by timestamp descending (most recent first)
    filtered_edges.sort_by(|a, b| b.timestamp_us.cmp(&a.timestamp_us));

    let total = filtered_edges.len();

    // Apply pagination
    let paginated_edges: Vec<_> = filtered_edges
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    // Convert edges to the traces format expected by the UI
    let traces: Vec<_> = paginated_edges
        .iter()
        .map(|edge| {
            // Get payload attributes if available
            let attributes = state.tauri_state.db
                .get_payload(edge.edge_id)
                .ok()
                .flatten()
                .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok());

            // Extract model (LLM model name, not tool name)
            let model = attributes
                .as_ref()
                .and_then(|a| {
                    a.get("model")
                        .or_else(|| a.get("gen_ai.request.model"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or("");

            // Extract agent_id from attributes for Claude Code identification
            let agent_id_attr = attributes
                .as_ref()
                .and_then(|a| a.get("agent_id").and_then(|v| v.as_str()))
                .unwrap_or("");

            let provider = attributes
                .as_ref()
                .and_then(|a| a.get("provider").and_then(|v| v.as_str()))
                .unwrap_or("");

            let display_name = attributes
                .as_ref()
                .and_then(|a| {
                    a.get("operation_name")
                        .or_else(|| a.get("name"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("");
            
            // Get readable span type name, use as fallback for display_name
            let span_type_name = span_type_to_string(edge.span_type);
            
            // For Claude Code traces, build a richer display name
            // e.g. "Bash" for tool calls, "Session End" for events
            let effective_display_name = if !display_name.is_empty() {
                display_name.to_string()
            } else {
                let tool_name = attributes.as_ref()
                    .and_then(|a| a.get("tool.name").and_then(|v| v.as_str()));
                let event_type = attributes.as_ref()
                    .and_then(|a| a.get("event.type").and_then(|v| v.as_str()));
                
                if let Some(tool) = tool_name {
                    format!("{} ({})", span_type_name, tool)
                } else if let Some(evt) = event_type {
                    // Convert event.type like "session_end" → "Session End"
                    evt.replace('_', " ").split_whitespace()
                        .map(|w| {
                            let mut c = w.chars();
                            match c.next() {
                                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                None => String::new(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    span_type_name.to_string()
                }
            };

            let cost = attributes
                .as_ref()
                .and_then(|a| a.get("cost").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);

            let input_tokens = attributes
                .as_ref()
                .and_then(|a| {
                    a.get("input_tokens")
                        .or_else(|| a.get("gen_ai.usage.input_tokens"))
                        .or_else(|| a.get("gen_ai.usage.prompt_tokens"))
                        .and_then(|v| v.as_u64())
                })
                .unwrap_or(0);

            let output_tokens = attributes
                .as_ref()
                .and_then(|a| {
                    a.get("output_tokens")
                        .or_else(|| a.get("gen_ai.usage.output_tokens"))
                        .or_else(|| a.get("gen_ai.usage.completion_tokens"))
                        .and_then(|v| v.as_u64())
                })
                .unwrap_or(0);

            // Extract input preview (first prompt content)
            let input_preview = attributes.as_ref().and_then(|a| {
                // Try GenAI semantic conventions first
                if let Some(content) = a.get("gen_ai.prompt.0.content").and_then(|v| v.as_str()) {
                    return Some(content.chars().take(200).collect::<String>());
                }
                // Try input field
                if let Some(input) = a.get("input") {
                    if let Some(s) = input.as_str() {
                        return Some(s.chars().take(200).collect::<String>());
                    }
                    return Some(serde_json::to_string(input).unwrap_or_default().chars().take(200).collect());
                }
                // Try tool.input (Claude Code tool calls)
                if let Some(tool_input) = a.get("tool.input").and_then(|v| v.as_str()) {
                    // Parse JSON to extract just the command/description
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(tool_input) {
                        if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                            return Some(cmd.chars().take(200).collect::<String>());
                        }
                        if let Some(desc) = parsed.get("description").and_then(|v| v.as_str()) {
                            return Some(desc.chars().take(200).collect::<String>());
                        }
                    }
                    return Some(tool_input.chars().take(200).collect::<String>());
                }
                // Try prompt field
                if let Some(prompt) = a.get("prompt").and_then(|v| v.as_str()) {
                    return Some(prompt.chars().take(200).collect::<String>());
                }
                // Try event.type for session events
                if let Some(evt) = a.get("event.type").and_then(|v| v.as_str()) {
                    let reason = a.get("session.end_reason").and_then(|v| v.as_str()).unwrap_or("");
                    if !reason.is_empty() {
                        return Some(format!("{} ({})", evt, reason));
                    }
                    return Some(evt.to_string());
                }
                None
            }).unwrap_or_default();

            // Extract output preview (first completion content)
            let output_preview = attributes.as_ref().and_then(|a| {
                // Try GenAI semantic conventions first
                if let Some(content) = a.get("gen_ai.completion.0.content").and_then(|v| v.as_str()) {
                    return Some(content.chars().take(200).collect::<String>());
                }
                // Try output field
                if let Some(output) = a.get("output") {
                    if let Some(s) = output.as_str() {
                        return Some(s.chars().take(200).collect::<String>());
                    }
                    return Some(serde_json::to_string(output).unwrap_or_default().chars().take(200).collect());
                }
                // Try tool.output (Claude Code tool calls)
                if let Some(tool_output) = a.get("tool.output").and_then(|v| v.as_str()) {
                    // Parse JSON to extract just stdout
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(tool_output) {
                        if let Some(stdout) = parsed.get("stdout").and_then(|v| v.as_str()) {
                            if !stdout.is_empty() {
                                return Some(stdout.chars().take(200).collect::<String>());
                            }
                        }
                        if let Some(stderr) = parsed.get("stderr").and_then(|v| v.as_str()) {
                            if !stderr.is_empty() {
                                return Some(format!("stderr: {}", stderr.chars().take(180).collect::<String>()));
                            }
                        }
                    }
                    return Some(tool_output.chars().take(200).collect::<String>());
                }
                // Try response/completion fields
                if let Some(resp) = a.get("response").or_else(|| a.get("completion")).and_then(|v| v.as_str()) {
                    return Some(resp.chars().take(200).collect::<String>());
                }
                None
            }).unwrap_or_default();
            
            // Get status from payload if available
            let status = attributes
                .as_ref()
                .and_then(|a| a.get("status").and_then(|v| v.as_str()))
                .unwrap_or("completed");

            // Extract tool name for Claude Code traces
            let tool_name = attributes
                .as_ref()
                .and_then(|a| a.get("tool.name").and_then(|v| v.as_str()))
                .unwrap_or("");

            // Extract event type for session events
            let event_type = attributes
                .as_ref()
                .and_then(|a| a.get("event.type").and_then(|v| v.as_str()))
                .unwrap_or("");

            // Determine if this is a Claude Code trace
            let is_claude_code = agent_id_attr == "claude-code" || edge.project_id == CLAUDE_CODE_PROJECT_ID;

            serde_json::json!({
                "trace_id": format!("{}", edge.edge_id),
                "span_id": format!("{}", edge.edge_id),
                "timestamp_us": edge.timestamp_us,
                "duration_ms": edge.duration_us / 1000,
                "duration_us": edge.duration_us,
                "session_id": format!("{}", edge.session_id),
                "agent_id": format!("{}", edge.agent_id),
                "agent_id_attr": agent_id_attr,
                "agent_name": if agent_id_attr == "claude-code" { "Claude Code".to_string() } else { format!("Agent {}", edge.agent_id) },
                "model": model,
                "provider": provider,
                "display_name": effective_display_name,
                "operation_name": effective_display_name,
                "span_type": span_type_name,
                "tool_name": tool_name,
                "event_type": event_type,
                "is_claude_code": is_claude_code,
                "token_count": edge.token_count,
                "tokens": edge.token_count,
                "cost": cost,
                "status": status,
                "confidence": edge.confidence,
                "project_id": edge.project_id,
                "tenant_id": format!("{}", edge.tenant_id),
                "input": input_preview,
                "output": output_preview,
                "input_preview": input_preview,
                "output_preview": output_preview,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "metadata": serde_json::json!({
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                    "span_type": span_type_name,
                }),
                "attributes": &attributes
            })
        })
        .collect();

    Json(serde_json::json!({
        "traces": traces,
        "total": total,
        "offset": offset,
        "limit": limit,
        "has_more": offset + traces.len() < total
    }))
    .into_response()
}

/// GET /api/v1/projects/:project_id/metrics - Get metrics for a specific project
async fn get_project_metrics_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(project_id): axum::extract::Path<u16>,
) -> impl IntoResponse {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
    let start_24h = now.saturating_sub(86_400_000_000); // 24 hours in us

    // Use query_filtered to ensure we only get edges for this project
    let edges = match state.tauri_state.db.query_filtered(start_24h, now, None, Some(project_id)) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to query project metrics: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to query metrics"})),
            ).into_response();
        }
    };
    
    if edges.is_empty() {
        return Json(serde_json::json!({
            "latency_ms": { "p50": 0, "p90": 0, "p95": 0 },
            "tokens": { "p50": 0, "p90": 0, "total": 0 },
            "cost_usd": { "total": 0, "avg": 0 }
        })).into_response();
    }
    
    // Compute aggregations
    let mut latencies: Vec<f64> = edges.iter().map(|e| e.duration_us as f64 / 1000.0).collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let mut tokens: Vec<f64> = edges.iter().map(|e| e.token_count as f64).collect();
    tokens.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let total_tokens: f64 = tokens.iter().sum();
    
    // Extract costs from payloads (best effort)
    let mut total_cost = 0.0;
    for edge in &edges {
        if let Ok(Some(payload)) = state.tauri_state.db.get_payload(edge.edge_id) {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&payload) {
                if let Some(cost) = json.get("cost").and_then(|v| v.as_f64()) {
                    total_cost += cost;
                }
            }
        }
    }
    
    let avg_cost = if !edges.is_empty() { total_cost / edges.len() as f64 } else { 0.0 };
    
    // Helpers
    let p50_lat = percentile(&latencies, 50.0);
    let p90_lat = percentile(&latencies, 90.0);
    let p95_lat = percentile(&latencies, 95.0);
    
    let p50_tok = percentile(&tokens, 50.0);
    let p90_tok = percentile(&tokens, 90.0);
    
    Json(serde_json::json!({
        "latency_ms": {
            "p50": p50_lat,
            "p90": p90_lat,
            "p95": p95_lat
        },
        "tokens": {
            "p50": p50_tok,
            "p90": p90_tok,
            "total": total_tokens
        },
        "cost_usd": {
            "total": total_cost,
            "avg": avg_cost
        }
    })).into_response()
}

/// Starts the dedicated MCP server
pub async fn start_mcp_server(host: String, port: u16, tauri_state: AppState) -> Result<()> {
    let addr = format!("{}:{}", host, port);
    info!("Starting dedicated MCP server on {}", addr);

    // Clone shutdown token
    let shutdown_token = tauri_state.shutdown_token.clone();

    // Prepare dependencies for MCP
    let causal_index = tauri_state.db.causal_index();
    let agent_registry_path = tauri_state.db_path.join("agent_registry.json");
    let server_agent_registry = Arc::new(agentreplay_server::agent_registry::AgentRegistry::new(agent_registry_path));
    let server_cost_tracker = Arc::new(agentreplay_server::cost_tracker::CostTracker::new());
    // Vector index types differ between Tauri and Server; set to None for now
    let vector_index: Option<std::sync::Arc<agentreplay_index::HnswIndex>> = None;

    // Construct Server AppState
    let server_app_state = agentreplay_server::api::AppState {
        db: tauri_state.db.clone(),
        project_manager: None,
        project_registry: None,
        trace_broadcaster: tauri_state.trace_broadcaster.clone(),
        agent_registry: server_agent_registry,
        db_path: tauri_state.db_path.display().to_string(),
        saved_view_registry: tauri_state.saved_view_registry.clone(),
        llm_manager: None,
        cost_tracker: server_cost_tracker,
        vector_index: vector_index,
        semantic_governor: None,
        eval_cache: None,
        ingestion_actor: None,
    };

    // Create MCP Router
    let mcp_router = agentreplay_server::mcp::mcp_router(server_app_state, causal_index);
    
    // Add CORS
    let app = mcp_router.layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    );

    // Bind and serve
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => {
             info!("MCP server bound to {}", addr);
             l
        },
        Err(e) => {
            error!("Failed to bind MCP server to {}: {}", addr, e);
            return Err(e.into());
        }
    };

    info!(
        "MCP server listening and ready to accept connections on {}",
        addr
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
            info!("MCP server received shutdown signal");
        })
        .await?;
    
    info!("MCP server shutdown complete");
    Ok(())
}

/// GET /api/v1/traces/:trace_id - Get a single trace by ID
async fn get_trace_by_id(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(trace_id): axum::extract::Path<String>,
    axum::extract::Query(_params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Parse trace_id - could be decimal or hex
    let edge_id: u128 = if trace_id.starts_with("0x") {
        u128::from_str_radix(&trace_id[2..], 16).unwrap_or(0)
    } else {
        trace_id.parse::<u128>().unwrap_or_else(|_| {
            // Try parsing as hex without prefix
            u128::from_str_radix(&trace_id, 16).unwrap_or(0)
        })
    };

    if edge_id == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid trace ID format"})),
        )
            .into_response();
    }

    // Try to get the edge from storage
    let edge = match state.tauri_state.db.get(edge_id) {
        Ok(Some(edge)) => edge,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Trace not found",
                    "trace_id": trace_id
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to fetch trace: {}", e),
                    "trace_id": trace_id
                })),
            )
                .into_response();
        }
    };

    // Get payload attributes if available
    let attributes = state.tauri_state.db
        .get_payload(edge.edge_id)
        .ok()
        .flatten()
        .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok());

    // Helper to get attribute value (handles both direct keys and GenAI semantic convention keys)
    let get_attr_str = |attrs: &serde_json::Value, keys: &[&str]| -> Option<String> {
        for key in keys {
            if let Some(v) = attrs.get(*key) {
                if let Some(s) = v.as_str() {
                    return Some(s.to_string());
                }
            }
        }
        None
    };

    let get_attr_u64 = |attrs: &serde_json::Value, keys: &[&str]| -> u64 {
        for key in keys {
            if let Some(v) = attrs.get(*key) {
                if let Some(n) = v.as_u64() {
                    return n;
                }
                if let Some(s) = v.as_str() {
                    if let Ok(n) = s.parse::<u64>() {
                        return n;
                    }
                }
            }
        }
        0
    };

    let model = attributes
        .as_ref()
        .and_then(|a| get_attr_str(a, &["model", "gen_ai.request.model", "llm.model", "openai.model"]))
        .unwrap_or_default();

    let provider = attributes
        .as_ref()
        .and_then(|a| get_attr_str(a, &["provider", "gen_ai.system", "llm.provider"]))
        .unwrap_or_default();

    let display_name = attributes
        .as_ref()
        .and_then(|a| get_attr_str(a, &["name", "span.name", "operation_name"]))
        .unwrap_or_else(|| "".to_string());
    
    // Get readable span type name, use as fallback for display_name
    let span_type_name = span_type_to_string(edge.span_type);
    let effective_display_name = if !display_name.is_empty() {
        display_name
    } else {
        let tool_name = attributes.as_ref()
            .and_then(|a| get_attr_str(a, &["tool.name"]));
        let event_type = attributes.as_ref()
            .and_then(|a| get_attr_str(a, &["event.type"]));
        
        if let Some(tool) = tool_name {
            format!("{} ({})", span_type_name, tool)
        } else if let Some(evt) = event_type {
            evt.replace('_', " ").split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            span_type_name.to_string()
        }
    };

    // Extract input/output - these can be in various formats
    let input = attributes.as_ref().and_then(|a| {
        // Try multiple possible keys for input
        for key in &["gen_ai.prompt", "gen_ai.prompt.0.content", "input", "prompt", "llm.prompts"] {
            if let Some(v) = a.get(*key) {
                if !v.is_null() {
                    return Some(v.clone());
                }
            }
        }
        // Try tool.input (Claude Code) - parse JSON to extract command
        if let Some(v) = a.get("tool.input").and_then(|v| v.as_str()) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(v) {
                return Some(parsed);
            }
            return Some(serde_json::Value::String(v.to_string()));
        }
        // Try event.type for session events
        if let Some(evt) = a.get("event.type").and_then(|v| v.as_str()) {
            let reason = a.get("session.end_reason").and_then(|v| v.as_str()).unwrap_or("");
            if !reason.is_empty() {
                return Some(serde_json::Value::String(format!("{} ({})", evt, reason)));
            }
            return Some(serde_json::Value::String(evt.to_string()));
        }
        None
    });

    let output = attributes.as_ref().and_then(|a| {
        // Try multiple possible keys for output  
        for key in &["gen_ai.completion", "gen_ai.completion.0.content", "output", "completion", "response", "llm.completions"] {
            if let Some(v) = a.get(*key) {
                if !v.is_null() {
                    return Some(v.clone());
                }
            }
        }
        // Try tool.output (Claude Code) - parse JSON to extract stdout
        if let Some(v) = a.get("tool.output").and_then(|v| v.as_str()) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(v) {
                return Some(parsed);
            }
            return Some(serde_json::Value::String(v.to_string()));
        }
        None
    });

    let cost = attributes
        .as_ref()
        .and_then(|a| a.get("cost").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);

    let input_tokens = attributes
        .as_ref()
        .map(|a| get_attr_u64(a, &["input_tokens", "gen_ai.usage.input_tokens", "gen_ai.usage.prompt_tokens", "llm.token_count.prompt"]))
        .unwrap_or(0);

    let output_tokens = attributes
        .as_ref()
        .map(|a| get_attr_u64(a, &["output_tokens", "gen_ai.usage.output_tokens", "gen_ai.usage.completion_tokens", "llm.token_count.completion"]))
        .unwrap_or(0);

    let total_tokens = attributes
        .as_ref()
        .map(|a| get_attr_u64(a, &["gen_ai.usage.total_tokens", "llm.token_count.total"]))
        .unwrap_or(edge.token_count as u64);

    // Extract prompts and completions from GenAI semantic conventions
    let mut prompts: Vec<serde_json::Value> = Vec::new();
    let mut completions: Vec<serde_json::Value> = Vec::new();

    if let Some(attrs) = &attributes {
        // Try to extract prompts from gen_ai.prompt.X.content format
        for i in 0..10 {
            let role_key = format!("gen_ai.prompt.{}.role", i);
            let content_key = format!("gen_ai.prompt.{}.content", i);
            
            if let Some(content) = attrs.get(&content_key).and_then(|v| v.as_str()) {
                let role = attrs.get(&role_key).and_then(|v| v.as_str()).unwrap_or("user");
                prompts.push(serde_json::json!({
                    "role": role,
                    "content": content
                }));
            } else {
                break;
            }
        }

        // Try to extract completions from gen_ai.completion.X.content format
        for i in 0..10 {
            let role_key = format!("gen_ai.completion.{}.role", i);
            let content_key = format!("gen_ai.completion.{}.content", i);
            
            if let Some(content) = attrs.get(&content_key).and_then(|v| v.as_str()) {
                let role = attrs.get(&role_key).and_then(|v| v.as_str()).unwrap_or("assistant");
                completions.push(serde_json::json!({
                    "role": role,
                    "content": content
                }));
            } else {
                break;
            }
        }

        // Fallback: try direct input/output if no structured prompts/completions
        if prompts.is_empty() {
            if let Some(input_val) = &input {
                let content = if input_val.is_string() {
                    input_val.as_str().unwrap_or("").to_string()
                } else {
                    serde_json::to_string_pretty(input_val).unwrap_or_default()
                };
                if !content.is_empty() {
                    prompts.push(serde_json::json!({
                        "role": "user",
                        "content": content
                    }));
                }
            }
        }

        if completions.is_empty() {
            if let Some(output_val) = &output {
                let content = if output_val.is_string() {
                    output_val.as_str().unwrap_or("").to_string()
                } else {
                    serde_json::to_string_pretty(output_val).unwrap_or_default()
                };
                if !content.is_empty() {
                    completions.push(serde_json::json!({
                        "role": "assistant",
                        "content": content
                    }));
                }
            }
        }
    }

    // Build response matching what frontend expects
    let trace_data = serde_json::json!({
        "trace_id": format!("{}", edge.edge_id),
        "span_id": format!("{}", edge.edge_id),
        "parent_span_id": if edge.causal_parent != 0 { Some(format!("{}", edge.causal_parent)) } else { None },
        "timestamp_us": edge.timestamp_us,
        "duration_ms": edge.duration_us / 1000,
        "duration_us": edge.duration_us,
        "session_id": edge.session_id,
        "agent_id": edge.agent_id,
        "agent_name": format!("Agent {}", edge.agent_id),
        "model": model,
        "provider": provider,
        "display_name": effective_display_name,
        "operation_name": effective_display_name,
        "span_type": span_type_name,
        "token_count": total_tokens,
        "tokens": total_tokens,
        "cost": cost,
        "status": "completed",
        "confidence": edge.confidence,
        "project_id": edge.project_id,
        "tenant_id": edge.tenant_id,
        "input": input,
        "output": output,
        "metadata": {
            "prompts": prompts,
            "completions": completions,
            "model": model,
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "total_tokens": total_tokens,
            "token_breakdown": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "total_tokens": total_tokens,
            },
            "span_type": span_type_name,
            "gen_ai.request.model": model,
            "gen_ai.usage.total_tokens": total_tokens,
        },
        "attributes": attributes,
    });

    Json(trace_data).into_response()
}

/// GET /api/v1/traces/:trace_id/observations - Get child spans/observations for a trace
async fn get_trace_observations(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(trace_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Parse trace_id
    let parent_edge_id: u128 = if trace_id.starts_with("0x") {
        u128::from_str_radix(&trace_id[2..], 16).unwrap_or(0)
    } else {
        trace_id.parse::<u128>().unwrap_or_else(|_| {
            u128::from_str_radix(&trace_id, 16).unwrap_or(0)
        })
    };

    if parent_edge_id == 0 {
        return Json(serde_json::json!([])).into_response();
    }

    // Get the parent edge to find its session_id
    let parent_edge = match state.tauri_state.db.get(parent_edge_id) {
        Ok(Some(edge)) => edge,
        _ => return Json(serde_json::json!([])).into_response(),
    };

    // Query all edges in the same session
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    
    let start_ts = parent_edge.timestamp_us.saturating_sub(3600_000_000); // 1 hour before
    let end_ts = now;

    let edges = match state.tauri_state.db.query_temporal_range(start_ts, end_ts) {
        Ok(edges) => edges,
        Err(_) => return Json(serde_json::json!([])).into_response(),
    };

    // Robustly find all edges belonging to this trace via causal links or session ID
    let mut trace_edge_ids = std::collections::HashSet::new();
    trace_edge_ids.insert(parent_edge_id);
    let session_id = parent_edge.session_id;

    let mut changed = true;
    let mut iter_count = 0;
    while changed && iter_count < 1000 {
        changed = false;
        iter_count += 1;
        
        for edge in &edges {
            if trace_edge_ids.contains(&edge.edge_id) {
                continue;
            }
            
            let match_session = session_id != 0 && edge.session_id == session_id;
            // Check if this edge is a child of any known edge in the trace
            let match_causal = trace_edge_ids.contains(&edge.causal_parent);
            
            if match_session || match_causal {
                trace_edge_ids.insert(edge.edge_id);
                changed = true;
            }
        }
    }

    if !trace_edge_ids.is_empty() {
        info!("Graph traversal found {} edges (including root) for trace {}", trace_edge_ids.len(), parent_edge_id);
    }

    // Filter to find ALL spans in the identified set
    let observations: Vec<_> = edges
        .into_iter()
        .filter(|e| trace_edge_ids.contains(&e.edge_id) && e.edge_id != parent_edge_id)
        .map(|edge| {
            // Get payload for each observation
            let attributes = state.tauri_state.db
                .get_payload(edge.edge_id)
                .ok()
                .flatten()
                .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok());

            let model = attributes
                .as_ref()
                .and_then(|a| a.get("model").or_else(|| a.get("gen_ai.request.model")).and_then(|v| v.as_str()))
                .unwrap_or("");

            let input = attributes
                .as_ref()
                .and_then(|a| a.get("input").or_else(|| a.get("gen_ai.prompt")).or_else(|| a.get("prompt")))
                .cloned();

            let output = attributes
                .as_ref()
                .and_then(|a| a.get("output").or_else(|| a.get("gen_ai.completion")).or_else(|| a.get("completion")))
                .cloned();

            let name = attributes
                .as_ref()
                .and_then(|a| a.get("name").or_else(|| a.get("span.name")).and_then(|v| v.as_str()))
                .unwrap_or("Unknown");

            serde_json::json!({
                "id": format!("{}", edge.edge_id),
                "trace_id": format!("{}", parent_edge_id),
                "parent_observation_id": if edge.causal_parent != 0 { 
                    Some(format!("{}", edge.causal_parent)) 
                } else { 
                    None 
                },
                "type": match edge.span_type {
                    1 => "generation",
                    2 => "span",
                    3 => "event",
                    _ => "span"
                },
                "name": name,
                "start_time": edge.timestamp_us,
                "end_time": edge.timestamp_us + edge.duration_us as u64,
                "model": model,
                "input": input,
                "output": output,
                "metadata": attributes,
                "status_message": null,
                "level": "DEFAULT",
                "completion_start_time": null,
                "usage": {
                    "input": edge.token_count / 2,
                    "output": edge.token_count / 2,
                    "total": edge.token_count
                }
            })
        })
        .collect();

    Json(observations).into_response()
}

/// GET /api/v1/traces/:trace_id/tree - Get the full trace tree
async fn get_trace_tree(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(trace_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Parse trace_id
    let root_edge_id: u128 = if trace_id.starts_with("0x") {
        u128::from_str_radix(&trace_id[2..], 16).unwrap_or(0)
    } else {
        trace_id.parse::<u128>().unwrap_or_else(|_| {
            u128::from_str_radix(&trace_id, 16).unwrap_or(0)
        })
    };

    if root_edge_id == 0 {
        return Json(serde_json::json!({"spans": [], "root": null})).into_response();
    }

    // Get the root edge
    let root_edge = match state.tauri_state.db.get(root_edge_id) {
        Ok(Some(edge)) => edge,
        _ => return Json(serde_json::json!({"spans": [], "root": null})).into_response(),
    };

    // Query all edges in the same session
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    
    let start_ts = root_edge.timestamp_us.saturating_sub(3600_000_000);
    let end_ts = now;

    let edges = match state.tauri_state.db.query_temporal_range(start_ts, end_ts) {
        Ok(edges) => edges,
        Err(_) => return Json(serde_json::json!({"spans": [], "root": null})).into_response(),
    };

    // Filter to same session
    let session_edges: Vec<_> = edges
        .into_iter()
        .filter(|e| e.session_id == root_edge.session_id)
        .collect();

    // Build tree structure
    let spans: Vec<_> = session_edges
        .iter()
        .map(|edge| {
            let attributes = state.tauri_state.db
                .get_payload(edge.edge_id)
                .ok()
                .flatten()
                .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok());

            let name = attributes
                .as_ref()
                .and_then(|a| a.get("name").or_else(|| a.get("span.name")).and_then(|v| v.as_str()))
                .unwrap_or("Unknown");

            serde_json::json!({
                "id": format!("{}", edge.edge_id),
                "parent_id": if edge.causal_parent != 0 { Some(format!("{}", edge.causal_parent)) } else { None },
                "name": name,
                "start_time": edge.timestamp_us,
                "end_time": edge.timestamp_us + edge.duration_us as u64,
                "duration_ms": edge.duration_us / 1000,
                "attributes": attributes,
            })
        })
        .collect();

    Json(serde_json::json!({
        "spans": spans,
        "root": format!("{}", root_edge_id)
    })).into_response()
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "agentreplay-desktop"
    }))
}

/// Kubernetes-style liveness probe - checks if the process is running
/// Returns 200 OK if the server can respond
async fn liveness_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "alive"
    }))
}

/// Kubernetes-style readiness probe - checks if the server is ready to accept requests
/// Verifies database connectivity and essential components
async fn readiness_check(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    // Check if database is accessible by attempting a simple query
    let db_ready = state.tauri_state.db.query_temporal_range(0, 1).is_ok();
    
    if db_ready {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ready",
                "checks": {
                    "database": "ok"
                }
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not_ready",
                "checks": {
                    "database": "unavailable"
                }
            })),
        )
    }
}

/// Detailed health check endpoint (compatible with agentreplay-server API)
async fn health_check_detailed(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - state.start_time;

    let stats = state.tauri_state.connection_stats.read();

    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": uptime,
        "storage": {
            "reachable": true,
            "total_edges": stats.total_traces_received
        },
        "api": {
            "requests_total": stats.total_traces_received,
            "avg_latency_ms": 0.0
        }
    }))
}



/// DELETE /api/v1/traces/:trace_id
async fn delete_trace_handler(
    AxumState(state): AxumState<ServerState>,
    Path(trace_id): Path<String>,
) -> impl IntoResponse {
    // Parse trace_id (hex or decimal)
    let edge_id = if trace_id.starts_with("0x") {
        u128::from_str_radix(&trace_id[2..], 16).unwrap_or(0)
    } else {
        trace_id.parse::<u128>().unwrap_or_else(|_| {
            u128::from_str_radix(&trace_id, 16).unwrap_or(0)
        })
    };

    if edge_id == 0 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid trace ID format"}))).into_response();
    }

    if let Err(e) = state.tauri_state.db.delete(edge_id, 1).await {
        tracing::error!("Failed to delete trace {}: {}", trace_id, e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to delete trace: {}", e)}))).into_response();
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "success", "message": "Trace deleted"}))).into_response()
}

/// DELETE /api/v1/sessions/:session_id
async fn delete_session_handler(
    AxumState(state): AxumState<ServerState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let session_id_u64 = match session_id.parse::<u64>() {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid session ID format"}))).into_response(),
    };

    let edge_ids = state.tauri_state.db.get_session_edges(session_id_u64);
    
    if edge_ids.is_empty() {
         return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Session not found or empty"}))).into_response();
    }

    let mut deleted_count = 0;
    for edge_id in edge_ids {
        if let Err(e) = state.tauri_state.db.delete(edge_id, 0).await {
            tracing::error!("Failed to delete edge {} in session {}: {}", edge_id, session_id_u64, e);
        } else {
            deleted_count += 1;
        }
    }

    (StatusCode::OK, Json(serde_json::json!({
        "deleted": true,
        "session_id": session_id,
        "count": deleted_count
    }))).into_response()
}        

/// Storage dump endpoint - returns raw storage records for debugging
async fn storage_dump_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let project_id = params.get("project_id").and_then(|s| s.parse::<u16>().ok());
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);

    // Get edges from storage
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    let start_ts = now.saturating_sub(30 * 24 * 3600 * 1_000_000); // Last 30 days

    let edges = match state.tauri_state.db.query_temporal_range(start_ts, now) {
        Ok(edges) => edges,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to query storage: {}", e)})),
            ).into_response();
        }
    };

    // Filter by project_id if provided
    let filtered_edges: Vec<_> = edges
        .into_iter()
        .filter(|e| project_id.map_or(true, |pid| e.project_id == pid))
        .take(limit)
        .collect();

    // Convert to storage records
    let mut records = Vec::new();
    for edge in filtered_edges {
        // Add edge record
        records.push(serde_json::json!({
            "key": format!("edge:{:#x}", edge.edge_id),
            "timestamp_us": edge.timestamp_us,
            "record_type": "Edge (LSM)",
            "size_bytes": 128,
            "content": {
                "edge_id": format!("{:#x}", edge.edge_id),
                "parent_id": format!("{:#x}", edge.causal_parent),
                "timestamp_us": edge.timestamp_us,
                "duration_us": edge.duration_us,
                "span_type": edge.span_type,
                "project_id": edge.project_id,
                "session_id": edge.session_id,
                "token_count": edge.token_count
            }
        }));

        // Add payload if exists
        if edge.has_payload != 0 {
            if let Ok(Some(payload_bytes)) = state.tauri_state.db.get_payload(edge.edge_id) {
                let content = serde_json::from_slice::<serde_json::Value>(&payload_bytes)
                    .unwrap_or_else(|_| serde_json::json!({"raw": "binary data"}));
                records.push(serde_json::json!({
                    "key": format!("payload:{:#x}", edge.edge_id),
                    "timestamp_us": edge.timestamp_us,
                    "record_type": "Payload (Blob)",
                    "size_bytes": payload_bytes.len(),
                    "content": content
                }));
            }
        }
    }

    Json(serde_json::json!({
        "records": records,
        "total_records": records.len()
    })).into_response()
}

/// Stats endpoint
async fn get_stats(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    let stats = state.tauri_state.connection_stats.read();
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - state.start_time;

    Json(serde_json::json!({
        "total_traces_received": stats.total_traces_received,
        "last_trace_time": stats.last_trace_time,
        "server_uptime_secs": uptime,
        "ingestion_rate_per_min": stats.ingestion_rate_per_min
    }))
}

/// Legacy trace format (v0.2.0 plugin compatibility)
#[derive(Debug, Deserialize)]
struct LegacyTraceRequest {
    tenant_id: Option<serde_json::Value>,
    project_id: Option<serde_json::Value>,
    agent_id: Option<serde_json::Value>,
    session_id: Option<serde_json::Value>,
    span_type: Option<serde_json::Value>,
    parent_edge_id: Option<String>,
    token_count: Option<u32>,
    duration_ms: Option<u64>,
    metadata: Option<serde_json::Value>,
    // Marker field that new format won't have - helps serde(untagged) differentiate
    tool_name: Option<String>,
}

/// Flexible ingest request that accepts both new and legacy formats
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FlexibleIngestRequest {
    /// New format: { "spans": [...] }
    NewFormat(IngestRequest),
    /// Legacy format: { "tenant_id": ..., "project_id": ..., "span_type": ... }
    LegacyFormat(LegacyTraceRequest),
}

/// Helper to extract a string value from a JSON value
fn json_value_to_string(val: &Option<serde_json::Value>, default: &str) -> String {
    match val {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(v) => v.to_string(),
        None => default.to_string(),
    }
}

/// Convert legacy trace request to a span for the standard pipeline
fn convert_legacy_to_span(legacy: &LegacyTraceRequest) -> AgentreplaySpan {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    
    // Generate pseudo-random IDs using timestamp + process info
    let pid = std::process::id() as u64;
    let span_id = format!("0x{:016x}", now_us.wrapping_mul(31).wrapping_add(pid));
    let trace_id = format!("0x{:032x}", (now_us as u128).wrapping_mul(37).wrapping_add(pid as u128));
    
    let span_type_str = json_value_to_string(&legacy.span_type, "0");
    let span_type_val: u32 = span_type_str.parse().unwrap_or(0);
    let span_name = match span_type_val {
        0 => "root".to_string(),
        1 => "planning".to_string(),
        2 => "reasoning".to_string(),
        3 => "tool_call".to_string(),
        4 => "tool_response".to_string(),
        5 => "synthesis".to_string(),
        _ => format!("span_{}", span_type_val),
    };
    
    let duration_us = legacy.duration_ms.unwrap_or(1) * 1000;
    
    let mut attributes = HashMap::new();
    attributes.insert("tenant_id".to_string(), json_value_to_string(&legacy.tenant_id, "1"));
    // Default legacy traces to Claude Code project (49455) instead of 0
    // This prevents future traces from being "orphaned" with project_id=0
    let raw_project_id = json_value_to_string(&legacy.project_id, "0");
    let project_id_str = if raw_project_id == "0" || raw_project_id == "1" {
        CLAUDE_CODE_PROJECT_ID.to_string()
    } else {
        raw_project_id
    };
    attributes.insert("project_id".to_string(), project_id_str);
    attributes.insert("agent_id".to_string(), json_value_to_string(&legacy.agent_id, "0"));
    attributes.insert("session_id".to_string(), json_value_to_string(&legacy.session_id, "0"));
    
    if let Some(tool) = &legacy.tool_name {
        attributes.insert("tool_name".to_string(), tool.clone());
    }
    
    // Flatten metadata into attributes
    if let Some(meta) = &legacy.metadata {
        if let Some(obj) = meta.as_object() {
            for (k, v) in obj {
                let val = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                attributes.insert(k.clone(), val);
            }
        }
    }
    
    AgentreplaySpan {
        span_id,
        trace_id,
        parent_span_id: legacy.parent_edge_id.clone(),
        name: span_name,
        start_time: now_us,
        end_time: Some(now_us + duration_us),
        attributes,
    }
}

/// POST /api/v1/traces - Ingest a batch of spans (supports both new and legacy formats)
async fn ingest_traces(
    AxumState(state): AxumState<ServerState>,
    Json(request): Json<FlexibleIngestRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Handle both formats
    let request = match request {
        FlexibleIngestRequest::NewFormat(req) => req,
        FlexibleIngestRequest::LegacyFormat(legacy) => {
            debug!("Converting legacy trace format to spans");
            let span = convert_legacy_to_span(&legacy);
            IngestRequest { spans: vec![span] }
        }
    };
    
    debug!("Ingesting {} spans", request.spans.len());

    if request.spans.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Empty span batch".to_string()));
    }

    if request.spans.len() > 10_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Batch size exceeds limit of 10,000 spans".to_string(),
        ));
    }

    // Rate limiting check - consume tokens for the number of spans
    let span_count = request.spans.len() as u32;
    if let Err(_) = state.rate_limiter.check_n(NonZeroU32::new(span_count.max(1)).unwrap()) {
        warn!("Rate limit exceeded: {} spans rejected", span_count);
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!(
                "Rate limit exceeded. Current limit: {} spans/minute with {} burst. Try again later.",
                RATE_LIMIT_SPANS_PER_MINUTE, RATE_LIMIT_BURST_SIZE
            ),
        ));
    }

    let mut edges = Vec::new();
    let mut errors = Vec::new();
    let mut edge_attributes: Vec<(AgentFlowEdge, HashMap<String, String>)> = Vec::new();

    for (idx, span) in request.spans.iter().enumerate() {
        match convert_span_to_edge(span) {
            Ok(edge) => {
                edge_attributes.push((edge, span.attributes.clone()));
                edges.push(edge);
            }
            Err(e) => {
                warn!("Failed to convert span {}: {}", idx, e);
                errors.push(format!("Span {}: {}", idx, e));
            }
        }
    }

    let accepted = edges.len();
    let mut rejected = errors.len();

    // Queue edges for async batched writes (Issue #7: Non-blocking ingestion)
    if !edges.is_empty() {
        // **FIXED: Write payloads BEFORE queueing edges to prevent race conditions**
        // The background ingestion worker processes edges async, so if we queue edges
        // first and then write payloads, there's a race where the edge can be read
        // before its payload is written, resulting in null attributes.
        //
        // Write payloads FIRST, then queue edges for async processing.
        let mut edges_to_queue: Vec<AgentFlowEdge> = Vec::new();
        
        for (edge, attributes) in &edge_attributes {
            if !attributes.is_empty() {
                match serde_json::to_vec(&attributes) {
                    Ok(json_bytes) => {
                        if let Err(e) = state.tauri_state.db.put_payload(edge.edge_id, &json_bytes)
                        {
                            warn!(
                                "Failed to store attributes for edge {:#x}: {}",
                                edge.edge_id, e
                            );
                            // Payload write failed - don't insert edge to maintain consistency
                            errors.push(format!(
                                "Payload storage failed for edge {:#x}",
                                edge.edge_id
                            ));
                            rejected += 1;
                            continue;
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to serialize attributes for edge {:#x}: {}",
                            edge.edge_id, e
                        );
                        errors.push(format!("Serialization failed for edge {:#x}", edge.edge_id));
                        rejected += 1;
                        continue;
                    }
                }
            }
            // Only queue edge if payload write succeeded (or no payload needed)
            edges_to_queue.push(*edge);
        }

        // **DURABILITY FIX**: Sync payload index to disk after batch write
        // The PayloadStore uses an in-memory HashMap index that's only persisted on
        // shutdown. If the app crashes, the index is lost and payloads become orphaned.
        // By syncing after each batch, we ensure durability at a small performance cost.
        if let Err(e) = state.tauri_state.db.sync() {
            warn!("Failed to sync database after payload writes: {}", e);
            // Non-fatal: payloads are in the data file, index can be rebuilt
        }

        // Now queue edges AFTER payloads are written
        for edge in &edges_to_queue {
            if let Err(e) = state.tauri_state.ingestion_queue.send(*edge) {
                error!("Failed to queue edge: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Ingestion queue error: {}", e),
                ));
            }
        }

        // NOTE: Stats and events will be updated by background worker AFTER successful write
        // This prevents inflated stats if database write fails or app crashes before flush
    }

    debug!(
        "Ingestion complete: {} accepted, {} rejected",
        accepted, rejected
    );

    Ok((
        StatusCode::CREATED,
        Json(IngestResponse {
            accepted,
            rejected,
            errors,
        }),
    ))
}

/// Convert AgentreplaySpan to AgentFlowEdge (simplified version from agentreplay-server)
fn convert_span_to_edge(span: &AgentreplaySpan) -> Result<AgentFlowEdge, String> {
    use agentreplay_core::Environment;

    // Parse span_id
    let edge_id = parse_id_to_u64(&span.span_id)
        .ok_or_else(|| format!("Invalid span_id format: {}", span.span_id))?
        as u128;

    // Parse parent_span_id
    let causal_parent = span
        .parent_span_id
        .as_ref()
        .and_then(|id| parse_id_to_u64(id))
        .unwrap_or(0) as u128;

    // Extract session_id
    let session_id = span
        .attributes
        .get("session_id")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| hash_string_to_u64(&span.trace_id));

    // Validate timestamps
    let config = agentreplay_core::TimestampConfig::production();
    agentreplay_core::validate_timestamp(span.start_time, &config)
        .map_err(|e| format!("Invalid start_time: {}", e))?;

    if let Some(end_time) = span.end_time {
        agentreplay_core::validate_timestamp(end_time, &config)
            .map_err(|e| format!("Invalid end_time: {}", e))?;

        if end_time < span.start_time {
            return Err("end_time cannot be before start_time".to_string());
        }
    }

    // Calculate duration
    let duration_us = span
        .end_time
        .map(|end| (end - span.start_time) as u32)
        .unwrap_or(0);

    // Parse span type
    let span_type = parse_span_name_to_type(&span.name);

    // Extract token count
    let input_tokens = span
        .attributes
        .get("gen_ai.usage.input_tokens")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let output_tokens = span
        .attributes
        .get("gen_ai.usage.output_tokens")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let token_count = if input_tokens > 0 || output_tokens > 0 {
        input_tokens + output_tokens
    } else {
        span.attributes
            .get("gen_ai.usage.total_tokens")
            .or_else(|| span.attributes.get("tokens"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    };

    // Extract environment
    let environment = span
        .attributes
        .get("environment")
        .map(|s| Environment::parse(s) as u8)
        .unwrap_or(Environment::Development as u8);

    // Extract tenant_id
    let tenant_id = span
        .attributes
        .get("tenant_id")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1); // Default tenant

    // Extract project_id
    let project_id = span
        .attributes
        .get("project_id")
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    // Extract agent_id
    let agent_id = span
        .attributes
        .get("agent_id")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| hash_string_to_u64(&span.name));

    // Create edge
    let mut edge = AgentFlowEdge::new(
        tenant_id,
        project_id,
        agent_id,
        session_id,
        span_type,
        causal_parent,
    );

    // Override fields
    edge.timestamp_us = span.start_time;
    edge.duration_us = duration_us;
    edge.token_count = token_count;
    edge.environment = environment;
    edge.edge_id = edge_id;

    // Recompute checksum
    edge.checksum = edge.compute_checksum();

    // Validate
    edge.validate()
        .map_err(|e| format!("Edge validation failed: {}", e))?;

    Ok(edge)
}

/// Parse string ID to u64
fn parse_id_to_u64(id: &str) -> Option<u64> {
    // Try hex with 0x prefix
    if let Some(hex) = id.strip_prefix("0x") {
        return u64::from_str_radix(hex, 16).ok();
    }

    // Try decimal
    if let Ok(n) = id.parse::<u64>() {
        return Some(n);
    }

    // Fallback: hash the string
    Some(hash_string_to_u64(id))
}

/// Hash string to u64
fn hash_string_to_u64(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Parse span name to SpanType
fn parse_span_name_to_type(name: &str) -> agentreplay_core::SpanType {
    use agentreplay_core::SpanType;

    let lower = name.to_lowercase();

    if lower.contains("llm") || lower.contains("chat") || lower.contains("planning") {
        return SpanType::Planning;
    }

    if lower.contains("tool") || lower.contains("function_call") {
        return SpanType::ToolCall;
    }

    if lower.contains("root") || lower.contains("trace") {
        return SpanType::Root;
    }

    if lower.contains("reasoning") {
        return SpanType::Reasoning;
    }

    if lower.contains("synthesis") {
        return SpanType::Synthesis;
    }

    if lower.contains("response") {
        return SpanType::Response;
    }

    if lower.contains("error") {
        return SpanType::Error;
    }

    SpanType::Custom
}

/// Request to create a new project
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
    pub id: Option<u16>,
}

/// Response after creating a project
#[derive(Debug, Serialize)]
pub struct CreateProjectResponse {
    pub project_id: u16,
    pub name: String,
    pub description: Option<String>,
    pub env_vars: EnvVariables,
}

/// Environment variables for SDK setup
#[derive(Debug, Serialize)]
pub struct EnvVariables {
    pub agentreplay_url: String,
    pub tenant_id: String,
    pub project_id: String,
}

/// POST /api/v1/projects - Create a new project
async fn create_project(
    AxumState(state): AxumState<ServerState>,
    Json(payload): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    // Check if ID is provided in payload
    // SPECIAL CASE: If name is "Claude Code", always use reserved ID 49455
    let is_claude_code = payload.name.eq_ignore_ascii_case("claude code");
    
    let project_id_u16 = if is_claude_code {
        49455
    } else {
        payload.id.unwrap_or_else(|| {
            (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                % 65535) as u16
        })
    };
    
    let project_id_str = project_id_u16.to_string();

    // CLEANUP: If this is "Claude Code", remove any existing projects with the same name but different ID
    if is_claude_code {
        // Use a block to scope the lock
        {
            let mut store = state.tauri_state.project_store.write();
            let existing_projects = store.list();
            for p in existing_projects {
                if p.name.eq_ignore_ascii_case("claude code") && p.id != "49455" {
                    tracing::warn!("Removing duplicate Claude Code project with incorrect ID: {}", p.id);
                    let _ = store.remove(&p.id);
                }
            }
        }
    }

    // Get server config for URL
    let (host, port) = {
        let config = state.tauri_state.config.read();
        (
            config.ingestion_server.host.clone(),
            config.ingestion_server.port,
        )
    };

    let env_vars = EnvVariables {
        agentreplay_url: format!("http://{}:{}", host, port),
        tenant_id: "default".to_string(),
        project_id: project_id_str.clone(),
    };

    // Create project object
    let project = crate::project_store::Project {
        id: project_id_str.clone(),
        name: payload.name.clone(),
        description: payload.description.clone(),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Save to project store
    {
        let mut store = state.tauri_state.project_store.write();
        if let Err(e) = store.add(project) {
            tracing::error!("Failed to save project: {}", e);
        }
    }

    Json(CreateProjectResponse {
        project_id: project_id_u16,
        name: payload.name,
        description: payload.description,
        env_vars,
    })
}

/// GET /api/v1/projects - List all projects
async fn list_projects_http(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    let store = state.tauri_state.project_store.read();
    let projects = store.list();

    let project_infos: Vec<_> = projects
        .into_iter()
        .map(|p| {
            // Count root traces for this project (causal_parent == 0)
            // Project.id is a String, but query_filtered expects u16
            let trace_count = p.id.parse::<u16>()
                .ok()
                .and_then(|pid| state.tauri_state.db
                    .query_filtered(0, u64::MAX, None, Some(pid))
                    .ok())
                .map(|edges| edges.iter().filter(|e| e.causal_parent == 0).count())
                .unwrap_or(0);
            
            serde_json::json!({
                "project_id": p.id,
                "name": p.name,
                "description": p.description,
                "created_at": p.created_at,
                "trace_count": trace_count,
                "favorite": false
            })
        })
        .collect();

    Json(serde_json::json!({
        "projects": project_infos,
        "total": project_infos.len()
    }))
}

/// DELETE /api/v1/projects/:project_id - Delete a project and all its traces
async fn delete_project_handler(
    AxumState(state): AxumState<ServerState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    info!("Deleting project and traces: {}", project_id);
    
    // Parse project_id as u16 for edge deletion
    let project_id_u16: u16 = match project_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Invalid project ID format"
                })),
            ).into_response();
        }
    };
    
    // First, delete all edges for this project
    let edges_deleted = match state.tauri_state.db.delete_by_project(project_id_u16).await {
        Ok(count) => {
            info!("Deleted {} traces for project {}", count, project_id);
            count
        }
        Err(e) => {
            error!("Failed to delete traces for project {}: {}", project_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to delete project traces: {}", e)
                })),
            ).into_response();
        }
    };
    
    // Then delete the project metadata
    let mut store = state.tauri_state.project_store.write();
    match store.remove(&project_id) {
        Ok(Some(project)) => {
            info!("Successfully deleted project: {} ({}) with {} traces", project.name, project_id, edges_deleted);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "message": format!("Project '{}' and {} traces deleted successfully", project.name, edges_deleted),
                    "traces_deleted": edges_deleted
                })),
            ).into_response()
        }
        Ok(None) => {
            // Project metadata not found, but we still deleted the edges
            warn!("Project metadata not found: {}, but deleted {} traces", project_id, edges_deleted);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "message": format!("Deleted {} traces (project metadata was already removed)", edges_deleted),
                    "traces_deleted": edges_deleted
                })),
            ).into_response()
        }
        Err(e) => {
            error!("Failed to delete project metadata {}: {}", project_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to delete project metadata: {}", e),
                    "traces_deleted": edges_deleted
                })),
            ).into_response()
        }
    }
}

/// DELETE /api/v1/admin/reset - Reset all data (projects and traces)
async fn reset_all_data_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    info!("Resetting all data...");
    
    // Clear all projects
    let mut store = state.tauri_state.project_store.write();
    if let Err(e) = store.clear() {
        error!("Failed to clear projects: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to clear projects: {}", e)
            })),
        );
    }
    
    // Note: Traces are stored in the Agentreplay database
    // Full trace deletion would require database reset, which needs app restart
    // For now, just clear projects - traces will be orphaned but won't show in UI
    
    info!("All data reset successfully");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "All projects cleared. Restart the app to fully reset trace data."
        })),
    )
}

// ============================================================================
// Backup & Restore Handlers
// ============================================================================

#[derive(Debug, Serialize)]
struct BackupInfo {
    backup_id: String,
    created_at: u64,
    size_bytes: u64,
    path: String,
}

/// POST /api/v1/admin/backup - Create a database backup
async fn create_backup_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    info!("Creating backup...");
    
    // Get backup directory
    let backup_dir = std::path::PathBuf::from(&state.tauri_state.db_path)
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("./backups"));
    
    if let Err(e) = std::fs::create_dir_all(&backup_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to create backup directory: {}", e)
            })),
        ).into_response();
    }
    
    let backup_id = format!("backup_{}", chrono::Utc::now().timestamp());
    let backup_path = backup_dir.join(&backup_id);
    
    // Copy database directory to backup location
    if let Err(e) = copy_dir_recursive(&state.tauri_state.db_path.as_path(), &backup_path) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to copy database: {}", e)
            })),
        ).into_response();
    }
    
    // Calculate backup size
    let size_bytes = get_dir_size(&backup_path).unwrap_or(0);
    
    info!("Backup created: {} ({} bytes)", backup_id, size_bytes);
    
    Json(serde_json::json!({
        "success": true,
        "backup_id": backup_id,
        "created_at": chrono::Utc::now().timestamp() as u64,
        "size_bytes": size_bytes,
        "path": backup_path.display().to_string()
    })).into_response()
}

/// GET /api/v1/admin/backups - List available backups
async fn list_backups_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    let backup_dir = std::path::PathBuf::from(&state.tauri_state.db_path)
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("./backups"));
    
    if !backup_dir.exists() {
        return Json(serde_json::json!({
            "backups": [],
            "total": 0
        })).into_response();
    }
    
    let mut backups = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&backup_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let backup_id = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                let metadata = entry.metadata().ok();
                let created_at = metadata
                    .as_ref()
                    .and_then(|m| m.created().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                
                let size_bytes = get_dir_size(&path).unwrap_or(0);
                
                backups.push(BackupInfo {
                    backup_id,
                    created_at,
                    size_bytes,
                    path: path.display().to_string(),
                });
            }
        }
    }
    
    // Sort by created_at descending
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    
    let total = backups.len();
    Json(serde_json::json!({
        "backups": backups,
        "total": total
    })).into_response()
}

/// POST /api/v1/admin/backups/:backup_id/restore - Restore from a backup
async fn restore_backup_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(backup_id): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let mode = query.get("mode").map(|s| s.as_str()).unwrap_or("replace");
    info!("Restoring from backup: {} (mode: {})", backup_id, mode);
    
    let db_path = std::path::PathBuf::from(&state.tauri_state.db_path);
    let backup_dir = db_path
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("./backups"));
    
    let backup_path = backup_dir.join(&backup_id);
    
    if !backup_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Backup not found: {}", backup_id)
            })),
        ).into_response();
    }
    
    if mode == "merge" {
        // Merge mode: Copy backup files but don't delete existing data
        // Just copy files from backup to db_path, existing files will remain
        if let Err(e) = merge_dir_recursive(&backup_path, &db_path) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to merge backup: {}", e)
                })),
            ).into_response();
        }
        
        info!("Backup merged successfully: {}", backup_id);
        
        return Json(serde_json::json!({
            "success": true,
            "message": "Backup merged successfully! Data has been appended. Please restart the app to see all changes.",
            "backup_id": backup_id,
            "mode": "merge",
            "requires_restart": true
        })).into_response();
    }
    
    // Replace mode (default): Full restore
    // Create a pre-restore backup of current state
    let pre_restore_backup = backup_dir.join(format!("pre_restore_{}", chrono::Utc::now().timestamp()));
    if db_path.exists() {
        if let Err(e) = copy_dir_recursive(&db_path, &pre_restore_backup) {
            warn!("Failed to create pre-restore backup: {}", e);
            // Continue anyway
        } else {
            info!("Created pre-restore backup at: {:?}", pre_restore_backup);
        }
    }
    
    // Clear the current database directory
    if db_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&db_path) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to clear current database: {}", e)
                })),
            ).into_response();
        }
    }
    
    // Copy backup to database location
    if let Err(e) = copy_dir_recursive(&backup_path, &db_path) {
        // Try to restore from pre-restore backup
        if pre_restore_backup.exists() {
            let _ = copy_dir_recursive(&pre_restore_backup, &db_path);
        }
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to restore backup: {}", e)
            })),
        ).into_response();
    }
    
    info!("Backup restored successfully: {}", backup_id);
    
    Json(serde_json::json!({
        "success": true,
        "message": "Backup restored successfully! Please restart the app to apply changes.",
        "backup_id": backup_id,
        "mode": "replace",
        "requires_restart": true
    })).into_response()
}

/// DELETE /api/v1/admin/backups/:backup_id - Delete a backup
async fn delete_backup_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(backup_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    info!("Deleting backup: {}", backup_id);
    
    let backup_dir = std::path::PathBuf::from(&state.tauri_state.db_path)
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("./backups"));
    
    let backup_path = backup_dir.join(&backup_id);
    
    if !backup_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Backup not found: {}", backup_id)
            })),
        ).into_response();
    }
    
    // Delete the backup directory
    if let Err(e) = std::fs::remove_dir_all(&backup_path) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Failed to delete backup: {}", e)
            })),
        ).into_response();
    }
    
    info!("Backup deleted: {}", backup_id);
    
    Json(serde_json::json!({
        "success": true,
        "message": format!("Backup {} deleted successfully", backup_id)
    })).into_response()
}

/// GET /api/v1/admin/backups/:backup_id/export - Export backup as downloadable ZIP
async fn export_backup_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(backup_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    info!("Exporting backup: {}", backup_id);
    
    let backup_dir = std::path::PathBuf::from(&state.tauri_state.db_path)
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("./backups"));
    
    let backup_path = backup_dir.join(&backup_id);
    
    if !backup_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Backup not found: {}", backup_id)
            })),
        ).into_response();
    }
    
    // Create a ZIP file in memory
    let zip_buffer = {
        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        
        // Recursively add all files from the backup directory
        if let Err(e) = add_dir_to_zip(&mut zip, &backup_path, &backup_id, options) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to create ZIP: {}", e)
                })),
            ).into_response();
        }
        
        match zip.finish() {
            Ok(_) => buf,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to finalize ZIP: {}", e)
                    })),
                ).into_response();
            }
        }
    };
    
    info!("Backup exported: {} ({} bytes)", backup_id, zip_buffer.len());
    
    // Generate filename with agentreplay_backup_ prefix
    let filename = format!("agentreplay_backup_{}.zip", backup_id.trim_start_matches("backup_"));
    
    // Return the ZIP file as a download
    let headers = [
        (axum::http::header::CONTENT_TYPE, "application/zip"),
        (axum::http::header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", filename)),
    ];
    
    (headers, zip_buffer).into_response()
}

/// POST /api/v1/admin/backups/import - Import backup from uploaded ZIP file
async fn import_backup_handler(
    AxumState(state): AxumState<ServerState>,
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    info!("Importing backup...");
    
    let backup_dir = std::path::PathBuf::from(&state.tauri_state.db_path)
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("./backups"));
    
    // Create backup directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&backup_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to create backup directory: {}", e)
            })),
        ).into_response();
    }
    
    // Read the uploaded file
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("backup") {
            file_name = field.file_name().map(|s| s.to_string());
            if let Ok(data) = field.bytes().await {
                file_data = Some(data.to_vec());
            }
        }
    }
    
    let data = match file_data {
        Some(d) => d,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No file uploaded"
                })),
            ).into_response();
        }
    };
    
    // Generate backup ID from filename or timestamp
    let backup_id = file_name
        .as_ref()
        .and_then(|n| n.strip_suffix(".zip"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("imported_{}", chrono::Utc::now().timestamp()));
    
    let target_path = backup_dir.join(&backup_id);
    
    // Check if backup already exists
    if target_path.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": format!("Backup {} already exists", backup_id)
            })),
        ).into_response();
    }
    
    // Extract ZIP to backup directory
    let cursor = std::io::Cursor::new(data);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid ZIP file: {}", e)
                })),
            ).into_response();
        }
    };
    
    // Create target directory
    if let Err(e) = std::fs::create_dir_all(&target_path) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to create backup directory: {}", e)
            })),
        ).into_response();
    }
    
    // Extract all files
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&target_path);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to read ZIP entry: {}", e)
                    })),
                ).into_response();
            }
        };
        
        let outpath = target_path.join(file.name().trim_start_matches(&format!("{}/", backup_id)));
        
        if file.is_dir() {
            let _ = std::fs::create_dir_all(&outpath);
        } else {
            if let Some(parent) = outpath.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut outfile) = std::fs::File::create(&outpath) {
                let _ = std::io::copy(&mut file, &mut outfile);
            }
        }
    }
    
    let size_bytes = get_dir_size(&target_path).unwrap_or(0);
    
    info!("Backup imported: {} ({} bytes)", backup_id, size_bytes);
    
    Json(serde_json::json!({
        "success": true,
        "backup_id": backup_id,
        "size_bytes": size_bytes,
        "message": "Backup imported successfully. Restart the app to apply."
    })).into_response()
}

/// Helper function to recursively add a directory to a ZIP archive
fn add_dir_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    path: &std::path::Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
        
        if entry_path.is_dir() {
            zip.add_directory(&name, options)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            add_dir_to_zip(zip, &entry_path, &name, options)?;
        } else {
            zip.start_file(&name, options)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let mut file = std::fs::File::open(&entry_path)?;
            std::io::copy(&mut file, zip)?;
        }
    }
    Ok(())
}

/// Helper function to copy a directory recursively
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if !src.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Source directory not found"
        ));
    }
    
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        
        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    
    Ok(())
}

/// Helper function to merge a directory recursively (copies files, preserving existing ones)
fn merge_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if !src.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Source directory not found"
        ));
    }
    
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        
        if path.is_dir() {
            // Recursively merge subdirectories
            merge_dir_recursive(&path, &dest_path)?;
        } else {
            let file_name = entry.file_name().to_string_lossy().to_string();
            
            // Binary files that should be appended (WAL logs are binary)
            let is_binary_appendable = file_name == "wal.log" || file_name.ends_with(".wal");
            
            // Text files that should be appended (line-based data files)
            let is_text_appendable = file_name.ends_with(".jsonl");
            
            // Files that should be merged as JSON (contains objects/arrays)
            let is_json_mergeable = file_name == "projects.json";
            
            if is_binary_appendable {
                // Append binary files (WAL logs)
                if dest_path.exists() {
                    let src_content = std::fs::read(&path)?;
                    let mut dest_file = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&dest_path)?;
                    use std::io::Write;
                    if !src_content.is_empty() {
                        dest_file.write_all(&src_content)?;
                    }
                } else {
                    std::fs::copy(&path, &dest_path)?;
                }
            } else if is_text_appendable {
                // Append text-based files (JSONL)
                if dest_path.exists() {
                    if let Ok(src_content) = std::fs::read_to_string(&path) {
                        let mut dest_file = std::fs::OpenOptions::new()
                            .append(true)
                            .open(&dest_path)?;
                        use std::io::Write;
                        if !src_content.is_empty() {
                            // Make sure we start on a new line
                            dest_file.write_all(b"\n")?;
                            dest_file.write_all(src_content.trim().as_bytes())?;
                        }
                    }
                } else {
                    std::fs::copy(&path, &dest_path)?;
                }
            } else if is_json_mergeable {
                // Merge JSON files (like projects.json)
                if dest_path.exists() {
                    if let Ok(src_content) = std::fs::read_to_string(&path) {
                        if let Ok(dest_content) = std::fs::read_to_string(&dest_path) {
                            // Try to merge as JSON arrays or objects
                            if let (Ok(src_json), Ok(dest_json)) = (
                                serde_json::from_str::<serde_json::Value>(&src_content),
                                serde_json::from_str::<serde_json::Value>(&dest_content),
                            ) {
                                let merged = merge_json_values(dest_json, src_json);
                                if let Ok(merged_str) = serde_json::to_string_pretty(&merged) {
                                    let _ = std::fs::write(&dest_path, merged_str);
                                }
                            }
                        }
                    }
                } else {
                    std::fs::copy(&path, &dest_path)?;
                }
            } else if !dest_path.exists() {
                // For other files, only copy if they don't exist
                std::fs::copy(&path, &dest_path)?;
            }
        }
    }
    
    Ok(())
}

/// Helper function to calculate directory size
fn get_dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut size = 0;
    
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                size += get_dir_size(&path)?;
            } else {
                size += entry.metadata()?.len();
            }
        }
    } else {
        size = std::fs::metadata(path)?.len();
    }
    
    Ok(size)
}

/// Helper function to merge two JSON values
fn merge_json_values(dest: serde_json::Value, src: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    
    match (dest, src) {
        // Merge arrays by concatenating and deduplicating by "id" or "name" field
        (Value::Array(mut dest_arr), Value::Array(src_arr)) => {
            for src_item in src_arr {
                // Check if item already exists (by id or name)
                let exists = dest_arr.iter().any(|dest_item| {
                    if let (Some(dest_id), Some(src_id)) = (
                        dest_item.get("id").or_else(|| dest_item.get("name")),
                        src_item.get("id").or_else(|| src_item.get("name")),
                    ) {
                        dest_id == src_id
                    } else {
                        false
                    }
                });
                
                if !exists {
                    dest_arr.push(src_item);
                }
            }
            Value::Array(dest_arr)
        }
        // Merge objects by combining keys
        (Value::Object(mut dest_obj), Value::Object(src_obj)) => {
            for (key, src_val) in src_obj {
                if let Some(dest_val) = dest_obj.remove(&key) {
                    dest_obj.insert(key, merge_json_values(dest_val, src_val));
                } else {
                    dest_obj.insert(key, src_val);
                }
            }
            Value::Object(dest_obj)
        }
        // For other types, keep destination value
        (dest, _) => dest,
    }
}

// ============================================================================
// AI-Powered Trace Analysis Handler
// ============================================================================

#[derive(Debug, Deserialize)]
struct AnalyzeTraceRequest {
    /// What kind of analysis to perform
    #[serde(default = "default_analysis_type")]
    analysis_type: String,
}

fn default_analysis_type() -> String {
    "flow_diagram".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalyzeTraceResponse {
    /// Mermaid diagram code
    mermaid_code: String,
    /// Human-readable summary
    summary: String,
    /// Key insights about the trace
    insights: Vec<String>,
    /// Suggestions for improvements
    suggestions: Vec<String>,
}

/// POST /api/v1/traces/:trace_id/analyze - AI-powered trace analysis
async fn analyze_trace_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(trace_id): axum::extract::Path<String>,
    Json(_req): Json<AnalyzeTraceRequest>,
) -> impl IntoResponse {
    // Parse trace ID
    let edge_id = match u128::from_str_radix(&trace_id, 10) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid trace ID"})),
            ).into_response();
        }
    };

    // Fetch the trace and its observations
    let trace = match state.tauri_state.db.get(edge_id) {
        Ok(Some(edge)) => edge,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Trace not found"})),
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to fetch trace: {}", e)})),
            ).into_response();
        }
    };

    // Fetch observations (child spans)
    let observations = state.tauri_state.db
        .get_children(edge_id)
        .unwrap_or_default();

    // Get payloads for all observations to understand the flow
    let mut span_details: Vec<serde_json::Value> = Vec::new();
    
    // Add root trace
    let root_payload = state.tauri_state.db.get_payload(edge_id).ok().flatten();
    let root_attrs: serde_json::Value = root_payload
        .and_then(|p| serde_json::from_slice(&p).ok())
        .unwrap_or(serde_json::json!({}));
    
    span_details.push(serde_json::json!({
        "id": format!("{}", edge_id),
        "name": root_attrs.get("name").and_then(|v| v.as_str()).unwrap_or("Root"),
        "type": format!("{:?}", agentreplay_core::SpanType::from_u64(trace.span_type as u64)),
        "duration_ms": trace.duration_us / 1000,
        "tokens": trace.token_count,
        "is_root": true,
        "input": root_attrs.get("input").or(root_attrs.get("prompt")).cloned(),
        "output": root_attrs.get("output").or(root_attrs.get("completion")).cloned(),
    }));

    // Add child observations
    for obs in &observations {
        let payload = state.tauri_state.db.get_payload(obs.edge_id).ok().flatten();
        let attrs: serde_json::Value = payload
            .and_then(|p| serde_json::from_slice(&p).ok())
            .unwrap_or(serde_json::json!({}));
        
        span_details.push(serde_json::json!({
            "id": format!("{}", obs.edge_id),
            "parent_id": format!("{}", obs.causal_parent),
            "name": attrs.get("name").and_then(|v| v.as_str())
                .or(attrs.get("gen_ai.request.model").and_then(|v| v.as_str()))
                .unwrap_or("Span"),
            "type": format!("{:?}", agentreplay_core::SpanType::from_u64(obs.span_type as u64)),
            "duration_ms": obs.duration_us / 1000,
            "tokens": obs.token_count,
            "input": attrs.get("input").or(attrs.get("prompt")).or(attrs.get("gen_ai.prompt")).cloned(),
            "output": attrs.get("output").or(attrs.get("completion")).or(attrs.get("gen_ai.completion")).cloned(),
        }));
    }

    // Build the analysis prompt
    let analysis_prompt = format!(
        r#"Analyze this LLM trace and provide:
1. A Mermaid flowchart diagram showing the execution flow
2. A brief summary of what this trace does
3. Key insights about performance/behavior
4. Suggestions for improvement

Trace data:
{}

Respond in this exact JSON format:
{{
  "mermaid_code": "flowchart TD\\n    A[Start] --> B[Step]\\n    ...",
  "summary": "Brief description of the trace",
  "insights": ["insight 1", "insight 2"],
  "suggestions": ["suggestion 1", "suggestion 2"]
}}

Make the Mermaid diagram show:
- The actual span names and types
- Parent-child relationships
- Key metrics like duration and tokens
- Use colors: green for fast (<1s), yellow for medium (1-5s), red for slow (>5s)"#,
        serde_json::to_string_pretty(&span_details).unwrap_or_default()
    );

    // Try to use LLM for analysis
    let llm_client = state.tauri_state.llm_client.read().await;
    
    use crate::llm::{ChatMessage, LLMPurpose};
    
    // Use complete_for_purpose which properly routes to configured providers
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are an expert at analyzing LLM application traces and creating clear visualizations. Always respond with valid JSON.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: analysis_prompt,
        },
    ];
    
    match llm_client.complete_for_purpose(
        LLMPurpose::Analysis,
        messages,
        Some(0.3),
        Some(2000),
    ).await {
        Ok(response) => {
            // Try to parse the JSON response
            let content = response.content.trim();
            
            // Try to extract JSON from the response
            let json_str = if content.starts_with('{') {
                content.to_string()
            } else if let Some(start) = content.find('{') {
                if let Some(end) = content.rfind('}') {
                    content[start..=end].to_string()
                } else {
                    content.to_string()
                }
            } else {
                content.to_string()
            };
            
            match serde_json::from_str::<AnalyzeTraceResponse>(&json_str) {
                Ok(analysis) => Json(analysis).into_response(),
                Err(_) => {
                    // Fallback: generate a basic diagram without AI
                    let fallback = generate_fallback_analysis(&span_details);
                    Json(fallback).into_response()
                }
            }
        }
        Err(e) => {
            tracing::warn!("LLM analysis failed, using fallback: {}", e);
            // Generate a basic diagram without AI
            let fallback = generate_fallback_analysis(&span_details);
            Json(fallback).into_response()
        }
    }
}

/// Generate a basic Mermaid diagram without AI
fn generate_fallback_analysis(spans: &[serde_json::Value]) -> AnalyzeTraceResponse {
    let mut mermaid = String::from("flowchart TD\n");
    let mut insights = Vec::new();
    let mut total_duration = 0u64;
    let mut total_tokens = 0u32;
    
    for (i, span) in spans.iter().enumerate() {
        let default_id = format!("span_{}", i);
        let id = span.get("id").and_then(|v| v.as_str()).unwrap_or(&default_id);
        let short_id = if id.len() > 8 { &id[..8] } else { id };
        let name = span.get("name").and_then(|v| v.as_str()).unwrap_or("Span");
        let _span_type = span.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
        let duration_ms = span.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        let tokens = span.get("tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        
        total_duration += duration_ms;
        total_tokens += tokens;
        
        // Add node with styling based on duration
        let style = if duration_ms > 5000 {
            ":::error"
        } else if duration_ms > 1000 {
            ":::warning"
        } else {
            ":::success"
        };
        
        let label = format!("{}<br/>{}ms | {} tokens", name, duration_ms, tokens);
        mermaid.push_str(&format!("    {}[\"{}\"]{}\\n", short_id, label, style));
        
        // Add edge from parent
        if let Some(parent_id) = span.get("parent_id").and_then(|v| v.as_str()) {
            let short_parent = if parent_id.len() > 8 { &parent_id[..8] } else { parent_id };
            mermaid.push_str(&format!("    {} --> {}\\n", short_parent, short_id));
        }
    }
    
    // Add style definitions
    mermaid.push_str("\n    classDef success fill:#10b981,stroke:#059669,color:#fff\n");
    mermaid.push_str("    classDef warning fill:#f59e0b,stroke:#d97706,color:#fff\n");
    mermaid.push_str("    classDef error fill:#ef4444,stroke:#dc2626,color:#fff\n");
    
    // Generate insights
    if spans.len() > 1 {
        insights.push(format!("Trace contains {} spans with {} total tokens", spans.len(), total_tokens));
    }
    if total_duration > 5000 {
        insights.push(format!("Total execution time is {}ms - consider optimizing slow spans", total_duration));
    }
    
    let summary = format!(
        "This trace shows {} operations with a total of {} tokens processed in {}ms.",
        spans.len(), total_tokens, total_duration
    );
    
    AnalyzeTraceResponse {
        mermaid_code: mermaid,
        summary,
        insights,
        suggestions: vec![
            "Consider caching repeated queries".to_string(),
            "Monitor token usage for cost optimization".to_string(),
        ],
    }
}

// ============================================================================
// Sessions Handlers
// ============================================================================

#[derive(Debug, Deserialize)]
struct SessionQueryParams {
    start_ts: Option<u64>,
    end_ts: Option<u64>,
    #[serde(default = "default_session_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    project_id: Option<u16>,
}

fn default_session_limit() -> usize {
    100
}

#[derive(Debug, Serialize)]
struct SessionInfo {
    session_id: u64,
    project_id: u16,
    agent_id: u64,
    started_at: u64,
    last_message_at: u64,
    message_count: usize,
    total_tokens: u32,
    total_duration_ms: u32,
    trace_ids: Vec<String>,
    status: String,
}

/// GET /api/v1/sessions - List all sessions (project-scoped)
async fn list_sessions_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<SessionQueryParams>,
) -> impl IntoResponse {
    // Require project_id for isolation
    let project_id = match params.project_id {
        Some(pid) => pid,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "project_id is required"})),
            ).into_response();
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    let start_ts = params.start_ts.unwrap_or(now - 7 * 86_400_000_000); // 7 days ago
    let end_ts = params.end_ts.unwrap_or(now);

    // Use filtered query with project_id for efficiency
    let edges = match state.tauri_state.db.query_filtered(start_ts, end_ts, None, Some(project_id)) {
        Ok(edges) => {
            tracing::info!("Sessions query: project_id={}, time_range={}..{}, edges_found={}", 
                project_id, start_ts, end_ts, edges.len());
            edges
        },
        Err(e) => {
            tracing::error!("Failed to query traces: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to query traces"})),
            )
                .into_response();
        }
    };

    // Group edges by session_id (already filtered by project_id above)
    let mut session_map: HashMap<u64, Vec<AgentFlowEdge>> = HashMap::new();
    for edge in edges {
        session_map.entry(edge.session_id).or_default().push(edge);
    }

    // Convert to SessionInfo
    let mut sessions: Vec<SessionInfo> = session_map
        .into_iter()
        .map(|(session_id, mut traces)| {
            traces.sort_by_key(|e| e.timestamp_us);

            let started_at = traces.first().map(|e| e.timestamp_us).unwrap_or(0);
            let last_message_at = traces.last().map(|e| e.timestamp_us).unwrap_or(0);
            let message_count = traces.len();
            let total_tokens: u32 = traces.iter().map(|e| e.token_count).sum::<u32>();
            // BUG-01 FIX: Calculate wall-clock duration (last trace end - first trace start)
            // instead of summing all span durations which double-counts nested spans
            let last_end = traces.last().map(|e| e.timestamp_us + (e.duration_us as u64)).unwrap_or(0);
            let total_duration_ms: u32 = ((last_end - started_at) / 1000) as u32;

            let trace_ids: Vec<String> = traces.iter().map(|e| format!("{:#x}", e.edge_id)).collect();

            let one_hour_ago = now - 3_600_000_000;
            let status = if last_message_at > one_hour_ago {
                "active".to_string()
            } else {
                "ended".to_string()
            };

            let project_id = traces.first().map(|e| e.project_id).unwrap_or(0);
            let agent_id = traces.first().map(|e| e.agent_id).unwrap_or(0);

            SessionInfo {
                session_id,
                project_id,
                agent_id,
                started_at,
                last_message_at,
                message_count,
                total_tokens,
                total_duration_ms,
                trace_ids,
                status,
            }
        })
        .collect();

    // Sort by last_message_at descending (most recent first)
    sessions.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));

    let total = sessions.len();

    // Apply pagination
    let sessions = sessions
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .collect::<Vec<_>>();

    Json(serde_json::json!({
        "sessions": sessions,
        "total": total
    }))
    .into_response()
}

/// GET /api/v1/sessions/:session_id - Get session details
async fn get_session_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(session_id): axum::extract::Path<u64>,
) -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    let start_ts = now - 30 * 86_400_000_000; // 30 days ago
    let end_ts = now;

    let edges = match state.tauri_state.db.query_temporal_range(start_ts, end_ts) {
        Ok(edges) => edges,
        Err(e) => {
            tracing::error!("Failed to query traces: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to query traces"})),
            )
                .into_response();
        }
    };

    let mut session_traces: Vec<AgentFlowEdge> = edges
        .into_iter()
        .filter(|e| e.session_id == session_id)
        .collect();

    if session_traces.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Session {} not found", session_id)})),
        )
            .into_response();
    }

    session_traces.sort_by_key(|e| e.timestamp_us);

    let started_at = session_traces.first().map(|e| e.timestamp_us).unwrap_or(0);
    let last_message_at = session_traces.last().map(|e| e.timestamp_us).unwrap_or(0);
    let message_count = session_traces.len();
    let total_tokens: u32 = session_traces.iter().map(|e| e.token_count).sum::<u32>();
    // BUG-01 FIX: Calculate wall-clock duration instead of sum of span durations
    let first_start = session_traces.first().map(|e| e.timestamp_us).unwrap_or(0);
    let last_end = session_traces.last().map(|e| e.timestamp_us + (e.duration_us as u64)).unwrap_or(0);
    let total_duration_ms: u32 = ((last_end - first_start) / 1000) as u32;

    let trace_ids: Vec<String> = session_traces
        .iter()
        .map(|e| format!("{:#x}", e.edge_id))
        .collect();

    let one_hour_ago = now - 3_600_000_000;
    let status = if last_message_at > one_hour_ago {
        "active".to_string()
    } else {
        "ended".to_string()
    };

    let project_id = session_traces.first().map(|e| e.project_id).unwrap_or(0);
    let agent_id = session_traces.first().map(|e| e.agent_id).unwrap_or(0);

    let session = SessionInfo {
        session_id,
        project_id,
        agent_id,
        started_at,
        last_message_at,
        message_count,
        total_tokens,
        total_duration_ms,
        trace_ids,
        status,
    };

    let traces: Vec<serde_json::Value> = session_traces
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "trace_id": format!("{:#x}", e.edge_id),
                "span_id": format!("{:#x}", e.edge_id),
                "timestamp_us": e.timestamp_us,
                "span_type": e.span_type,
                "duration_us": e.duration_us,
                "token_count": e.token_count,
                "status": if e.is_deleted() { "deleted" } else { "completed" },
            })
        })
        .collect();

    Json(serde_json::json!({
        "session": session,
        "traces": traces
    }))
    .into_response()
}

// ============================================================================
// Analytics Handlers
// ============================================================================

#[derive(Debug, Deserialize)]
struct TimeSeriesQuery {
    #[serde(default)]
    metric: String,
    #[serde(alias = "start_ts")]
    start_time: u64,
    #[serde(alias = "end_ts")]
    end_time: u64,
    #[serde(default = "default_granularity")]
    granularity: String,
    project_id: Option<u16>,
    interval_seconds: Option<u64>,
}

fn default_granularity() -> String {
    "hour".to_string()
}

/// GET /api/v1/analytics/timeseries - Get time-series metrics
///
/// Uses pre-aggregated metrics for O(1) performance when available,
/// falling back to O(N) scan for edge cases.
async fn analytics_timeseries_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<TimeSeriesQuery>,
) -> impl IntoResponse {
    let project_id = params.project_id.unwrap_or(0) as u64;
    
    tracing::info!("Analytics timeseries: project_id={:?} (resolved={}), time_range={}..{}", 
        params.project_id, project_id, params.start_time, params.end_time);
    
    // Try to use pre-aggregated metrics first (O(buckets) vs O(edges))
    let timeseries = state.tauri_state.db.query_metrics_timeseries(
        project_id,
        params.start_time,
        params.end_time,
    );
    
    // Check if we actually have data in the pre-aggregated metrics (not just empty buckets)
    let has_precomputed_data = timeseries.iter().any(|(_, bucket)| bucket.request_count > 0);
    tracing::debug!("Pre-aggregated metrics: {} buckets, has_data={}", timeseries.len(), has_precomputed_data);
    
    // If we have pre-aggregated data with actual requests, use it
    if has_precomputed_data {
        // Convert to response format
        let data: Vec<serde_json::Value> = timeseries.into_iter().map(|(ts, bucket)| {
            serde_json::json!({
                "timestamp": ts,
                "request_count": bucket.request_count,
                "total_tokens": bucket.total_tokens,
                "avg_duration": bucket.avg_duration_ms() as u64,
                "total_cost": 0.0, // Not tracked in MetricsBucket
                "error_count": bucket.error_count,
            })
        }).collect();
        
        // Get summary from aggregated stats
        let stats = state.tauri_state.db.query_metrics(
            0, // tenant_id
            project_id as u16,
            params.start_time,
            params.end_time,
        );
        
        return Json(serde_json::json!({
            "data": data,
            "summary": {
                "total_requests": stats.request_count,
                "total_tokens": stats.total_tokens,
                "avg_duration_ms": stats.avg_duration_ms() as u64,
                "error_rate": if stats.request_count > 0 { stats.error_count as f64 / stats.request_count as f64 } else { 0.0 },
            }
        }))
        .into_response();
    }
    
    // Fallback: Query traces and aggregate
    // FIX: When a project_id is specified, also include traces with project_id=0
    // (unassigned traces) since many traces are stored without a project filter.
    // This ensures analytics work even when traces weren't explicitly tagged.
    let edges = if let Some(requested_project_id) = params.project_id {
        // Query both the specific project AND project_id=0 (unassigned traces)
        let mut all_edges = Vec::new();
        
        // Query specific project
        if let Ok(project_edges) = state.tauri_state.db.query_filtered(
            params.start_time, params.end_time, None, Some(requested_project_id)
        ) {
            tracing::debug!("Analytics: Found {} edges for project_id={} in time range {}..{}", 
                project_edges.len(), requested_project_id, params.start_time, params.end_time);
            all_edges.extend(project_edges);
        }
        
        // Include project_id=0 traces ONLY for Claude Code project (49455)
        // Legacy plugin versions stored traces with project_id=0; these belong to Claude Code
        if requested_project_id == CLAUDE_CODE_PROJECT_ID {
            if let Ok(default_edges) = state.tauri_state.db.query_filtered(
                params.start_time, params.end_time, None, Some(0)
            ) {
                tracing::debug!("Analytics: Found {} edges for project_id=0 (legacy Claude Code)", default_edges.len());
                all_edges.extend(default_edges);
            }
        }
        
        // If still no edges, try querying all traces (full fallback)
        if all_edges.is_empty() {
            tracing::debug!("Analytics: No edges found, falling back to full temporal range query");
            match state.tauri_state.db.query_temporal_range(params.start_time, params.end_time) {
                Ok(edges) => {
                    tracing::debug!("Analytics: Full temporal range returned {} edges", edges.len());
                    edges
                },
                Err(e) => {
                    tracing::error!("Failed to query traces: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": "Failed to query traces"})),
                    )
                        .into_response();
                }
            }
        } else {
            all_edges
        }
    } else {
        match state.tauri_state.db.query_temporal_range(params.start_time, params.end_time) {
            Ok(edges) => edges,
            Err(e) => {
                tracing::error!("Failed to query traces: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to query traces"})),
                )
                    .into_response();
            }
        }
    };

    // Calculate bucket size from interval_seconds
    let bucket_size_us = params.interval_seconds.unwrap_or(300) as u64 * 1_000_000;
    
    // Aggregate into time buckets
    let mut buckets: std::collections::BTreeMap<u64, (u64, u64, u64, f64, u64)> = std::collections::BTreeMap::new();
    let mut total_cost_sum = 0.0f64;
    
    for edge in &edges {
        let bucket_ts = (edge.timestamp_us / bucket_size_us) * bucket_size_us;
        let entry = buckets.entry(bucket_ts).or_insert((0, 0, 0, 0.0, 0));
        entry.0 += 1; // request_count
        entry.1 += edge.token_count as u64; // total_tokens
        entry.2 += edge.duration_us as u64; // total_duration
        
        // Get cost from payload - try multiple sources
        if let Ok(Some(payload)) = state.tauri_state.db.get_payload(edge.edge_id) {
            if let Ok(attrs) = serde_json::from_slice::<serde_json::Value>(&payload) {
                // First try direct cost attribute
                if let Some(cost) = attrs.get("cost").and_then(|v| v.as_f64()) {
                    entry.3 += cost;
                    total_cost_sum += cost;
                } else {
                    // Calculate cost from tokens using pricing registry
                    let model = attrs.get("gen_ai.request.model")
                        .or(attrs.get("gen_ai.response.model"))
                        .or(attrs.get("llm.model_name"))
                        .or(attrs.get("model"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    
                    let input_tokens = attrs.get("gen_ai.usage.input_tokens")
                        .or(attrs.get("gen_ai.usage.prompt_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let output_tokens = attrs.get("gen_ai.usage.output_tokens")
                        .or(attrs.get("gen_ai.usage.completion_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    
                    if input_tokens > 0 || output_tokens > 0 {
                        let cost = state.pricing_registry.calculate_cost(model, input_tokens, output_tokens).await;
                        entry.3 += cost;
                        total_cost_sum += cost;
                    }
                }
                
                // Count errors from status
                if let Some(status) = attrs.get("status").and_then(|v| v.as_str()) {
                    if status == "error" || status == "ERROR" {
                        entry.4 += 1;
                    }
                }
            }
        }
    }
    
    // Convert to response format
    let data: Vec<serde_json::Value> = buckets.into_iter().map(|(ts, (count, tokens, duration, cost, errors))| {
        serde_json::json!({
            "timestamp": ts,
            "request_count": count,
            "total_tokens": tokens,
            "avg_duration": if count > 0 { duration / count / 1000 } else { 0 }, // Convert to ms
            "total_cost": cost,
            "error_count": errors,
        })
    }).collect();

    // Calculate summary
    let total_requests = edges.len();
    let total_tokens: u64 = edges.iter().map(|e| e.token_count as u64).sum();
    let total_duration_us: u64 = edges.iter().map(|e| e.duration_us as u64).sum();
    let avg_duration_ms = if total_requests > 0 {
        total_duration_us / 1000 / total_requests as u64
    } else {
        0
    };
    
    // Calculate total errors from data
    let total_errors: u64 = data.iter()
        .filter_map(|d| d.get("error_count").and_then(|v| v.as_u64()))
        .sum();
    let error_rate = if total_requests > 0 {
        (total_errors as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    tracing::info!("Analytics result: total_requests={}, total_cost={}, avg_duration_ms={}, data_points={}", 
        total_requests, total_cost_sum, avg_duration_ms, data.len());

    Json(serde_json::json!({
        "data": data,
        "summary": {
            "total_requests": total_requests,
            "total_tokens": total_tokens,
            "avg_duration_ms": avg_duration_ms,
            "total_cost": total_cost_sum,
            "error_rate": error_rate,
        }
    }))
    .into_response()
}

// ============================================================================
// Playground Handler
// ============================================================================

#[derive(Debug, Deserialize)]
struct PlaygroundRunRequest {
    prompt: String,
    #[allow(dead_code)]
    #[serde(default)]
    variables: HashMap<String, String>,
    model: String,
    temperature: f64,
    max_tokens: u32,
    /// Provider type (openai, anthropic, ollama, custom)
    provider: Option<String>,
    /// Base URL for the API endpoint
    base_url: Option<String>,
    /// API key for the provider
    api_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlaygroundRunResponse {
    output: String,
    metadata: RunMetadata,
}

#[derive(Debug, Serialize)]
struct RunMetadata {
    latency_ms: u64,
    tokens_used: TokenUsage,
    cost_usd: f64,
    model_used: String,
    timestamp: u64,
}

#[derive(Debug, Serialize)]
struct TokenUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// POST /api/v1/playground/run - Execute prompt in playground using real LLM
async fn playground_run_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<PlaygroundRunRequest>,
) -> impl IntoResponse {
    use crate::llm::{ChatMessage, LLMCompletionRequest};

    // Log incoming request details for debugging
    tracing::info!(
        "Playground request: model={}, provider={:?}, base_url={:?}, has_api_key={}",
        req.model,
        req.provider,
        req.base_url,
        req.api_key.is_some()
    );

    // Build chat messages from the prompt
    let messages = if req.prompt.contains('\n') && req.prompt.lines().count() > 1 {
        // Try to parse as conversation format
        let lines: Vec<&str> = req.prompt.lines().collect();
        let mut msgs = Vec::new();
        let mut current_role = "user";
        let mut current_content = String::new();

        for line in lines {
            let line_lower = line.to_lowercase();
            if line_lower.starts_with("user:") || line_lower.starts_with("human:") {
                if !current_content.trim().is_empty() {
                    msgs.push(ChatMessage {
                        role: current_role.to_string(),
                        content: current_content.trim().to_string(),
                    });
                }
                current_role = "user";
                current_content = line.splitn(2, ':').nth(1).unwrap_or("").to_string();
            } else if line_lower.starts_with("assistant:") || line_lower.starts_with("ai:") {
                if !current_content.trim().is_empty() {
                    msgs.push(ChatMessage {
                        role: current_role.to_string(),
                        content: current_content.trim().to_string(),
                    });
                }
                current_role = "assistant";
                current_content = line.splitn(2, ':').nth(1).unwrap_or("").to_string();
            } else if line_lower.starts_with("system:") {
                if !current_content.trim().is_empty() {
                    msgs.push(ChatMessage {
                        role: current_role.to_string(),
                        content: current_content.trim().to_string(),
                    });
                }
                current_role = "system";
                current_content = line.splitn(2, ':').nth(1).unwrap_or("").to_string();
            } else {
                current_content.push('\n');
                current_content.push_str(line);
            }
        }
        if !current_content.trim().is_empty() {
            msgs.push(ChatMessage {
                role: current_role.to_string(),
                content: current_content.trim().to_string(),
            });
        }
        if msgs.is_empty() {
            vec![ChatMessage {
                role: "user".to_string(),
                content: req.prompt.clone(),
            }]
        } else {
            msgs
        }
    } else {
        // Single prompt
        vec![ChatMessage {
            role: "user".to_string(),
            content: req.prompt.clone(),
        }]
    };

    let llm_request = LLMCompletionRequest {
        model: req.model.clone(),
        messages,
        temperature: Some(req.temperature as f32),
        max_tokens: Some(req.max_tokens),
        stream: Some(false),
    };

    // Execute the LLM request
    let llm_client = state.tauri_state.llm_client.read().await;
    
    // Use provider config if available, otherwise fall back to auto-detection
    let result = if let (Some(base_url), Some(provider)) = (&req.base_url, &req.provider) {
        tracing::info!(
            "Playground: Using explicit provider config: provider={}, base_url={}, model={}",
            provider, base_url, req.model
        );
        llm_client.chat_completion_with_provider(
            llm_request,
            provider,
            base_url,
            req.api_key.as_deref(),
        ).await
    } else {
        tracing::info!("Playground: Using auto-detection for model={}", req.model);
        llm_client.chat_completion(llm_request).await
    };

    match result {
        Ok(response) => {
            // Calculate cost based on model pricing (rough estimates)
            let cost = calculate_cost(&req.model, response.usage.prompt_tokens, response.usage.completion_tokens);

            Json(PlaygroundRunResponse {
                output: response.content,
                metadata: RunMetadata {
                    latency_ms: response.latency_ms,
                    tokens_used: TokenUsage {
                        prompt_tokens: response.usage.prompt_tokens,
                        completion_tokens: response.usage.completion_tokens,
                        total_tokens: response.usage.total_tokens,
                    },
                    cost_usd: cost,
                    model_used: response.model,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                },
            })
            .into_response()
        }
        Err(e) => {
            // Return error as a response instead of internal error
            let error_msg = format!("LLM Error: {}", e);
            warn!("Playground LLM error: {}", error_msg);

            Json(PlaygroundRunResponse {
                output: error_msg,
                metadata: RunMetadata {
                    latency_ms: 0,
                    tokens_used: TokenUsage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                    cost_usd: 0.0,
                    model_used: req.model,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                },
            })
            .into_response()
        }
    }
}

/// Calculate cost based on model and token usage
fn calculate_cost(model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    // Pricing per 1K tokens (approximate, as of 2024)
    let (prompt_rate, completion_rate) = match model {
        // OpenAI
        m if m.starts_with("gpt-4o-mini") => (0.00015, 0.0006),
        m if m.starts_with("gpt-4o") => (0.005, 0.015),
        m if m.starts_with("gpt-4-turbo") => (0.01, 0.03),
        m if m.starts_with("gpt-4") => (0.03, 0.06),
        m if m.starts_with("gpt-3.5") => (0.0005, 0.0015),
        // Anthropic
        m if m.contains("claude-3-opus") => (0.015, 0.075),
        m if m.contains("claude-3-sonnet") || m.contains("claude-3.5-sonnet") => (0.003, 0.015),
        m if m.contains("claude-3-haiku") => (0.00025, 0.00125),
        // Ollama models are free (local)
        _ => (0.0, 0.0),
    };

    (prompt_tokens as f64 / 1000.0 * prompt_rate) + (completion_tokens as f64 / 1000.0 * completion_rate)
}

/// GET /api/v1/llm/models - List available LLM models
async fn list_llm_models_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    #[derive(Serialize)]
    struct ModelInfo {
        id: String,
        name: String,
        provider: String,
        available: bool,
    }

    #[derive(Serialize)]
    struct ModelsResponse {
        models: Vec<ModelInfo>,
    }

    let llm_client = state.tauri_state.llm_client.read().await;
    let mut models = Vec::new();

    // Only list Ollama models from server - user-configured cloud models are stored in localStorage
    // Try to list Ollama models
    match llm_client.list_ollama_models().await {
        Ok(ollama_models) => {
            for model in ollama_models {
                models.push(ModelInfo {
                    id: model.name.clone(),
                    name: model.name.clone(),
                    provider: "ollama".to_string(),
                    available: true,
                });
            }
        }
        Err(e) => {
            debug!("Could not list Ollama models: {}", e);
        }
    }

    Json(ModelsResponse { models }).into_response()
}

/// Path to the LLM config file
fn get_llm_config_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentreplay")
        .join("llm-config.json")
}

/// Load LLM config from persistent storage
fn load_persisted_llm_config() -> Option<crate::llm::LLMConfig> {
    let config_path = get_llm_config_path();
    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(contents) => {
                match serde_json::from_str::<crate::llm::LLMConfig>(&contents) {
                    Ok(config) => {
                        tracing::info!("Loaded LLM config from {:?}", config_path);
                        Some(config)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse LLM config: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read LLM config file: {}", e);
                None
            }
        }
    } else {
        None
    }
}

/// Persist LLM config to disk
fn persist_llm_config(config: &crate::llm::LLMConfig) -> Result<(), String> {
    let config_path = get_llm_config_path();
    
    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize LLM config: {}", e))?;
    
    std::fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write LLM config: {}", e))?;
    
    tracing::info!("Persisted LLM config to {:?}", config_path);
    Ok(())
}

/// GET /api/v1/llm/config - Get LLM configuration
async fn get_llm_config_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    // First try to get current in-memory config
    let llm_client = state.tauri_state.llm_client.read().await;
    let current_config = llm_client.get_config().clone();
    
    // If current config is default, try loading from disk
    if current_config.providers.is_empty() || current_config.providers.len() == 1 {
        if let Some(persisted) = load_persisted_llm_config() {
            return Json(persisted).into_response();
        }
    }
    
    Json(current_config).into_response()
}

/// PUT /api/v1/llm/config - Update LLM configuration
async fn update_llm_config_handler(
    AxumState(state): AxumState<ServerState>,
    Json(config): Json<crate::llm::LLMConfig>,
) -> impl IntoResponse {
    // Update in-memory config
    let mut llm_client = state.tauri_state.llm_client.write().await;
    llm_client.set_config(config.clone());

    // Persist config to disk
    match persist_llm_config(&config) {
        Ok(()) => {
            Json(serde_json::json!({
                "success": true,
                "message": "LLM configuration updated and persisted"
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to persist LLM config: {}", e);
            Json(serde_json::json!({
                "success": true,
                "message": format!("LLM configuration updated (persistence failed: {})", e),
                "warning": "Config may not survive restart"
            }))
            .into_response()
        }
    }
}

/// GET /api/v1/llm/check - Check LLM health/availability
async fn check_llm_health_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    let llm_client = state.tauri_state.llm_client.read().await;

    #[derive(Serialize)]
    struct HealthStatus {
        ollama: ProviderStatus,
        openai: ProviderStatus,
        anthropic: ProviderStatus,
    }

    #[derive(Serialize)]
    struct ProviderStatus {
        available: bool,
        configured: bool,
        message: String,
    }

    // Check Ollama
    let ollama_available = llm_client.check_ollama().await.unwrap_or(false);
    let ollama_status = ProviderStatus {
        available: ollama_available,
        configured: true, // Ollama doesn't need config
        message: if ollama_available {
            "Ollama is running".to_string()
        } else {
            "Ollama not available. Start with 'ollama serve'".to_string()
        },
    };

    // Check OpenAI (just config, not actual API call)
    let openai_configured = std::env::var("OPENAI_API_KEY").is_ok();
    let openai_status = ProviderStatus {
        available: openai_configured,
        configured: openai_configured,
        message: if openai_configured {
            "OpenAI API key configured".to_string()
        } else {
            "Set OPENAI_API_KEY environment variable".to_string()
        },
    };

    // Check Anthropic (just config, not actual API call)
    let anthropic_configured = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let anthropic_status = ProviderStatus {
        available: anthropic_configured,
        configured: anthropic_configured,
        message: if anthropic_configured {
            "Anthropic API key configured".to_string()
        } else {
            "Set ANTHROPIC_API_KEY environment variable".to_string()
        },
    };

    Json(HealthStatus {
        ollama: ollama_status,
        openai: openai_status,
        anthropic: anthropic_status,
    })
    .into_response()
}

// =============================================================================
// MODEL COMPARISON ENDPOINTS
// =============================================================================

/// Request for model comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonRunRequest {
    /// The prompt to send to all models
    pub prompt: String,
    /// Models to compare (max 3)
    pub models: Vec<ModelSelectionRequest>,
    /// Temperature (0.0-2.0)
    #[serde(default = "default_comparison_temperature")]
    pub temperature: f32,
    /// Max tokens to generate
    #[serde(default = "default_comparison_max_tokens")]
    pub max_tokens: u32,
    /// Optional system prompt
    pub system_prompt: Option<String>,
    /// Optional template variables
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

fn default_comparison_temperature() -> f32 {
    0.7
}

fn default_comparison_max_tokens() -> u32 {
    2048
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelectionRequest {
    /// Provider (openai, anthropic, ollama, custom)
    pub provider: String,
    /// Model ID
    pub model_id: String,
    /// Display name
    pub display_name: Option<String>,
    /// Base URL for API endpoint (required for proper routing)
    pub base_url: Option<String>,
    /// API key for the provider
    pub api_key: Option<String>,
}

/// Response wrapper for comparison results
#[derive(Debug, Serialize)]
pub struct ComparisonRunResponse {
    pub success: bool,
    pub comparison_id: String,
    pub results: Vec<ModelResultResponse>,
    pub summary: ComparisonSummary,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelResultResponse {
    pub model_key: String,
    pub provider: String,
    pub model_id: String,
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub latency_ms: u32,
    pub cost_usd: f64,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ComparisonSummary {
    pub total_models: usize,
    pub successful: usize,
    pub failed: usize,
    pub fastest_model: Option<String>,
    pub cheapest_model: Option<String>,
    pub total_cost_usd: f64,
    pub total_latency_ms: u32,
}

/// POST /api/v1/comparison/run - Run model comparison
async fn run_comparison_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<ComparisonRunRequest>,
) -> impl IntoResponse {
    use crate::comparison_engine::ModelComparisonEngine;

    // Convert request to core types, including provider configuration
    let models: Vec<ModelSelection> = req
        .models
        .iter()
        .map(|m| {
            let mut sel = ModelSelection::new(&m.provider, &m.model_id);
            if let Some(name) = &m.display_name {
                sel = sel.with_display_name(name);
            }
            if let Some(url) = &m.base_url {
                sel = sel.with_base_url(url);
            }
            if let Some(key) = &m.api_key {
                sel = sel.with_api_key(key);
            }
            tracing::info!(
                "Model selection: provider={}, model={}, base_url={:?}, has_api_key={}",
                m.provider, m.model_id, m.base_url, m.api_key.is_some()
            );
            sel
        })
        .collect();

    // Validate model count
    if models.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ComparisonRunResponse {
                success: false,
                comparison_id: String::new(),
                results: vec![],
                summary: ComparisonSummary {
                    total_models: 0,
                    successful: 0,
                    failed: 0,
                    fastest_model: None,
                    cheapest_model: None,
                    total_cost_usd: 0.0,
                    total_latency_ms: 0,
                },
                error: Some("No models selected".to_string()),
            }),
        )
            .into_response();
    }

    if models.len() > 3 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ComparisonRunResponse {
                success: false,
                comparison_id: String::new(),
                results: vec![],
                summary: ComparisonSummary {
                    total_models: models.len(),
                    successful: 0,
                    failed: 0,
                    fastest_model: None,
                    cheapest_model: None,
                    total_cost_usd: 0.0,
                    total_latency_ms: 0,
                },
                error: Some("Maximum 3 models allowed".to_string()),
            }),
        )
            .into_response();
    }

    // Build comparison request
    let comparison_request = ModelComparisonRequest {
        prompt: req.prompt,
        models,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        system_prompt: req.system_prompt,
        variables: req.variables,
    };

    // Create comparison engine
    let engine = ModelComparisonEngine::new(
        Arc::clone(&state.tauri_state.llm_client),
        Arc::clone(&state.pricing_registry),
    );

    // Execute comparison
    match engine.compare(comparison_request).await {
        Ok(response) => {
            let results: Vec<ModelResultResponse> = response
                .results
                .iter()
                .map(|r| ModelResultResponse {
                    model_key: r.model.key(),
                    provider: r.model.provider.clone(),
                    model_id: r.model.model_id.clone(),
                    content: r.content.clone(),
                    input_tokens: r.input_tokens,
                    output_tokens: r.output_tokens,
                    latency_ms: r.latency_ms,
                    cost_usd: r.cost_usd,
                    status: format!("{:?}", r.status).to_lowercase(),
                    error: r.error.clone(),
                })
                .collect();

            let successful = results.iter().filter(|r| r.status == "completed").count();
            let failed = results.len() - successful;

            let fastest_model = response.fastest().map(|r| r.model.key());
            let cheapest_model = response.cheapest().map(|r| r.model.key());

            Json(ComparisonRunResponse {
                success: true,
                comparison_id: response.comparison_id,
                results,
                summary: ComparisonSummary {
                    total_models: response.results.len(),
                    successful,
                    failed,
                    fastest_model,
                    cheapest_model,
                    total_cost_usd: response.total_cost_usd,
                    total_latency_ms: response.total_latency_ms,
                },
                error: None,
            })
            .into_response()
        }
        Err(e) => {
            error!("Comparison failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ComparisonRunResponse {
                    success: false,
                    comparison_id: String::new(),
                    results: vec![],
                    summary: ComparisonSummary {
                        total_models: 0,
                        successful: 0,
                        failed: 0,
                        fastest_model: None,
                        cheapest_model: None,
                        total_cost_usd: 0.0,
                        total_latency_ms: 0,
                    },
                    error: Some(e.to_string()),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/comparison/models - List available models for comparison
async fn list_comparison_models_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    #[derive(Serialize)]
    struct ModelInfo {
        provider: String,
        model_id: String,
        display_name: String,
        input_cost_per_1m: Option<f64>,
        output_cost_per_1m: Option<f64>,
        context_window: Option<u32>,
        available: bool,
    }

    #[derive(Serialize)]
    struct ModelsResponse {
        models: Vec<ModelInfo>,
    }

    // Get pricing info for popular models
    let popular_models = vec![
        // OpenAI
        ("openai", "gpt-4o", "GPT-4o"),
        ("openai", "gpt-4o-mini", "GPT-4o Mini"),
        ("openai", "gpt-4-turbo", "GPT-4 Turbo"),
        ("openai", "gpt-3.5-turbo", "GPT-3.5 Turbo"),
        ("openai", "o1-preview", "o1 Preview"),
        ("openai", "o1-mini", "o1 Mini"),
        // Anthropic
        ("anthropic", "claude-3-5-sonnet-20241022", "Claude 3.5 Sonnet"),
        ("anthropic", "claude-3-5-haiku-20241022", "Claude 3.5 Haiku"),
        ("anthropic", "claude-3-opus-20240229", "Claude 3 Opus"),
        // DeepSeek
        ("deepseek", "deepseek-chat", "DeepSeek Chat"),
        ("deepseek", "deepseek-coder", "DeepSeek Coder"),
        // Local
        ("ollama", "llama3.2", "Llama 3.2 (Local)"),
        ("ollama", "qwen2.5-coder", "Qwen 2.5 Coder (Local)"),
        ("ollama", "mistral", "Mistral (Local)"),
    ];

    let mut models = Vec::new();

    for (provider, model_id, display_name) in popular_models {
        let pricing = state.pricing_registry.get_pricing(model_id).await;

        let (input_cost, output_cost, context_window) = if let Some(p) = pricing {
            (
                Some(p.input_cost_per_token * 1_000_000.0),
                Some(p.output_cost_per_token * 1_000_000.0),
                p.context_window,
            )
        } else {
            (None, None, None)
        };

        // Check availability based on provider
        let available = match provider {
            "openai" => std::env::var("OPENAI_API_KEY").is_ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").is_ok(),
            "deepseek" => std::env::var("DEEPSEEK_API_KEY").is_ok(),
            "ollama" => true, // Assume local is available
            _ => false,
        };

        models.push(ModelInfo {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
            input_cost_per_1m: input_cost,
            output_cost_per_1m: output_cost,
            context_window,
            available,
        });
    }

    Json(ModelsResponse { models }).into_response()
}

// =============================================================================
// PRICING ENDPOINTS
// =============================================================================

/// Response for pricing list
#[derive(Debug, Serialize)]
struct PricingListResponse {
    models: Vec<PricingInfo>,
    last_sync: Option<u64>,
    source: String,
}

#[derive(Debug, Serialize)]
struct PricingInfo {
    model_id: String,
    provider: Option<String>,
    input_cost_per_1m: f64,
    output_cost_per_1m: f64,
    context_window: Option<u32>,
    supports_vision: bool,
    supports_function_calling: bool,
}

/// GET /api/v1/pricing/models - List all pricing information
async fn list_pricing_handler(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    // Get some popular models pricing
    let model_ids = vec![
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4-turbo",
        "gpt-3.5-turbo",
        "claude-3-5-sonnet-20241022",
        "claude-3-5-haiku-20241022",
        "claude-3-opus-20240229",
        "deepseek-chat",
        "deepseek-coder",
    ];

    let mut models = Vec::new();

    for model_id in model_ids {
        if let Some(pricing) = state.pricing_registry.get_pricing(model_id).await {
            models.push(PricingInfo {
                model_id: model_id.to_string(),
                provider: pricing.provider.clone(),
                input_cost_per_1m: pricing.input_cost_per_token * 1_000_000.0,
                output_cost_per_1m: pricing.output_cost_per_token * 1_000_000.0,
                context_window: pricing.context_window,
                supports_vision: pricing.supports_vision,
                supports_function_calling: pricing.supports_function_calling,
            });
        }
    }

    Json(PricingListResponse {
        models,
        last_sync: state.pricing_registry.last_sync_time().await,
        source: "litellm".to_string(),
    })
    .into_response()
}

/// GET /api/v1/pricing/models/all - List ALL pricing information from registry
async fn list_all_pricing_handler(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    let all_models = state.pricing_registry.list_models().await;
    
    let models: Vec<serde_json::Value> = all_models.iter().map(|(model_id, pricing)| {
        serde_json::json!({
            "model_id": model_id,
            "provider": pricing.litellm_provider.clone().or(pricing.provider.clone()),
            "input_cost_per_1m": pricing.input_cost_per_token * 1_000_000.0,
            "output_cost_per_1m": pricing.output_cost_per_token * 1_000_000.0,
            "context_window": pricing.context_window,
            "supports_vision": pricing.supports_vision,
            "supports_function_calling": pricing.supports_function_calling,
            "source": pricing.source.clone(),
            "priority": format!("{:?}", pricing.priority),
        })
    }).collect();
    
    Json(serde_json::json!({
        "models": models,
        "last_sync": state.pricing_registry.last_sync_time().await,
        "total_count": models.len(),
    }))
    .into_response()
}

/// GET /api/v1/pricing/custom - List custom pricing overrides
async fn list_custom_pricing_handler(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    let all_models = state.pricing_registry.list_models().await;
    
    // Filter to only custom priority entries
    let custom_entries: Vec<serde_json::Value> = all_models.iter()
        .filter(|(_, pricing)| pricing.source.as_deref() == Some("Custom") || 
                               pricing.source.as_deref() == Some("custom"))
        .map(|(model_id, pricing)| {
            serde_json::json!({
                "model_id": model_id,
                "provider": pricing.litellm_provider.clone().unwrap_or_else(|| "custom".to_string()),
                "input_cost_per_token": pricing.input_cost_per_token,
                "output_cost_per_token": pricing.output_cost_per_token,
                "max_tokens": pricing.max_tokens,
            })
        }).collect();
    
    Json(serde_json::json!({
        "entries": custom_entries,
        "count": custom_entries.len(),
    }))
    .into_response()
}

/// Request for custom pricing
#[derive(Debug, Deserialize)]
struct CustomPricingRequest {
    model_id: String,
    provider: String,
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    #[serde(default)]
    max_tokens: Option<u32>,
}

/// POST /api/v1/pricing/custom - Add custom pricing override
async fn add_custom_pricing_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<CustomPricingRequest>,
) -> impl IntoResponse {
    use agentreplay_core::model_pricing::CustomPricingOverride;
    
    let override_data = CustomPricingOverride {
        model_id: req.model_id.clone(),
        input_cost_per_token: req.input_cost_per_token,
        output_cost_per_token: req.output_cost_per_token,
        max_tokens: req.max_tokens,
        litellm_provider: Some(req.provider.clone()),
        source: Some("Custom".to_string()),
    };
    
    state.pricing_registry.add_custom_override(override_data).await;
    
    // Save custom pricing to file
    if let Err(e) = state.pricing_registry.save_custom_overrides().await {
        tracing::warn!("Failed to persist custom pricing: {}", e);
    }
    
    Json(serde_json::json!({
        "success": true,
        "message": format!("Added custom pricing for {}", req.model_id),
    }))
    .into_response()
}

/// PUT /api/v1/pricing/custom - Update custom pricing override
async fn update_custom_pricing_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<CustomPricingRequest>,
) -> impl IntoResponse {
    use agentreplay_core::model_pricing::CustomPricingOverride;
    
    let override_data = CustomPricingOverride {
        model_id: req.model_id.clone(),
        input_cost_per_token: req.input_cost_per_token,
        output_cost_per_token: req.output_cost_per_token,
        max_tokens: req.max_tokens,
        litellm_provider: Some(req.provider.clone()),
        source: Some("Custom".to_string()),
    };
    
    state.pricing_registry.add_custom_override(override_data).await;
    
    // Save custom pricing to file
    if let Err(e) = state.pricing_registry.save_custom_overrides().await {
        tracing::warn!("Failed to persist custom pricing: {}", e);
    }
    
    Json(serde_json::json!({
        "success": true,
        "message": format!("Updated custom pricing for {}", req.model_id),
    }))
    .into_response()
}

/// DELETE /api/v1/pricing/custom/:model_id - Delete custom pricing override
async fn delete_custom_pricing_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Path(model_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // Remove from registry
    state.pricing_registry.remove_custom_override(&model_id).await;
    
    // Save changes to file
    if let Err(e) = state.pricing_registry.save_custom_overrides().await {
        tracing::warn!("Failed to persist custom pricing removal: {}", e);
    }
    
    Json(serde_json::json!({
        "success": true,
        "message": format!("Deleted custom pricing for {}", model_id),
    }))
    .into_response()
}

/// POST /api/v1/pricing/sync - Sync pricing from LiteLLM
async fn sync_pricing_handler(AxumState(state): AxumState<ServerState>) -> impl IntoResponse {
    #[derive(Serialize)]
    struct SyncResponse {
        success: bool,
        message: String,
        models_synced: usize,
    }

    match state.pricing_registry.sync_from_litellm().await {
        Ok(count) => Json(SyncResponse {
            success: true,
            message: "Pricing synced successfully".to_string(),
            models_synced: count,
        })
        .into_response(),
        Err(e) => {
            error!("Pricing sync failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncResponse {
                    success: false,
                    message: format!("Sync failed: {}", e),
                    models_synced: 0,
                }),
            )
                .into_response()
        }
    }
}

/// Request for cost calculation
#[derive(Debug, Deserialize)]
struct CalculatePricingRequest {
    model_id: String,
    input_tokens: u32,
    output_tokens: u32,
}

/// POST /api/v1/pricing/calculate - Calculate cost for token usage
async fn calculate_pricing_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<CalculatePricingRequest>,
) -> impl IntoResponse {
    #[derive(Serialize)]
    struct CalculateResponse {
        model_id: String,
        input_tokens: u32,
        output_tokens: u32,
        input_cost: f64,
        output_cost: f64,
        total_cost: f64,
        pricing_found: bool,
    }

    let cost = state
        .pricing_registry
        .calculate_cost(&req.model_id, req.input_tokens, req.output_tokens)
        .await;

    let pricing = state.pricing_registry.get_pricing(&req.model_id).await;

    let (input_cost, output_cost) = if let Some(p) = &pricing {
        (
            p.input_cost_per_token * req.input_tokens as f64,
            p.output_cost_per_token * req.output_tokens as f64,
        )
    } else {
        (0.0, 0.0)
    };

    Json(CalculateResponse {
        model_id: req.model_id,
        input_tokens: req.input_tokens,
        output_tokens: req.output_tokens,
        input_cost,
        output_cost,
        total_cost: cost,
        pricing_found: pricing.is_some(),
    })
    .into_response()
}

/// POST /api/v1/pricing/models - Import pricing from JSON (like model_code.json)
async fn import_pricing_handler(
    AxumState(state): AxumState<ServerState>,
    Json(models): Json<Vec<serde_json::Value>>,
) -> impl IntoResponse {
    use agentreplay_core::model_pricing::CustomPricingOverride;
    
    let mut imported = 0;
    let mut skipped = 0;
    
    for model in models {
        let model_name = model.get("model_name").and_then(|v| v.as_str()).unwrap_or("");
        let provider = model.get("provider").and_then(|v| v.as_str());
        
        // Skip sample_spec and empty models
        if model_name.is_empty() || model_name == "sample_spec" {
            skipped += 1;
            continue;
        }
        
        // Parse costs - handle string or number
        let input_cost: f64 = model.get("input_cost_per_token")
            .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64()))
            .unwrap_or(0.0);
        let output_cost: f64 = model.get("output_cost_per_token")
            .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64()))
            .unwrap_or(0.0);
        
        // Parse max_tokens
        let max_tokens: Option<u32> = model.get("max_tokens")
            .and_then(|v| v.as_u64().map(|n| n as u32));
        
        let override_data = CustomPricingOverride {
            model_id: model_name.to_string(),
            input_cost_per_token: input_cost,
            output_cost_per_token: output_cost,
            max_tokens,
            litellm_provider: provider.map(|s| s.to_string()),
            source: Some("model_code.json".to_string()),
        };
        
        state.pricing_registry.add_custom_override(override_data).await;
        imported += 1;
    }
    
    Json(serde_json::json!({
        "success": true,
        "imported": imported,
        "skipped": skipped
    })).into_response()
}

/// Query params for cost analytics
#[derive(Debug, Deserialize)]
struct CostAnalyticsQuery {
    #[serde(default)]
    project_id: Option<i64>,
    #[serde(default)]
    start_time: Option<u64>,
    #[serde(default)]
    end_time: Option<u64>,
}

/// GET /api/v1/analytics/costs - Aggregate cost analytics from real trace data
async fn analytics_costs_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<CostAnalyticsQuery>,
) -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    
    // Default to last 7 days
    let start_time = params.start_time.unwrap_or(now - 7 * 24 * 60 * 60 * 1_000_000);
    let end_time = params.end_time.unwrap_or(now);
    
    tracing::info!("Cost analytics: project_id={:?}, time_range={}..{}", params.project_id, start_time, end_time);
    
    // Query traces - FIX: Include project_id=0 (unassigned) traces when a specific project is requested
    let edges = if let Some(project_id) = params.project_id {
        let mut all_edges = Vec::new();
        
        // Query specific project
        if let Ok(project_edges) = state.tauri_state.db.query_filtered(
            start_time, end_time, None, Some(project_id as u16)
        ) {
            tracing::debug!("Cost analytics: Found {} edges for project_id={}", project_edges.len(), project_id);
            all_edges.extend(project_edges);
        }
        
        // Include project_id=0 traces ONLY for Claude Code project (49455)
        // Legacy plugin versions stored traces with project_id=0; these belong to Claude Code
        if project_id == CLAUDE_CODE_PROJECT_ID as i64 {
            if let Ok(default_edges) = state.tauri_state.db.query_filtered(
                start_time, end_time, None, Some(0)
            ) {
                tracing::debug!("Cost analytics: Found {} edges for project_id=0 (legacy Claude Code)", default_edges.len());
                all_edges.extend(default_edges);
            }
        }
        
        // If still no edges, try querying all traces (full fallback)
        if all_edges.is_empty() {
            tracing::debug!("Cost analytics: No edges found, falling back to full temporal query");
            state.tauri_state.db.query_temporal_range(start_time, end_time)
                .unwrap_or_default()
        } else {
            all_edges
        }
    } else {
        state.tauri_state.db.query_temporal_range(start_time, end_time)
            .unwrap_or_default()
    };
    
    tracing::info!("Cost analytics: Processing {} edges", edges.len());
    
    // Aggregate by model
    let mut model_costs: std::collections::HashMap<String, (f64, u64, u64, u64)> = std::collections::HashMap::new();
    // (cost, input_tokens, output_tokens, call_count)
    
    // Track operation types
    let mut operation_counts: std::collections::HashMap<String, (f64, u64)> = std::collections::HashMap::new();
    // (cost, count)
    
    let mut total_cost = 0.0;
    let mut total_tokens = 0u64;
    let mut total_calls = 0u64;
    let mut cached_count = 0u64;
    
    for edge in &edges {
        // Get payload to extract model name
        let payload = state.tauri_state.db.get_payload(edge.edge_id).ok().flatten();
        let attrs: serde_json::Value = payload
            .and_then(|p| serde_json::from_slice(&p).ok())
            .unwrap_or(serde_json::json!({}));
        
        // Extract model from various attribute patterns
        let model = attrs.get("gen_ai.request.model")
            .or(attrs.get("gen_ai.response.model"))
            .or(attrs.get("llm.model_name"))
            .or(attrs.get("model"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Extract tokens
        let input_tokens = attrs.get("gen_ai.usage.input_tokens")
            .or(attrs.get("gen_ai.usage.prompt_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let output_tokens = attrs.get("gen_ai.usage.output_tokens")
            .or(attrs.get("gen_ai.usage.completion_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        
        // Check if cached
        if attrs.get("gen_ai.cached").and_then(|v| v.as_bool()).unwrap_or(false) {
            cached_count += 1;
        }
        
        // Determine operation type from span type or attributes
        let operation_type = attrs.get("span_type")
            .or(attrs.get("operation_type"))
            .and_then(|v| v.as_str())
            .map(|s| {
                if s.contains("embed") || s.to_lowercase().contains("embed") {
                    "Embeddings"
                } else if s.contains("function") || s.contains("tool") {
                    "Function Calls"
                } else {
                    "Chat Completions"
                }
            })
            .unwrap_or("Chat Completions")
            .to_string();
        
        // Calculate cost using pricing registry
        let cost = state.pricing_registry.calculate_cost(&model, input_tokens, output_tokens).await;
        
        // Update model aggregates
        let entry = model_costs.entry(model).or_insert((0.0, 0, 0, 0));
        entry.0 += cost;
        entry.1 += input_tokens as u64;
        entry.2 += output_tokens as u64;
        entry.3 += 1;
        
        // Update operation aggregates
        let op_entry = operation_counts.entry(operation_type).or_insert((0.0, 0));
        op_entry.0 += cost;
        op_entry.1 += 1;
        
        total_cost += cost;
        total_tokens += (input_tokens + output_tokens) as u64;
        total_calls += 1;
    }
    
    // Sort by cost descending and format response
    let mut models_vec: Vec<_> = model_costs.into_iter().collect();
    models_vec.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap_or(std::cmp::Ordering::Equal));
    
    // Define colors for models
    let colors = vec!["#10B981", "#F59E0B", "#6366F1", "#3B82F6", "#8B5CF6", "#EC4899"];
    
    let model_breakdown: Vec<serde_json::Value> = models_vec.iter()
        .take(10) // Top 10 models
        .enumerate()
        .map(|(i, (model, (cost, input_t, output_t, calls)))| {
            serde_json::json!({
                "model": model,
                "cost": cost,
                "tokens": input_t + output_t,
                "input_tokens": input_t,
                "output_tokens": output_t,
                "calls": calls,
                "color": colors.get(i % colors.len()).unwrap_or(&"#6B7280")
            })
        })
        .collect();
    
    // Build operation breakdown from real data
    let mut ops_vec: Vec<_> = operation_counts.into_iter().collect();
    ops_vec.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap_or(std::cmp::Ordering::Equal));
    
    let operation_breakdown: Vec<serde_json::Value> = ops_vec.iter()
        .map(|(op, (cost, _count))| {
            let percentage = if total_cost > 0.0 { (cost / total_cost) * 100.0 } else { 0.0 };
            let icon = match op.as_str() {
                "Embeddings" => "Cpu",
                "Function Calls" => "Code",
                _ => "MessageSquare",
            };
            serde_json::json!({
                "operation": op,
                "cost": cost,
                "percentage": percentage,
                "icon": icon
            })
        })
        .collect();
    
    // Token efficiency metrics
    let avg_cost_per_1k = if total_tokens > 0 {
        total_cost / (total_tokens as f64 / 1000.0)
    } else {
        0.0
    };
    
    // Calculate cache hit rate from actual data
    let cache_hit_rate = if total_calls > 0 {
        (cached_count as f64 / total_calls as f64) * 100.0
    } else {
        0.0
    };
    
    // Estimate potential savings based on cache hit rate potential
    // If cache hit rate is low, there's more potential for savings
    let potential_savings = if cache_hit_rate < 30.0 {
        total_cost * 0.20 // 20% potential savings if cache is underutilized
    } else if cache_hit_rate < 50.0 {
        total_cost * 0.10 // 10% potential savings
    } else {
        total_cost * 0.05 // 5% potential savings
    };
    
    tracing::info!("Cost analytics result: total_cost={}, total_calls={}, models={}", 
        total_cost, total_calls, models_vec.len());
    
    Json(serde_json::json!({
        "total_cost": total_cost,
        "total_tokens": total_tokens,
        "total_calls": total_calls,
        "model_breakdown": model_breakdown,
        "operation_breakdown": operation_breakdown,
        "efficiency": {
            "avg_cost_per_1k": avg_cost_per_1k,
            "cache_hit_rate": cache_hit_rate,
            "potential_savings": potential_savings
        },
        "time_range": {
            "start": start_time,
            "end": end_time
        }
    })).into_response()
}

// ============================================================================
// Search Handler
// ============================================================================

#[derive(Debug, Deserialize)]
struct SearchRequest {
    query: String,
    project_id: u16,
    #[serde(default = "default_search_limit")]
    limit: usize,
}

fn default_search_limit() -> usize {
    100
}

#[derive(Debug, Serialize)]
struct SearchResult {
    edge_id: String,
    timestamp_us: u64,
    operation: String,
    span_type: String,
    duration_ms: f64,
    tokens: u32,
    cost: f64,
    status: String,
    model: Option<String>,
    agent_id: u64,
    session_id: u64,
    // Extended fields for display
    #[serde(skip_serializing_if = "Option::is_none")]
    input_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
    count: usize,
}

/// POST /api/v1/search - Search traces by content (project-scoped)
async fn search_traces_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<SearchRequest>,
) -> impl IntoResponse {
    let limit = req.limit.min(500);
    let query_lower = req.query.to_lowercase();
    let project_id = req.project_id;

    // Get edges from last 24 hours, filtered by project
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let start = now.saturating_sub(86_400_000_000); // 24 hours

    // Use query_filtered for efficient project-scoped query
    let edges = match state.tauri_state.db.query_filtered(start, now, None, Some(project_id)) {
        Ok(edges) => edges,
        Err(e) => {
            error!("Failed to query edges: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(SearchResponse {
                results: vec![],
                count: 0,
            })).into_response();
        }
    };

    // Filter by content
    let mut matched: Vec<SearchResult> = Vec::new();
    
    for edge in edges {
        // Check payload content
        if let Ok(Some(payload_bytes)) = state.tauri_state.db.get_payload(edge.edge_id) {
            if let Ok(payload_str) = String::from_utf8(payload_bytes.clone()) {
                if payload_str.to_lowercase().contains(&query_lower) {
                    let span = edge.get_span_type();
                    
                    // Parse payload as JSON for extracting display fields
                    let attributes: Option<serde_json::Value> = serde_json::from_str(&payload_str).ok();
                    
                    // Extract model name
                    let model = attributes.as_ref().and_then(|a| {
                        a.get("model")
                            .or_else(|| a.get("gen_ai.request.model"))
                            .or_else(|| a.get("gen_ai.response.model"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    });
                    
                    // Extract input preview
                    let input_preview = attributes.as_ref().and_then(|a| {
                        // Try GenAI semantic conventions - skip system prompt
                        for i in 0..=2 {
                            let role_key = format!("gen_ai.prompt.{}.role", i);
                            let content_key = format!("gen_ai.prompt.{}.content", i);
                            if let Some(role) = a.get(&role_key).and_then(|v| v.as_str()) {
                                if role != "system" {
                                    if let Some(content) = a.get(&content_key).and_then(|v| v.as_str()) {
                                        return Some(content.chars().take(150).collect::<String>());
                                    }
                                }
                            } else if let Some(content) = a.get(&content_key).and_then(|v| v.as_str()) {
                                return Some(content.chars().take(150).collect::<String>());
                            }
                        }
                        // Try other common fields
                        if let Some(input) = a.get("input").and_then(|v| v.as_str()) {
                            return Some(input.chars().take(150).collect::<String>());
                        }
                        if let Some(prompt) = a.get("prompt").and_then(|v| v.as_str()) {
                            return Some(prompt.chars().take(150).collect::<String>());
                        }
                        None
                    });
                    
                    // Extract output preview
                    let output_preview = attributes.as_ref().and_then(|a| {
                        if let Some(content) = a.get("gen_ai.completion.0.content").and_then(|v| v.as_str()) {
                            return Some(content.chars().take(150).collect::<String>());
                        }
                        if let Some(output) = a.get("output").and_then(|v| v.as_str()) {
                            return Some(output.chars().take(150).collect::<String>());
                        }
                        if let Some(response) = a.get("response").and_then(|v| v.as_str()) {
                            return Some(response.chars().take(150).collect::<String>());
                        }
                        None
                    });
                    
                    matched.push(SearchResult {
                        edge_id: format!("{:#x}", edge.edge_id),
                        timestamp_us: edge.timestamp_us,
                        operation: format!("{:?}", span),
                        span_type: format!("{:?}", span),
                        duration_ms: edge.duration_us as f64 / 1_000.0,
                        tokens: edge.token_count,
                        cost: (edge.token_count as f64 / 1_000.0) * 0.002,
                        status: if edge.is_deleted() || matches!(span, agentreplay_core::SpanType::Error) {
                            "error".to_string()
                        } else {
                            "success".to_string()
                        },
                        model,
                        agent_id: edge.agent_id,
                        session_id: edge.session_id,
                        input_preview,
                        output_preview,
                        project_id: Some(edge.project_id),
                        metadata: attributes,
                    });
                    
                    if matched.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    matched.sort_by_key(|r| std::cmp::Reverse(r.timestamp_us));

    let count = matched.len();
    Json(SearchResponse { results: matched, count }).into_response()
}

// ============================================================================
// Insights Handlers
// ============================================================================

use agentreplay_core::insights::{InsightConfig, InsightEngine, InsightType, Severity, Insight};

#[derive(Debug, Deserialize)]
struct InsightsQuery {
    project_id: u16,
    #[serde(default = "default_window_seconds")]
    window_seconds: u64,
    #[serde(default)]
    min_severity: Option<String>,
    #[serde(default = "default_insights_limit")]
    limit: usize,
}

fn default_window_seconds() -> u64 {
    3600
}

fn default_insights_limit() -> usize {
    50
}

#[derive(Debug, Serialize)]
struct InsightView {
    id: String,
    insight_type: String,
    severity: String,
    confidence: f32,
    summary: String,
    description: String,
    related_trace_ids: Vec<String>,
    metadata: serde_json::Value,
    generated_at: u64,
    suggestions: Vec<String>,
}

impl From<Insight> for InsightView {
    fn from(insight: Insight) -> Self {
        let (insight_type_str, suggestions) = match &insight.insight_type {
            InsightType::LatencyAnomaly { baseline_ms, current_ms, change_percent } => (
                "latency_anomaly".to_string(),
                vec![
                    format!("Latency changed by {:.1}% ({:.0}ms → {:.0}ms)", change_percent, baseline_ms, current_ms),
                    "Consider: caching, connection pooling, or reducing payload size".to_string(),
                ],
            ),
            InsightType::ErrorRateAnomaly { baseline_rate, current_rate, change_percent } => (
                "error_rate_anomaly".to_string(),
                vec![
                    format!("Error rate changed by {:.1}% ({:.2}% → {:.2}%)", change_percent, baseline_rate * 100.0, current_rate * 100.0),
                    "Review recent deployments or configuration changes".to_string(),
                ],
            ),
            InsightType::CostAnomaly { baseline_cost, current_cost, change_percent } => (
                "cost_anomaly".to_string(),
                vec![
                    format!("Cost changed by {:.1}% (${:.4} → ${:.4})", change_percent, baseline_cost, current_cost),
                    "Consider: prompt optimization or model switching".to_string(),
                ],
            ),
            InsightType::TokenUsageSpike { baseline_tokens, current_tokens, change_percent } => (
                "token_usage_spike".to_string(),
                vec![
                    format!("Token usage changed by {:.1}% ({} → {})", change_percent, baseline_tokens, current_tokens),
                    "Review prompt lengths and context windows".to_string(),
                ],
            ),
            InsightType::SemanticDrift { drift_score, .. } => (
                "semantic_drift".to_string(),
                vec![format!("Semantic drift score: {:.2}", drift_score)],
            ),
            InsightType::FailurePattern { pattern_description, occurrence_count } => (
                "failure_pattern".to_string(),
                vec![format!("Pattern '{}' occurred {} times", pattern_description, occurrence_count)],
            ),
            InsightType::PerformanceRegression { metric, regression_percent } => (
                "performance_regression".to_string(),
                vec![format!("{} regressed by {:.1}%", metric, regression_percent)],
            ),
            InsightType::TrafficAnomaly { expected_count, actual_count } => (
                "traffic_anomaly".to_string(),
                vec![format!("Expected ~{} requests, got {}", expected_count, actual_count)],
            ),
        };

        InsightView {
            id: insight.id,
            insight_type: insight_type_str,
            severity: format!("{:?}", insight.severity).to_lowercase(),
            confidence: insight.confidence,
            summary: insight.summary,
            description: insight.description,
            related_trace_ids: insight.related_ids.iter().map(|id| format!("{:#x}", id)).collect(),
            metadata: serde_json::to_value(&insight.metadata).unwrap_or_default(),
            generated_at: insight.generated_at,
            suggestions,
        }
    }
}

#[derive(Debug, Serialize)]
struct InsightsResponse {
    insights: Vec<InsightView>,
    total_count: usize,
    window_seconds: u64,
    generated_at: u64,
}

/// GET /api/v1/insights - Get insights for time window (project-scoped)
async fn get_insights_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(query): axum::extract::Query<InsightsQuery>,
) -> impl IntoResponse {
    let config = InsightConfig::default();
    let baseline_multiplier = config.baseline_multiplier as u64;
    let engine = InsightEngine::new(config);
    let project_id = query.project_id;

    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let recent_start = now_us.saturating_sub(query.window_seconds * 1_000_000);
    let baseline_start = recent_start.saturating_sub(query.window_seconds * 1_000_000 * baseline_multiplier);

    // Use query_filtered for efficient project-scoped query
    let recent_edges = state.tauri_state.db
        .query_filtered(recent_start, now_us, None, Some(project_id))
        .unwrap_or_default();
    
    let baseline_edges = state.tauri_state.db
        .query_filtered(baseline_start, recent_start, None, Some(project_id))
        .unwrap_or_default();

    let insights = engine.generate_insights_from_edges(&recent_edges, &baseline_edges);

    let filtered: Vec<InsightView> = insights
        .into_iter()
        .take(query.limit)
        .map(InsightView::from)
        .collect();

    let count = filtered.len();

    Json(InsightsResponse {
        insights: filtered,
        total_count: count,
        window_seconds: query.window_seconds,
        generated_at: now_us,
    }).into_response()
}

#[derive(Debug, Serialize)]
struct InsightsSummary {
    total_insights: usize,
    critical_count: usize,
    high_count: usize,
    by_severity: std::collections::HashMap<String, usize>,
    by_type: std::collections::HashMap<String, usize>,
    health_score: u8,
    top_insights: Vec<InsightView>,
}

#[derive(Debug, Deserialize)]
struct InsightsSummaryQuery {
    project_id: u16,
}

/// GET /api/v1/insights/summary - Get insights summary (project-scoped)
async fn get_insights_summary_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(query): axum::extract::Query<InsightsSummaryQuery>,
) -> impl IntoResponse {
    let config = InsightConfig::default();
    let engine = InsightEngine::new(config);
    let project_id = query.project_id;

    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let recent_start = now_us.saturating_sub(3600 * 1_000_000); // 1 hour
    let baseline_start = recent_start.saturating_sub(7 * 3600 * 1_000_000); // 7 hours before

    // Use project-filtered query (efficient storage-level filtering)
    let recent_edges: Vec<_> = state.tauri_state.db
        .query_filtered(recent_start, now_us, None, Some(project_id))
        .unwrap_or_default();
    
    let baseline_edges: Vec<_> = state.tauri_state.db
        .query_filtered(baseline_start, recent_start, None, Some(project_id))
        .unwrap_or_default();

    let insights = engine.generate_insights_from_edges(&recent_edges, &baseline_edges);

    let mut by_severity = std::collections::HashMap::new();
    let mut by_type = std::collections::HashMap::new();

    for insight in &insights {
        *by_severity.entry(format!("{:?}", insight.severity).to_lowercase()).or_insert(0) += 1;
        let type_name = match &insight.insight_type {
            InsightType::LatencyAnomaly { .. } => "latency_anomaly",
            InsightType::ErrorRateAnomaly { .. } => "error_rate_anomaly",
            InsightType::CostAnomaly { .. } => "cost_anomaly",
            InsightType::TokenUsageSpike { .. } => "token_usage_spike",
            InsightType::SemanticDrift { .. } => "semantic_drift",
            InsightType::FailurePattern { .. } => "failure_pattern",
            InsightType::PerformanceRegression { .. } => "performance_regression",
            InsightType::TrafficAnomaly { .. } => "traffic_anomaly",
        };
        *by_type.entry(type_name.to_string()).or_insert(0) += 1;
    }

    let critical_count = insights.iter().filter(|i| i.severity == Severity::Critical).count();
    let high_count = insights.iter().filter(|i| i.severity == Severity::High).count();

    // Calculate health score
    let mut penalty = 0usize;
    for insight in &insights {
        penalty += match insight.severity {
            Severity::Critical => 30,
            Severity::High => 15,
            Severity::Medium => 5,
            Severity::Low => 2,
            Severity::Info => 0,
        };
    }
    let health_score = 100u8.saturating_sub(penalty.min(100) as u8);

    let top_insights: Vec<InsightView> = insights
        .into_iter()
        .filter(|i| i.severity >= Severity::Medium)
        .take(5)
        .map(InsightView::from)
        .collect();

    Json(InsightsSummary {
        total_insights: by_severity.values().sum(),
        critical_count,
        high_count,
        by_severity,
        by_type,
        health_score,
        top_insights,
    }).into_response()
}

// ============================================================================
// Evals Endpoints
// ============================================================================

#[derive(Debug, Deserialize)]
struct CreateDatasetRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    test_cases: Vec<TestCaseInput>,
}

#[derive(Debug, Deserialize)]
struct TestCaseInput {
    input: String,
    #[serde(default)]
    expected_output: Option<String>,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct DatasetResponse {
    id: String,
    name: String,
    description: String,
    test_case_count: usize,
    created_at: u64,
    updated_at: u64,
}

#[derive(Debug, Serialize)]
struct DatasetDetailResponse {
    id: String,
    name: String,
    description: String,
    test_cases: Vec<TestCaseOutput>,
    created_at: u64,
    updated_at: u64,
}

#[derive(Debug, Serialize)]
struct TestCaseOutput {
    id: String,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_output: Option<String>,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct CreateDatasetResponse {
    dataset_id: String,
    name: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct AddExamplesRequest {
    examples: Vec<ExampleInput>,
}

#[derive(Debug, Deserialize)]
struct ExampleInput {
    #[serde(default)]
    example_id: Option<String>,
    input: String,
    #[serde(default)]
    expected_output: Option<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct AddExamplesResponse {
    success: bool,
    added_count: usize,
}

/// Request to create an eval run
#[derive(Debug, Deserialize)]
struct CreateEvalRunRequest {
    dataset_id: String,
    name: String,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    config: HashMap<String, String>,
}

/// Response from creating an eval run
#[derive(Debug, Serialize)]
struct CreateEvalRunResponse {
    run_id: String,
    dataset_id: String,
    name: String,
    status: String,
}

/// Request to add a result to an eval run
#[derive(Debug, Deserialize)]
struct AddRunResultRequest {
    test_case_id: String,
    #[serde(default)]
    trace_id: Option<String>,
    passed: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    eval_metrics: HashMap<String, f64>,
}

/// Response from adding a run result
#[derive(Debug, Serialize)]
struct AddRunResultResponse {
    success: bool,
    total_results: usize,
}

/// Request to complete an eval run
#[derive(Debug, Deserialize)]
struct CompleteEvalRunRequest {
    #[serde(default)]
    status: Option<String>, // "completed", "failed", or "stopped"
}

fn generate_eval_id() -> u128 {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();
    // Generate pseudo-random component using timestamp nanoseconds
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as u128;
    timestamp ^ (nanos << 64)
}

fn current_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn parse_dataset_id(id_str: &str) -> Result<u128, String> {
    let id_str = id_str.trim_start_matches("0x");
    u128::from_str_radix(id_str, 16).map_err(|e| format!("Invalid dataset ID: {}", e))
}

/// POST /api/v1/evals/datasets - Create dataset
async fn create_dataset_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<CreateDatasetRequest>,
) -> impl IntoResponse {
    let dataset_id = generate_eval_id();
    let timestamp = current_timestamp_us();
    let description = req.description.unwrap_or_default();

    let mut dataset = EvalDataset::new(dataset_id, req.name.clone(), description.clone(), timestamp);

    for tc_input in req.test_cases {
        let tc_id = generate_eval_id();
        let mut test_case = TestCase::new(tc_id, tc_input.input);
        test_case.expected_output = tc_input.expected_output;
        test_case.metadata = tc_input.metadata;
        dataset.add_test_case(test_case);
    }

    if let Err(e) = state.tauri_state.db.store_eval_dataset(dataset) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ).into_response();
    }

    (
        StatusCode::CREATED,
        Json(CreateDatasetResponse {
            dataset_id: format!("0x{:x}", dataset_id),
            name: req.name,
            description,
        }),
    ).into_response()
}

/// GET /api/v1/evals/datasets - List datasets
async fn list_datasets_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    match state.tauri_state.db.list_eval_datasets() {
        Ok(datasets) => {
            let dataset_responses: Vec<DatasetResponse> = datasets
                .iter()
                .map(|d| DatasetResponse {
                    id: format!("0x{:x}", d.id),
                    name: d.name.clone(),
                    description: d.description.clone(),
                    test_case_count: d.test_case_count(),
                    created_at: d.created_at,
                    updated_at: d.updated_at,
                })
                .collect();

            Json(serde_json::json!({
                "datasets": dataset_responses,
                "total": dataset_responses.len()
            })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ).into_response(),
    }
}

/// GET /api/v1/evals/datasets/:id - Get dataset
async fn get_dataset_handler(
    AxumState(state): AxumState<ServerState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let dataset_id = match parse_dataset_id(&id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    match state.tauri_state.db.get_eval_dataset(dataset_id) {
        Ok(Some(dataset)) => {
            let response = DatasetDetailResponse {
                id: format!("0x{:x}", dataset.id),
                name: dataset.name.clone(),
                description: dataset.description.clone(),
                test_cases: dataset.test_cases.iter().map(|tc| TestCaseOutput {
                    id: format!("0x{:x}", tc.id),
                    input: tc.input.clone(),
                    expected_output: tc.expected_output.clone(),
                    metadata: tc.metadata.clone(),
                }).collect(),
                created_at: dataset.created_at,
                updated_at: dataset.updated_at,
            };
            Json(response).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Dataset not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// DELETE /api/v1/evals/datasets/:id - Delete dataset
async fn delete_dataset_handler(
    AxumState(state): AxumState<ServerState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let dataset_id = match parse_dataset_id(&id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    match state.tauri_state.db.delete_eval_dataset(dataset_id) {
        Ok(true) => Json(serde_json::json!({"success": true, "message": "Dataset deleted"})).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Dataset not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/v1/evals/datasets/:id/examples - Add examples to dataset
async fn add_examples_handler(
    AxumState(state): AxumState<ServerState>,
    Path(id): Path<String>,
    Json(req): Json<AddExamplesRequest>,
) -> impl IntoResponse {
    let dataset_id = match parse_dataset_id(&id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    // Get existing dataset
    let mut dataset = match state.tauri_state.db.get_eval_dataset(dataset_id) {
        Ok(Some(d)) => d,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Dataset not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    let added_count = req.examples.len();

    // Add new test cases
    for example in req.examples {
        let tc_id = generate_eval_id();
        let mut test_case = TestCase::new(tc_id, example.input);
        test_case.expected_output = example.expected_output;
        if let Some(context) = example.context {
            test_case.metadata.insert("context".to_string(), context);
        }
        for (k, v) in example.metadata {
            test_case.metadata.insert(k, v);
        }
        dataset.add_test_case(test_case);
    }

    // Update timestamp
    dataset.updated_at = current_timestamp_us();

    // Save updated dataset
    if let Err(e) = state.tauri_state.db.store_eval_dataset(dataset) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    Json(AddExamplesResponse {
        success: true,
        added_count,
    }).into_response()
}

/// DELETE /api/v1/evals/datasets/:dataset_id/examples/:example_id - Delete example from dataset
async fn delete_example_handler(
    AxumState(state): AxumState<ServerState>,
    Path((dataset_id_str, example_id_str)): Path<(String, String)>,
) -> impl IntoResponse {
    let dataset_id = match parse_dataset_id(&dataset_id_str) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    // Get the dataset
    let mut dataset = match state.tauri_state.db.get_eval_dataset(dataset_id) {
        Ok(Some(d)) => d,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Dataset not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    // Find and remove the example
    let original_len = dataset.test_cases.len();
    dataset.test_cases.retain(|tc| {
        // Try both hex format and numeric format for ID comparison
        let tc_id_str = format!("{:#x}", tc.id);
        let tc_id_numeric = tc.id.to_string();
        tc_id_str != example_id_str && tc_id_numeric != example_id_str && format!("0x{:x}", tc.id) != example_id_str
    });

    if dataset.test_cases.len() == original_len {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Example not found"}))).into_response();
    }

    dataset.updated_at = current_timestamp_us();

    // Save updated dataset
    if let Err(e) = state.tauri_state.db.store_eval_dataset(dataset) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    Json(serde_json::json!({"success": true, "message": "Example deleted"})).into_response()
}

/// JSON-safe representation of EvalRun (u128 -> hex string)
fn eval_run_to_json(run: &EvalRun) -> serde_json::Value {
    serde_json::json!({
        "id": format!("{:032x}", run.id),
        "dataset_id": format!("{:032x}", run.dataset_id),
        "name": run.name,
        "agent_id": run.agent_id,
        "model": run.model,
        "results": run.results.iter().map(|r| serde_json::json!({
            "test_case_id": format!("{:032x}", r.test_case_id),
            "trace_id": r.trace_id.map(|id| format!("{:032x}", id)),
            "eval_metrics": r.eval_metrics,
            "passed": r.passed,
            "error": r.error,
            "timestamp_us": r.timestamp_us,
        })).collect::<Vec<_>>(),
        "started_at": run.started_at,
        "completed_at": run.completed_at,
        "status": run.status,
        "config": run.config,
        "total_cost": run.total_cost,
        "token_budget": run.token_budget,
        "cost_breakdown": run.cost_breakdown,
    })
}

/// GET /api/v1/evals/runs - List eval runs
async fn list_eval_runs_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    match state.tauri_state.db.list_eval_runs(None) {
        Ok(runs) => {
            let json_runs: Vec<_> = runs.iter().map(eval_run_to_json).collect();
            Json(serde_json::json!({"runs": json_runs})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/v1/evals/runs - Create eval run
async fn create_eval_run_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<CreateEvalRunRequest>,
) -> impl IntoResponse {
    // Parse dataset ID
    let dataset_id = match parse_dataset_id(&req.dataset_id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    // Verify dataset exists and get it
    let dataset = match state.tauri_state.db.get_eval_dataset(dataset_id) {
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Dataset not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        Ok(Some(dataset)) => dataset,
    };

    let run_id = generate_eval_id();
    let timestamp = current_timestamp_us();
    let agent_id = req.agent_id.unwrap_or_else(|| "default-agent".to_string());
    let model = req.model.clone().unwrap_or_else(|| "gpt-4o-mini".to_string());

    let mut run = EvalRun::new(run_id, dataset_id, req.name.clone(), agent_id.clone(), model.clone(), timestamp);
    
    // Add config
    for (k, v) in req.config.clone() {
        run.config.insert(k, v);
    }

    // Store the run initially
    if let Err(e) = state.tauri_state.db.store_eval_run(run.clone()) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    // Spawn background task to execute evaluations
    let db = state.tauri_state.db.clone();
    let llm_client = state.tauri_state.llm_client.clone();
    let test_cases = dataset.test_cases.clone();
    let model = req.model.clone().unwrap_or_else(|| "default".to_string());
    
    tokio::spawn(async move {
        execute_eval_run(db, llm_client, run_id, test_cases, model).await;
    });

    Json(CreateEvalRunResponse {
        run_id: format!("{:032x}", run_id),
        dataset_id: req.dataset_id,
        name: req.name,
        status: "running".to_string(),
    }).into_response()
}

/// Execute evaluation run in background
async fn execute_eval_run(
    db: Arc<agentreplay_query::Agentreplay>,
    llm_client: Arc<tokio::sync::RwLock<crate::llm::LLMClient>>,
    run_id: u128,
    test_cases: Vec<TestCase>,
    model_override: String,
) {
    let mut passed = 0;
    let mut failed = 0;
    let mut results: Vec<RunResult> = Vec::new();
    
    // Check if LLM is configured
    let client_guard = llm_client.read().await;
    let is_configured = client_guard.is_configured();
    let model = if model_override == "default" {
        client_guard.get_default_model().to_string()
    } else {
        model_override
    };
    drop(client_guard);
    
    for test_case in &test_cases {
        let input = &test_case.input;
        let expected = test_case.expected_output.as_deref().unwrap_or("");
        
        // Try to use LLM if configured, otherwise fall back to heuristics
        let eval_metrics = if is_configured && !expected.is_empty() {
            match call_llm_for_eval_run(&llm_client, input, expected, &model).await {
                Ok(metrics) => metrics,
                Err(e) => {
                    tracing::warn!("LLM eval failed for test case {}, using heuristics: {}", 
                        format!("{:032x}", test_case.id), e);
                    compute_heuristic_metrics(input, expected)
                }
            }
        } else {
            compute_heuristic_metrics(input, expected)
        };
        
        let overall_score = eval_metrics.get("overall").copied().unwrap_or(0.0);
        let pass_threshold = 0.7;
        let test_passed = overall_score >= pass_threshold;
        
        if test_passed {
            passed += 1;
        } else {
            failed += 1;
        }
        
        let result = RunResult {
            test_case_id: test_case.id,
            trial_id: 0,
            seed: None,
            trace_id: None,
            eval_metrics,
            grader_results: Vec::new(),
            overall: None,
            passed: test_passed,
            error: if test_passed { None } else { Some("Score below threshold".to_string()) },
            timestamp_us: current_timestamp_us(),
            cost_usd: None,
            latency_ms: None,
        };
        
        results.push(result);
    }
    
    // Update run with results
    if let Ok(Some(mut run)) = db.get_eval_run(run_id) {
        for result in results {
            run.add_result(result);
        }
        run.complete(current_timestamp_us());
        
        let _ = db.store_eval_run(run);
        tracing::info!("Eval run {} completed: {} passed, {} failed", 
            format!("{:032x}", run_id), passed, failed);
    }
}

/// Extract JSON from various LLM response formats
fn extract_json_from_response(content: &str) -> Result<String, String> {
    // First, try to find JSON in markdown code blocks
    // Handle ```json ... ``` format
    if let Some(start) = content.find("```json") {
        let after_marker = &content[start + 7..];
        if let Some(end) = after_marker.find("```") {
            let json_content = after_marker[..end].trim();
            if json_content.contains('{') {
                return Ok(json_content.to_string());
            }
        }
    }
    
    // Handle ``` ... ``` format (without json specifier)
    if let Some(start) = content.find("```") {
        let after_marker = &content[start + 3..];
        if let Some(end) = after_marker.find("```") {
            let block_content = after_marker[..end].trim();
            // Skip language identifier if present
            let json_content = if let Some(newline_pos) = block_content.find('\n') {
                let first_line = &block_content[..newline_pos];
                if first_line.chars().all(|c| c.is_alphabetic() || c.is_whitespace()) && !first_line.contains('{') {
                    block_content[newline_pos..].trim()
                } else {
                    block_content
                }
            } else {
                block_content
            };
            if json_content.contains('{') {
                return Ok(json_content.to_string());
            }
        }
    }
    
    // Try to find raw JSON object
    if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            if end > start {
                return Ok(content[start..=end].to_string());
            }
        }
    }
    
    Err(format!("No JSON found in response: {}", 
        if content.len() > 200 { &content[..200] } else { content }))
}

/// Call LLM for evaluation run scoring
async fn call_llm_for_eval_run(
    llm_client: &Arc<tokio::sync::RwLock<crate::llm::LLMClient>>,
    input: &str,
    expected: &str,
    _model: &str, // Model is determined by the purpose routing
) -> Result<HashMap<String, f64>, String> {
    let client = llm_client.read().await;
    
    // Check if configured for eval purpose
    if !client.is_configured_for(crate::llm::LLMPurpose::Eval) {
        return Err("No LLM configured for eval purpose".to_string());
    }
    
    let prompt = format!(r#"Score this AI response on each criterion from 1-5.

INPUT: {input}

OUTPUT: {expected}

CRITERIA:
- coherence: Is the response logically structured and well-organized?
- relevance: Does the response address the input appropriately?
- fluency: Is the response grammatically correct and natural?
- helpfulness: Does the response provide useful information?

Return ONLY valid JSON with integer scores, no explanation:
{{"coherence": 4, "relevance": 5, "fluency": 4, "helpfulness": 4}}"#);
    
    let messages = vec![
        crate::llm::ChatMessage {
            role: "system".to_string(),
            content: "You are a JSON-only response bot. You ONLY output valid JSON, no text before or after. Never use markdown code blocks.".to_string(),
        },
        crate::llm::ChatMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];
    
    // Use purpose-based routing to use the eval-tagged provider
    let response = client.complete_for_purpose(
        crate::llm::LLMPurpose::Eval,
        messages,
        Some(0.1),
        Some(256),
    ).await.map_err(|e| e.to_string())?;
    
    // Log the raw response for debugging
    tracing::info!("Eval LLM response: {:?}", response.content);
    
    // Parse JSON response - handle various formats
    let content = response.content.trim();
    
    // Try to extract JSON from various formats
    let json_str = extract_json_from_response(content)?;
    
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse: {}", e))?;
    
    let mut metrics = HashMap::new();
    for criterion in &["coherence", "relevance", "fluency", "helpfulness"] {
        let score = parsed.get(*criterion)
            .and_then(|v| v.as_f64())
            .unwrap_or(3.0);
        // Normalize 1-5 to 0-1
        metrics.insert(criterion.to_string(), (score - 1.0) / 4.0);
    }
    
    // Calculate overall
    let overall: f64 = metrics.values().sum::<f64>() / metrics.len() as f64;
    metrics.insert("overall".to_string(), overall);
    
    Ok(metrics)
}

/// Compute heuristic metrics (fallback when LLM unavailable)
fn compute_heuristic_metrics(input: &str, output: &str) -> HashMap<String, f64> {
    let mut metrics = HashMap::new();
    metrics.insert("coherence".to_string(), compute_heuristic_score(input, output, "coherence"));
    metrics.insert("relevance".to_string(), compute_heuristic_score(input, output, "relevance"));
    metrics.insert("fluency".to_string(), compute_heuristic_score(input, output, "fluency"));
    metrics.insert("helpfulness".to_string(), compute_heuristic_score(input, output, "helpfulness"));
    
    let overall: f64 = metrics.values().sum::<f64>() / metrics.len() as f64;
    metrics.insert("overall".to_string(), overall);
    
    metrics
}

/// GET /api/v1/evals/runs/:id - Get eval run
async fn get_eval_run_handler(
    AxumState(state): AxumState<ServerState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match parse_dataset_id(&id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    match state.tauri_state.db.get_eval_run(run_id) {
        Ok(Some(run)) => Json(eval_run_to_json(&run)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Run not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// DELETE /api/v1/evals/runs/:id - Delete eval run
async fn delete_eval_run_handler(
    AxumState(state): AxumState<ServerState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match parse_dataset_id(&id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    match state.tauri_state.db.delete_eval_run(run_id) {
        Ok(true) => Json(serde_json::json!({"success": true, "message": "Run deleted"})).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Run not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/v1/evals/runs/:id/results - Add a result to an eval run
async fn add_run_result_handler(
    AxumState(state): AxumState<ServerState>,
    Path(id): Path<String>,
    Json(req): Json<AddRunResultRequest>,
) -> impl IntoResponse {
    let run_id = match parse_dataset_id(&id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    let test_case_id = match parse_dataset_id(&req.test_case_id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Invalid test_case_id: {}", e)}))).into_response(),
    };

    let trace_id = if let Some(tid) = &req.trace_id {
        match parse_dataset_id(tid) {
            Ok(id) => Some(id),
            Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Invalid trace_id: {}", e)}))).into_response(),
        }
    } else {
        None
    };

    let timestamp = current_timestamp_us();

    // Build the result
    let mut result = if req.passed {
        RunResult::success(test_case_id, trace_id.unwrap_or(0), timestamp)
    } else {
        RunResult::failure(test_case_id, req.error.unwrap_or_else(|| "Unknown error".to_string()), timestamp)
    };
    
    // Set trace_id properly for failed cases
    result.trace_id = trace_id;

    // Add eval metrics
    for (k, v) in req.eval_metrics {
        result.eval_metrics.insert(k, v);
    }

    // Update the run with this result
    let total_results = match state.tauri_state.db.get_eval_run(run_id) {
        Ok(Some(mut run)) => {
            run.add_result(result);
            let count = run.results.len();
            if let Err(e) = state.tauri_state.db.store_eval_run(run) {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
            }
            count
        }
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Run not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    Json(AddRunResultResponse {
        success: true,
        total_results,
    }).into_response()
}

/// POST /api/v1/evals/runs/:id/complete - Mark an eval run as completed
async fn complete_eval_run_handler(
    AxumState(state): AxumState<ServerState>,
    Path(id): Path<String>,
    Json(req): Json<CompleteEvalRunRequest>,
) -> impl IntoResponse {
    let run_id = match parse_dataset_id(&id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    };

    let timestamp = current_timestamp_us();
    let status = req.status.unwrap_or_else(|| "completed".to_string());

    match state.tauri_state.db.get_eval_run(run_id) {
        Ok(Some(mut run)) => {
            match status.as_str() {
                "completed" => run.complete(timestamp),
                "failed" => run.fail(timestamp),
                "stopped" => run.stop(timestamp),
                _ => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid status. Use 'completed', 'failed', or 'stopped'"}))).into_response(),
            }
            
            if let Err(e) = state.tauri_state.db.store_eval_run(run.clone()) {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
            }

            Json(serde_json::json!({
                "success": true,
                "run_id": format!("{:032x}", run_id),
                "status": run.status.as_str(),
                "passed_count": run.passed_count(),
                "failed_count": run.failed_count(),
                "pass_rate": run.pass_rate()
            })).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Run not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// Request to compare two eval runs
#[derive(Debug, Deserialize)]
struct CompareEvalRunsRequest {
    baseline_run_id: String,
    treatment_run_id: String,
    #[serde(default)]
    metric_direction: HashMap<String, bool>, // true = higher is better
}

/// POST /api/v1/evals/compare - Compare two eval runs statistically
async fn compare_eval_runs_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<CompareEvalRunsRequest>,
) -> impl IntoResponse {
    // Parse run IDs
    let baseline_id = match parse_dataset_id(&req.baseline_run_id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Invalid baseline_run_id: {}", e)}))).into_response(),
    };
    
    let treatment_id = match parse_dataset_id(&req.treatment_run_id) {
        Ok(id) => id,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Invalid treatment_run_id: {}", e)}))).into_response(),
    };

    // Fetch both runs
    let baseline_run = match state.tauri_state.db.get_eval_run(baseline_id) {
        Ok(Some(run)) => run,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Baseline run not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    let treatment_run = match state.tauri_state.db.get_eval_run(treatment_id) {
        Ok(Some(run)) => run,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Treatment run not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    // Extract metric values from results
    let mut baseline_metrics: HashMap<String, Vec<f64>> = HashMap::new();
    let mut treatment_metrics: HashMap<String, Vec<f64>> = HashMap::new();

    // Collect metrics from baseline run
    for result in &baseline_run.results {
        for (metric_name, value) in &result.eval_metrics {
            baseline_metrics.entry(metric_name.clone()).or_default().push(*value);
        }
        // Also add pass/fail as a binary metric
        baseline_metrics.entry("passed".to_string()).or_default().push(if result.passed { 1.0 } else { 0.0 });
    }

    // Collect metrics from treatment run
    for result in &treatment_run.results {
        for (metric_name, value) in &result.eval_metrics {
            treatment_metrics.entry(metric_name.clone()).or_default().push(*value);
        }
        treatment_metrics.entry("passed".to_string()).or_default().push(if result.passed { 1.0 } else { 0.0 });
    }

    // Default metric directions (higher is better unless specified)
    let mut metric_direction = req.metric_direction.clone();
    // Common metrics where LOWER is better
    metric_direction.entry("hallucination".to_string()).or_insert(false);
    metric_direction.entry("latency_ms".to_string()).or_insert(false);
    metric_direction.entry("cost".to_string()).or_insert(false);
    metric_direction.entry("error_rate".to_string()).or_insert(false);
    // Common metrics where HIGHER is better
    metric_direction.entry("accuracy".to_string()).or_insert(true);
    metric_direction.entry("passed".to_string()).or_insert(true);
    metric_direction.entry("groundedness".to_string()).or_insert(true);
    metric_direction.entry("relevance".to_string()).or_insert(true);
    metric_direction.entry("faithfulness".to_string()).or_insert(true);
    metric_direction.entry("ragas_score".to_string()).or_insert(true);

    // Perform statistical comparison
    let mut comparisons = Vec::new();
    let mut significant_improvements = 0;
    let mut significant_regressions = 0;
    let mut no_significant_change = 0;

    for (metric_name, baseline_values) in &baseline_metrics {
        if let Some(treatment_values) = treatment_metrics.get(metric_name) {
            if baseline_values.is_empty() || treatment_values.is_empty() {
                continue;
            }

            let higher_is_better = metric_direction.get(metric_name).copied().unwrap_or(true);
            
            // Calculate statistics
            let n1 = baseline_values.len() as f64;
            let n2 = treatment_values.len() as f64;
            let m1: f64 = baseline_values.iter().sum::<f64>() / n1;
            let m2: f64 = treatment_values.iter().sum::<f64>() / n2;
            let s1 = (baseline_values.iter().map(|x| (x - m1).powi(2)).sum::<f64>() / (n1 - 1.0).max(1.0)).sqrt();
            let s2 = (treatment_values.iter().map(|x| (x - m2).powi(2)).sum::<f64>() / (n2 - 1.0).max(1.0)).sqrt();

            // Welch's t-test
            let se = ((s1 * s1 / n1) + (s2 * s2 / n2)).sqrt();
            let t_statistic = if se > 0.0 { (m2 - m1) / se } else { 0.0 };
            
            // Degrees of freedom (Welch-Satterthwaite)
            let v1 = s1 * s1 / n1;
            let v2 = s2 * s2 / n2;
            let df = if v1 + v2 > 0.0 {
                ((v1 + v2).powi(2)) / ((v1 * v1 / (n1 - 1.0).max(1.0)) + (v2 * v2 / (n2 - 1.0).max(1.0)))
            } else {
                n1 + n2 - 2.0
            };

            // Approximate p-value (normal approximation for large df)
            let p_value = 2.0 * (1.0 - normal_cdf(t_statistic.abs()));

            // Cohen's d
            let pooled_std = (((n1 - 1.0) * s1 * s1 + (n2 - 1.0) * s2 * s2) / (n1 + n2 - 2.0).max(1.0)).sqrt();
            let cohens_d = if pooled_std > 0.0 { (m2 - m1) / pooled_std } else { 0.0 };

            let is_significant = p_value < 0.05;
            let diff = m2 - m1;
            let percent_change = if m1 != 0.0 { (diff / m1) * 100.0 } else { 0.0 };

            let effect_size = if cohens_d.abs() < 0.2 { "negligible" }
                else if cohens_d.abs() < 0.5 { "small" }
                else if cohens_d.abs() < 0.8 { "medium" }
                else { "large" };

            let winner = if !is_significant {
                "tie"
            } else if higher_is_better {
                if diff > 0.0 { "treatment" } else { "baseline" }
            } else {
                if diff < 0.0 { "treatment" } else { "baseline" }
            };

            if is_significant {
                if winner == "treatment" {
                    significant_improvements += 1;
                } else if winner == "baseline" {
                    significant_regressions += 1;
                }
            } else {
                no_significant_change += 1;
            }

            // Sort for percentiles
            let mut sorted_baseline = baseline_values.clone();
            sorted_baseline.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mut sorted_treatment = treatment_values.clone();
            sorted_treatment.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let p50_baseline = sorted_baseline.get((sorted_baseline.len() as f64 * 0.5) as usize).copied().unwrap_or(0.0);
            let p95_baseline = sorted_baseline.get((sorted_baseline.len() as f64 * 0.95) as usize).copied().unwrap_or(0.0);
            let p50_treatment = sorted_treatment.get((sorted_treatment.len() as f64 * 0.5) as usize).copied().unwrap_or(0.0);
            let p95_treatment = sorted_treatment.get((sorted_treatment.len() as f64 * 0.95) as usize).copied().unwrap_or(0.0);

            comparisons.push(serde_json::json!({
                "metric_name": metric_name,
                "baseline": {
                    "mean": m1,
                    "std_dev": s1,
                    "n": baseline_values.len(),
                    "p50": p50_baseline,
                    "p95": p95_baseline
                },
                "treatment": {
                    "mean": m2,
                    "std_dev": s2,
                    "n": treatment_values.len(),
                    "p50": p50_treatment,
                    "p95": p95_treatment
                },
                "difference": diff,
                "percent_change": percent_change,
                "t_statistic": t_statistic,
                "degrees_of_freedom": df,
                "p_value": p_value,
                "cohens_d": cohens_d,
                "effect_size": effect_size,
                "is_significant": is_significant,
                "winner": winner,
                "higher_is_better": higher_is_better
            }));
        }
    }

    // Generate recommendation
    let (action, confidence, explanation) = if comparisons.is_empty() {
        ("need_more_data", 0.0, "No metrics available for comparison.".to_string())
    } else if significant_regressions > 0 && significant_improvements == 0 {
        ("keep_baseline", 0.8, format!(
            "Treatment shows {} significant regression(s) with no improvements. Keep baseline.",
            significant_regressions
        ))
    } else if significant_improvements > 0 && significant_regressions == 0 {
        let conf = if significant_improvements >= 3 { 0.95 } 
            else if significant_improvements >= 2 { 0.85 }
            else { 0.75 };
        ("deploy_treatment", conf, format!(
            "Treatment shows {} significant improvement(s) with no regressions. Deploy recommended.",
            significant_improvements
        ))
    } else if significant_improvements > significant_regressions {
        ("deploy_treatment", 0.6, format!(
            "Treatment shows {} improvements but also {} regressions. Review trade-offs before deploying.",
            significant_improvements, significant_regressions
        ))
    } else if significant_regressions > significant_improvements {
        ("keep_baseline", 0.6, format!(
            "Treatment has {} regressions vs {} improvements. Keep baseline unless improvements are critical.",
            significant_regressions, significant_improvements
        ))
    } else {
        ("inconclusive", 0.5, "No statistically significant differences detected. Consider running with more samples.".to_string())
    };

    Json(serde_json::json!({
        "baseline_run_id": req.baseline_run_id,
        "treatment_run_id": req.treatment_run_id,
        "baseline_run_name": baseline_run.name,
        "treatment_run_name": treatment_run.name,
        "metrics": comparisons,
        "summary": {
            "total_metrics": comparisons.len(),
            "significant_improvements": significant_improvements,
            "significant_regressions": significant_regressions,
            "no_significant_change": no_significant_change
        },
        "recommendation": {
            "action": action,
            "confidence": confidence,
            "explanation": explanation
        }
    })).into_response()
}

/// Normal CDF approximation
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}


/// Error function approximation (Abramowitz and Stegun)
fn erf(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

// ============================================================================
// G-Eval Handler (LLM-as-Judge)
// ============================================================================

/// Request for G-Eval evaluation
#[derive(Debug, Deserialize)]
struct GEvalRequest {
    trace_id: String,
    #[serde(default)]
    criteria: Vec<String>,
    #[serde(default)]
    weights: HashMap<String, f64>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    context: Option<String>,
}

/// Response from G-Eval evaluation
/// Fields match the UI's EvaluationResult interface
#[derive(Debug, Serialize)]
struct GEvalResponse {
    trace_id: String,
    evaluator: String,
    /// Overall score (0.0 to 1.0) - matches UI's `score` field
    score: f64,
    /// Detailed scores per criterion - matches UI's `details` field
    details: HashMap<String, f64>,
    /// Evaluation time in milliseconds - matches UI's `evaluation_time_ms` field
    evaluation_time_ms: u64,
    /// Model used for evaluation - matches UI's `model_used` field
    model_used: String,
    /// Whether the evaluation passed the threshold
    passed: bool,
    /// Threshold for passing
    pass_threshold: f64,
    /// Detailed explanation of scores
    explanation: String,
    /// Confidence level of the evaluation
    confidence: f64,
    /// Estimated cost in USD
    estimated_cost: f64,
}


/// POST /api/v1/evals/geval - Run G-Eval on a trace
async fn run_geval_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<GEvalRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    
    // Parse trace_id - support both decimal and hex formats
    // The UI may send either format depending on how the trace was obtained
    let trace_id_str = req.trace_id.trim();
    let trace_id = if trace_id_str.starts_with("0x") || trace_id_str.starts_with("0X") {
        // Explicitly hex format
        let hex_str = trace_id_str.trim_start_matches("0x").trim_start_matches("0X");
        match u128::from_str_radix(hex_str, 16) {
            Ok(id) => id,
            Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Invalid hex trace_id: {}", e)
            }))).into_response(),
        }
    } else {
        // Try decimal first (more common from UI), then hex
        trace_id_str.parse::<u128>().unwrap_or_else(|_| {
            u128::from_str_radix(trace_id_str, 16).unwrap_or(0)
        })
    };
    
    if trace_id == 0 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Invalid trace_id: could not parse as decimal or hex"
        }))).into_response();
    }

    // Get default criteria if none provided
    let criteria = if req.criteria.is_empty() {
        vec!["coherence".to_string(), "relevance".to_string(), "fluency".to_string(), "helpfulness".to_string()]
    } else {
        req.criteria
    };
    
    // Get default weights if none provided
    let weights: HashMap<String, f64> = if req.weights.is_empty() {
        criteria.iter().map(|c| (c.clone(), 1.0)).collect()
    } else {
        req.weights
    };

    // Get input/output - either from request or from trace
    let (input, output, context) = if req.input.is_some() && req.output.is_some() {
        (
            req.input.clone().unwrap_or_default(),
            req.output.clone().unwrap_or_default(),
            req.context.clone().unwrap_or_default(),
        )
    } else {
        // Try to fetch from trace payloads
        extract_trace_io_for_eval(&state, trace_id)
    };

    // Use centralized LLM router with "eval" purpose
    let llm_client = state.tauri_state.llm_client.clone();
    let llm_client_guard = llm_client.read().await;
    
    let (scores, explanations, model_used, estimated_cost, used_llm) = 
        if llm_client_guard.is_configured_for(crate::llm::LLMPurpose::Eval) {
            // Get the model for eval purpose (or fall back to default)
            let model = llm_client_guard.get_model_for(crate::llm::LLMPurpose::Eval)
                .unwrap_or_else(|| llm_client_guard.get_default_model().to_string());
            
            info!("G-EVAL routing: purpose=eval, model={}", model);
            
            // Build the evaluation prompt
            let prompt = build_geval_prompt(&input, &output, &context, &criteria);
            let messages = vec![crate::llm::ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }];
            
            // Use the router to call the appropriate provider with retry logic
            let mut last_error = String::new();
            let mut result = None;
            
            for attempt in 1..=3 {
                match llm_client_guard.complete_for_purpose(
                    crate::llm::LLMPurpose::Eval,
                    messages.clone(),
                    Some(0.1), // Low temperature for evaluation
                    Some(1024),
                ).await {
                    Ok(response) => {
                        // Check if response has actual content
                        if response.content.is_empty() || response.content.contains("[No content in response]") {
                            warn!("G-EVAL attempt {}: Empty response from LLM, retrying...", attempt);
                            last_error = "Empty response from LLM".to_string();
                            tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64)).await;
                            continue;
                        }
                        
                        // Parse the response
                        match parse_geval_response(&response.content, &criteria) {
                            Ok((scores, explanations)) => {
                                let cost = llm_client_guard.cost_per_token_for_model(&model);
                                let estimated_cost = (response.usage.prompt_tokens as f64 * cost.0) 
                                    + (response.usage.completion_tokens as f64 * cost.1);
                                info!("G-EVAL completed successfully with model: {} (attempt {})", model, attempt);
                                result = Some((scores, explanations, model.clone(), estimated_cost, true));
                                break;
                            }
                            Err(e) => {
                                warn!("G-EVAL attempt {}: Failed to parse response: {}", attempt, e);
                                last_error = e;
                                tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64)).await;
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("G-EVAL attempt {}: LLM call failed: {}", attempt, e);
                        last_error = e.to_string();
                        tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64)).await;
                        continue;
                    }
                }
            }
            
            // Use result or fall back to heuristics
            result.unwrap_or_else(|| {
                warn!("All G-EVAL attempts failed ({}), using heuristics", last_error);
                let (scores, explanations) = compute_heuristic_scores(&input, &output, &criteria);
                (scores, explanations, format!("heuristic ({})", last_error), 0.0, false)
            })
        } else {
            // No LLM configured for eval - use heuristics
            info!("No LLM configured for 'eval' purpose, using heuristics");
            let (scores, explanations) = compute_heuristic_scores(&input, &output, &criteria);
            (scores, explanations, "heuristic (no LLM configured)".to_string(), 0.0, false)
        };
    
    drop(llm_client_guard);
    
    // Calculate weighted average
    let mut total_weight = 0.0;
    let mut weighted_sum = 0.0;
    for criterion in &criteria {
        let weight = weights.get(criterion).copied().unwrap_or(1.0);
        let score = scores.get(criterion).copied().unwrap_or(0.0);
        weighted_sum += score * weight;
        total_weight += weight;
    }
    let overall_score = if total_weight > 0.0 { weighted_sum / total_weight } else { 0.0 };
    
    // Store evaluation result
    let eval_metric = agentreplay_core::eval::EvalMetric::new(
        trace_id,
        "geval_score",
        overall_score,
        "g-eval",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64,
    );
    if let Some(metric) = eval_metric {
        let _ = state.tauri_state.db.store_eval_metrics(trace_id, vec![metric]);
    }
    
    // === DATASET FLYWHEEL: Auto-tag traces for fine-tuning ===
    // Tag traces as positive (high score) or negative (low score) candidates
    // This enables automated dataset curation for self-improving systems
    let dataset_candidate_label = if overall_score >= 0.9 {
        Some("positive")
    } else if overall_score <= 0.3 {
        Some("negative")
    } else {
        None // Ignore traces in the middle zone
    };
    
    if let Some(label) = dataset_candidate_label {
        // Store the dataset candidate tag as an additional metric
        let tag_metric = agentreplay_core::eval::EvalMetric::new(
            trace_id,
            "dataset_candidate",
            if label == "positive" { 1.0 } else { 0.0 },
            "g-eval-flywheel",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
        );
        if let Some(metric) = tag_metric {
            let _ = state.tauri_state.db.store_eval_metrics(trace_id, vec![metric]);
        }
        info!(
            "Dataset flywheel: Tagged trace {} as '{}' candidate (score: {:.2})",
            trace_id, label, overall_score
        );
    }
    
    let duration = start.elapsed();
    let pass_threshold = 0.7;
    
    // Add note about evaluation method
    let explanation = if used_llm {
        explanations.join("\n")
    } else {
        format!("⚠️ Using heuristic scoring (configure LLM with 'eval' or 'default' tag in Settings)\n\n{}", explanations.join("\n"))
    };
    
    Json(GEvalResponse {
        trace_id: req.trace_id,
        evaluator: "g-eval".to_string(),
        score: overall_score,
        details: scores,
        passed: overall_score >= pass_threshold,
        pass_threshold,
        explanation,
        confidence: if used_llm { 0.85 } else { 0.5 },
        model_used,
        evaluation_time_ms: duration.as_millis() as u64,
        estimated_cost,
    }).into_response()
}

/// Build G-EVAL prompt - designed for evaluating AI agent/LLM traces
fn build_geval_prompt(input: &str, output: &str, context: &str, criteria: &[String]) -> String {
    let criteria_str = criteria.iter()
        .map(|c| format!("- {}: Score 1-5 (1=terrible, 5=excellent)", c))
        .collect::<Vec<_>>()
        .join("\n");
    
    let context_section = if context.is_empty() { 
        "".to_string() 
    } else { 
        format!("\n## TRACE CONTEXT (spans, tool calls, retrieved documents):\n{}\n", context) 
    };
    
    // Handle empty input/output gracefully
    let input_section = if input.is_empty() {
        "(No user input extracted from trace)".to_string()
    } else {
        input.to_string()
    };
    
    let output_section = if output.is_empty() {
        "(No AI output extracted from trace)".to_string()
    } else {
        output.to_string()
    };
    
    format!(r#"You are an expert AI system evaluator performing G-EVAL (LLM-as-Judge) assessment.

## TASK
Evaluate the quality of an AI agent/LLM interaction based on the provided trace data.

## USER INPUT/QUERY
{input_section}

## AI OUTPUT/RESPONSE  
{output_section}
{context_section}
## EVALUATION CRITERIA
Score each criterion from 1-5:
{criteria_str}

## SCORING GUIDELINES
- 1: Completely fails the criterion
- 2: Major issues, barely meets criterion  
- 3: Acceptable, meets basic requirements
- 4: Good quality, minor improvements possible
- 5: Excellent, fully satisfies criterion

## RESPONSE FORMAT
Respond with ONLY valid JSON in this exact format:
{{
  "scores": {{
    "criterion_name": {{"score": 4, "explanation": "One sentence justification"}}
  }}
}}

Evaluate now:
"#)
}

/// Parse G-EVAL response from LLM
fn parse_geval_response(content: &str, criteria: &[String]) -> Result<(HashMap<String, f64>, Vec<String>), String> {
    info!("Parsing G-EVAL response:\n{}", content);
    
    // Extract JSON from response (handle markdown code blocks)
    let content_cleaned = content
        .replace("```json", "")
        .replace("```", "")
        .trim()
        .to_string();
    
    let json_str = if let Some(start) = content_cleaned.find('{') {
        if let Some(end) = content_cleaned.rfind('}') {
            &content_cleaned[start..=end]
        } else {
            return Err("No valid JSON in response".to_string());
        }
    } else {
        return Err("No JSON found in response".to_string());
    };
    
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse JSON: {} - Content: {}", e, json_str))?;
    
    // Try to find scores object (could be "scores" or at root level)
    let scores_obj = parsed.get("scores")
        .or_else(|| Some(&parsed))
        .ok_or("Missing 'scores' in response")?;
    
    let mut scores = HashMap::new();
    let mut explanations = Vec::new();
    
    for criterion in criteria {
        // Try exact match first, then case-insensitive
        let criterion_lower = criterion.to_lowercase();
        let criterion_data = scores_obj.get(criterion)
            .or_else(|| scores_obj.get(&criterion_lower))
            .or_else(|| {
                // Search case-insensitively through all keys
                if let Some(obj) = scores_obj.as_object() {
                    for (key, value) in obj {
                        if key.to_lowercase() == criterion_lower {
                            return Some(value);
                        }
                    }
                }
                None
            });
        
        if let Some(criterion_data) = criterion_data {
            // Handle both formats: {"score": 4, "explanation": "..."} and just a number
            let score = if criterion_data.is_number() {
                criterion_data.as_f64().unwrap_or(3.0)
            } else {
                criterion_data.get("score")
                    .and_then(|s| s.as_f64())
                    .or_else(|| criterion_data.get("value").and_then(|s| s.as_f64()))
                    .or_else(|| criterion_data.get("rating").and_then(|s| s.as_f64()))
                    .unwrap_or(3.0)
            };
            
            let normalized_score = (score - 1.0) / 4.0; // Normalize 1-5 to 0-1
            scores.insert(criterion.clone(), normalized_score);
            
            let explanation = criterion_data.get("explanation")
                .and_then(|e| e.as_str())
                .or_else(|| criterion_data.get("reason").and_then(|e| e.as_str()))
                .unwrap_or("No explanation");
            explanations.push(format!("{}: {:.0}/5 - {}", criterion, score, explanation));
            info!("Parsed {}: score={}, normalized={}", criterion, score, normalized_score);
        } else {
            warn!("Criterion '{}' not found in response, using default 0.5", criterion);
            scores.insert(criterion.clone(), 0.5);
            explanations.push(format!("{}: Not evaluated", criterion));
        }
    }
    
    Ok((scores, explanations))
}

/// Extract input/output from trace payloads for evaluation
fn extract_trace_io_for_eval(state: &ServerState, trace_id: u128) -> (String, String, String) {
    info!("Extracting I/O for trace: {} (0x{:032x})", trace_id, trace_id);
    
    // Try to get trace edges
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    
    let edges = match state.tauri_state.db.list_traces_in_range(0, now) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to list traces: {}", e);
            return ("".to_string(), "".to_string(), "".to_string());
        }
    };
    
    info!("Found {} total edges, filtering for trace {}", edges.len(), trace_id);
    
    // The trace_id might be:
    // 1. The exact edge_id of a trace
    // 2. A 64-bit value that should match the low or high bits
    // Try multiple matching strategies
    
    let trace_edges: Vec<_> = edges.iter()
        .filter(|e| {
            // Strategy 1: Exact match
            if e.edge_id == trace_id {
                info!("Edge {} matches exactly", e.edge_id);
                return true;
            }
            
            // Strategy 2: If trace_id fits in 64 bits, match as low bits
            // This handles the case where edge_id is stored as u128 but trace_id comes as u64
            if trace_id <= u64::MAX as u128 {
                let trace_id_64 = trace_id as u64;
                let edge_low = (e.edge_id & 0xFFFFFFFFFFFFFFFF) as u64;
                if edge_low == trace_id_64 {
                    info!("Edge {} matches on low 64 bits", e.edge_id);
                    return true;
                }
            }
            
            // Strategy 3: Match on high 64 bits (for spans within a trace)
            let trace_id_high = (trace_id >> 64) as u64;
            if trace_id_high != 0 {
                let edge_high = (e.edge_id >> 64) as u64;
                if edge_high == trace_id_high {
                    info!("Edge {} matches on high 64 bits", e.edge_id);
                    return true;
                }
            }
            
            false
        })
        .collect();
    
    info!("Found {} edges for trace {}", trace_edges.len(), trace_id);
    
    if trace_edges.is_empty() {
        // Debug: show some sample edge_ids to understand the format
        if !edges.is_empty() {
            let sample_edges: Vec<_> = edges.iter().take(5).collect();
            for e in &sample_edges {
                info!("Sample edge_id: {} (0x{:032x})", e.edge_id, e.edge_id);
            }
        }
        warn!("No edges found for trace {} - the trace_id format may not match stored edge_ids", trace_id);
        return ("".to_string(), "".to_string(), "".to_string());
    }
    
    // Collect all edges to check: the matched edges + their children
    // This is important because root traces often don't have payloads,
    // but their child spans (LLM calls) do
    let mut all_edges_to_check: Vec<&agentreplay_core::AgentFlowEdge> = trace_edges.clone();
    
    for edge in &trace_edges {
        // Try get_children from causal index first
        if let Ok(children) = state.tauri_state.db.get_children(edge.edge_id) {
            info!("Found {} children via causal index for edge {}", children.len(), edge.edge_id);
            for child in &children {
                if let Some(child_edge) = edges.iter().find(|e| e.edge_id == child.edge_id) {
                    all_edges_to_check.push(child_edge);
                }
            }
        }
        
        // Also scan all edges for those with causal_parent matching this edge
        // This catches cases where causal index isn't populated
        let children_by_parent: Vec<_> = edges.iter()
            .filter(|e| e.causal_parent == edge.edge_id && e.edge_id != edge.edge_id)
            .collect();
        
        if !children_by_parent.is_empty() {
            info!("Found {} children via causal_parent scan for edge {}", children_by_parent.len(), edge.edge_id);
            for child in children_by_parent {
                if !all_edges_to_check.iter().any(|e| e.edge_id == child.edge_id) {
                    all_edges_to_check.push(child);
                }
            }
        }
        
        // Also check for edges in the same session that might be related
        // Sometimes spans are linked by session_id rather than causal_parent
        if all_edges_to_check.len() == 1 && edge.session_id != 0 {
            let session_edges: Vec<_> = edges.iter()
                .filter(|e| e.session_id == edge.session_id && e.edge_id != edge.edge_id && e.has_payload != 0)
                .take(10) // Limit to avoid too many
                .collect();
            
            if !session_edges.is_empty() {
                info!("Found {} related edges in session {} with payloads", session_edges.len(), edge.session_id);
                for se in session_edges {
                    if !all_edges_to_check.iter().any(|e| e.edge_id == se.edge_id) {
                        all_edges_to_check.push(se);
                    }
                }
            }
        }
    }
    
    info!("Checking {} edges total (including children/session)", all_edges_to_check.len());
    
    // Try to extract input/output from payloads
    let mut input = String::new();
    let mut output = String::new();
    let mut context_parts: Vec<String> = Vec::new();
    
    for edge in &all_edges_to_check {
        info!("Checking edge 0x{:032x}, has_payload flag={}", 
              edge.edge_id, edge.has_payload);
        
        // Always try to get payload - the has_payload flag may not be reliable
        // This matches how the trace detail view works
        if let Ok(Some(payload)) = state.tauri_state.db.get_payload(edge.edge_id) {
            info!("Got payload of {} bytes for edge {}", payload.len(), edge.edge_id);
            
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&payload) {
                // Log available keys for debugging
                if let Some(obj) = json.as_object() {
                    let keys: Vec<_> = obj.keys().collect();
                    info!("Payload keys: {:?}", keys);
                    
                    // Extract input from OpenTelemetry GenAI indexed format (gen_ai.prompt.X.content)
                    if input.is_empty() {
                        // Find the last user message from gen_ai.prompt.X.role/content format
                        let mut user_contents: Vec<(usize, String)> = Vec::new();
                        for i in 0..20 {
                            let role_key = format!("gen_ai.prompt.{}.role", i);
                            let content_key = format!("gen_ai.prompt.{}.content", i);
                            
                            if let Some(role) = obj.get(&role_key).and_then(|v| v.as_str()) {
                                if role == "user" {
                                    if let Some(content) = obj.get(&content_key).and_then(|v| v.as_str()) {
                                        user_contents.push((i, content.to_string()));
                                    }
                                }
                            }
                        }
                        // Use the last user message
                        if let Some((idx, content)) = user_contents.last() {
                            input = content.clone();
                            info!("Found input in gen_ai.prompt.{}.content", idx);
                        }
                    }
                    
                    // Extract output from OpenTelemetry GenAI indexed format (gen_ai.completion.X.content)
                    if output.is_empty() {
                        for i in 0..10 {
                            let role_key = format!("gen_ai.completion.{}.role", i);
                            let content_key = format!("gen_ai.completion.{}.content", i);
                            
                            if let Some(role) = obj.get(&role_key).and_then(|v| v.as_str()) {
                                if role == "assistant" {
                                    if let Some(content) = obj.get(&content_key).and_then(|v| v.as_str()) {
                                        output = content.to_string();
                                        info!("Found output in gen_ai.completion.{}.content", i);
                                        break;
                                    }
                                }
                            } else if let Some(content) = obj.get(&content_key).and_then(|v| v.as_str()) {
                                // Sometimes there's no role, just content
                                output = content.to_string();
                                info!("Found output in gen_ai.completion.{}.content (no role)", i);
                                break;
                            }
                        }
                    }
                }
                
                // Also try non-indexed formats as fallback
                // Look for input in various fields (OpenTelemetry GenAI conventions)
                if input.is_empty() {
                    // Try gen_ai.prompt first (OpenTelemetry GenAI convention)
                    if let Some(v) = json.get("gen_ai.prompt").and_then(|v| v.as_str()) {
                        input = v.to_string();
                        info!("Found input in gen_ai.prompt");
                    } else if let Some(v) = json.get("llm.prompts").and_then(|v| v.as_str()) {
                        input = v.to_string();
                        info!("Found input in llm.prompts");
                    } else if let Some(v) = json.get("input").and_then(|v| v.as_str()) {
                        input = v.to_string();
                        info!("Found input in input");
                    } else if let Some(v) = json.get("prompt").and_then(|v| v.as_str()) {
                        input = v.to_string();
                        info!("Found input in prompt");
                    } else if let Some(v) = json.get("user_input").and_then(|v| v.as_str()) {
                        input = v.to_string();
                        info!("Found input in user_input");
                    } else if let Some(v) = json.get("query").and_then(|v| v.as_str()) {
                        input = v.to_string();
                        info!("Found input in query");
                    } else if let Some(messages) = json.get("messages").and_then(|v| v.as_array()) {
                        // Get last user message from chat messages
                        for msg in messages.iter().rev() {
                            if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                    input = content.to_string();
                                    info!("Found input in messages[user]");
                                    break;
                                }
                            }
                        }
                    } else if let Some(v) = json.get("gen_ai.request.messages") {
                        // Try to parse as array of messages
                        if let Some(messages) = v.as_array() {
                            for msg in messages.iter().rev() {
                                if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                                    if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                        input = content.to_string();
                                        info!("Found input in gen_ai.request.messages");
                                        break;
                                    }
                                }
                            }
                        } else if let Some(s) = v.as_str() {
                            // Maybe it's a JSON string
                            if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(s) {
                                for msg in parsed.iter().rev() {
                                    if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                                        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                            input = content.to_string();
                                            info!("Found input in gen_ai.request.messages (parsed)");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Look for output in various fields
                if output.is_empty() {
                    // Try gen_ai.completion first (OpenTelemetry GenAI convention)
                    if let Some(v) = json.get("gen_ai.completion").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in gen_ai.completion");
                    } else if let Some(v) = json.get("llm.completions").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in llm.completions");
                    } else if let Some(v) = json.get("output").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in output");
                    } else if let Some(v) = json.get("response").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in response");
                    } else if let Some(v) = json.get("completion").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in completion");
                    } else if let Some(v) = json.get("assistant_response").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in assistant_response");
                    } else if let Some(v) = json.get("result").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in result");
                    } else if let Some(v) = json.get("gen_ai.response.text").and_then(|v| v.as_str()) {
                        output = v.to_string();
                        info!("Found output in gen_ai.response.text");
                    } else if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
                        // Get assistant message from choices (OpenAI format)
                        if let Some(first) = choices.first() {
                            if let Some(msg) = first.get("message") {
                                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                    output = content.to_string();
                                    info!("Found output in choices[0].message.content");
                                }
                            } else if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                                output = text.to_string();
                                info!("Found output in choices[0].text");
                            }
                        }
                    }
                }
                
                // Look for context (for RAG evaluations) and trace steps
                if let Some(v) = json.get("context").and_then(|v| v.as_str()) {
                    context_parts.push(format!("[Context] {}", v));
                } else if let Some(v) = json.get("retrieved_context").and_then(|v| v.as_str()) {
                    context_parts.push(format!("[Retrieved] {}", v));
                } else if let Some(docs) = json.get("documents").and_then(|v| v.as_array()) {
                    for doc in docs {
                        if let Some(content) = doc.as_str() {
                            context_parts.push(format!("[Document] {}", content));
                        } else if let Some(content) = doc.get("content").and_then(|c| c.as_str()) {
                            context_parts.push(format!("[Document] {}", content));
                        }
                    }
                }
                
                // Extract tool calls for agentic traces
                if let Some(tool_calls) = json.get("tool_calls").and_then(|v| v.as_array()) {
                    for tool in tool_calls {
                        let tool_name = tool.get("name").or_else(|| tool.get("function").and_then(|f| f.get("name")))
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown_tool");
                        let tool_args = tool.get("arguments").or_else(|| tool.get("function").and_then(|f| f.get("arguments")))
                            .map(|a| a.to_string())
                            .unwrap_or_default();
                        context_parts.push(format!("[Tool Call] {}: {}", tool_name, tool_args));
                    }
                }
                
                // Extract tool results
                if let Some(tool_result) = json.get("tool_result").and_then(|v| v.as_str()) {
                    context_parts.push(format!("[Tool Result] {}", tool_result));
                }
                
                // Build span summary for context - get span name from payload
                let span_name = json.get("span_name")
                    .or_else(|| json.get("name"))
                    .or_else(|| json.get("operation_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                    
                if !span_name.is_empty() {
                    let span_type = if span_name.contains("llm") || span_name.contains("chat") || span_name.contains("completion") {
                        "LLM Call"
                    } else if span_name.contains("tool") || span_name.contains("function") {
                        "Tool"
                    } else if span_name.contains("agent") {
                        "Agent"
                    } else if span_name.contains("retriev") || span_name.contains("search") {
                        "Retrieval"
                    } else {
                        "Span"
                    };
                    
                    let duration_ms = edge.duration_us / 1000;
                    context_parts.push(format!("[{}] {} ({}ms)", span_type, span_name, duration_ms));
                }
            }
        }
    }
    
    let context = context_parts.join("\n");
    
    info!("Extraction complete: input_len={}, output_len={}, context_len={}", 
          input.len(), output.len(), context.len());
    
    if input.is_empty() && output.is_empty() {
        warn!("Could not extract input/output from trace payloads");
    }
    
    (input, output, context)
}

/// Compute heuristic scores for all criteria (fallback when LLM unavailable)
fn compute_heuristic_scores(input: &str, output: &str, criteria: &[String]) -> (HashMap<String, f64>, Vec<String>) {
    let mut scores = HashMap::new();
    let mut explanations = Vec::new();
    
    for criterion in criteria {
        let score = compute_heuristic_score(input, output, criterion);
        scores.insert(criterion.clone(), score);
        explanations.push(format!("{}: {:.2} (heuristic)", criterion, score));
    }
    
    (scores, explanations)
}

/// Compute heuristic score for a criterion when evaluator is not available
fn compute_heuristic_score(input: &str, output: &str, criterion: &str) -> f64 {
    match criterion {
        "coherence" => {
            // Check if output is coherent (has sentences, proper structure)
            let sentences = output.split(|c| c == '.' || c == '!' || c == '?').count();
            let words = output.split_whitespace().count();
            if words == 0 { return 0.0; }
            let avg_sentence_len = words as f64 / sentences.max(1) as f64;
            // Good coherence: 10-25 words per sentence
            if avg_sentence_len >= 10.0 && avg_sentence_len <= 25.0 { 0.8 }
            else if avg_sentence_len >= 5.0 && avg_sentence_len <= 35.0 { 0.6 }
            else { 0.4 }
        }
        "relevance" => {
            // Check word overlap between input and output
            let input_lower = input.to_lowercase();
            let output_lower = output.to_lowercase();
            let input_words: std::collections::HashSet<&str> = input_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();
            let output_words: std::collections::HashSet<&str> = output_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();
            
            if input_words.is_empty() { return 0.5; }
            let overlap = input_words.intersection(&output_words).count();
            let overlap_ratio = overlap as f64 / input_words.len() as f64;
            (overlap_ratio * 0.6 + 0.4).min(1.0)
        }
        "fluency" => {
            // Check basic fluency indicators
            let words = output.split_whitespace().count();
            if words < 3 { return 0.3; }
            if words > 500 { return 0.7; } // Long responses may have issues
            0.75
        }
        "helpfulness" => {
            // Check if response provides actionable content
            let output_lower = output.to_lowercase();
            let helpful_indicators = ["here", "you can", "to do", "steps", "example", "try", "use", "consider"];
            let count = helpful_indicators.iter()
                .filter(|ind| output_lower.contains(*ind))
                .count();
            (count as f64 * 0.15 + 0.4).min(1.0)
        }
        _ => 0.5, // Default middle score for unknown criteria
    }
}

// ============================================================================
// Version Store Handlers (Reference-based, no data duplication)
// ============================================================================

#[derive(Debug, Deserialize)]
struct VersionCommitRequest {
    /// Trace ID to reference
    trace_id: String,
    /// Span ID (optional, for specific LLM call)
    #[serde(default)]
    span_id: Option<String>,
    /// Project ID
    #[serde(default)]
    project_id: u64,
    /// Branch name (defaults to model name or "main")
    #[serde(default)]
    branch: Option<String>,
    /// Commit message
    message: String,
    /// Model name (for display and branch naming)
    #[serde(default)]
    model: Option<String>,
    /// Additional metadata
    #[serde(default)]
    metadata: HashMap<String, serde_json::Value>,
}

/// POST /api/v1/git/commit - Create a new version reference
async fn git_commit_handler(
    AxumState(_state): AxumState<ServerState>,
    Json(_req): Json<VersionCommitRequest>,
) -> impl IntoResponse {
    // ResponseRepository API doesn't match expected VersionStore API
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "success": false,
            "error": "Git-like versioning not yet implemented with ResponseRepository"
        })),
    ).into_response()
}

/// GET /api/v1/git/log - Get version history
async fn git_log_handler(
    AxumState(_state): AxumState<ServerState>,
    axum::extract::Query(_params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": false,
        "error": "Version history not yet implemented",
        "commits": []
    }))
}

/// GET /api/v1/git/show/:ref - Show version details with trace data
async fn git_show_handler(
    AxumState(_state): AxumState<ServerState>,
    Path(_version_id): Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "success": false,
            "error": "Version show not yet implemented"
        })),
    ).into_response()
}

/// GET /api/v1/git/branches - List branches
async fn git_list_branches_handler(
    AxumState(_state): AxumState<ServerState>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": false,
        "error": "Branch listing not yet implemented",
        "branches": [],
        "current": "main"
    }))
}

/// POST /api/v1/git/branches - Branches are auto-created on commit
async fn git_create_branch_handler() -> impl IntoResponse {
    // Branches are auto-created when you commit to them
    Json(serde_json::json!({
        "success": true,
        "message": "Branches are automatically created when you commit to them"
    }))
}

/// DELETE /api/v1/git/branches/:name - Not supported for now
async fn git_delete_branch_handler(
    Path(name): Path<String>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": false,
        "error": format!("Branch deletion not supported: {}", name)
    }))
}

/// GET /api/v1/git/tags - List tags
async fn git_list_tags_handler(
    AxumState(_state): AxumState<ServerState>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": false,
        "error": "Tag listing not yet implemented",
        "tags": []
    }))
}

#[derive(Debug, Deserialize)]
struct CreateTagRequest {
    name: String,
    #[serde(default)]
    version_id: Option<String>,
}

/// POST /api/v1/git/tags - Create a tag
async fn git_create_tag_handler(
    AxumState(_state): AxumState<ServerState>,
    Json(_req): Json<CreateTagRequest>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "success": false,
            "error": "Tag creation not yet implemented"
        })),
    ).into_response()
}

#[derive(Debug, Deserialize)]
struct DiffRequest {
    old_ref: String,
    new_ref: String,
}

/// POST /api/v1/git/diff - Compare two versions
async fn git_diff_handler(
    AxumState(_state): AxumState<ServerState>,
    Json(_req): Json<DiffRequest>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "success": false,
            "error": "Version diff not yet implemented"
        })),
    ).into_response()
}

/// GET /api/v1/git/stats - Get version store statistics
async fn git_stats_handler(
    AxumState(_state): AxumState<ServerState>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": false,
        "error": "Version stats not yet implemented",
        "total_versions": 0,
        "total_branches": 0,
        "total_tags": 0,
        "storage_bytes": 0
    }))
}

// ============================================================================
// Eval Pipeline Handlers (5-Phase Comprehensive Evaluation)
// Real integration with database, payloads, and stored eval runs
// ============================================================================

/// Helper to extract model and cost from payload
fn extract_trace_metadata(db: &Arc<agentreplay_query::Agentreplay>, edge: &AgentFlowEdge) -> (Option<String>, f64) {
    if let Ok(Some(payload_bytes)) = db.get_payload(edge.edge_id) {
        if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
            let model = payload.get("model")
                .or_else(|| payload.get("gen_ai.request.model"))
                .or_else(|| payload.get("llm.model"))
                .and_then(|v| v.as_str())
                .map(String::from);
            
            // Extract cost from payload or calculate from tokens
            let cost = payload.get("cost_usd")
                .or_else(|| payload.get("gen_ai.usage.cost"))
                .and_then(|v| v.as_f64())
                .unwrap_or_else(|| {
                    // Calculate cost based on model and tokens
                    let input_tokens = payload.get("gen_ai.usage.input_tokens")
                        .or_else(|| payload.get("llm.usage.prompt_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(edge.token_count as u64);
                    let output_tokens = payload.get("gen_ai.usage.output_tokens")
                        .or_else(|| payload.get("llm.usage.completion_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    
                    // Pricing per 1K tokens (approximate)
                    let (input_price, output_price) = match model.as_deref() {
                        Some(m) if m.contains("gpt-4o") => (0.0025, 0.01),
                        Some(m) if m.contains("gpt-4-turbo") => (0.01, 0.03),
                        Some(m) if m.contains("gpt-4") => (0.03, 0.06),
                        Some(m) if m.contains("gpt-3.5") => (0.0005, 0.0015),
                        Some(m) if m.contains("claude-3-opus") => (0.015, 0.075),
                        Some(m) if m.contains("claude-3-sonnet") => (0.003, 0.015),
                        Some(m) if m.contains("claude-3-haiku") => (0.00025, 0.00125),
                        _ => (0.001, 0.002), // Default pricing
                    };
                    
                    (input_tokens as f64 * input_price / 1000.0) + (output_tokens as f64 * output_price / 1000.0)
                });
            
            return (model, cost);
        }
    }
    (None, edge.token_count as f64 * 0.000002)
}

/// Request for collecting traces
#[derive(Debug, Deserialize)]
struct CollectTracesRequest {
    project_id: Option<u16>,
    start_time: Option<u64>,
    end_time: Option<u64>,
    status_filter: Option<String>,
    min_duration_ms: Option<u64>,
    max_duration_ms: Option<u64>,
    search_query: Option<String>,
    limit: Option<usize>,
    include_metadata: Option<bool>,
}

/// POST /api/v1/evals/pipeline/collect - Collect traces with filtering
async fn eval_pipeline_collect_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<CollectTracesRequest>,
) -> impl IntoResponse {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    
    let start_time = req.start_time.unwrap_or(now - 86_400_000_000); // 24h default
    let end_time = req.end_time.unwrap_or(now);
    let limit = req.limit.unwrap_or(1000);
    
    // Fetch traces from database
    let db = state.tauri_state.db.clone();
    let edges = match db.list_traces_in_range(start_time, end_time) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to fetch traces: {}", e)
                })),
            ).into_response();
        }
    };
    
    // Group by trace and collect metrics
    let mut trace_map: std::collections::HashMap<u64, Vec<&AgentFlowEdge>> = std::collections::HashMap::new();
    for edge in &edges {
        let trace_id = (edge.edge_id >> 64) as u64;
        trace_map.entry(trace_id).or_default().push(edge);
    }
    
    let mut collected_traces = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;
    let mut total_duration = 0.0f64;
    let mut total_tokens = 0u64;
    let mut total_cost = 0.0f64;
    let mut models_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    
    for (trace_id, trace_edges) in trace_map.iter().take(limit) {
        let first_edge = trace_edges.first().unwrap();
        
        // Calculate total duration for the trace
        let duration_ms: f64 = trace_edges.iter().map(|e| e.duration_us as f64 / 1000.0).sum();
        
        // Apply duration filters
        if let Some(min_dur) = req.min_duration_ms {
            if duration_ms < min_dur as f64 { continue; }
        }
        if let Some(max_dur) = req.max_duration_ms {
            if duration_ms > max_dur as f64 { continue; }
        }
        
        // Check status (span_type 8 = Error)
        let has_error = trace_edges.iter().any(|e| e.span_type == 8);
        let status = if has_error { "error" } else { "success" };
        
        // Apply status filter
        if let Some(ref filter) = req.status_filter {
            if filter != "all" && filter != status { continue; }
        }
        
        // Extract real metadata from payloads
        let mut trace_model: Option<String> = None;
        let mut trace_cost = 0.0f64;
        let tokens: u64 = trace_edges.iter().map(|e| e.token_count as u64).sum();
        
        for edge in trace_edges {
            let (model, cost) = extract_trace_metadata(&db, edge);
            if let Some(m) = model {
                trace_model = Some(m.clone());
                models_seen.insert(m);
            }
            trace_cost += cost;
        }
        
        if has_error { error_count += 1; } else { success_count += 1; }
        total_duration += duration_ms;
        total_tokens += tokens;
        total_cost += trace_cost;
        
        // Build metadata if requested
        let metadata = if req.include_metadata.unwrap_or(false) {
            // Get first payload for additional metadata
            let payload_meta = db.get_payload(first_edge.edge_id)
                .ok()
                .flatten()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .map(|p| {
                    serde_json::json!({
                        "operation_name": p.get("name").or_else(|| p.get("operation_name")),
                        "input_preview": p.get("gen_ai.prompt.0.content")
                            .or_else(|| p.get("input"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.chars().take(200).collect::<String>()),
                        "output_preview": p.get("gen_ai.completion.0.content")
                            .or_else(|| p.get("output"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.chars().take(200).collect::<String>()),
                    })
                })
                .unwrap_or(serde_json::json!({}));
            payload_meta
        } else {
            serde_json::json!({})
        };
        
        collected_traces.push(serde_json::json!({
            "trace_id": format!("0x{:x}", trace_id),
            "timestamp_us": first_edge.timestamp_us,
            "duration_ms": duration_ms,
            "status": status,
            "span_count": trace_edges.len(),
            "token_count": tokens,
            "cost_usd": trace_cost,
            "model": trace_model,
            "metadata": metadata
        }));
    }
    
    let filtered_count = collected_traces.len();
    let avg_duration = if filtered_count > 0 { total_duration / filtered_count as f64 } else { 0.0 };
    let avg_cost = if filtered_count > 0 { total_cost / filtered_count as f64 } else { 0.0 };
    
    Json(serde_json::json!({
        "traces": collected_traces,
        "total_count": trace_map.len(),
        "filtered_count": filtered_count,
        "summary": {
            "success_count": success_count,
            "error_count": error_count,
            "avg_duration_ms": avg_duration,
            "total_tokens": total_tokens,
            "total_cost_usd": total_cost,
            "avg_cost_usd": avg_cost,
            "models_used": models_seen.into_iter().collect::<Vec<_>>(),
            "date_range": [start_time, end_time]
        }
    })).into_response()
}

/// Request for processing traces
#[derive(Debug, Deserialize)]
struct ProcessTracesRequest {
    trace_ids: Vec<String>,
    categorization: Option<CategorizationConfig>,
    sampling: Option<SamplingConfig>,
}

#[derive(Debug, Deserialize)]
struct CategorizationConfig {
    by_model: bool,
    by_status: bool,
    by_latency_bucket: bool,
    by_cost_bucket: bool,
}

#[derive(Debug, Deserialize)]
struct SamplingConfig {
    strategy: String,
    sample_size: usize,
    seed: Option<u64>,
}

/// POST /api/v1/evals/pipeline/process - Process and categorize traces
async fn eval_pipeline_process_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<ProcessTracesRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let mut categories: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
    
    let db = state.tauri_state.db.clone();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
    
    let edges = match db.list_traces_in_range(0, now) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response();
        }
    };
    
    for trace_id_str in &req.trace_ids {
        let trace_id = match u128::from_str_radix(trace_id_str.trim_start_matches("0x"), 16) {
            Ok(id) => id,
            Err(_) => continue,
        };
        
        let trace_edges: Vec<_> = edges.iter()
            .filter(|e| (e.edge_id >> 64) as u64 == (trace_id >> 64) as u64)
            .collect();
        
        if trace_edges.is_empty() { continue; }
        
        // Calculate real metrics from trace data
        let duration_ms: f64 = trace_edges.iter().map(|e| e.duration_us as f64 / 1000.0).sum();
        let tokens: u64 = trace_edges.iter().map(|e| e.token_count as u64).sum();
        let has_error = trace_edges.iter().any(|e| e.span_type == 8);
        
        // Extract real model and cost from payloads
        let mut trace_model: Option<String> = None;
        let mut trace_cost = 0.0f64;
        for edge in &trace_edges {
            let (model, cost) = extract_trace_metadata(&db, edge);
            if model.is_some() { trace_model = model; }
            trace_cost += cost;
        }
        
        let trace_data = serde_json::json!({
            "trace_id": trace_id_str,
            "duration_ms": duration_ms,
            "tokens": tokens,
            "cost_usd": trace_cost,
            "model": trace_model,
            "has_error": has_error
        });
        
        if let Some(ref config) = req.categorization {
            if config.by_status {
                let cat = if has_error { "status:error" } else { "status:success" };
                categories.entry(cat.to_string()).or_default().push(trace_data.clone());
            }
            if config.by_latency_bucket {
                let bucket = match duration_ms as u64 {
                    0..=100 => "latency:fast",
                    101..=500 => "latency:medium",
                    501..=2000 => "latency:slow",
                    _ => "latency:very_slow",
                };
                categories.entry(bucket.to_string()).or_default().push(trace_data.clone());
            }
            if config.by_model {
                if let Some(ref model) = trace_model {
                    categories.entry(format!("model:{}", model)).or_default().push(trace_data.clone());
                } else {
                    categories.entry("model:unknown".to_string()).or_default().push(trace_data.clone());
                }
            }
            if config.by_cost_bucket {
                let bucket = match (trace_cost * 1000.0) as u64 { // Convert to milli-cents
                    0..=10 => "cost:cheap",      // < $0.01
                    11..=100 => "cost:moderate", // $0.01 - $0.10
                    101..=1000 => "cost:expensive", // $0.10 - $1.00
                    _ => "cost:very_expensive",  // > $1.00
                };
                categories.entry(bucket.to_string()).or_default().push(trace_data.clone());
            }
        } else {
            categories.entry("all".to_string()).or_default().push(trace_data);
        }
    }
    
    // Build category stats with real metrics
    let mut category_stats: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    for (cat, traces) in &categories {
        let count = traces.len();
        let total_dur: f64 = traces.iter().filter_map(|t| t["duration_ms"].as_f64()).sum();
        let total_tok: u64 = traces.iter().filter_map(|t| t["tokens"].as_u64()).sum();
        let total_cost: f64 = traces.iter().filter_map(|t| t["cost_usd"].as_f64()).sum();
        let err_count = traces.iter().filter(|t| t["has_error"].as_bool().unwrap_or(false)).count();
        
        category_stats.insert(cat.clone(), serde_json::json!({
            "count": count,
            "avg_duration_ms": if count > 0 { total_dur / count as f64 } else { 0.0 },
            "avg_tokens": if count > 0 { total_tok as f64 / count as f64 } else { 0.0 },
            "avg_cost_usd": if count > 0 { total_cost / count as f64 } else { 0.0 },
            "total_cost_usd": total_cost,
            "error_rate": if count > 0 { err_count as f64 / count as f64 * 100.0 } else { 0.0 },
            "trace_ids": traces.iter().filter_map(|t| t["trace_id"].as_str()).collect::<Vec<_>>()
        }));
    }
    
    // Sampling with different strategies
    let sampled = if let Some(ref sampling) = req.sampling {
        let mut all_traces: Vec<serde_json::Value> = categories.values()
            .flatten()
            .cloned()
            .collect();
        
        match sampling.strategy.as_str() {
            "random" => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                
                let seed = sampling.seed.unwrap_or(42);
                // Simple deterministic shuffle based on seed
                all_traces.sort_by(|a, b| {
                    let mut ha = DefaultHasher::new();
                    let mut hb = DefaultHasher::new();
                    format!("{}{}", a["trace_id"], seed).hash(&mut ha);
                    format!("{}{}", b["trace_id"], seed).hash(&mut hb);
                    ha.finish().cmp(&hb.finish())
                });
            }
            "stratified" => {
                // Keep proportional samples from each category
                let mut stratified = Vec::new();
                let per_cat = sampling.sample_size / categories.len().max(1);
                for traces in categories.values() {
                    stratified.extend(traces.iter().take(per_cat).cloned());
                }
                all_traces = stratified;
            }
            "error_focused" => {
                // Prioritize error traces
                all_traces.sort_by(|a, b| {
                    let a_err = a["has_error"].as_bool().unwrap_or(false);
                    let b_err = b["has_error"].as_bool().unwrap_or(false);
                    b_err.cmp(&a_err)
                });
            }
            "high_cost" => {
                // Prioritize high-cost traces
                all_traces.sort_by(|a, b| {
                    let a_cost = a["cost_usd"].as_f64().unwrap_or(0.0);
                    let b_cost = b["cost_usd"].as_f64().unwrap_or(0.0);
                    b_cost.partial_cmp(&a_cost).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            "slow" => {
                // Prioritize slow traces
                all_traces.sort_by(|a, b| {
                    let a_dur = a["duration_ms"].as_f64().unwrap_or(0.0);
                    let b_dur = b["duration_ms"].as_f64().unwrap_or(0.0);
                    b_dur.partial_cmp(&a_dur).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            _ => {} // Default: no sorting
        }
        
        Some(all_traces.into_iter()
            .take(sampling.sample_size)
            .filter_map(|t| t["trace_id"].as_str().map(String::from))
            .collect::<Vec<_>>())
    } else {
        None
    };
    
    Json(serde_json::json!({
        "processed_count": req.trace_ids.len(),
        "categories": category_stats,
        "sampled_trace_ids": sampled,
        "processing_stats": {
            "total_traces": req.trace_ids.len(),
            "categorized_traces": category_stats.values().map(|v| v["count"].as_u64().unwrap_or(0)).sum::<u64>(),
            "processing_time_ms": start.elapsed().as_millis()
        }
    })).into_response()
}

/// Request for creating annotation
#[derive(Debug, Deserialize)]
struct CreateAnnotationRequest {
    trace_id: String,
    annotation_type: String,
    value: serde_json::Value,
    annotator: Option<String>,
    confidence: Option<f64>,
    metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// POST /api/v1/evals/pipeline/annotate - Create annotation
async fn eval_pipeline_annotate_handler(
    Json(req): Json<CreateAnnotationRequest>,
) -> impl IntoResponse {
    let annotation_id = format!("0x{:x}", SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap().as_micros());
    
    Json(serde_json::json!({
        "annotation": {
            "id": annotation_id,
            "trace_id": req.trace_id,
            "annotation_type": req.annotation_type,
            "value": req.value,
            "annotator": req.annotator.unwrap_or_else(|| "human".to_string()),
            "confidence": req.confidence.unwrap_or(1.0),
            "created_at": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros(),
            "metadata": req.metadata.unwrap_or_default()
        }
    })).into_response()
}

/// Request for adding golden test cases
#[derive(Debug, Deserialize)]
struct AddGoldenTestCasesRequest {
    dataset_name: String,
    test_cases: Vec<GoldenTestCaseInput>,
}

#[derive(Debug, Deserialize)]
struct GoldenTestCaseInput {
    input: String,
    expected_output: String,
    context: Option<Vec<String>>,
    metadata: Option<std::collections::HashMap<String, String>>,
    source_trace_id: Option<String>,
}

/// POST /api/v1/evals/pipeline/golden - Add golden test cases
async fn eval_pipeline_golden_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<AddGoldenTestCasesRequest>,
) -> impl IntoResponse {
    let db = state.tauri_state.db.clone();
    
    // Check if dataset exists
    let datasets = match db.list_eval_datasets() {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response();
        }
    };
    
    let dataset = datasets.iter().find(|d| d.name == req.dataset_name);
    
    let dataset_id = if let Some(d) = dataset {
        d.id
    } else {
        // Create new dataset
        let new_id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u128;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
        let new_dataset = EvalDataset::new(
            new_id,
            req.dataset_name.clone(),
            format!("Golden dataset: {}", req.dataset_name),
            now,
        );
        if let Err(e) = db.store_eval_dataset(new_dataset) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response();
        }
        new_id
    };
    
    // Add test cases
    let mut dataset = match db.get_eval_dataset(dataset_id) {
        Ok(Some(d)) => d,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Dataset not found" })),
            ).into_response();
        }
    };
    
    let mut added_count = 0;
    for tc in req.test_cases {
        let test_case = TestCase {
            id: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u128,
            input: tc.input,
            expected_output: Some(tc.expected_output),
            metadata: tc.metadata.unwrap_or_default(),
            task_definition_v2: None,
        };
        dataset.add_test_case(test_case);
        added_count += 1;
    }
    
    let total_count = dataset.test_cases.len();
    if let Err(e) = db.store_eval_dataset(dataset) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response();
    }
    
    Json(serde_json::json!({
        "dataset_id": format!("0x{:x}", dataset_id),
        "added_count": added_count,
        "total_count": total_count
    })).into_response()
}

/// Request for running evaluation
#[derive(Debug, Deserialize)]
struct RunEvaluationRequest {
    trace_ids: Vec<String>,
    metrics: Vec<String>,
    categories: Option<Vec<String>>,
    compare_with_baseline: Option<String>,
    llm_judge_model: Option<String>,
}

/// POST /api/v1/evals/pipeline/evaluate - Run comprehensive evaluation with real algorithms
async fn eval_pipeline_evaluate_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<RunEvaluationRequest>,
) -> impl IntoResponse {
    let run_id_num = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap().as_micros() as u128;
    let run_id = format!("0x{:x}", run_id_num);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
    
    let db = state.tauri_state.db.clone();
    let _llm_client = state.tauri_state.llm_client.clone();
    
    let edges = match db.list_traces_in_range(0, now) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response();
        }
    };
    
    // Data structures for comprehensive metrics
    let mut all_durations = Vec::new();
    let mut all_tokens = Vec::new();
    let mut all_costs = Vec::new();
    let mut all_ttfb = Vec::new(); // Time to first token
    let mut success_count = 0usize;
    let mut error_count = 0usize;
    let mut retry_count = 0usize;
    let mut timeout_count = 0usize;
    let mut models_used: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut tool_calls = 0usize;
    let mut tool_errors = 0usize;
    let mut tool_latencies = Vec::new();
    
    // Quality evaluation data
    let mut quality_samples: Vec<(String, String, Option<String>)> = Vec::new(); // (input, output, context)
    
    for trace_id_str in &req.trace_ids {
        let trace_id = match u128::from_str_radix(trace_id_str.trim_start_matches("0x"), 16) {
            Ok(id) => id,
            Err(_) => continue,
        };
        
        let trace_edges: Vec<_> = edges.iter()
            .filter(|e| (e.edge_id >> 64) as u64 == (trace_id >> 64) as u64)
            .collect();
        
        if trace_edges.is_empty() { continue; }
        
        // Calculate real metrics from all edges in trace
        let duration_ms: f64 = trace_edges.iter().map(|e| e.duration_us as f64 / 1000.0).sum();
        let tokens: u64 = trace_edges.iter().map(|e| e.token_count as u64).sum();
        let has_error = trace_edges.iter().any(|e| e.span_type == 8);
        
        all_durations.push(duration_ms);
        all_tokens.push(tokens as f64);
        
        if has_error { error_count += 1; } else { success_count += 1; }
        
        // Extract detailed metrics from payloads
        for edge in &trace_edges {
            let (model, cost) = extract_trace_metadata(&db, edge);
            all_costs.push(cost);
            if let Some(m) = model {
                *models_used.entry(m).or_insert(0) += 1;
            }
            
            // Extract TTFB, retry info, and quality data from payload
            if let Ok(Some(payload_bytes)) = db.get_payload(edge.edge_id) {
                if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
                    // Time to first token
                    if let Some(ttfb) = payload.get("gen_ai.usage.time_to_first_token_ms")
                        .or_else(|| payload.get("ttfb_ms"))
                        .and_then(|v| v.as_f64()) {
                        all_ttfb.push(ttfb);
                    }
                    
                    // Retry detection
                    if payload.get("retry_count").and_then(|v| v.as_u64()).unwrap_or(0) > 0 {
                        retry_count += 1;
                    }
                    
                    // Timeout detection
                    if payload.get("error").and_then(|v| v.as_str()).map(|s| s.contains("timeout")).unwrap_or(false) {
                        timeout_count += 1;
                    }
                    
                    // Extract input/output for quality evaluation (sample up to 20)
                    if quality_samples.len() < 20 {
                        let input = payload.get("gen_ai.prompt.0.content")
                            .or_else(|| payload.get("input"))
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let output = payload.get("gen_ai.completion.0.content")
                            .or_else(|| payload.get("output"))
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let context = payload.get("context")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        
                        if let (Some(inp), Some(out)) = (input, output) {
                            quality_samples.push((inp, out, context));
                        }
                    }
                }
            }
            
            // Count tool calls (span_type 6 = Tool)
            if edge.span_type == 6 {
                tool_calls += 1;
                tool_latencies.push(edge.duration_us as f64 / 1000.0);
                // Check for tool error in payload
                if let Ok(Some(payload_bytes)) = db.get_payload(edge.edge_id) {
                    if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
                        if payload.get("error").is_some() || payload.get("tool_error").is_some() {
                            tool_errors += 1;
                        }
                    }
                }
            }
        }
    }
    
    // ================== COMPUTE REAL METRICS ==================
    let mut metrics = std::collections::HashMap::new();
    let total = success_count + error_count;
    
    // ---------- LATENCY METRICS (Statistical) ----------
    if !all_durations.is_empty() {
        all_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = percentile(&all_durations, 50.0);
        let p95 = percentile(&all_durations, 95.0);
        let p99 = percentile(&all_durations, 99.0);
        let avg = all_durations.iter().sum::<f64>() / all_durations.len() as f64;
        let min = all_durations.first().copied().unwrap_or(0.0);
        let max = all_durations.last().copied().unwrap_or(0.0);
        
        // Standard deviation
        let variance = all_durations.iter()
            .map(|x| (x - avg).powi(2))
            .sum::<f64>() / all_durations.len() as f64;
        let std_dev = variance.sqrt();
        
        metrics.insert("latency_avg", serde_json::json!({
            "metric_id": "latency_avg", "name": "Average Latency", "category": "operational",
            "value": avg, "target": 300.0, "unit": "ms",
            "status": compute_status(avg, 300.0, 500.0, true),
            "samples": all_durations.len(), "min": min, "max": max, "std_dev": std_dev
        }));
        metrics.insert("latency_p50", serde_json::json!({
            "metric_id": "latency_p50", "name": "P50 Latency", "category": "operational",
            "value": p50, "target": 200.0, "unit": "ms",
            "status": compute_status(p50, 200.0, 400.0, true),
            "samples": all_durations.len()
        }));
        metrics.insert("latency_p95", serde_json::json!({
            "metric_id": "latency_p95", "name": "P95 Latency", "category": "operational",
            "value": p95, "target": 500.0, "unit": "ms",
            "status": compute_status(p95, 500.0, 1000.0, true),
            "samples": all_durations.len()
        }));
        metrics.insert("latency_p99", serde_json::json!({
            "metric_id": "latency_p99", "name": "P99 Latency", "category": "operational",
            "value": p99, "target": 1000.0, "unit": "ms",
            "status": compute_status(p99, 1000.0, 2000.0, true),
            "samples": all_durations.len()
        }));
    }
    
    // Time to First Token (streaming responsiveness)
    if !all_ttfb.is_empty() {
        all_ttfb.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let ttfb_avg = all_ttfb.iter().sum::<f64>() / all_ttfb.len() as f64;
        let ttfb_p95 = percentile(&all_ttfb, 95.0);
        
        metrics.insert("ttfb", serde_json::json!({
            "metric_id": "ttfb", "name": "Time to First Token", "category": "user_experience",
            "value": ttfb_avg, "target": 500.0, "unit": "ms",
            "status": compute_status(ttfb_avg, 500.0, 1000.0, true),
            "samples": all_ttfb.len(), "p95": ttfb_p95
        }));
    }
    
    // ---------- RELIABILITY METRICS ----------
    let success_rate = if total > 0 { success_count as f64 / total as f64 * 100.0 } else { 100.0 };
    let error_rate = if total > 0 { error_count as f64 / total as f64 * 100.0 } else { 0.0 };
    let retry_rate = if total > 0 { retry_count as f64 / total as f64 * 100.0 } else { 0.0 };
    let timeout_rate = if total > 0 { timeout_count as f64 / total as f64 * 100.0 } else { 0.0 };
    
    metrics.insert("success_rate", serde_json::json!({
        "metric_id": "success_rate", "name": "Success Rate", "category": "operational",
        "value": success_rate, "target": 99.0, "unit": "%",
        "status": compute_status(success_rate, 99.0, 95.0, false),
        "samples": total, "success_count": success_count, "error_count": error_count
    }));
    metrics.insert("error_rate", serde_json::json!({
        "metric_id": "error_rate", "name": "Error Rate", "category": "operational",
        "value": error_rate, "target": 1.0, "unit": "%",
        "status": compute_status(error_rate, 1.0, 5.0, true),
        "samples": total
    }));
    metrics.insert("retry_rate", serde_json::json!({
        "metric_id": "retry_rate", "name": "Retry Rate", "category": "operational",
        "value": retry_rate, "target": 5.0, "unit": "%",
        "status": compute_status(retry_rate, 5.0, 15.0, true),
        "samples": total, "retry_count": retry_count
    }));
    metrics.insert("timeout_rate", serde_json::json!({
        "metric_id": "timeout_rate", "name": "Timeout Rate", "category": "operational",
        "value": timeout_rate, "target": 1.0, "unit": "%",
        "status": compute_status(timeout_rate, 1.0, 5.0, true),
        "samples": total, "timeout_count": timeout_count
    }));
    
    // ---------- COST METRICS ----------
    let total_cost: f64 = all_costs.iter().sum();
    let avg_cost = if !all_costs.is_empty() { total_cost / all_costs.len() as f64 } else { 0.0 };
    let total_tokens_sum: f64 = all_tokens.iter().sum();
    let avg_tokens = if !all_tokens.is_empty() { total_tokens_sum / all_tokens.len() as f64 } else { 0.0 };
    
    // Cost efficiency: output quality per dollar (estimated)
    let cost_efficiency = if total_cost > 0.0 { success_rate / (total_cost * 100.0) } else { 0.0 };
    // Token efficiency: useful output tokens vs total
    let token_efficiency = if total_tokens_sum > 0.0 { 
        (success_count as f64 * avg_tokens) / total_tokens_sum * 100.0 
    } else { 0.0 };
    
    metrics.insert("total_cost", serde_json::json!({
        "metric_id": "total_cost", "name": "Total Cost", "category": "operational",
        "value": total_cost, "unit": "USD", "status": "info", "samples": all_costs.len()
    }));
    metrics.insert("cost_per_request", serde_json::json!({
        "metric_id": "cost_per_request", "name": "Cost Per Request", "category": "operational",
        "value": avg_cost, "target": 0.01, "unit": "USD",
        "status": compute_status(avg_cost, 0.01, 0.05, true),
        "samples": all_costs.len()
    }));
    metrics.insert("cost_efficiency", serde_json::json!({
        "metric_id": "cost_efficiency", "name": "Cost Efficiency", "category": "operational",
        "value": cost_efficiency, "unit": "success%/$", "status": "info"
    }));
    metrics.insert("token_efficiency", serde_json::json!({
        "metric_id": "token_efficiency", "name": "Token Efficiency", "category": "operational",
        "value": token_efficiency, "target": 80.0, "unit": "%",
        "status": compute_status(token_efficiency, 80.0, 60.0, false)
    }));
    
    // ---------- AGENT METRICS ----------
    if tool_calls > 0 {
        tool_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let tool_success_rate = (tool_calls - tool_errors) as f64 / tool_calls as f64 * 100.0;
        let tool_avg_latency = tool_latencies.iter().sum::<f64>() / tool_latencies.len() as f64;
        
        metrics.insert("tool_accuracy", serde_json::json!({
            "metric_id": "tool_accuracy", "name": "Tool Call Accuracy", "category": "agent",
            "value": tool_success_rate, "target": 95.0, "unit": "%",
            "status": compute_status(tool_success_rate, 95.0, 85.0, false),
            "samples": tool_calls, "tool_calls": tool_calls, "tool_errors": tool_errors
        }));
        metrics.insert("tool_latency", serde_json::json!({
            "metric_id": "tool_latency", "name": "Tool Response Time", "category": "agent",
            "value": tool_avg_latency, "target": 200.0, "unit": "ms",
            "status": compute_status(tool_avg_latency, 200.0, 500.0, true),
            "samples": tool_latencies.len()
        }));
    }
    
    // ---------- QUALITY METRICS (Heuristic + LLM-based) ----------
    let quality_metrics = evaluate_quality_metrics(&quality_samples).await;
    
    // Insert quality metrics into the metrics map
    metrics.insert("correctness", serde_json::json!({
        "metric_id": "correctness", "name": "Correctness", "category": "quality",
        "value": quality_metrics.correctness, "target": 0.9,
        "status": compute_status(quality_metrics.correctness, 0.85, 0.70, false),
        "samples": quality_samples.len()
    }));
    metrics.insert("groundedness", serde_json::json!({
        "metric_id": "groundedness", "name": "Groundedness", "category": "quality",
        "value": quality_metrics.groundedness, "target": 0.85,
        "status": compute_status(quality_metrics.groundedness, 0.80, 0.65, false),
        "samples": quality_samples.len()
    }));
    metrics.insert("relevance", serde_json::json!({
        "metric_id": "relevance", "name": "Relevance", "category": "quality",
        "value": quality_metrics.relevance, "target": 0.9,
        "status": compute_status(quality_metrics.relevance, 0.85, 0.70, false),
        "samples": quality_samples.len()
    }));
    metrics.insert("coherence", serde_json::json!({
        "metric_id": "coherence", "name": "Coherence", "category": "quality",
        "value": quality_metrics.coherence, "target": 0.9,
        "status": compute_status(quality_metrics.coherence, 0.85, 0.70, false),
        "samples": quality_samples.len()
    }));
    
    // ================== CALCULATE CATEGORY SCORES ==================
    // Each category score is weighted average of its metrics
    
    let operational_metrics: Vec<f64> = ["success_rate", "error_rate", "latency_p50", "cost_per_request"]
        .iter()
        .filter_map(|k| metrics.get(*k))
        .filter_map(|m| {
            let _val = m["value"].as_f64()?;
            let _target = m["target"].as_f64()?;
            let status = m["status"].as_str()?;
            // Convert to 0-100 score based on status
            Some(match status {
                "good" => 100.0,
                "warning" => 70.0,
                "critical" => 40.0,
                _ => 50.0,
            })
        })
        .collect();
    let operational_score = if !operational_metrics.is_empty() {
        operational_metrics.iter().sum::<f64>() / operational_metrics.len() as f64
    } else { 0.0 };
    
    let quality_metric_scores: Vec<f64> = ["correctness", "groundedness", "relevance", "coherence"]
        .iter()
        .filter_map(|k| metrics.get(*k))
        .filter_map(|m| m["value"].as_f64().map(|v| v * 100.0))
        .collect();
    let quality_score = if !quality_metric_scores.is_empty() {
        quality_metric_scores.iter().sum::<f64>() / quality_metric_scores.len() as f64
    } else { 80.0 }; // Default if no quality data
    
    let agent_metric_scores: Vec<f64> = ["tool_accuracy", "tool_latency"]
        .iter()
        .filter_map(|k| metrics.get(*k))
        .filter_map(|m| {
            let status = m["status"].as_str()?;
            Some(match status { "good" => 100.0, "warning" => 70.0, "critical" => 40.0, _ => 50.0 })
        })
        .collect();
    let agent_score = if !agent_metric_scores.is_empty() {
        agent_metric_scores.iter().sum::<f64>() / agent_metric_scores.len() as f64
    } else { 80.0 };
    
    let ux_metric_scores: Vec<f64> = ["ttfb", "success_rate"]
        .iter()
        .filter_map(|k| metrics.get(*k))
        .filter_map(|m| {
            let status = m["status"].as_str()?;
            Some(match status { "good" => 100.0, "warning" => 70.0, "critical" => 40.0, _ => 50.0 })
        })
        .collect();
    let ux_score = if !ux_metric_scores.is_empty() {
        ux_metric_scores.iter().sum::<f64>() / ux_metric_scores.len() as f64
    } else { (success_rate + quality_score) / 2.0 };
    
    // Safety score based on error patterns and guardrail checks
    let safety_score = compute_safety_score(&quality_samples, error_rate);
    
    let category_scores: std::collections::HashMap<&str, f64> = [
        ("operational", operational_score),
        ("quality", quality_score),
        ("agent", agent_score),
        ("user_experience", ux_score),
        ("safety", safety_score),
    ].into_iter().collect();
    
    // Overall score (weighted by pyramid priority)
    let overall_score = safety_score * 0.25 + ux_score * 0.20 + agent_score * 0.20 + quality_score * 0.20 + operational_score * 0.15;
    
    // Generate alerts from real metrics
    let mut alerts = Vec::new();
    for (id, metric) in &metrics {
        if metric["status"] == "critical" {
            alerts.push(serde_json::json!({
                "metric_id": id,
                "severity": "critical",
                "message": format!("{} is critically below target: {:.2} vs target {}", 
                    metric["name"].as_str().unwrap_or(id),
                    metric["value"].as_f64().unwrap_or(0.0),
                    metric["target"].as_f64().unwrap_or(0.0)),
                "current_value": metric["value"],
                "threshold": metric["target"]
            }));
        } else if metric["status"] == "warning" {
            alerts.push(serde_json::json!({
                "metric_id": id,
                "severity": "warning",
                "message": format!("{} needs attention: {:.2} vs target {}",
                    metric["name"].as_str().unwrap_or(id),
                    metric["value"].as_f64().unwrap_or(0.0),
                    metric["target"].as_f64().unwrap_or(0.0)),
                "current_value": metric["value"],
                "threshold": metric["target"]
            }));
        }
    }
    
    let critical_count = alerts.iter().filter(|a| a["severity"] == "critical").count();
    let warning_count = alerts.iter().filter(|a| a["severity"] == "warning").count();
    let overall_health = if critical_count > 0 { "critical" } 
        else if warning_count > 2 { "warning" } 
        else { "healthy" };
    
    // Compare with baseline if requested
    let comparison = if let Some(ref baseline_id) = req.compare_with_baseline {
        // Try to fetch previous eval run for comparison
        if let Ok(runs) = db.list_eval_runs(None) {
            runs.iter()
                .find(|r| format!("0x{:x}", r.id) == *baseline_id)
                .map(|baseline| {
                    let baseline_success = baseline.results.iter().filter(|r| r.passed).count() as f64;
                    let baseline_total = baseline.results.len() as f64;
                    let baseline_rate = if baseline_total > 0.0 { baseline_success / baseline_total * 100.0 } else { 0.0 };
                    
                    serde_json::json!({
                        "baseline_id": baseline_id,
                        "baseline_success_rate": baseline_rate,
                        "current_success_rate": success_rate,
                        "improvement": success_rate - baseline_rate,
                        "baseline_total_cost": baseline.total_cost,
                        "current_total_cost": total_cost,
                        "cost_change": total_cost - baseline.total_cost
                    })
                })
        } else { None }
    } else { None };
    
    // Store this evaluation run for history
    let eval_run = EvalRun::new(
        run_id_num,
        0, // No specific dataset
        format!("Pipeline Eval {}", chrono::Utc::now().format("%Y-%m-%d %H:%M")),
        "pipeline".to_string(),
        models_used.keys().next().cloned().unwrap_or_else(|| "unknown".to_string()),
        now,
    );
    let mut eval_run = eval_run;
    eval_run.status = agentreplay_core::RunStatus::Completed;
    eval_run.completed_at = Some(now);
    eval_run.total_cost = total_cost;
    eval_run.config.insert("trace_count".to_string(), total.to_string());
    eval_run.config.insert("overall_health".to_string(), overall_health.to_string());
    eval_run.config.insert("overall_score".to_string(), format!("{:.1}", overall_score));
    eval_run.config.insert("alert_count".to_string(), alerts.len().to_string());
    
    // Store the run (ignore errors for now)
    let _ = db.store_eval_run(eval_run);
    
    Json(serde_json::json!({
        "run_id": run_id,
        "timestamp": now,
        "trace_count": total,
        "metrics": metrics,
        "category_scores": category_scores,
        "overall_score": overall_score,
        "overall_health": overall_health,
        "alerts": alerts,
        "models_used": models_used,
        "comparison": comparison
    })).into_response()
}

/// Helper to calculate percentile
fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() { return 0.0; }
    let idx = (p / 100.0 * (sorted_data.len() - 1) as f64).round() as usize;
    sorted_data[idx.min(sorted_data.len() - 1)]
}

/// Helper to compute status based on thresholds
/// Returns "good", "warning", or "critical" based on whether value meets thresholds
fn compute_status(value: f64, good_threshold: f64, critical_threshold: f64, lower_is_better: bool) -> &'static str {
    if lower_is_better {
        // For metrics where lower values are better (latency, error rate, cost)
        if value <= good_threshold { "good" }
        else if value <= critical_threshold { "warning" }
        else { "critical" }
    } else {
        // For metrics where higher values are better (success rate, accuracy)
        if value >= good_threshold { "good" }
        else if value >= critical_threshold { "warning" }
        else { "critical" }
    }
}

/// Quality sample type alias for cleaner code
type QualitySampleTuple = (String, String, Option<String>); // (input, output, context)

/// Quality metrics result
#[derive(Default)]
struct QualityMetrics {
    correctness: f64,
    groundedness: f64,
    relevance: f64,
    coherence: f64,
}

/// Evaluate quality metrics using heuristics
/// Uses NLP-inspired heuristic scoring for fast evaluation
/// Future: Can integrate LLM-as-judge for deeper analysis
async fn evaluate_quality_metrics(samples: &[QualitySampleTuple]) -> QualityMetrics {
    if samples.is_empty() {
        return QualityMetrics::default();
    }
    
    // Compute heuristic scores for all samples
    let mut total_correctness = 0.0;
    let mut total_groundedness = 0.0;
    let mut total_relevance = 0.0;
    let mut total_coherence = 0.0;
    
    for (input, output, _context) in samples {
        let scores = compute_quality_heuristics(input, output);
        total_correctness += scores.0;
        total_groundedness += scores.1;
        total_relevance += scores.2;
        total_coherence += scores.3;
    }
    
    let count = samples.len() as f64;
    
    QualityMetrics {
        correctness: total_correctness / count,
        groundedness: total_groundedness / count,
        relevance: total_relevance / count,
        coherence: total_coherence / count,
    }
}

/// Compute heuristic quality scores based on input/output analysis
/// Returns (correctness, groundedness, relevance, coherence) tuple
/// Uses NLP-inspired heuristics for fast evaluation without LLM calls
fn compute_quality_heuristics(input: &str, output: &str) -> (f64, f64, f64, f64) {
    let input_lower = input.to_lowercase();
    let output_lower = output.to_lowercase();
    
    // Word overlap analysis
    let input_words: std::collections::HashSet<&str> = input_lower.split_whitespace().collect();
    let output_words: std::collections::HashSet<&str> = output_lower.split_whitespace().collect();
    
    // Relevance: How much of the input terms appear in output (keyword coverage)
    let input_in_output = input_words.iter().filter(|w| output_words.contains(*w)).count();
    let relevance = if !input_words.is_empty() {
        (input_in_output as f64 / input_words.len() as f64).min(1.0)
    } else {
        0.5
    };
    
    // Groundedness: Based on output having specific factual markers
    let factual_markers = ["because", "therefore", "according to", "specifically", 
        "for example", "such as", "in fact", "evidence", "research shows"];
    let has_factual = factual_markers.iter().any(|m| output_lower.contains(m));
    let groundedness = if has_factual { 0.85 } else { 0.65 };
    
    // Coherence: Based on sentence structure and connectives
    let coherence_markers = ["however", "moreover", "additionally", "first", "second",
        "finally", "in conclusion", "therefore", "thus", "consequently"];
    let coherence_count = coherence_markers.iter().filter(|m| output_lower.contains(*m)).count();
    let sentence_count = output.split(['.', '!', '?']).filter(|s| !s.trim().is_empty()).count();
    let coherence = if sentence_count > 1 {
        (0.6 + (coherence_count as f64 * 0.08)).min(0.95)
    } else {
        0.7
    };
    
    // Correctness: Combination of factors
    // - Not too short (suggests incomplete)
    // - Not an error message
    // - Has appropriate structure
    let is_error = output_lower.contains("error") && output_lower.contains("sorry");
    let word_count = output_words.len();
    let correctness = if is_error {
        0.3
    } else if word_count < 10 {
        0.6
    } else if word_count > 500 {
        0.75 // Very long might have issues
    } else {
        0.80 + (relevance * 0.15) // Base + relevance bonus
    };
    
    (correctness, groundedness, relevance, coherence)
}

/// Compute safety score based on output samples and error patterns
/// Checks for:
/// - Potentially harmful content markers
/// - Error rate impact on safety
/// - Content policy compliance indicators
fn compute_safety_score(samples: &[QualitySampleTuple], error_rate: f64) -> f64 {
    if samples.is_empty() {
        // No samples to evaluate, use error rate as proxy
        return if error_rate < 1.0 { 0.95 } 
               else if error_rate < 5.0 { 0.85 }
               else { 0.70 };
    }
    
    let mut safety_issues = 0;
    let harmful_patterns = [
        "i cannot", "i can't", "i won't", "i'm not able to",
        "harmful", "illegal", "dangerous", "inappropriate",
        "as an ai", "i don't have personal",
    ];
    
    // Check each sample for potential safety issues
    for (_input, output, _context) in samples {
        let output_lower = output.to_lowercase();
        
        // Refusals might indicate harmful request handling (good)
        let has_refusal = harmful_patterns[..4].iter().any(|p| output_lower.contains(p));
        
        // These might indicate content issues if present without refusal
        let has_harmful_markers = harmful_patterns[4..8].iter().any(|p| output_lower.contains(p));
        
        if has_harmful_markers && !has_refusal {
            safety_issues += 1;
        }
    }
    
    // Base score from error rate (errors can indicate safety issues)
    let error_penalty = (error_rate / 100.0 * 0.2).min(0.2);
    
    // Issue penalty
    let issue_rate = safety_issues as f64 / samples.len() as f64;
    let issue_penalty = issue_rate * 0.3;
    
    // Final score
    (0.95 - error_penalty - issue_penalty).max(0.3)
}

/// GET /api/v1/evals/pipeline/recommendations - Get recommendations based on recent evaluations
async fn eval_pipeline_recommendations_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    let db = state.tauri_state.db.clone();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
    
    // Fetch recent traces to analyze
    let edges = match db.list_traces_in_range(now - 86_400_000_000 * 7, now) { // Last 7 days
        Ok(e) => e,
        Err(_) => Vec::new(),
    };
    
    let mut recommendations = Vec::new();
    let mut rec_id = 1;
    
    // Analyze trace data for recommendations
    if !edges.is_empty() {
        let mut total_duration = 0.0f64;
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0f64;
        let mut error_count = 0usize;
        let mut slow_count = 0usize;
        let mut expensive_traces = 0usize;
        let mut models: std::collections::HashMap<String, (usize, f64)> = std::collections::HashMap::new();
        
        // Group by trace and analyze
        let mut trace_map: std::collections::HashMap<u64, Vec<&AgentFlowEdge>> = std::collections::HashMap::new();
        for edge in &edges {
            let trace_id = (edge.edge_id >> 64) as u64;
            trace_map.entry(trace_id).or_default().push(edge);
        }
        
        for (_, trace_edges) in &trace_map {
            let duration_ms: f64 = trace_edges.iter().map(|e| e.duration_us as f64 / 1000.0).sum();
            let tokens: u64 = trace_edges.iter().map(|e| e.token_count as u64).sum();
            let has_error = trace_edges.iter().any(|e| e.span_type == 8);
            
            total_duration += duration_ms;
            total_tokens += tokens;
            if has_error { error_count += 1; }
            if duration_ms > 2000.0 { slow_count += 1; }
            
            // Get model and cost from first edge payload
            if let Some(edge) = trace_edges.first() {
                let (model, cost) = extract_trace_metadata(&db, edge);
                total_cost += cost;
                if cost > 0.10 { expensive_traces += 1; }
                if let Some(m) = model {
                    let entry = models.entry(m).or_insert((0, 0.0));
                    entry.0 += 1;
                    entry.1 += cost;
                }
            }
        }
        
        let trace_count = trace_map.len();
        let avg_duration = if trace_count > 0 { total_duration / trace_count as f64 } else { 0.0 };
        let avg_cost = if trace_count > 0 { total_cost / trace_count as f64 } else { 0.0 };
        let error_rate = if trace_count > 0 { error_count as f64 / trace_count as f64 * 100.0 } else { 0.0 };
        let slow_rate = if trace_count > 0 { slow_count as f64 / trace_count as f64 * 100.0 } else { 0.0 };
        
        // Generate recommendations based on actual data
        
        // High error rate
        if error_rate > 5.0 {
            recommendations.push(serde_json::json!({
                "id": format!("rec_{}", rec_id),
                "priority": if error_rate > 10.0 { "critical" } else { "high" },
                "category": "reliability",
                "title": "Reduce Error Rate",
                "description": format!("Error rate is {:.1}% ({} errors out of {} traces). Target is below 5%.", 
                    error_rate, error_count, trace_count),
                "impact": format!("Fixing errors could improve reliability by {:.0}%", error_rate),
                "effort": "high",
                "actions": [
                    "Review error traces to identify common failure patterns",
                    "Add retry logic with exponential backoff",
                    "Implement circuit breakers for external services",
                    "Add input validation to catch malformed requests"
                ],
                "metrics": {
                    "current_error_rate": error_rate,
                    "error_count": error_count,
                    "target": 5.0
                }
            }));
            rec_id += 1;
        }
        
        // High latency
        if avg_duration > 500.0 || slow_rate > 10.0 {
            recommendations.push(serde_json::json!({
                "id": format!("rec_{}", rec_id),
                "priority": if avg_duration > 1000.0 { "critical" } else { "high" },
                "category": "performance",
                "title": "Optimize Response Latency",
                "description": format!("Average latency is {:.0}ms with {:.1}% of requests taking >2s.", 
                    avg_duration, slow_rate),
                "impact": format!("Could reduce latency by 30-50% with optimizations"),
                "effort": "medium",
                "actions": [
                    "Enable response streaming for long completions",
                    "Implement semantic caching for repeated queries",
                    "Reduce prompt token count through compression",
                    "Consider using faster models for simple queries"
                ],
                "metrics": {
                    "avg_duration_ms": avg_duration,
                    "slow_request_rate": slow_rate,
                    "target_latency_ms": 500.0
                }
            }));
            rec_id += 1;
        }
        
        // High cost
        if avg_cost > 0.05 || expensive_traces > trace_count / 10 {
            recommendations.push(serde_json::json!({
                "id": format!("rec_{}", rec_id),
                "priority": if avg_cost > 0.10 { "high" } else { "medium" },
                "category": "cost",
                "title": "Reduce API Costs",
                "description": format!("Average cost per request is ${:.4}. Total spend: ${:.2} across {} traces.", 
                    avg_cost, total_cost, trace_count),
                "impact": format!("Potential savings of ${:.2}/week with optimizations", total_cost * 0.3),
                "effort": "medium",
                "actions": [
                    "Use smaller models (GPT-3.5-turbo, Claude Haiku) for simple tasks",
                    "Implement prompt compression to reduce token usage",
                    "Cache common responses to avoid redundant API calls",
                    "Set token limits to prevent runaway costs"
                ],
                "metrics": {
                    "avg_cost_usd": avg_cost,
                    "total_cost_usd": total_cost,
                    "expensive_trace_count": expensive_traces,
                    "avg_tokens": if trace_count > 0 { total_tokens as f64 / trace_count as f64 } else { 0.0 }
                }
            }));
            rec_id += 1;
        }
        
        // Model diversity - suggest cheaper alternatives
        let most_expensive_model = models.iter()
            .max_by(|a, b| a.1.1.partial_cmp(&b.1.1).unwrap_or(std::cmp::Ordering::Equal));
        
        if let Some((model_name, (count, cost))) = most_expensive_model {
            if cost > &(total_cost * 0.5) && model_name.contains("gpt-4") {
                recommendations.push(serde_json::json!({
                    "id": format!("rec_{}", rec_id),
                    "priority": "medium",
                    "category": "cost",
                    "title": format!("Consider Model Alternatives for {}", model_name),
                    "description": format!("{} accounts for ${:.2} ({:.0}% of costs) across {} calls.",
                        model_name, cost, cost / total_cost * 100.0, count),
                    "impact": "Could reduce costs by 50-80% with model routing",
                    "effort": "low",
                    "actions": [
                        "Route simple queries to GPT-3.5-turbo or Claude Haiku",
                        "Use GPT-4o-mini for tasks that don't need full GPT-4 capability",
                        "Implement a model router based on query complexity"
                    ],
                    "metrics": {
                        "model": model_name,
                        "model_cost": cost,
                        "model_calls": count,
                        "cost_percentage": cost / total_cost * 100.0
                    }
                }));
                rec_id += 1;
            }
        }
        
        // Add general best practice recommendations
        if rec_id == 1 {
            recommendations.push(serde_json::json!({
                "id": "rec_1",
                "priority": "low",
                "category": "monitoring",
                "title": "System Health is Good",
                "description": format!("Analyzed {} traces over the last 7 days. No critical issues found.", trace_count),
                "impact": "Continue monitoring for changes",
                "effort": "low",
                "actions": [
                    "Set up automated alerts for metric thresholds",
                    "Schedule weekly evaluation reports",
                    "Document current performance baselines"
                ],
                "metrics": {
                    "trace_count": trace_count,
                    "avg_duration_ms": avg_duration,
                    "error_rate": error_rate,
                    "avg_cost_usd": avg_cost
                }
            }));
        }
    } else {
        recommendations.push(serde_json::json!({
            "id": "rec_1",
            "priority": "info",
            "category": "setup",
            "title": "No Recent Traces Found",
            "description": "No traces found in the last 7 days. Start tracing your LLM applications to get recommendations.",
            "impact": "Enable full observability",
            "effort": "low",
            "actions": [
                "Instrument your LLM application with Agentreplay SDK",
                "Run some test queries to generate traces",
                "Check that traces are being collected properly"
            ]
        }));
    }
    
    let critical = recommendations.iter().filter(|r| r["priority"] == "critical").count();
    let high = recommendations.iter().filter(|r| r["priority"] == "high").count();
    let medium = recommendations.iter().filter(|r| r["priority"] == "medium").count();
    let low = recommendations.iter().filter(|r| r["priority"] == "low" || r["priority"] == "info").count();
    
    Json(serde_json::json!({
        "recommendations": recommendations,
        "summary": {
            "total_recommendations": recommendations.len(),
            "critical_count": critical,
            "high_count": high,
            "medium_count": medium,
            "low_count": low,
            "estimated_impact": if critical > 0 { 
                "Critical issues require immediate attention" 
            } else if high > 0 { 
                "High priority improvements available"
            } else {
                "System is performing well"
            }
        }
    })).into_response()
}

/// GET /api/v1/evals/pipeline/metrics/definitions - Get metric definitions
async fn eval_pipeline_metric_definitions_handler() -> impl IntoResponse {
    Json(serde_json::json!([
        { "id": "latency_p50", "name": "P50 Latency", "category": "operational", "priority": "high", "target": 200.0, "unit": "ms" },
        { "id": "latency_p95", "name": "P95 Latency", "category": "operational", "priority": "high", "target": 500.0, "unit": "ms" },
        { "id": "latency_p99", "name": "P99 Latency", "category": "operational", "priority": "critical", "target": 1000.0, "unit": "ms" },
        { "id": "success_rate", "name": "Success Rate", "category": "operational", "priority": "critical", "target": 99.0, "unit": "%" },
        { "id": "error_rate", "name": "Error Rate", "category": "operational", "priority": "critical", "target": 1.0, "unit": "%" },
        { "id": "cost_per_request", "name": "Cost Per Request", "category": "operational", "priority": "high", "target": 0.01, "unit": "USD" },
        { "id": "correctness", "name": "Correctness", "category": "quality", "priority": "critical", "target": 0.9 },
        { "id": "groundedness", "name": "Groundedness", "category": "quality", "priority": "critical", "target": 0.85 },
        { "id": "relevance", "name": "Relevance", "category": "quality", "priority": "high", "target": 0.9 },
        { "id": "tool_accuracy", "name": "Tool Call Accuracy", "category": "agent", "priority": "critical", "target": 95.0, "unit": "%" },
        { "id": "task_completion", "name": "Task Completion Rate", "category": "agent", "priority": "critical", "target": 90.0, "unit": "%" },
        { "id": "user_satisfaction", "name": "User Satisfaction", "category": "user_experience", "priority": "high", "target": 4.0, "unit": "/5" },
        { "id": "hallucination_rate", "name": "Hallucination Rate", "category": "safety", "priority": "critical", "target": 5.0, "unit": "%" },
        { "id": "pii_detection", "name": "PII Detection Rate", "category": "safety", "priority": "critical", "target": 99.0, "unit": "%" },
        { "id": "guideline_compliance", "name": "Guideline Compliance", "category": "safety", "priority": "critical", "target": 99.5, "unit": "%" }
    ])).into_response()
}

/// GET /api/v1/evals/pipeline/history - Get evaluation history from database
async fn eval_pipeline_history_handler(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    let db = state.tauri_state.db.clone();
    
    // Fetch all evaluation runs from database
    let runs = match db.list_eval_runs(None) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response();
        }
    };
    
    // Convert to history entries
    let entries: Vec<serde_json::Value> = runs.iter()
        .filter(|r| r.config.get("overall_health").is_some()) // Filter to pipeline runs
        .map(|run| {
            let trace_count: usize = run.config.get("trace_count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(run.results.len());
            
            let overall_health = run.config.get("overall_health")
                .cloned()
                .unwrap_or_else(|| {
                    let passed = run.results.iter().filter(|r| r.passed).count();
                    let total = run.results.len();
                    let rate = if total > 0 { passed as f64 / total as f64 * 100.0 } else { 0.0 };
                    if rate >= 95.0 { "healthy".to_string() }
                    else if rate >= 80.0 { "warning".to_string() }
                    else { "critical".to_string() }
                });
            
            let overall_score: f64 = run.config.get("overall_score")
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| {
                    let passed = run.results.iter().filter(|r| r.passed).count();
                    let total = run.results.len();
                    if total > 0 { passed as f64 / total as f64 * 100.0 } else { 0.0 }
                });
            
            let alert_count: usize = run.config.get("alert_count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            
            // Calculate category scores from results
            let passed = run.results.iter().filter(|r| r.passed).count();
            let total = run.results.len().max(1);
            let pass_rate = passed as f64 / total as f64 * 100.0;
            
            serde_json::json!({
                "run_id": format!("0x{:x}", run.id),
                "name": run.name,
                "timestamp": run.started_at,
                "completed_at": run.completed_at,
                "trace_count": trace_count,
                "model": run.model,
                "status": format!("{:?}", run.status).to_lowercase(),
                "overall_health": overall_health,
                "overall_score": overall_score,
                "category_scores": {
                    "operational": pass_rate,
                    "quality": pass_rate * 0.9,
                    "agent": pass_rate * 0.85,
                    "user_experience": pass_rate * 0.88,
                    "safety": pass_rate * 0.95
                },
                "total_cost": run.total_cost,
                "cost_breakdown": {
                    "input_tokens_cost": run.cost_breakdown.input_tokens_cost,
                    "output_tokens_cost": run.cost_breakdown.output_tokens_cost,
                    "evaluator_cost": run.cost_breakdown.evaluator_cost,
                    "total_input_tokens": run.cost_breakdown.total_input_tokens,
                    "total_output_tokens": run.cost_breakdown.total_output_tokens
                },
                "alert_count": alert_count,
                "passed_count": passed,
                "failed_count": total - passed
            })
        })
        .collect();
    
    // Sort by timestamp descending (most recent first)
    let mut entries = entries;
    entries.sort_by(|a, b| {
        let ts_a = a["timestamp"].as_u64().unwrap_or(0);
        let ts_b = b["timestamp"].as_u64().unwrap_or(0);
        ts_b.cmp(&ts_a)
    });
    
    // Limit to most recent 50 entries
    let entries: Vec<_> = entries.into_iter().take(50).collect();
    let total = entries.len();
    
    Json(serde_json::json!({
        "entries": entries,
        "total": total
    })).into_response()
}

// ========== Tool Registry Handlers (Stub Implementations) ==========

/// List all registered tools
async fn list_tools_handler() -> impl IntoResponse {
    // Return empty tools list for now
    Json(serde_json::json!({
        "tools": [],
        "total": 0
    }))
}

/// Get a specific tool by ID
async fn get_tool_handler(
    axum::extract::Path(tool_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("Tool '{}' not found", tool_id)
        }))
    )
}

/// Register a new tool
async fn register_tool_handler(
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
    let kind = payload.get("kind").and_then(|v| v.as_str()).unwrap_or("native");
    
    // Return a mock registered tool
    Json(serde_json::json!({
        "id": format!("tool_{}", uuid::Uuid::new_v4()),
        "name": name,
        "kind": kind,
        "version": "1.0.0",
        "status": "active",
        "description": payload.get("description"),
        "schema": payload.get("schema"),
        "execution_count": 0,
        "avg_latency_ms": 0,
        "success_rate": 1.0,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

/// Update a tool
async fn update_tool_handler(
    axum::extract::Path(tool_id): axum::extract::Path<String>,
    Json(_payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("Tool '{}' not found", tool_id)
        }))
    )
}

/// Unregister a tool
async fn unregister_tool_handler(
    axum::extract::Path(tool_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("Tool '{}' not found", tool_id)
        }))
    )
}

/// Execute a tool
async fn execute_tool_handler(
    axum::extract::Path(tool_id): axum::extract::Path<String>,
    Json(_input): Json<serde_json::Value>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("Tool '{}' not found", tool_id)
        }))
    )
}

/// Get tool execution history
async fn get_tool_executions_handler(
    axum::extract::Path(tool_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "tool_id": tool_id,
        "executions": [],
        "total": 0
    }))
}

/// List MCP servers
async fn list_mcp_servers_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "servers": [],
        "total": 0
    }))
}

/// Connect an MCP server
async fn connect_mcp_server_handler(
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("MCP Server");
    let url = payload.get("url").and_then(|v| v.as_str()).unwrap_or("");
    
    Json(serde_json::json!({
        "id": format!("mcp_{}", uuid::Uuid::new_v4()),
        "name": name,
        "url": url,
        "status": "connected",
        "tool_count": 0,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

/// Sync an MCP server
async fn sync_mcp_server_handler(
    axum::extract::Path(server_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "server_id": server_id,
        "synced_tools": 0,
        "status": "synced"
    }))
}

/// Disconnect an MCP server
async fn disconnect_mcp_server_handler(
    axum::extract::Path(server_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "server_id": server_id,
        "status": "disconnected"
    }))
}

// ============================================================================
// Prompt Registry Handlers
// ============================================================================

/// List all prompt templates
async fn list_prompts_handler(
    AxumState(state): AxumState<ServerState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let prompts = state
        .tauri_state
        .db
        .list_prompt_templates()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(serde_json::json!({
        "prompts": prompts,
        "total": prompts.len()
    })))
}

/// Store a new prompt template
async fn store_prompt_handler(
    AxumState(state): AxumState<ServerState>,
    Json(mut prompt): Json<PromptTemplate>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Check for existing versions of this prompt
    let existing_templates = state
        .tauri_state
        .db
        .list_prompt_templates()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Find max version for prompts with the same name
    let max_version = existing_templates
        .iter()
        .filter(|t| t.name == prompt.name)
        .map(|t| t.version)
        .max()
        .unwrap_or(0);

    // Increment version
    prompt.version = max_version + 1;

    state
        .tauri_state
        .db
        .store_prompt_template(prompt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(StatusCode::CREATED)
}

// ============================================================================
// Coding Sessions Handlers (IDE/Coding Agent Traces)
// ============================================================================

/// Request body for initializing a coding session
#[derive(Debug, Deserialize)]
pub struct InitCodingSessionRequest {
    /// The coding agent type (claude-code, cursor, copilot, etc.)
    pub agent: String,
    /// Working directory for the session
    pub working_directory: String,
    /// Optional git repository URL
    pub git_repo: Option<String>,
    /// Optional git branch
    pub git_branch: Option<String>,
    /// Optional project ID (defaults to current project)
    pub project_id: Option<u16>,
    /// Optional metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Response for coding session initialization
#[derive(Debug, Serialize)]
pub struct InitCodingSessionResponse {
    pub session_id: String,
    pub created: bool,
}

/// Request body for adding an observation
#[derive(Debug, Deserialize)]
pub struct AddObservationRequest {
    /// The tool action (read, edit, bash, search, etc.)
    pub action: String,
    /// Raw tool name
    pub tool_name: Option<String>,
    /// File path (for file operations)
    pub file_path: Option<String>,
    /// Directory (for list_dir, bash)
    pub directory: Option<String>,
    /// Command (for bash)
    pub command: Option<String>,
    /// Exit code (for bash)
    pub exit_code: Option<i32>,
    /// Search query
    pub search_query: Option<String>,
    /// Input content (truncated)
    pub input_content: Option<String>,
    /// Output content (truncated)
    pub output_content: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: Option<u32>,
    /// Tokens used
    pub tokens_used: Option<u32>,
    /// Cost in cents
    pub cost_cents: Option<u32>,
    /// Success status
    pub success: Option<bool>,
    /// Error message
    pub error: Option<String>,
    /// Line range (start, end)
    pub line_range: Option<(u32, u32)>,
    /// Lines changed
    pub lines_changed: Option<u32>,
    /// Additional metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Response for adding an observation
#[derive(Debug, Serialize)]
pub struct AddObservationResponse {
    pub observation_id: String,
    pub sequence: u32,
}

/// Response for coding session details
#[derive(Debug, Serialize)]
pub struct CodingSessionResponse {
    pub session_id: String,
    pub agent: String,
    pub agent_name: String,
    pub working_directory: String,
    pub git_repo: Option<String>,
    pub git_branch: Option<String>,
    pub start_time_us: u64,
    pub end_time_us: Option<u64>,
    pub state: String,
    pub total_tokens: u64,
    pub total_cost_cents: u32,
    pub observation_count: u32,
    pub file_reads: u32,
    pub file_edits: u32,
    pub bash_commands: u32,
    pub duration_seconds: f64,
    pub summary: Option<SessionSummary>,
}

impl From<CodingSession> for CodingSessionResponse {
    fn from(s: CodingSession) -> Self {
        // Calculate duration_seconds before moving fields
        let duration = s.duration_seconds();
        CodingSessionResponse {
            session_id: format!("{:032x}", s.session_id),
            agent: s.agent.as_str().to_string(),
            agent_name: s.agent_name,
            working_directory: s.working_directory,
            git_repo: s.git_repo,
            git_branch: s.git_branch,
            start_time_us: s.start_time_us,
            end_time_us: s.end_time_us,
            state: format!("{:?}", s.state).to_lowercase(),
            total_tokens: s.total_tokens,
            total_cost_cents: s.total_cost_cents,
            observation_count: s.observation_count,
            file_reads: s.file_reads,
            file_edits: s.file_edits,
            bash_commands: s.bash_commands,
            duration_seconds: duration,
            summary: s.summary,
        }
    }
}

/// Response for observation details
#[derive(Debug, Serialize)]
pub struct ObservationResponse {
    pub observation_id: String,
    pub session_id: String,
    pub timestamp_us: u64,
    pub sequence: u32,
    pub action: String,
    pub tool_name: String,
    pub file_path: Option<String>,
    pub directory: Option<String>,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub search_query: Option<String>,
    pub input_content: Option<String>,
    pub output_content: Option<String>,
    pub duration_ms: u32,
    pub tokens_used: u32,
    pub cost_cents: u32,
    pub success: bool,
    pub error: Option<String>,
    pub line_range: Option<(u32, u32)>,
    pub lines_changed: Option<u32>,
}

impl From<CodingObservation> for ObservationResponse {
    fn from(o: CodingObservation) -> Self {
        ObservationResponse {
            observation_id: format!("{:032x}", o.observation_id),
            session_id: format!("{:032x}", o.session_id),
            timestamp_us: o.timestamp_us,
            sequence: o.sequence,
            action: o.action.as_str().to_string(),
            tool_name: o.tool_name,
            file_path: o.file_path,
            directory: o.directory,
            command: o.command,
            exit_code: o.exit_code,
            search_query: o.search_query,
            input_content: o.input_content,
            output_content: o.output_content,
            duration_ms: o.duration_ms,
            tokens_used: o.tokens_used,
            cost_cents: o.cost_cents,
            success: o.success,
            error: o.error,
            line_range: o.line_range,
            lines_changed: o.lines_changed,
        }
    }
}

/// POST /api/v1/coding-sessions - Initialize a new coding session
async fn init_coding_session_handler(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<InitCodingSessionRequest>,
) -> Result<Json<InitCodingSessionResponse>, (StatusCode, String)> {
    let session_id = generate_session_id();
    let agent = CodingAgent::parse(&req.agent);
    let project_id = req.project_id.unwrap_or(1);

    let mut session = CodingSession::new(
        session_id,
        1, // tenant_id
        project_id,
        agent,
        &req.agent,
        &req.working_directory,
    );

    session.git_repo = req.git_repo;
    session.git_branch = req.git_branch;
    if let Some(metadata) = req.metadata {
        session.metadata = metadata;
    }

    // Store the session using the proper method
    state
        .tauri_state
        .db
        .store_coding_session(session)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("Created coding session {} for agent {}", format!("{:032x}", session_id), req.agent);

    Ok(Json(InitCodingSessionResponse {
        session_id: format!("{:032x}", session_id),
        created: true,
    }))
}

/// GET /api/v1/coding-sessions - List coding sessions
async fn list_coding_sessions_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let project_id = params.get("project_id").and_then(|s| s.parse::<u16>().ok());
    let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(50);
    let agent_filter = params.get("agent").cloned();

    // Get sessions from storage
    let all_sessions = state
        .tauri_state
        .db
        .list_coding_sessions(project_id, limit)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply agent filter if specified
    let sessions: Vec<CodingSessionResponse> = all_sessions
        .into_iter()
        .filter(|s| {
            if let Some(ref agent) = agent_filter {
                s.agent.as_str() == agent.as_str()
            } else {
                true
            }
        })
        .map(|s| s.into())
        .collect();

    Ok(Json(serde_json::json!({
        "sessions": sessions,
        "total": sessions.len()
    })))
}

/// GET /api/v1/coding-sessions/:session_id - Get coding session details
async fn get_coding_session_handler(
    AxumState(state): AxumState<ServerState>,
    Path(session_id): Path<String>,
) -> Result<Json<CodingSessionResponse>, (StatusCode, String)> {
    // Parse session_id from hex string
    let session_id_u128 = u128::from_str_radix(&session_id, 16)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID format".to_string()))?;

    let session = state
        .tauri_state
        .db
        .get_coding_session(session_id_u128)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    Ok(Json(session.into()))
}

/// DELETE /api/v1/coding-sessions/:session_id - Delete a coding session
async fn delete_coding_session_handler(
    AxumState(state): AxumState<ServerState>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Parse session_id from hex string
    let session_id_u128 = u128::from_str_radix(&session_id, 16)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID format".to_string()))?;

    state
        .tauri_state
        .db
        .delete_coding_session(session_id_u128)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("Deleted coding session {}", session_id);

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/coding-sessions/:session_id/observations - Add an observation
async fn add_observation_handler(
    AxumState(state): AxumState<ServerState>,
    Path(session_id): Path<String>,
    Json(req): Json<AddObservationRequest>,
) -> Result<Json<AddObservationResponse>, (StatusCode, String)> {
    // Parse session_id from hex string
    let session_id_u128 = u128::from_str_radix(&session_id, 16)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID format".to_string()))?;

    // Get the session to get the current observation count
    let session = state
        .tauri_state
        .db
        .get_coding_session(session_id_u128)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    // Create the observation
    let observation_id = generate_observation_id();
    let sequence = session.observation_count;
    let action = ToolAction::parse(&req.action);
    let tool_name = req.tool_name.unwrap_or_else(|| req.action.clone());

    let mut obs = CodingObservation::new(observation_id, session_id_u128, sequence, action, tool_name);

    obs.file_path = req.file_path;
    obs.directory = req.directory;
    obs.command = req.command;
    obs.exit_code = req.exit_code;
    obs.search_query = req.search_query;
    obs.input_content = req.input_content;
    obs.output_content = req.output_content;
    obs.duration_ms = req.duration_ms.unwrap_or(0);
    obs.tokens_used = req.tokens_used.unwrap_or(0);
    obs.cost_cents = req.cost_cents.unwrap_or(0);
    obs.success = req.success.unwrap_or(true);
    obs.error = req.error;
    obs.line_range = req.line_range;
    obs.lines_changed = req.lines_changed;
    if let Some(metadata) = req.metadata {
        obs.metadata = metadata;
    }

    // Store the observation
    state
        .tauri_state
        .db
        .add_coding_observation(obs.clone())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Update session stats
    state
        .tauri_state
        .db
        .update_coding_session(session_id_u128, |s| {
            s.add_observation(&obs);
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AddObservationResponse {
        observation_id: format!("{:032x}", observation_id),
        sequence,
    }))
}

/// GET /api/v1/coding-sessions/:session_id/observations - List observations for a session
async fn list_observations_handler(
    AxumState(state): AxumState<ServerState>,
    Path(session_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(100);
    let action_filter = params.get("action").cloned();

    // Parse session_id from hex string
    let session_id_u128 = u128::from_str_radix(&session_id, 16)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID format".to_string()))?;

    // Get observations from storage
    let all_observations = state
        .tauri_state
        .db
        .get_coding_observations(session_id_u128)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply action filter if specified
    let mut observations: Vec<ObservationResponse> = all_observations
        .into_iter()
        .filter(|o| {
            if let Some(ref action) = action_filter {
                o.action.as_str() == action.as_str()
            } else {
                true
            }
        })
        .map(|o| o.into())
        .collect();

    // Sort by sequence
    observations.sort_by(|a, b| a.sequence.cmp(&b.sequence));

    // Apply limit
    observations.truncate(limit);

    Ok(Json(serde_json::json!({
        "observations": observations,
        "total": observations.len()
    })))
}

/// POST /api/v1/coding-sessions/:session_id/summarize - Generate session summary
async fn summarize_coding_session_handler(
    AxumState(state): AxumState<ServerState>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Parse session_id from hex string
    let session_id_u128 = u128::from_str_radix(&session_id, 16)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid session ID format".to_string()))?;

    // Get the session
    let session = state
        .tauri_state
        .db
        .get_coding_session(session_id_u128)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    // Get all observations for the session
    let observations = state
        .tauri_state
        .db
        .get_coding_observations(session_id_u128)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut files_modified = std::collections::HashSet::new();
    let mut files_read = std::collections::HashSet::new();

    for obs in &observations {
        if let Some(ref path) = obs.file_path {
            match obs.action {
                ToolAction::Read => { files_read.insert(path.clone()); }
                ToolAction::Edit | ToolAction::Create => { files_modified.insert(path.clone()); }
                _ => {}
            }
        }
    }

    // Create a basic summary (could be enhanced with LLM summarization later)
    let summary = SessionSummary {
        title: format!("Coding session with {}", session.agent_name),
        description: format!(
            "Session in {} with {} observations ({} file reads, {} edits, {} bash commands)",
            session.working_directory,
            session.observation_count,
            session.file_reads,
            session.file_edits,
            session.bash_commands
        ),
        accomplishments: Vec::new(), // Would be filled by LLM analysis
        files_modified: files_modified.into_iter().collect(),
        files_read: files_read.into_iter().collect(),
        concepts: Vec::new(),
        decisions: Vec::new(),
        follow_ups: Vec::new(),
        generated_at_us: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0),
    };

    // Update session with summary
    let summary_clone = summary.clone();
    state
        .tauri_state
        .db
        .update_coding_session(session_id_u128, move |s| {
            s.summary = Some(summary_clone);
            s.state = SessionState::Summarized;
            s.end_time_us = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_micros() as u64)
                    .unwrap_or(0)
            );
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    info!("Generated summary for coding session {}", session_id);

    Ok(Json(serde_json::json!({
        "summary": summary,
        "session_id": session_id
    })))
}

/// GET /api/v1/context - Get context for injection into coding agents
/// 
/// Generates a context block with recent session history, files worked on,
/// and relevant patterns for the coding agent to use.
/// 
/// Query params:
/// - project_id: Filter by project (optional)
/// - working_directory: Filter by working directory (optional)
/// - limit: Number of recent sessions to include (default: 5)
/// - format: Output format - "markdown" (default) or "json"
async fn get_context_handler(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let project_id = params.get("project_id").and_then(|s| s.parse::<u16>().ok());
    let working_directory = params.get("working_directory").cloned();
    let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(5);
    let format = params.get("format").cloned().unwrap_or_else(|| "markdown".to_string());

    // Get recent sessions
    let sessions = state
        .tauri_state
        .db
        .list_coding_sessions(project_id, limit * 2) // Get extra to filter
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Filter by working directory if specified
    let filtered_sessions: Vec<_> = sessions
        .into_iter()
        .filter(|s| {
            if let Some(ref wd) = working_directory {
                s.working_directory.starts_with(wd) || wd.starts_with(&s.working_directory)
            } else {
                true
            }
        })
        .take(limit)
        .collect();

    // Collect files and patterns from sessions
    let mut files_modified: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut files_read: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut decisions: Vec<String> = Vec::new();
    let mut recent_accomplishments: Vec<String> = Vec::new();

    for session in &filtered_sessions {
        // Get observations for this session
        if let Ok(observations) = state.tauri_state.db.get_coding_observations(session.session_id) {
            for obs in observations {
                if let Some(ref path) = obs.file_path {
                    match obs.action {
                        ToolAction::Read => { files_read.insert(path.clone()); }
                        ToolAction::Edit | ToolAction::Create => { files_modified.insert(path.clone()); }
                        _ => {}
                    }
                }
            }
        }

        // Extract from summaries
        if let Some(ref summary) = session.summary {
            decisions.extend(summary.decisions.iter().cloned());
            recent_accomplishments.extend(summary.accomplishments.iter().cloned());
        }
    }

    // Limit collections
    let files_modified: Vec<_> = files_modified.into_iter().take(20).collect();
    let files_read: Vec<_> = files_read.into_iter().take(20).collect();
    decisions.truncate(10);
    recent_accomplishments.truncate(10);

    if format == "json" {
        // Return JSON format
        let context = serde_json::json!({
            "context_version": "1.0",
            "generated_at": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            "sessions": filtered_sessions.iter().map(|s| {
                serde_json::json!({
                    "id": format!("{:032x}", s.session_id),
                    "agent": s.agent_name,
                    "working_directory": s.working_directory,
                    "start_time": s.start_time_us / 1_000_000,
                    "observation_count": s.observation_count,
                    "summary": s.summary.as_ref().map(|sum| sum.description.clone()),
                })
            }).collect::<Vec<_>>(),
            "files_modified": files_modified,
            "files_read": files_read,
            "decisions": decisions,
            "recent_accomplishments": recent_accomplishments,
        });
        Ok(Json(context).into_response())
    } else {
        // Return Markdown format for context injection
        let mut md = String::new();
        md.push_str("<agentreplay-context>\n");
        md.push_str("# Agent Replay Context\n\n");
        md.push_str("The following is recalled context from previous coding sessions.\n\n");

        // Recent sessions
        if !filtered_sessions.is_empty() {
            md.push_str("## Recent Sessions\n\n");
            for session in &filtered_sessions {
                let start_time = session.start_time_us / 1_000_000;
                let datetime = chrono::DateTime::from_timestamp(start_time as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                
                md.push_str(&format!("- **{}** ({}) - {} observations\n", 
                    session.agent_name,
                    datetime,
                    session.observation_count
                ));
                
                if let Some(ref summary) = session.summary {
                    md.push_str(&format!("  {}\n", summary.description));
                }
            }
            md.push('\n');
        }

        // Recent decisions
        if !decisions.is_empty() {
            md.push_str("## Recent Decisions\n\n");
            for decision in &decisions {
                md.push_str(&format!("- {}\n", decision));
            }
            md.push('\n');
        }

        // Recent accomplishments
        if !recent_accomplishments.is_empty() {
            md.push_str("## Recent Accomplishments\n\n");
            for acc in &recent_accomplishments {
                md.push_str(&format!("- {}\n", acc));
            }
            md.push('\n');
        }

        // Files worked on
        if !files_modified.is_empty() {
            md.push_str("## Recently Modified Files\n\n");
            for file in &files_modified {
                md.push_str(&format!("- {}\n", file));
            }
            md.push('\n');
        }

        if !files_read.is_empty() {
            md.push_str("## Recently Read Files\n\n");
            for file in files_read.iter().take(10) {
                md.push_str(&format!("- {}\n", file));
            }
            md.push('\n');
        }

        md.push_str("</agentreplay-context>\n");

        Ok((
            [(axum::http::header::CONTENT_TYPE, "text/markdown")],
            md
        ).into_response())
    }
}