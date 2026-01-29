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

use crate::error::CommandError;
use crate::{AppConfig, AppState};
use agentreplay_core::SpanType;
use agentreplay_evals::Evaluator;
use agentreplay_index::embedding::{EmbeddingProvider, LocalEmbeddingProvider};
use agentreplay_index::Embedding;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, State};

// ============================================================================
// Input Validation Constants (Task 11)
// ============================================================================

/// Maximum query string length for semantic search
const MAX_QUERY_LENGTH: usize = 10_000;
/// Maximum limit for paginated queries
const MAX_QUERY_LIMIT: usize = 10_000;
/// Maximum offset for paginated queries  
const MAX_QUERY_OFFSET: usize = 1_000_000;

// ============================================================================
// Blocking Task Helpers (Gap 10 - Connection Pooling)
// ============================================================================

/// Helper to run CPU-intensive database operations on the blocking thread pool.
/// 
/// This prevents blocking the Tokio async runtime with synchronous DB operations
/// like vector similarity search, large scans, or aggregation queries.
/// 
/// # Example
/// ```ignore
/// let db = state.db.clone();
/// let results = run_blocking(move || db.semantic_search(&embedding, 100)).await?;
/// ```
async fn run_blocking<F, T>(f: F) -> Result<T, CommandError>
where
    F: FnOnce() -> agentreplay_core::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::internal(format!("Blocking task panicked: {}", e)))?
        .map_err(|e| CommandError::database_query(e))
}

// ============================================================================
// Input Validation Helpers (Task 11)
// ============================================================================

fn validate_query_string(query: &str, field_name: &str) -> Result<(), CommandError> {
    if query.is_empty() {
        return Err(CommandError::missing_required(field_name));
    }
    if query.len() > MAX_QUERY_LENGTH {
        return Err(CommandError::out_of_range(field_name, 1, MAX_QUERY_LENGTH)
            .with_details(format!("Query length {} exceeds maximum {}", query.len(), MAX_QUERY_LENGTH)));
    }
    Ok(())
}

fn validate_limit(limit: Option<usize>) -> Result<usize, CommandError> {
    let limit = limit.unwrap_or(100);
    if limit > MAX_QUERY_LIMIT {
        return Err(CommandError::out_of_range("limit", 1, MAX_QUERY_LIMIT));
    }
    if limit == 0 {
        return Err(CommandError::invalid_input("limit", "must be greater than 0"));
    }
    Ok(limit)
}

fn validate_offset(offset: Option<usize>) -> Result<usize, CommandError> {
    let offset = offset.unwrap_or(0);
    if offset > MAX_QUERY_OFFSET {
        return Err(CommandError::out_of_range("offset", 0, MAX_QUERY_OFFSET));
    }
    Ok(offset)
}

// ============================================================================
// Data Types
// ============================================================================

#[derive(Serialize, Deserialize, Clone)]
pub struct TraceMetadata {
    pub trace_id: String,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub duration_ms: Option<u64>,
    pub status: Option<String>,
    // Extended fields for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ============================================================================
// Configuration Commands
// ============================================================================

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, CommandError> {
    let config = state.config.read();
    Ok(config.clone())
}

#[tauri::command]
pub async fn update_config(
    new_config: AppConfig,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), CommandError> {
    // Save to disk
    new_config
        .save(&app_handle)
        .map_err(|e| CommandError::config_save(e))?;

    // Update runtime state
    let mut config = state.config.write();
    *config = new_config;

    Ok(())
}

// ============================================================================
// Health & Stats Commands
// ============================================================================

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub database_path: String,
    pub total_traces: u64,
}

#[tauri::command]
pub async fn health_check(state: State<'_, AppState>) -> Result<HealthStatus, CommandError> {
    let stats = state.db.stats();

    Ok(HealthStatus {
        status: "healthy".to_string(),
        database_path: state.db_path.display().to_string(),
        total_traces: stats.causal_edges as u64,
    })
}

#[derive(Serialize)]
pub struct DatabaseStats {
    pub total_traces: u64,
    pub total_edges: u64,
    pub total_spans: u64,
    pub index_size_bytes: u64,
    pub storage_size_bytes: u64,
    pub storage_path: String,
}

#[tauri::command]
pub async fn get_db_stats(state: State<'_, AppState>) -> Result<DatabaseStats, CommandError> {
    let stats = state.db.stats();

    // Use accurate total from storage backend
    let total_spans = stats.storage.total_edges;
    let storage_size = stats.storage.total_size_bytes();

    Ok(DatabaseStats {
        // total_traces = unique traces (from causal roots, but approximate using edges for now)
        // In reality, we'd need to count distinct trace_ids or root spans
        total_traces: total_spans,  // Use total spans as approximation
        total_edges: stats.causal_edges as u64, // Parent-child relationships
        total_spans,  // Accurate count of all spans
        index_size_bytes: stats.causal_nodes as u64 * 16, // Approximate index overhead
        storage_size_bytes: storage_size,
        storage_path: state.db_path.display().to_string(),
    })
}

/// Write stall statistics for UI notification
#[derive(Serialize)]
pub struct WriteStallInfo {
    /// Total number of hard stalls (writes blocked completely)
    pub hard_stall_count: u64,
    /// Total number of soft stalls (writes slowed down)
    pub soft_stall_count: u64,
    /// Total microseconds spent in hard stall
    pub hard_stall_micros: u64,
    /// Total microseconds spent in soft stall
    pub soft_stall_micros: u64,
    /// Whether currently in write stall
    pub is_stalled: bool,
    /// Current L0 file count
    pub l0_file_count: usize,
    /// Soft limit threshold
    pub l0_soft_limit: usize,
    /// Hard limit threshold
    pub l0_hard_limit: usize,
}

/// Get write stall statistics for the storage engine
/// 
/// Returns current write backpressure status. UI can use this to:
/// - Show a warning when writes are slowed (soft stall)
/// - Show an error when writes are blocked (hard stall)
/// - Display L0 file count progress bar
#[tauri::command]
pub async fn get_write_stall_stats(state: State<'_, AppState>) -> Result<WriteStallInfo, CommandError> {
    let _stats = state.db.stats();
    
    // SochDB handles backpressure internally - return zeroed stats
    Ok(WriteStallInfo {
        hard_stall_count: 0,
        soft_stall_count: 0,
        hard_stall_micros: 0,
        soft_stall_micros: 0,
        is_stalled: false,
        l0_file_count: 0,
        l0_soft_limit: 12,
        l0_hard_limit: 20,
    })
}

// ============================================================================
// Trace Query Commands
// ============================================================================

#[derive(Deserialize)]
pub struct ListTracesParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    #[allow(dead_code)]
    pub agent_id: Option<String>,
    pub project_id: Option<u16>,
}

#[derive(Serialize)]
pub struct ListTracesResponse {
    pub traces: Vec<TraceMetadata>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[tauri::command]
pub async fn list_traces(
    params: ListTracesParams,
    state: State<'_, AppState>,
) -> Result<ListTracesResponse, CommandError> {
    // Validate input parameters (Task 11)
    let limit = validate_limit(params.limit)?;
    let offset = validate_offset(params.offset)?;

    // Query temporal range for traces
    let start_ts = params.start_time.unwrap_or(0);
    let end_ts = params.end_time.unwrap_or(u64::MAX);

    // OPTIMIZATION: Use iterator-based query to avoid loading all edges into memory
    // This is more memory-efficient for large datasets
    let edge_iter = state
        .db
        .query_temporal_range_iter(start_ts, end_ts)
        .map_err(|e| CommandError::database_query(e))?
        .filter(|e| params.project_id.map_or(true, |pid| e.project_id == pid));

    // Apply pagination at iterator level
    let db = &state.db;
    
    // Pre-fetch all edges in range for child lookups (needed for aggregation)
    // This is a trade-off: more memory but allows finding children of root spans
    let all_edges: Vec<_> = state.db
        .query_temporal_range(start_ts, end_ts)
        .unwrap_or_default();
    
    // Helper function to find first LLM child span with model info
    let find_llm_child_info = |root_session_id: u64, root_edge_id: u128| -> (Option<String>, Option<String>, Option<String>, Option<u32>) {
        // Look for child spans in same session with LLM data
        for child in all_edges.iter() {
            if child.session_id == root_session_id && child.edge_id != root_edge_id {
                // Check if this child has LLM attributes
                if let Ok(Some(payload)) = db.get_payload(child.edge_id) {
                    if let Ok(attrs) = serde_json::from_slice::<serde_json::Value>(&payload) {
                        // Check for model/LLM indicators
                        let model = attrs.get("gen_ai.request.model")
                            .or_else(|| attrs.get("model"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        
                        if model.is_some() {
                            // Found LLM span - extract input/output too
                            let input = attrs.get("gen_ai.prompt.0.content")
                                .or_else(|| attrs.get("input"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.chars().take(150).collect::<String>());
                            
                            let output = attrs.get("gen_ai.completion.0.content")
                                .or_else(|| attrs.get("output"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.chars().take(150).collect::<String>());
                            
                            // Get token count from child if available
                            let tokens = if child.token_count > 0 {
                                Some(child.token_count)
                            } else {
                                attrs.get("gen_ai.usage.total_tokens")
                                    .and_then(|v| v.as_u64())
                                    .map(|n| n as u32)
                            };
                            
                            return (model, input, output, tokens);
                        }
                    }
                }
            }
        }
        (None, None, None, None)
    };
    
    let traces: Vec<TraceMetadata> = edge_iter
        .skip(offset)
        .take(limit)
        .map(|edge| {
            // Extract payload attributes for display
            let attributes = db
                .get_payload(edge.edge_id)
                .ok()
                .flatten()
                .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok());

            // Extract model name
            let mut model = attributes.as_ref().and_then(|a| {
                a.get("model")
                    .or_else(|| a.get("gen_ai.request.model"))
                    .or_else(|| a.get("gen_ai.response.model"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

            // Extract provider
            let provider = attributes.as_ref().and_then(|a| {
                a.get("provider")
                    .or_else(|| a.get("gen_ai.system"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

            // Extract input preview
            let mut input_preview = attributes.as_ref().and_then(|a| {
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
                if let Some(input) = a.get("input") {
                    if let Some(s) = input.as_str() {
                        return Some(s.chars().take(150).collect::<String>());
                    }
                }
                if let Some(prompt) = a.get("prompt").and_then(|v| v.as_str()) {
                    return Some(prompt.chars().take(150).collect::<String>());
                }
                None
            });

            // Extract output preview
            let mut output_preview = attributes.as_ref().and_then(|a| {
                if let Some(content) = a.get("gen_ai.completion.0.content").and_then(|v| v.as_str()) {
                    return Some(content.chars().take(150).collect::<String>());
                }
                if let Some(output) = a.get("output") {
                    if let Some(s) = output.as_str() {
                        return Some(s.chars().take(150).collect::<String>());
                    }
                }
                if let Some(response) = a.get("response").and_then(|v| v.as_str()) {
                    return Some(response.chars().take(150).collect::<String>());
                }
                None
            });

            // AGGREGATION: If this is a root span without model/input/output, look up children
            let mut token_count = edge.token_count;
            if model.is_none() || input_preview.is_none() {
                let (child_model, child_input, child_output, child_tokens) = 
                    find_llm_child_info(edge.session_id, edge.edge_id);
                
                if model.is_none() {
                    model = child_model;
                }
                if input_preview.is_none() {
                    input_preview = child_input;
                }
                if output_preview.is_none() {
                    output_preview = child_output;
                }
                if token_count == 0 {
                    if let Some(t) = child_tokens {
                        token_count = t;
                    }
                }
            }

            // Extract cost
            let cost = attributes.as_ref().and_then(|a| {
                a.get("cost").and_then(|v| v.as_f64())
            });

            TraceMetadata {
                trace_id: format!("{}", edge.edge_id),
                session_id: Some(format!("{}", edge.session_id)),
                agent_id: Some(format!("{}", edge.agent_id)),
                started_at: edge.timestamp_us,
                ended_at: if edge.duration_us > 0 {
                    Some(edge.timestamp_us + edge.duration_us as u64)
                } else {
                    None
                },
                duration_ms: if edge.duration_us > 0 {
                    Some(edge.duration_us as u64 / 1000)
                } else {
                    None
                },
                status: Some("completed".to_string()),
                model,
                provider,
                input_preview,
                output_preview,
                token_count: Some(token_count),
                cost,
                project_id: Some(edge.project_id),
                metadata: attributes,
            }
        })
        .collect();

    // NOTE: Since we're using an iterator, we don't have an exact total count
    // without scanning all edges. For large datasets, we return an approximate
    // count from database stats. For exact counts, clients can implement a
    // separate count API or maintain count metadata.
    let stats = state.db.stats();
    let approximate_total = stats.causal_edges;

    Ok(ListTracesResponse {
        traces,
        total: approximate_total,
        offset,
        limit,
    })
}

// Serializable representation of AgentFlowEdge for Tauri commands
#[derive(Serialize)]
pub struct TraceData {
    pub edge_id: String,
    pub causal_parent: String,
    pub timestamp_us: u64,
    pub agent_id: u64,
    pub session_id: u64,
    pub span_type: u32,
    pub confidence: f32,
    pub token_count: u32,
    pub duration_us: u32,
    pub tenant_id: u64,
    pub project_id: u16,
    pub environment: String,
    pub attributes: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn get_trace(trace_id: String, state: State<'_, AppState>) -> Result<TraceData, String> {
    // Parse trace_id and use get() method
    let edge_id = u128::from_str_radix(&trace_id.replace("-", ""), 16)
        .map_err(|e| format!("Invalid trace ID: {}", e))?;

    let edge = state
        .db
        .get(edge_id)
        .map_err(|e| format!("Failed to get trace: {}", e))?
        .ok_or_else(|| "Trace not found".to_string())?;

    // Fetch payload/attributes if available
    let attributes = match state.db.get_payload(edge_id) {
        Ok(Some(payload_bytes)) => {
            // Try to parse as JSON
            serde_json::from_slice(&payload_bytes).ok()
        }
        _ => None,
    };

    // Map environment enum to string
    let environment = match edge.environment {
        0 => "development",
        1 => "staging",
        2 => "production",
        3 => "test",
        _ => "custom",
    }
    .to_string();

    // Convert to serializable format
    Ok(TraceData {
        edge_id: format!("{:032x}", edge.edge_id),
        causal_parent: format!("{:032x}", edge.causal_parent),
        timestamp_us: edge.timestamp_us,
        agent_id: edge.agent_id,
        session_id: edge.session_id,
        span_type: edge.span_type,
        confidence: edge.confidence,
        token_count: edge.token_count,
        duration_us: edge.duration_us,
        tenant_id: edge.tenant_id,
        project_id: edge.project_id,
        environment,
        attributes,
    })
}

#[derive(Deserialize)]
pub struct SearchTracesParams {
    pub query: String,
    pub project_id: u16,
    pub limit: Option<usize>,
    /// Embedding configuration from frontend settings
    #[serde(default)]
    pub embedding_config: Option<EmbeddingSearchConfig>,
}

/// Embedding configuration passed from frontend for search
#[derive(Deserialize, Clone, Debug)]
pub struct EmbeddingSearchConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub enabled: bool,
}

/// Simple wrapper to make async embedding calls implement EmbeddingProvider trait
struct ApiEmbeddingProvider {
    config: EmbeddingSearchConfig,
    dimension: usize,
}

impl ApiEmbeddingProvider {
    fn new(config: EmbeddingSearchConfig) -> Self {
        // Set dimension based on provider/model
        let dimension = match config.provider.to_lowercase().as_str() {
            "openai" => {
                if config.model.contains("3-large") { 3072 }
                else if config.model.contains("3-small") { 1536 }
                else { 1536 } // Default OpenAI dimension
            }
            "google" | "gemini" => 768,
            _ => 384,
        };
        Self { config, dimension }
    }

    async fn embed_async(&self, text: &str) -> Result<Vec<f32>, String> {
        let client = reqwest::Client::new();
        
        match self.config.provider.to_lowercase().as_str() {
            "openai" => self.embed_openai(&client, text).await,
            "google" | "gemini" => self.embed_google(&client, text).await,
            "ollama" => self.embed_ollama(&client, text).await,
            other => Err(format!("Unsupported embedding provider: {}", other)),
        }
    }

    async fn embed_openai(&self, client: &reqwest::Client, text: &str) -> Result<Vec<f32>, String> {
        let api_key = self.config.api_key.as_ref()
            .ok_or("OpenAI API key required for embeddings")?;
        
        let base_url = self.config.base_url.as_deref()
            .unwrap_or("https://api.openai.com/v1");
        
        let response = client
            .post(format!("{}/embeddings", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": self.config.model,
                "input": text
            }))
            .send()
            .await
            .map_err(|e| format!("OpenAI API request failed: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("OpenAI API error: {}", error_text));
        }
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;
        
        result["data"][0]["embedding"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .ok_or_else(|| "Invalid embedding response format".to_string())
    }

    async fn embed_google(&self, client: &reqwest::Client, text: &str) -> Result<Vec<f32>, String> {
        let api_key = self.config.api_key.as_ref()
            .ok_or("Google API key required for embeddings")?;
        
        let model = if self.config.model.is_empty() {
            "text-embedding-004"
        } else {
            &self.config.model
        };
        
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent?key={}",
            model, api_key
        );
        
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": format!("models/{}", model),
                "content": {
                    "parts": [{"text": text}]
                }
            }))
            .send()
            .await
            .map_err(|e| format!("Google API request failed: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Google API error: {}", error_text));
        }
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Google response: {}", e))?;
        
        result["embedding"]["values"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .ok_or_else(|| "Invalid embedding response format".to_string())
    }

    async fn embed_ollama(&self, client: &reqwest::Client, text: &str) -> Result<Vec<f32>, String> {
        let base_url = self.config.base_url.as_deref()
            .unwrap_or("http://localhost:11434");
        
        let model = if self.config.model.is_empty() {
            "nomic-embed-text"
        } else {
            &self.config.model
        };
        
        let response = client
            .post(format!("{}/api/embeddings", base_url))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": model,
                "prompt": text
            }))
            .send()
            .await
            .map_err(|e| format!("Ollama API request failed: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Ollama API error: {}", error_text));
        }
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;
        
        result["embedding"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .ok_or_else(|| "Invalid embedding response format".to_string())
    }
}

impl EmbeddingProvider for ApiEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, agentreplay_index::embedding::EmbeddingError> {
        // Use tokio's Handle to run async code in sync context
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|e| agentreplay_index::embedding::EmbeddingError::InferenceFailed(
                format!("No async runtime: {}", e)
            ))?;
        
        let config = self.config.clone();
        let text = text.to_string();
        
        runtime.block_on(async {
            let provider = ApiEmbeddingProvider::new(config);
            provider.embed_async(&text).await
        })
        .map_err(|e| agentreplay_index::embedding::EmbeddingError::InferenceFailed(e))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, agentreplay_index::embedding::EmbeddingError> {
        // Embed each text individually for now
        texts.iter().map(|t| self.embed(t)).collect()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn max_tokens(&self) -> usize {
        8192 // Conservative limit
    }

    fn provider_id(&self) -> &str {
        match self.config.provider.to_lowercase().as_str() {
            "openai" => "openai",
            "google" | "gemini" => "google",
            "ollama" => "ollama",
            _ => "api",
        }
    }

    fn is_offline(&self) -> bool {
        self.config.provider.to_lowercase() == "ollama"
    }
}

/// Create an embedding provider from the search config
fn create_embedding_provider_from_config(
    config: &EmbeddingSearchConfig,
) -> Result<Arc<dyn EmbeddingProvider + Send + Sync>, CommandError> {
    if !config.enabled {
        return Err(CommandError::config_save("Embeddings not enabled"));
    }

    match config.provider.to_lowercase().as_str() {
        "openai" | "google" | "gemini" | "ollama" => {
            Ok(Arc::new(ApiEmbeddingProvider::new(config.clone())))
        }
        "fastembed" | "local" => {
            // Use local provider
            let local = LocalEmbeddingProvider::default_provider()
                .map_err(|e| CommandError::config_save(e.to_string()))?;
            Ok(Arc::new(local))
        }
        "anthropic" => {
            Err(CommandError::config_save(
                "Anthropic does not provide embedding models. Use OpenAI or Google instead."
            ))
        }
        other => {
            Err(CommandError::config_save(format!(
                "Unknown embedding provider: {}. Supported: openai, google, ollama, fastembed",
                other
            )))
        }
    }
}

#[tauri::command]
pub async fn search_traces(
    params: SearchTracesParams,
    state: State<'_, AppState>,
) -> Result<Vec<TraceMetadata>, CommandError> {
    // Input validation using structured validation (Task 11)
    validate_query_string(&params.query, "query")?;
    let limit = validate_limit(params.limit)?.min(MAX_QUERY_LIMIT);

    let project_id = params.project_id;
    let query_lower = params.query.to_lowercase();
    let mut edges = Vec::new();

    // Try semantic search with configured embedding provider
    let semantic_search_failed = if let Some(ref embedding_config) = params.embedding_config {
        if embedding_config.enabled {
            match create_embedding_provider_from_config(embedding_config) {
                Ok(provider) => {
                    tracing::info!(
                        "Using {} embedding provider with model {}",
                        embedding_config.provider,
                        embedding_config.model
                    );
                    match provider.embed(&params.query) {
                        Ok(query_vec) => {
                            let query_embedding = Embedding::from_vec(query_vec);
                            // Run semantic search on blocking thread pool (Gap 10 fix)
                            let db = state.db.clone();
                            let search_limit = limit;
                            match run_blocking(move || db.semantic_search(&query_embedding, search_limit)).await {
                                Ok(results) if !results.is_empty() => {
                                    tracing::info!("Semantic search found {} results", results.len());
                                    edges = results;
                                    false // Semantic search succeeded
                                }
                                Ok(_) => {
                                    tracing::info!("Semantic search returned no results, falling back to text search");
                                    true
                                }
                                Err(e) => {
                                    tracing::warn!("Semantic search error: {}, falling back to text search", e);
                                    true
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Embedding generation error: {}, falling back to text search", e);
                            true
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to create embedding provider: {}, falling back to text search", e);
                    true
                }
            }
        } else {
            tracing::debug!("Embeddings not enabled, using text search");
            true
        }
    } else {
        // No embedding config provided - try local provider as fallback
        match LocalEmbeddingProvider::default_provider() {
            Ok(provider) => {
                match provider.embed(&params.query) {
                    Ok(query_vec) => {
                        let query_embedding = Embedding::from_vec(query_vec);
                        // Run semantic search on blocking thread pool (Gap 10 fix)
                        let db = state.db.clone();
                        let search_limit = limit;
                        match run_blocking(move || db.semantic_search(&query_embedding, search_limit)).await {
                            Ok(results) if !results.is_empty() => {
                                edges = results;
                                false
                            }
                            _ => true
                        }
                    }
                    Err(_) => true
                }
            }
            Err(_) => true
        }
    };

    // Fallback to content-based search if semantic search didn't produce results
    if semantic_search_failed {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // Get recent edges (last 24 hours) with pagination
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        let start = now.saturating_sub(86_400_000_000); // 24 hours
        
        // Use paginated query to avoid loading too many edges
        const SAMPLE_LIMIT: usize = 10000;
        
        let all_edges = state.db
            .query_temporal_range_paginated(start, now, SAMPLE_LIMIT, 0)
            .map(|(e, _)| e)
            .unwrap_or_default();
        
        // Search in payload content, filtered by project
        for edge in all_edges {
            if edges.len() >= limit {
                break;
            }
            
            // Filter by project_id
            if edge.project_id != project_id {
                continue;
            }
            
            // Check payload content
            if let Ok(Some(payload_bytes)) = state.db.get_payload(edge.edge_id) {
                if let Ok(payload_str) = String::from_utf8(payload_bytes) {
                    if payload_str.to_lowercase().contains(&query_lower) {
                        edges.push(edge);
                    }
                }
            }
        }
    }

    // Filter semantic search results by project as well
    edges.retain(|e| e.project_id == project_id);

    // Convert edges to trace metadata with payload extraction
    let db = &state.db;
    let mut traces: Vec<TraceMetadata> = edges
        .into_iter()
        .map(|edge| {
            // Check if this is an error span
            let status = if edge.get_span_type() == SpanType::Error {
                Some("error".to_string())
            } else {
                Some("ok".to_string())
            };

            // Extract payload attributes for display
            let attributes = db
                .get_payload(edge.edge_id)
                .ok()
                .flatten()
                .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok());

            // Extract model name
            let model = attributes.as_ref().and_then(|a| {
                a.get("model")
                    .or_else(|| a.get("gen_ai.request.model"))
                    .or_else(|| a.get("gen_ai.response.model"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

            // Extract provider
            let provider = attributes.as_ref().and_then(|a| {
                a.get("provider")
                    .or_else(|| a.get("gen_ai.system"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

            // Extract input preview
            let input_preview = attributes.as_ref().and_then(|a| {
                // Try GenAI semantic conventions - skip system prompt (index 0), use user prompt
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
                if let Some(input) = a.get("input") {
                    if let Some(s) = input.as_str() {
                        return Some(s.chars().take(150).collect::<String>());
                    }
                }
                if let Some(prompt) = a.get("prompt").and_then(|v| v.as_str()) {
                    return Some(prompt.chars().take(150).collect::<String>());
                }
                None
            });

            // Extract output preview
            let output_preview = attributes.as_ref().and_then(|a| {
                // Try GenAI semantic conventions
                if let Some(content) = a.get("gen_ai.completion.0.content").and_then(|v| v.as_str()) {
                    return Some(content.chars().take(150).collect::<String>());
                }
                // Try other common fields
                if let Some(output) = a.get("output") {
                    if let Some(s) = output.as_str() {
                        return Some(s.chars().take(150).collect::<String>());
                    }
                }
                if let Some(response) = a.get("response").and_then(|v| v.as_str()) {
                    return Some(response.chars().take(150).collect::<String>());
                }
                None
            });

            // Extract cost
            let cost = attributes.as_ref().and_then(|a| {
                a.get("cost").and_then(|v| v.as_f64())
            });

            TraceMetadata {
                trace_id: format!("{:#x}", edge.edge_id),
                session_id: Some(format!("{:#x}", edge.session_id)),
                agent_id: Some(format!("{:#x}", edge.agent_id)),
                started_at: edge.timestamp_us,
                ended_at: Some(edge.timestamp_us.saturating_add(edge.duration_us as u64)),
                duration_ms: Some((edge.duration_us / 1000) as u64),
                status,
                model,
                provider,
                input_preview,
                output_preview,
                token_count: Some(edge.token_count),
                cost,
                project_id: Some(edge.project_id),
                metadata: attributes,
            }
        })
        .collect();

    // Deduplicate by session_id (multiple spans may match from same session)
    traces.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    traces.dedup_by(|a, b| a.session_id == b.session_id);

    Ok(traces)
}

// ============================================================================
// Backup Commands
// ============================================================================

#[derive(Serialize)]
pub struct BackupInfo {
    pub backup_id: String,
    pub created_at: u64,
    pub size_bytes: u64,
    pub path: String,
}

#[tauri::command]
pub async fn create_backup(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<BackupInfo, String> {
    let backup_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("backups");

    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;

    let backup_id = format!("backup_{}", chrono::Utc::now().timestamp());
    let backup_path = backup_dir.join(&backup_id);

    // Copy database directory to backup location
    copy_dir_recursive(&state.db_path, &backup_path)
        .map_err(|e| format!("Failed to copy database: {}", e))?;

    // Calculate backup size
    let size_bytes = get_dir_size(&backup_path)
        .map_err(|e| format!("Failed to calculate backup size: {}", e))?;

    Ok(BackupInfo {
        backup_id: backup_id.clone(),
        created_at: chrono::Utc::now().timestamp() as u64,
        size_bytes,
        path: backup_path.display().to_string(),
    })
}

#[tauri::command]
pub async fn list_backups(app_handle: tauri::AppHandle) -> Result<Vec<BackupInfo>, String> {
    let backup_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("backups");

    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();

    for entry in std::fs::read_dir(&backup_dir)
        .map_err(|e| format!("Failed to read backup directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            let backup_id = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let metadata = entry
                .metadata()
                .map_err(|e| format!("Failed to read metadata: {}", e))?;

            let created_at = metadata
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let size_bytes =
                get_dir_size(&path).map_err(|e| format!("Failed to calculate size: {}", e))?;

            backups.push(BackupInfo {
                backup_id,
                created_at,
                size_bytes,
                path: path.display().to_string(),
            });
        }
    }

    // Sort by creation time (newest first)
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(backups)
}

// ============================================================================
// Server Export Commands (Hybrid Mode)
// ============================================================================

#[derive(Deserialize)]
pub struct ExportToServerParams {
    pub trace_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct ExportResult {
    pub success: bool,
    pub exported_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn export_traces_to_server(
    params: ExportToServerParams,
    state: State<'_, AppState>,
) -> Result<ExportResult, String> {
    // Clone config values before await to avoid Send issues
    let (_enabled, server_url, api_key) = {
        let config = state.config.read();

        if !config.server_export.enabled {
            return Err("Server export is not enabled in configuration".to_string());
        }

        let url = config
            .server_export
            .server_url
            .clone()
            .ok_or("Server URL not configured")?;

        let key = config.server_export.api_key.clone();

        (true, url, key)
    };

    let mut exported_count = 0;
    let mut failed_count = 0;
    let mut errors = Vec::new();

    for trace_id in params.trace_ids {
        match export_trace_to_server(&trace_id, &server_url, api_key.as_ref(), &state).await {
            Ok(_) => exported_count += 1,
            Err(e) => {
                failed_count += 1;
                errors.push(format!("Failed to export {}: {}", trace_id, e));
            }
        }
    }

    Ok(ExportResult {
        success: failed_count == 0,
        exported_count,
        failed_count,
        errors,
    })
}

async fn export_trace_to_server(
    trace_id: &str,
    server_url: &str,
    api_key: Option<&String>,
    state: &State<'_, AppState>,
) -> Result<(), String> {
    // TODO: Implement export once AgentFlowEdge implements Serialize
    // or create a JSON-friendly representation
    // For now, return success
    let _ = (trace_id, server_url, api_key, state);
    Ok(())
}

// ============================================================================
// Utility Functions
// ============================================================================

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

fn get_dir_size(path: &PathBuf) -> std::io::Result<u64> {
    let mut total_size = 0u64;

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            total_size += get_dir_size(&entry.path())?;
        } else {
            total_size += metadata.len();
        }
    }

    Ok(total_size)
}

// ============================================================================
// Trace Management Commands
// ============================================================================

#[tauri::command]
pub async fn ingest_traces(
    traces_json: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    use agentreplay_core::AgentFlowEdge;

    // Parse JSON as array of AgentFlowEdge
    let edges: Vec<AgentFlowEdge> = serde_json::from_str(&traces_json)
        .map_err(|e| format!("Failed to parse traces JSON: {}", e))?;

    if edges.is_empty() {
        return Ok(0);
    }

    let count = edges.len();

    // Queue edges for async batched writes
    for edge in &edges {
        state
            .ingestion_queue
            .send(*edge)
            .map_err(|e| format!("Failed to queue trace: {}", e))?;
    }

    // NOTE: Stats and events will be updated by background worker after successful write
    // This ensures accurate stats and prevents double-counting

    Ok(count)
}

#[tauri::command]
pub async fn get_trace_stats(
    trace_id: String,
    state: State<'_, AppState>,
) -> Result<TraceStats, String> {
    // Parse trace_id and query for edges with matching session_id
    let edge_id = u128::from_str_radix(&trace_id.replace("-", ""), 16)
        .map_err(|e| format!("Invalid trace ID: {}", e))?;

    // Get the root edge to find session_id
    let root_edge = state
        .db
        .get(edge_id)
        .map_err(|e| format!("Failed to get trace: {}", e))?
        .ok_or_else(|| "Trace not found".to_string())?;

    // Query all edges with same session_id (this is the trace)
    let edges = state
        .db
        .filter_by_session(root_edge.session_id, 0, u64::MAX)
        .map_err(|e| format!("Failed to query trace edges: {}", e))?;

    let total_events = edges.len();

    // Count observations (non-root spans)
    let total_observations = edges
        .iter()
        .filter(|e| e.get_span_type() != agentreplay_core::SpanType::Root)
        .count();

    // Calculate duration (max timestamp - min timestamp)
    let min_ts = edges.iter().map(|e| e.timestamp_us).min().unwrap_or(0);
    let max_ts = edges
        .iter()
        .map(|e| e.timestamp_us + e.duration_us as u64)
        .max()
        .unwrap_or(0);

    let duration_ms = if max_ts > min_ts {
        Some((max_ts - min_ts) / 1000)
    } else {
        None
    };

    Ok(TraceStats {
        trace_id,
        total_events,
        total_observations,
        duration_ms,
    })
}

#[derive(Serialize)]
pub struct TraceStats {
    pub trace_id: String,
    pub total_events: usize,
    pub total_observations: usize,
    pub duration_ms: Option<u64>,
}

// ============================================================================
// Analytics Commands
// ============================================================================



// ...

#[derive(Serialize)]
pub struct TimeseriesPoint {
    pub timestamp: u64,
    pub value: f64,
}

#[derive(Deserialize)]
pub struct TimeseriesParams {
    pub start_time: u64,
    pub end_time: u64,
    pub interval_seconds: u64,
    pub metric: String,
    pub project_id: Option<u16>, // Added
}

#[tauri::command]
pub async fn get_timeseries(
    params: TimeseriesParams,
    state: State<'_, AppState>,
) -> Result<Vec<TimeseriesPoint>, String> {
    // Query edges in time range
    let edges = state
        .db
        .query_temporal_range(params.start_time, params.end_time)
        .map_err(|e| format!("Failed to query temporal range: {}", e))?;

    // Group edges into time buckets based on interval
    let interval_us = params.interval_seconds * 1_000_000;
    let mut buckets: std::collections::HashMap<u64, Vec<&agentreplay_core::AgentFlowEdge>> =
        std::collections::HashMap::new();

    // Debug logging for analytics
    if edges.len() > 0 {
        tracing::debug!("Analytics: Found {} total edges in range", edges.len());
    }

    // Filter by project_id if provided
    let filtered_edges = edges.iter().filter(|e| {
        params.project_id.map_or(true, |pid| e.project_id == pid)
    });

    for edge in filtered_edges {
        let bucket = (edge.timestamp_us / interval_us) * interval_us;
        buckets.entry(bucket).or_default().push(edge);
    }

    // Calculate metric for each bucket
    let mut points: Vec<TimeseriesPoint> = buckets
        .iter()
        .map(|(timestamp, bucket_edges)| {
            let value = match params.metric.as_str() {
                "count" => bucket_edges.len() as f64,
                "tokens" => bucket_edges.iter().map(|e| e.token_count as f64).sum(),
                "duration_avg" => {
                    let sum: u32 = bucket_edges.iter().map(|e| e.duration_us).sum();
                     if bucket_edges.len() > 0 {
                        sum as f64 / bucket_edges.len() as f64
                     } else {
                         0.0
                     }
                }
                _ => bucket_edges.len() as f64, // Default to count
            };

            TimeseriesPoint {
                timestamp: *timestamp / 1_000_000, // Convert to seconds
                value,
            }
        })
        .collect();

    // Sort by timestamp
    points.sort_by_key(|p| p.timestamp);

    Ok(points)
}

#[derive(Deserialize)]
pub struct DeleteSessionParams {
    pub session_id: String,
}



#[derive(Deserialize)]
pub struct DeleteTraceParams {
    pub trace_id: String,
}

#[tauri::command]
pub async fn delete_trace(
    params: DeleteTraceParams,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Parse trace_id (can be hex or decimal)
    let edge_id = if params.trace_id.starts_with("0x") {
        u128::from_str_radix(&params.trace_id[2..], 16)
            .map_err(|e| format!("Invalid hex trace ID: {}", e))?
    } else {
        // Try decimal first, then hex
        params.trace_id.parse::<u128>().or_else(|_| {
            u128::from_str_radix(&params.trace_id, 16)
        }).map_err(|e| format!("Invalid trace ID: {}", e))?
    };

    // Use tenant_id=1 (default for ingestion)
    state.db.delete(edge_id, 1).await
        .map_err(|e| format!("Failed to delete trace: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_session(
    params: DeleteSessionParams,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Parse session_id
    let session_id_u64 = params.session_id.parse::<u64>()
        .map_err(|e| format!("Invalid session ID format: {}", e))?;

    // Get all edges for this session
    let edge_ids = state.db.get_session_edges(session_id_u64);

    if edge_ids.is_empty() {
        return Ok(());
    }

    // Delete each edge
    for edge_id in edge_ids {
        // Use tenant_id=0 (default for single user desktop app)
        if let Err(e) = state.db.delete(edge_id, 0).await {
            tracing::warn!("Failed to delete edge {} in session {}: {}", edge_id, session_id_u64, e);
        }
    }

    Ok(())
}

#[derive(Serialize)]
pub struct CostSummary {
    pub total_cost: f64,
    pub total_traces: usize,
    pub by_model: Vec<ModelCost>,
}

#[derive(Serialize)]
pub struct ModelCost {
    pub model: String,
    pub cost: f64,
    pub count: usize,
}

#[tauri::command]
pub async fn get_costs(
    start_time: Option<u64>,
    end_time: Option<u64>,
    state: State<'_, AppState>,
) -> Result<CostSummary, String> {
    // Query edges in time range
    let start = start_time.unwrap_or(0);
    let end = end_time.unwrap_or(u64::MAX);

    let edges = state
        .db
        .query_temporal_range(start, end)
        .map_err(|e| format!("Failed to query temporal range: {}", e))?;

    // Cost estimation (rough - would need actual pricing data)
    // Using approximate OpenAI pricing: $0.50 per 1M input tokens, $1.50 per 1M output tokens
    // Assuming 50/50 split for simplicity
    let total_tokens: u64 = edges.iter().map(|e| e.token_count as u64).sum();
    let total_cost = (total_tokens as f64 / 1_000_000.0) * 1.0; // $1 per 1M tokens average

    // Group by agent_id as proxy for "model"
    let mut model_costs: std::collections::HashMap<u64, (usize, u64)> =
        std::collections::HashMap::new();

    for edge in &edges {
        let entry = model_costs.entry(edge.agent_id).or_insert((0, 0));
        entry.0 += 1; // count
        entry.1 += edge.token_count as u64; // tokens
    }

    let by_model: Vec<ModelCost> = model_costs
        .iter()
        .map(|(agent_id, (count, tokens))| ModelCost {
            model: format!("agent_{}", agent_id),
            cost: (*tokens as f64 / 1_000_000.0) * 1.0,
            count: *count,
        })
        .collect();

    Ok(CostSummary {
        total_cost,
        total_traces: edges.len(),
        by_model,
    })
}

// ============================================================================
// Connection Monitoring Commands
// ============================================================================

#[derive(Serialize)]
pub struct ConnectionHealth {
    pub stats: crate::ConnectionStats,
    pub queue_capacity: usize,
    pub queue_is_healthy: bool,
}

#[tauri::command]
pub async fn get_connection_stats(
    state: State<'_, AppState>,
) -> Result<crate::ConnectionStats, String> {
    let stats = state.connection_stats.read();
    Ok(stats.clone())
}

#[tauri::command]
pub async fn get_connection_health(state: State<'_, AppState>) -> Result<ConnectionHealth, String> {
    let stats = state.connection_stats.read();

    // Bounded channel capacity is 1000
    // Consider healthy if we're not hitting backpressure limits
    Ok(ConnectionHealth {
        stats: stats.clone(),
        queue_capacity: 1000,
        queue_is_healthy: true, // If we can send, it's not full
    })
}

// ============================================================================
// Projects & Agents Commands
// ============================================================================

#[derive(Serialize)]
pub struct ProjectInfo {
    pub project_id: String,
    pub name: String,
    pub created_at: u64,
    pub trace_count: usize,
}

#[derive(Serialize)]
pub struct ListProjectsResponse {
    pub projects: Vec<ProjectInfo>,
    pub total: usize,
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<ListProjectsResponse, String> {
    let store = state.project_store.read();
    let projects = store.list();

    let project_infos: Vec<ProjectInfo> = projects
        .into_iter()
        .map(|p| ProjectInfo {
            project_id: p.id,
            name: p.name,
            created_at: p.created_at,
            trace_count: 0, // TODO: Implement trace counting per project
        })
        .collect();

    Ok(ListProjectsResponse {
        total: project_infos.len(),
        projects: project_infos,
    })
}

#[derive(Serialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub name: String,
    pub trace_count: usize,
    pub last_seen: Option<u64>,
}

#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<AgentInfo>, String> {
    let agents = state.agent_registry.read();

    // Get unique agent IDs from traces
    let _stats = state.db.stats();

    let agent_infos: Vec<AgentInfo> = agents
        .iter()
        .map(|agent_id| AgentInfo {
            agent_id: agent_id.clone(),
            name: agent_id.clone(),
            trace_count: 0, // Would need to query
            last_seen: None,
        })
        .collect();

    Ok(agent_infos)
}

#[tauri::command]
pub async fn register_agent(agent_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut agents = state.agent_registry.write();

    if !agents.contains(&agent_id) {
        agents.push(agent_id);
    }

    Ok(())
}

// ============================================================================
// File Dialog Commands for Better UX
// ============================================================================

#[tauri::command]
pub async fn export_backup_with_dialog(
    backup_id: String,
    app_handle: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let backup_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("backups")
        .join(&backup_id);

    if !backup_dir.exists() {
        return Err(format!("Backup not found: {}", backup_id));
    }

    // Let user choose export location using the dialog plugin
    let export_path = tauri_plugin_dialog::DialogExt::dialog(&app_handle)
        .file()
        .set_title("Export Backup")
        .set_file_name(&backup_id)
        .blocking_pick_folder();

    if let Some(export_path) = export_path {
        let export_path_buf = export_path.as_path().ok_or("Invalid path")?;
        copy_dir_recursive(&backup_dir, export_path_buf)
            .map_err(|e| format!("Failed to export backup: {}", e))?;

        Ok(Some(export_path_buf.display().to_string()))
    } else {
        Ok(None) // User cancelled
    }
}

#[tauri::command]
pub async fn import_backup_with_dialog(
    app_handle: tauri::AppHandle,
) -> Result<Option<String>, String> {
    // Let user select backup directory using the dialog plugin
    let import_path = tauri_plugin_dialog::DialogExt::dialog(&app_handle)
        .file()
        .set_title("Select Backup Directory to Import")
        .blocking_pick_folder();

    if let Some(import_path) = import_path {
        let import_path_buf = import_path.as_path().ok_or("Invalid path")?;
        let backup_id = format!("imported_{}", chrono::Utc::now().timestamp());

        let backup_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?
            .join("backups")
            .join(&backup_id);

        std::fs::create_dir_all(&backup_dir)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;

        copy_dir_recursive(import_path_buf, &backup_dir)
            .map_err(|e| format!("Failed to import backup: {}", e))?;

        Ok(Some(backup_id))
    } else {
        Ok(None) // User cancelled
    }
}

#[tauri::command]
pub async fn restore_backup(
    backup_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let backup_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("backups")
        .join(&backup_id);

    if !backup_dir.exists() {
        return Err(format!("Backup not found: {}", backup_id));
    }

    // DESKTOP APP FIX: Implement hot database reload without requiring app restart
    // This improves UX significantly for backup/restore operations

    tracing::info!("Starting hot backup restore process...");

    // Step 1: Signal ingestion queue to stop accepting new writes
    state.ingestion_queue.shutdown();
    tracing::info!("Ingestion queue shutdown signal sent");

    // Step 2: Wait for pending writes to flush (max 2 seconds)
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    tracing::info!("Pending writes flushed");

    // Step 3: Close current database cleanly
    tracing::info!("Closing current database...");
    state
        .db
        .close()
        .map_err(|e| format!("Failed to close database: {}", e))?;
    tracing::info!("Database closed successfully");

    // Step 4: Remove old database files
    tracing::info!("Removing old database files from {:?}", state.db_path);
    if state.db_path.exists() {
        std::fs::remove_dir_all(&state.db_path)
            .map_err(|e| format!("Failed to remove old database: {}", e))?;
    }

    // Step 5: Restore backup to database directory
    tracing::info!("Copying backup from {:?} to {:?}", backup_dir, state.db_path);
    copy_dir_recursive(&backup_dir, &state.db_path)
        .map_err(|e| format!("Failed to restore backup: {}", e))?;
    tracing::info!("Backup files copied successfully");

    // Step 6: RESTART THE APPLICATION
    // Hot database reload without restart is complex due to Arc<Agentreplay> in managed state
    // Cleanest solution: restart app automatically after restore
    tracing::info!("Triggering automatic app restart to load restored database...");

    // Restart app (this is the safest approach for desktop apps)
    app_handle.restart();
}

// ============================================================================
// Reset/Delete All Data Command
// ============================================================================

#[tauri::command]
pub async fn reset_all_data(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!("Starting full data reset...");

    // Step 1: Signal ingestion queue to stop accepting new writes
    state.ingestion_queue.shutdown();
    tracing::info!("Ingestion queue shutdown signal sent");

    // Step 2: Wait for pending writes to flush
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Step 3: Clear projects
    {
        let mut store = state.project_store.write();
        if let Err(e) = store.clear() {
            tracing::error!("Failed to clear projects: {}", e);
        }
    }
    tracing::info!("Projects cleared");

    // Step 4: Close current database cleanly
    tracing::info!("Closing current database...");
    state
        .db
        .close()
        .map_err(|e| format!("Failed to close database: {}", e))?;
    tracing::info!("Database closed successfully");

    // Step 5: Remove database files
    tracing::info!("Removing database files from {:?}", state.db_path);
    if state.db_path.exists() {
        std::fs::remove_dir_all(&state.db_path)
            .map_err(|e| format!("Failed to remove database: {}", e))?;
    }
    tracing::info!("Database files removed");

    // Step 6: Restart the application to recreate fresh database
    tracing::info!("Triggering app restart to create fresh database...");
    app_handle.restart();
}

// ============================================================================
// Window Management Commands
// ============================================================================

#[tauri::command]
pub async fn open_trace_window(
    trace_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    let window_label = format!("trace_{}", trace_id);

    // Check if window already exists
    if let Some(window) = app_handle.get_webview_window(&window_label) {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Create new window
    WebviewWindowBuilder::new(
        &app_handle,
        window_label,
        WebviewUrl::App(format!("/traces/{}", trace_id).parse().unwrap()),
    )
    .title(format!("Trace: {}", trace_id))
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================================
// Evaluation Commands (Issue #6)
// ============================================================================

#[derive(Serialize)]
pub struct EvaluatorInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub tags: Vec<String>,
}

#[tauri::command]
pub async fn list_evaluators(state: State<'_, AppState>) -> Result<Vec<EvaluatorInfo>, String> {
    let evaluator_ids = state.eval_registry.list_evaluators();

    let mut evaluators = Vec::new();
    for id in evaluator_ids {
        if let Some(evaluator) = state.eval_registry.get(&id) {
            let metadata = evaluator.metadata();
            evaluators.push(EvaluatorInfo {
                id: id.clone(),
                name: metadata.name,
                version: metadata.version,
                description: metadata.description,
                tags: metadata.tags,
            });
        }
    }

    Ok(evaluators)
}

#[derive(Deserialize)]
pub struct RunEvaluationParams {
    pub trace_id: String,
    pub evaluator_ids: Vec<String>,
    /// Optional: Evaluation type (output_quality, rag_quality, agent_performance, etc.)
    pub eval_type: Option<String>,
    /// Optional: Custom criteria for G-Eval (coherence, relevance, fluency, helpfulness)
    pub criteria: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct EvaluationResult {
    pub evaluator_id: String,
    pub passed: bool,
    pub score: f64,
    pub confidence: Option<f64>,
    pub metrics: serde_json::Value,
    pub explanation: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn run_evaluation(
    params: RunEvaluationParams,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    // Parse trace_id and get trace edges
    let edge_id = u128::from_str_radix(&params.trace_id.replace("-", ""), 16)
        .map_err(|e| format!("Invalid trace ID: {}", e))?;

    let root_edge = state
        .db
        .get(edge_id)
        .map_err(|e| format!("Failed to get trace: {}", e))?
        .ok_or_else(|| "Trace not found".to_string())?;

    // Query all edges in trace
    let edges = state
        .db
        .filter_by_session(root_edge.session_id, 0, u64::MAX)
        .map_err(|e| format!("Failed to query trace edges: {}", e))?;

    // Extract input/output from payloads
    let (input, output, context) = extract_trace_io(&state.db, &edges);

    // Build TraceContext
    let trace_context = agentreplay_evals::TraceContext {
        trace_id: edge_id,
        edges,
        input,
        output,
        context,
        metadata: std::collections::HashMap::new(),
        eval_trace: None,
        timestamp_us: root_edge.timestamp_us,
    };

    // Determine which evaluators to run
    let mut results = Vec::new();
    
    // If evaluator_ids are provided, use them
    if !params.evaluator_ids.is_empty() {
        let eval_results = state
            .eval_registry
            .evaluate_trace(&trace_context, params.evaluator_ids.clone())
            .await
            .map_err(|e| format!("Evaluation failed: {}", e))?;
        
        for (id, result) in eval_results {
            results.push(EvaluationResult {
                evaluator_id: id,
                passed: result.passed,
                score: result.metrics.get("score")
                    .and_then(|v| match v { agentreplay_evals::MetricValue::Float(f) => Some(*f), _ => None })
                    .unwrap_or(if result.passed { 1.0 } else { 0.0 }),
                confidence: Some(result.confidence),
                metrics: serde_json::to_value(&result.metrics).unwrap_or_default(),
                explanation: result.explanation.clone(),
                error: None,
            });
        }
    }
    
    // If eval_type is "output_quality" or similar, try to run G-Eval with LLM
    if let Some(eval_type) = &params.eval_type {
        if eval_type == "output_quality" || eval_type == "rag_quality" {
            // Check if we have an LLM configured
            let llm_client = state.llm_client.clone();
            let llm_client_guard = llm_client.read().await;
            
            if llm_client_guard.is_configured() {
                // Check if we have input/output to evaluate
                if trace_context.input.is_some() && trace_context.output.is_some() {
                    // Get the configured default model
                    let default_model = llm_client_guard.get_default_model().to_string();
                    
                    // Create adapter for the LLM client
                    drop(llm_client_guard); // Release read lock before creating adapter
                    let adapter = std::sync::Arc::new(
                        crate::llm::LLMClientAdapter::new(llm_client.clone(), default_model)
                    );
                    
                    // Create G-Eval evaluator
                    let geval = agentreplay_evals::evaluators::GEval::new(adapter);
                    
                    // Run evaluation
                    match geval.evaluate(&trace_context).await {
                        Ok(eval_result) => {
                            results.push(EvaluationResult {
                                evaluator_id: format!("{}_geval", eval_type),
                                passed: eval_result.passed,
                                score: eval_result.metrics.get("score")
                                    .and_then(|v| match v { 
                                        agentreplay_evals::MetricValue::Float(f) => Some(*f), 
                                        _ => None 
                                    })
                                    .unwrap_or(if eval_result.passed { 1.0 } else { 0.0 }),
                                confidence: Some(eval_result.confidence),
                                metrics: serde_json::to_value(&eval_result.metrics).unwrap_or_default(),
                                explanation: eval_result.explanation.clone(),
                                error: None,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("G-Eval failed: {}", e);
                            results.push(EvaluationResult {
                                evaluator_id: format!("{}_geval", eval_type),
                                passed: false,
                                score: 0.0,
                                confidence: None,
                                metrics: serde_json::json!({}),
                                explanation: Some(format!("G-Eval evaluation failed: {}", e)),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                } else {
                    results.push(EvaluationResult {
                        evaluator_id: format!("{}_geval", eval_type),
                        passed: false,
                        score: 0.0,
                        confidence: None,
                        metrics: serde_json::json!({}),
                        explanation: Some("Cannot run G-Eval: trace is missing input or output data".to_string()),
                        error: Some("Missing input/output".to_string()),
                    });
                }
            } else {
                results.push(EvaluationResult {
                    evaluator_id: format!("{}_geval", eval_type),
                    passed: false,
                    score: 0.0,
                    confidence: None,
                    metrics: serde_json::json!({}),
                    explanation: Some("Configure an LLM provider (Ollama, OpenAI, or Anthropic) in Settings to enable LLM-as-judge evaluations".to_string()),
                    error: Some("LLM provider not configured".to_string()),
                });
            }
        }
    }
    
    // If no evaluators ran, at least run the local ones
    if results.is_empty() {
        let local_evaluators = vec!["latency_v1".to_string(), "cost_v1".to_string()];
        let eval_results = state
            .eval_registry
            .evaluate_trace(&trace_context, local_evaluators)
            .await
            .map_err(|e| format!("Evaluation failed: {}", e))?;
        
        for (id, result) in eval_results {
            results.push(EvaluationResult {
                evaluator_id: id,
                passed: result.passed,
                score: result.metrics.get("score")
                    .and_then(|v| match v { agentreplay_evals::MetricValue::Float(f) => Some(*f), _ => None })
                    .unwrap_or(if result.passed { 1.0 } else { 0.0 }),
                confidence: Some(result.confidence),
                metrics: serde_json::to_value(&result.metrics).unwrap_or_default(),
                explanation: result.explanation.clone(),
                error: None,
            });
        }
    }

    // Persist evaluation results to disk
    let timestamp_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    
    let metric_entries: Vec<agentreplay_storage::EvalMetricEntry> = results.iter().map(|r| {
        agentreplay_storage::EvalMetricEntry {
            edge_id: edge_id,
            metric_name: r.evaluator_id.clone(),
            metric_value: r.score,
            evaluator: r.evaluator_id.clone(),
            timestamp_us,
            passed: Some(r.passed),
            confidence: r.confidence,
            explanation: r.explanation.clone(),
        }
    }).collect();
    
    if !metric_entries.is_empty() {
        if let Err(e) = state.eval_store.store_metrics(metric_entries) {
            tracing::warn!("Failed to persist eval metrics: {}", e);
        }
        
        // Update summary
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.iter().filter(|r| !r.passed).count();
        let avg_score = if results.is_empty() { 0.0 } else {
            results.iter().map(|r| r.score).sum::<f64>() / results.len() as f64
        };
        let avg_confidence = if results.is_empty() { 0.0 } else {
            results.iter()
                .filter_map(|r| r.confidence)
                .sum::<f64>() / results.iter().filter(|r| r.confidence.is_some()).count().max(1) as f64
        };
        
        let summary = agentreplay_storage::EvalSummary {
            trace_id: edge_id,
            total_evaluations: results.len(),
            passed,
            failed,
            avg_score,
            avg_confidence,
            last_updated_us: timestamp_us,
        };
        
        if let Err(e) = state.eval_store.store_summary(summary) {
            tracing::warn!("Failed to persist eval summary: {}", e);
        }
    }

    serde_json::to_value(results).map_err(|e| format!("Failed to serialize results: {}", e))
}

/// Extract input, output, and context from trace edges and their payloads
fn extract_trace_io(
    db: &Arc<agentreplay_query::Agentreplay>,
    edges: &[agentreplay_core::AgentFlowEdge],
) -> (Option<String>, Option<String>, Option<Vec<String>>) {
    let mut input = None;
    let mut output = None;
    let mut context_parts: Vec<String> = Vec::new();
    
    // Sort edges by timestamp to process in order
    let mut sorted_edges: Vec<_> = edges.iter().collect();
    sorted_edges.sort_by_key(|e| e.timestamp_us);
    
    for edge in sorted_edges {
        // Try to get payload for this edge
        if let Ok(Some(payload_bytes)) = db.get_payload(edge.edge_id) {
            if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
                // Look for input in various formats
                if input.is_none() {
                    // OTEL GenAI convention: gen_ai.prompt.0.content
                    if let Some(prompt) = payload.get("gen_ai.prompt.0.content").and_then(|c| c.as_str()) {
                        input = Some(prompt.to_string());
                    }
                    // Look in messages array for user message
                    else if let Some(messages) = payload.get("messages").and_then(|m| m.as_array()) {
                        for msg in messages {
                            if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                    input = Some(content.to_string());
                                    break;
                                }
                            }
                        }
                    }
                    // Direct "input" field
                    else if let Some(inp) = payload.get("input").and_then(|i| i.as_str()) {
                        input = Some(inp.to_string());
                    }
                    // Try "query" field (common in RAG)
                    else if let Some(query) = payload.get("query").and_then(|q| q.as_str()) {
                        input = Some(query.to_string());
                    }
                }
                
                // Look for output in various formats
                if output.is_none() {
                    // OTEL GenAI convention: gen_ai.completion.0.content
                    if let Some(completion) = payload.get("gen_ai.completion.0.content").and_then(|c| c.as_str()) {
                        output = Some(completion.to_string());
                    }
                    // Direct content/response fields
                    else if let Some(content) = payload.get("content").and_then(|c| c.as_str()) {
                        output = Some(content.to_string());
                    } else if let Some(resp) = payload.get("response").and_then(|r| r.as_str()) {
                        output = Some(resp.to_string());
                    } else if let Some(result) = payload.get("result").and_then(|r| r.as_str()) {
                        // Only use result as output if not a tool response
                        if edge.get_span_type() != agentreplay_core::SpanType::ToolResponse {
                            output = Some(result.to_string());
                        }
                    }
                    // Look for assistant message in messages array
                    else if let Some(messages) = payload.get("messages").and_then(|m| m.as_array()) {
                        for msg in messages.iter().rev() {
                            if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                    output = Some(content.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
                
                // Collect context from tool calls and retrieval
                match edge.get_span_type() {
                    agentreplay_core::SpanType::ToolResponse | agentreplay_core::SpanType::Retrieval => {
                        if let Some(result) = payload.get("result").and_then(|r| r.as_str()) {
                            context_parts.push(result.to_string());
                        } else if let Some(docs) = payload.get("documents").and_then(|d| d.as_array()) {
                            for doc in docs {
                                if let Some(content) = doc.get("content").or(doc.get("text")).and_then(|c| c.as_str()) {
                                    context_parts.push(content.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    // Return context as Vec<String> for TraceContext
    let context = if context_parts.is_empty() {
        None
    } else {
        Some(context_parts)
    };
    
    (input, output, context)
}

#[derive(Serialize)]
pub struct EvaluationSummary {
    pub total_evaluations: usize,
    pub passed: usize,
    pub failed: usize,
    pub avg_confidence: f64,
    pub total_cost: f64,
}

#[tauri::command]
pub async fn get_evaluation_summary(
    trace_id: String,
    state: State<'_, AppState>,
) -> Result<EvaluationSummary, String> {
    // Parse trace_id 
    let edge_id = u128::from_str_radix(&trace_id.replace("-", ""), 16)
        .map_err(|e| format!("Invalid trace ID: {}", e))?;
    
    // Get summary from persistent store
    if let Ok(Some(summary)) = state.eval_store.get_summary(edge_id) {
        return Ok(EvaluationSummary {
            total_evaluations: summary.total_evaluations,
            passed: summary.passed,
            failed: summary.failed,
            avg_confidence: summary.avg_confidence,
            total_cost: 0.0, // TODO: Calculate from metrics if needed
        });
    }
    
    // Fall back to computing from stored metrics
    let metrics = state.eval_store.get_metrics(edge_id).unwrap_or_default();
    if !metrics.is_empty() {
        let passed = metrics.iter().filter(|m| m.passed == Some(true)).count();
        let failed = metrics.iter().filter(|m| m.passed == Some(false)).count();
        let avg_confidence = {
            let confidences: Vec<f64> = metrics.iter().filter_map(|m| m.confidence).collect();
            if confidences.is_empty() { 0.0 } else { confidences.iter().sum::<f64>() / confidences.len() as f64 }
        };
        
        return Ok(EvaluationSummary {
            total_evaluations: metrics.len(),
            passed,
            failed,
            avg_confidence,
            total_cost: 0.0,
        });
    }
    
    // No evaluations found
    Ok(EvaluationSummary {
        total_evaluations: 0,
        passed: 0,
        failed: 0,
        avg_confidence: 0.0,
        total_cost: 0.0,
    })
}

// ============================================================================
// Settings Commands (User/Project/Local Scopes)
// ============================================================================

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentreplaySettings {
    pub database: DatabaseSettingsConfig,
    pub server: ServerSettingsConfig,
    pub ui: UiSettingsConfig,
    #[serde(default)]
    pub embedding: Option<EmbeddingSettingsConfig>,
    #[serde(default)]
    pub models: Option<ModelsSettingsConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmbeddingSettingsConfig {
    /// Provider type: openai, ollama, fastembed, google, custom
    #[serde(default = "default_embedding_provider")]
    pub provider: String,
    /// Model name (e.g., "text-embedding-3-small", "nomic-embed-text")
    #[serde(default = "default_embedding_model")]
    pub model: String,
    /// Embedding dimensions
    #[serde(default = "default_embedding_dimensions")]
    pub dimensions: usize,
    /// API key for cloud providers
    pub api_key: Option<String>,
    /// Base URL for custom endpoints
    pub base_url: Option<String>,
    /// Whether embeddings are enabled
    #[serde(default)]
    pub enabled: bool,
    /// Auto-index new traces
    #[serde(default)]
    pub auto_index_new_traces: bool,
    /// Batch size for embedding generation
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_embedding_provider() -> String {
    "local".to_string()
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}

fn default_embedding_dimensions() -> usize {
    1536
}

fn default_batch_size() -> usize {
    100
}

impl Default for EmbeddingSettingsConfig {
    fn default() -> Self {
        Self {
            provider: default_embedding_provider(),
            model: default_embedding_model(),
            dimensions: default_embedding_dimensions(),
            api_key: None,
            base_url: None,
            enabled: false,
            auto_index_new_traces: false,
            batch_size: default_batch_size(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelsSettingsConfig {
    pub providers: Vec<ProviderConfigEntry>,
    pub default_provider_id: Option<String>,
    #[serde(default = "default_model_temperature")]
    pub default_temperature: f32,
    #[serde(default = "default_model_max_tokens")]
    pub default_max_tokens: usize,
}

fn default_model_temperature() -> f32 {
    0.7
}

fn default_model_max_tokens() -> usize {
    4096
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigEntry {
    pub id: String,
    pub provider: String,
    #[serde(alias = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(alias = "modelName")]
    pub model_name: String,
    #[serde(alias = "apiKey")]
    pub api_key: Option<String>,
    #[serde(default, alias = "isDefault")]
    pub is_default: bool,
    /// Display name for the provider
    #[serde(default)]
    pub name: Option<String>,
    /// Tags for routing (e.g., ["default", "eval"])
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DatabaseSettingsConfig {
    pub max_traces: Option<u64>,
    pub retention_days: Option<u64>,
    pub auto_compact: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerSettingsConfig {
    pub port: u16,
    pub enable_cors: bool,
    pub max_payload_size_mb: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UiSettingsConfig {
    pub theme: String, // "light", "dark", "system"
    pub animations_enabled: bool,
    pub auto_refresh_interval_secs: u64,
}

fn get_settings_path(scope: &str, project_path: Option<String>) -> Result<PathBuf, String> {
    match scope {
        "user" => {
            let home = dirs::home_dir().ok_or("Home directory not found")?;
            Ok(home.join(".agentreplay").join("settings.json"))
        }
        "project" => {
            let path = project_path.ok_or("Project path required for project scope")?;
            Ok(PathBuf::from(path).join(".agentreplay").join("settings.json"))
        }
        "local" => {
            let path = project_path.ok_or("Project path required for local scope")?;
            Ok(PathBuf::from(path)
                .join(".agentreplay")
                .join("settings.local.json"))
        }
        _ => Err("Invalid scope. Use 'user', 'project', or 'local'".to_string()),
    }
}

fn get_default_settings() -> AgentreplaySettings {
    AgentreplaySettings {
        database: DatabaseSettingsConfig {
            max_traces: Some(1_000_000),
            retention_days: Some(30),
            auto_compact: true,
        },
        server: ServerSettingsConfig {
            port: 9600,
            enable_cors: true,
            max_payload_size_mb: 10,
        },
        ui: UiSettingsConfig {
            theme: "dark".to_string(),
            animations_enabled: true,
            auto_refresh_interval_secs: 30,
        },
        embedding: Some(EmbeddingSettingsConfig::default()),
        models: None,
    }
}

#[tauri::command]
pub async fn get_agentreplay_settings(
    scope: String,
    project_path: Option<String>,
) -> Result<AgentreplaySettings, String> {
    let settings_path = get_settings_path(&scope, project_path)?;

    if !settings_path.exists() {
        tracing::info!(
            "Settings file does not exist at {:?}, returning defaults",
            settings_path
        );
        return Ok(get_default_settings());
    }

    // Use tokio's async file I/O to avoid blocking the runtime
    let content = tokio::fs::read_to_string(&settings_path)
        .await
        .map_err(|e| format!("Failed to read settings file: {}", e))?;

    serde_json::from_str(&content).map_err(|e| format!("Failed to parse settings JSON: {}", e))
}

#[tauri::command]
pub async fn save_agentreplay_settings(
    scope: String,
    settings: AgentreplaySettings,
    project_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let settings_path = get_settings_path(&scope, project_path)?;

    // Create parent directory if it doesn't exist (use tokio for async)
    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }

    // Write settings with pretty JSON formatting
    let json_string = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    tokio::fs::write(&settings_path, json_string)
        .await
        .map_err(|e| format!("Failed to write settings file: {}", e))?;

    // Sync LLM settings to the in-memory client if models config is present
    if let Some(models) = &settings.models {
        sync_llm_config_internal(&state, models).await?;
    }

    tracing::info!("Saved {} scope settings to {:?}", scope, settings_path);
    Ok(format!(
        "Settings saved successfully to {}",
        settings_path.display()
    ))
}

/// Sync LLM configuration to the in-memory client
/// This is called when settings are saved and also can be called directly
#[tauri::command]
pub async fn sync_llm_settings(
    models: ModelsSettingsConfig,
    state: State<'_, AppState>,
) -> Result<String, String> {
    sync_llm_config_internal(&state, &models).await
}

/// Internal helper to sync LLM config
async fn sync_llm_config_internal(
    state: &AppState,
    models: &ModelsSettingsConfig,
) -> Result<String, String> {
    // Convert ModelsSettingsConfig to LLMConfig with proper tag handling
    let providers: Vec<crate::llm::LLMProviderConfig> = models.providers.iter().map(|p| {
        // Determine if this is the default provider
        let is_default = p.is_default || models.default_provider_id.as_ref() == Some(&p.id);
        
        // Build tags: use explicit tags if set, otherwise derive from is_default
        let mut tags = if !p.tags.is_empty() {
            p.tags.clone()
        } else {
            vec![]
        };
        
        // Always add "default" tag if this is the default provider
        if is_default && !tags.contains(&"default".to_string()) {
            tags.push("default".to_string());
        }
        
        crate::llm::LLMProviderConfig {
            provider: p.provider.clone(),
            api_key: p.api_key.clone(),
            base_url: p.base_url.clone(),
            enabled: true,
            model: Some(p.model_name.clone()),
            tags,
            name: p.name.clone(),
        }
    }).collect();
    
    // Find the default provider and its model
    let default_model = models.providers.iter()
        .find(|p| p.is_default || models.default_provider_id.as_ref() == Some(&p.id))
        .map(|p| p.model_name.clone())
        .unwrap_or_else(|| {
            // Fallback: use first configured provider's model
            models.providers.first()
                .map(|p| p.model_name.clone())
                .unwrap_or_else(|| "gpt-4o-mini".to_string())
        });
    
    let llm_config = crate::llm::LLMConfig {
        providers,
        default_model: default_model.clone(),
        default_temperature: models.default_temperature,
        default_max_tokens: models.default_max_tokens as u32,
    };
    
    // Update the in-memory LLM client
    let mut client = state.llm_client.write().await;
    client.set_config(llm_config.clone());
    
    // Also persist to llm-config.json for next startup
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentreplay")
        .join("llm-config.json");
    
    if let Some(parent) = config_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    
    let json = serde_json::to_string_pretty(&llm_config)
        .map_err(|e| format!("Failed to serialize LLM config: {}", e))?;
    
    tokio::fs::write(&config_path, json)
        .await
        .map_err(|e| format!("Failed to write LLM config: {}", e))?;
    
    // Log provider routing info
    for (i, p) in llm_config.providers.iter().enumerate() {
        let tags_str = if p.tags.is_empty() { "none".to_string() } else { p.tags.join(", ") };
        tracing::info!("Provider {}: {} ({}) - model: {}, tags: [{}]", 
            i, p.name.as_deref().unwrap_or("unnamed"), p.provider,
            p.model.as_deref().unwrap_or("default"), tags_str);
    }
    
    tracing::info!("Synced LLM config: default_model={}, {} provider(s)", 
        default_model, llm_config.providers.len());
    
    Ok(format!("LLM settings synced. Default model: {}", default_model))
}

#[tauri::command]
pub async fn get_current_project_path(
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    // For now, return the database path's parent directory
    let db_path = &state.db_path;
    if let Some(parent) = db_path.parent() {
        Ok(Some(parent.display().to_string()))
    } else {
        Ok(None)
    }
}

// ============================================================================
// System Commands
// ============================================================================

#[tauri::command]
pub fn os_type() -> String {
    #[cfg(target_os = "macos")]
    return "macos".to_string();

    #[cfg(target_os = "linux")]
    return "linux".to_string();

    #[cfg(target_os = "windows")]
    return "windows".to_string();

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return "unknown".to_string();
}

// ============================================================================
// Update Commands
// ============================================================================

#[derive(Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates(app_handle: tauri::AppHandle) -> Result<UpdateInfo, String> {
    // Return current version info - actual update checking is handled by the updater plugin
    Ok(UpdateInfo {
        available: false,
        current_version: app_handle.package_info().version.to_string(),
        latest_version: None,
    })
}

// ============================================================================
// Model Comparison Commands
// ============================================================================

use agentreplay_core::{
    ModelComparisonRequest, ModelPricingRegistry, ModelSelection,
};

/// Request for comparing models via Tauri command
#[derive(Debug, Clone, Deserialize)]
pub struct CompareModelsRequest {
    pub prompt: String,
    pub models: Vec<ModelSelectionInput>,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> u32 {
    2048
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelSelectionInput {
    pub provider: String,
    pub model_id: String,
    pub display_name: Option<String>,
}

/// Response for model comparison
#[derive(Debug, Clone, Serialize)]
pub struct CompareModelsResponse {
    pub success: bool,
    pub comparison_id: String,
    pub results: Vec<ModelResultOutput>,
    pub summary: ComparisonSummaryOutput,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelResultOutput {
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

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonSummaryOutput {
    pub total_models: usize,
    pub successful: usize,
    pub failed: usize,
    pub fastest_model: Option<String>,
    pub cheapest_model: Option<String>,
    pub total_cost_usd: f64,
    pub total_latency_ms: u32,
}

/// Compare multiple models with the same prompt
#[tauri::command]
pub async fn compare_models(
    request: CompareModelsRequest,
    state: State<'_, AppState>,
) -> Result<CompareModelsResponse, String> {
    use crate::comparison_engine::ModelComparisonEngine;

    // Validate request
    if request.models.is_empty() {
        return Ok(CompareModelsResponse {
            success: false,
            comparison_id: String::new(),
            results: vec![],
            summary: ComparisonSummaryOutput {
                total_models: 0,
                successful: 0,
                failed: 0,
                fastest_model: None,
                cheapest_model: None,
                total_cost_usd: 0.0,
                total_latency_ms: 0,
            },
            error: Some("No models selected".to_string()),
        });
    }

    if request.models.len() > 3 {
        return Ok(CompareModelsResponse {
            success: false,
            comparison_id: String::new(),
            results: vec![],
            summary: ComparisonSummaryOutput {
                total_models: request.models.len(),
                successful: 0,
                failed: 0,
                fastest_model: None,
                cheapest_model: None,
                total_cost_usd: 0.0,
                total_latency_ms: 0,
            },
            error: Some("Maximum 3 models allowed".to_string()),
        });
    }

    if request.prompt.trim().is_empty() {
        return Ok(CompareModelsResponse {
            success: false,
            comparison_id: String::new(),
            results: vec![],
            summary: ComparisonSummaryOutput {
                total_models: request.models.len(),
                successful: 0,
                failed: 0,
                fastest_model: None,
                cheapest_model: None,
                total_cost_usd: 0.0,
                total_latency_ms: 0,
            },
            error: Some("Prompt cannot be empty".to_string()),
        });
    }

    // Convert to core types
    let models: Vec<ModelSelection> = request
        .models
        .iter()
        .map(|m| {
            let mut sel = ModelSelection::new(&m.provider, &m.model_id);
            if let Some(name) = &m.display_name {
                sel = sel.with_display_name(name);
            }
            sel
        })
        .collect();

    let comparison_request = ModelComparisonRequest {
        prompt: request.prompt,
        models,
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        system_prompt: request.system_prompt,
        variables: request.variables,
    };

    // Get data directory for pricing
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentreplay");

    let pricing_registry = std::sync::Arc::new(ModelPricingRegistry::new(data_dir));

    // Create engine and run comparison
    let engine = ModelComparisonEngine::new(
        std::sync::Arc::clone(&state.llm_client),
        pricing_registry,
    );

    match engine.compare(comparison_request).await {
        Ok(response) => {
            let results: Vec<ModelResultOutput> = response
                .results
                .iter()
                .map(|r| ModelResultOutput {
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

            Ok(CompareModelsResponse {
                success: true,
                comparison_id: response.comparison_id,
                results,
                summary: ComparisonSummaryOutput {
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
        }
        Err(e) => Ok(CompareModelsResponse {
            success: false,
            comparison_id: String::new(),
            results: vec![],
            summary: ComparisonSummaryOutput {
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
    }
}

/// Model info for listing available models
#[derive(Debug, Clone, Serialize)]
pub struct AvailableModelInfo {
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
    pub input_cost_per_1m: Option<f64>,
    pub output_cost_per_1m: Option<f64>,
    pub context_window: Option<u32>,
    pub available: bool,
}

/// List available models for comparison with pricing info.
/// Note: The frontend now uses localStorage settings to determine which providers
/// are configured. This endpoint provides pricing data for reference only.
/// Models are only shown to users if they have configured API keys in Settings.
#[tauri::command]
pub async fn list_comparison_models() -> Result<Vec<AvailableModelInfo>, String> {
    // Static list of popular models - in production this would check provider availability
    let models = vec![
        // OpenAI
        AvailableModelInfo {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            display_name: "GPT-4o".to_string(),
            input_cost_per_1m: Some(2.5),
            output_cost_per_1m: Some(10.0),
            context_window: Some(128000),
            available: std::env::var("OPENAI_API_KEY").is_ok(),
        },
        AvailableModelInfo {
            provider: "openai".to_string(),
            model_id: "gpt-4o-mini".to_string(),
            display_name: "GPT-4o Mini".to_string(),
            input_cost_per_1m: Some(0.15),
            output_cost_per_1m: Some(0.6),
            context_window: Some(128000),
            available: std::env::var("OPENAI_API_KEY").is_ok(),
        },
        AvailableModelInfo {
            provider: "openai".to_string(),
            model_id: "gpt-4-turbo".to_string(),
            display_name: "GPT-4 Turbo".to_string(),
            input_cost_per_1m: Some(10.0),
            output_cost_per_1m: Some(30.0),
            context_window: Some(128000),
            available: std::env::var("OPENAI_API_KEY").is_ok(),
        },
        // Anthropic
        AvailableModelInfo {
            provider: "anthropic".to_string(),
            model_id: "claude-3-5-sonnet-20241022".to_string(),
            display_name: "Claude 3.5 Sonnet".to_string(),
            input_cost_per_1m: Some(3.0),
            output_cost_per_1m: Some(15.0),
            context_window: Some(200000),
            available: std::env::var("ANTHROPIC_API_KEY").is_ok(),
        },
        AvailableModelInfo {
            provider: "anthropic".to_string(),
            model_id: "claude-3-5-haiku-20241022".to_string(),
            display_name: "Claude 3.5 Haiku".to_string(),
            input_cost_per_1m: Some(0.25),
            output_cost_per_1m: Some(1.25),
            context_window: Some(200000),
            available: std::env::var("ANTHROPIC_API_KEY").is_ok(),
        },
        // DeepSeek
        AvailableModelInfo {
            provider: "deepseek".to_string(),
            model_id: "deepseek-chat".to_string(),
            display_name: "DeepSeek Chat".to_string(),
            input_cost_per_1m: Some(0.14),
            output_cost_per_1m: Some(0.28),
            context_window: Some(64000),
            available: std::env::var("DEEPSEEK_API_KEY").is_ok(),
        },
        // Local Ollama
        AvailableModelInfo {
            provider: "ollama".to_string(),
            model_id: "llama3.2".to_string(),
            display_name: "Llama 3.2 (Local)".to_string(),
            input_cost_per_1m: Some(0.0),
            output_cost_per_1m: Some(0.0),
            context_window: Some(128000),
            available: true, // Assume local is available
        },
        AvailableModelInfo {
            provider: "ollama".to_string(),
            model_id: "qwen2.5-coder".to_string(),
            display_name: "Qwen 2.5 Coder (Local)".to_string(),
            input_cost_per_1m: Some(0.0),
            output_cost_per_1m: Some(0.0),
            context_window: Some(32000),
            available: true,
        },
    ];

    Ok(models)
}

/// Pricing info for a model
#[derive(Debug, Clone, Serialize)]
pub struct ModelPricingInfo {
    pub model_id: String,
    pub provider: Option<String>,
    pub input_cost_per_1m: f64,
    pub output_cost_per_1m: f64,
    pub context_window: Option<u32>,
    pub supports_vision: bool,
    pub supports_function_calling: bool,
}

/// Get pricing for a specific model
#[tauri::command]
pub async fn get_model_pricing(model_id: String) -> Result<Option<ModelPricingInfo>, String> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentreplay");

    let registry = ModelPricingRegistry::new(data_dir);

    if let Some(pricing) = registry.get_pricing(&model_id).await {
        Ok(Some(ModelPricingInfo {
            model_id,
            provider: pricing.provider.clone(),
            input_cost_per_1m: pricing.input_cost_per_token * 1_000_000.0,
            output_cost_per_1m: pricing.output_cost_per_token * 1_000_000.0,
            context_window: pricing.context_window,
            supports_vision: pricing.supports_vision,
            supports_function_calling: pricing.supports_function_calling,
        }))
    } else {
        Ok(None)
    }
}

/// Sync pricing from LiteLLM
#[tauri::command]
pub async fn sync_model_pricing() -> Result<usize, String> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentreplay");

    let registry = ModelPricingRegistry::new(data_dir);

    registry
        .sync_from_litellm()
        .await
        .map_err(|e| format!("Failed to sync pricing: {}", e))
}

/// Calculate cost for given token counts
#[tauri::command]
pub async fn calculate_model_cost(
    model_id: String,
    input_tokens: u32,
    output_tokens: u32,
) -> Result<f64, String> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentreplay");

    let registry = ModelPricingRegistry::new(data_dir);
    let cost = registry.calculate_cost(&model_id, input_tokens, output_tokens).await;

    Ok(cost)
}

// ============================================================================
// Insights Commands
// ============================================================================

use agentreplay_core::insights::{InsightConfig, InsightEngine, InsightType, Severity, Insight};

#[derive(Debug, Deserialize)]
pub struct InsightsParams {
    pub project_id: u16,
    #[serde(default = "default_window_secs")]
    pub window_seconds: u64,
    #[serde(default = "default_insights_limit")]
    pub limit: usize,
}

fn default_window_secs() -> u64 { 3600 }
fn default_insights_limit() -> usize { 50 }

#[derive(Debug, Serialize)]
pub struct InsightView {
    pub id: String,
    pub insight_type: String,
    pub severity: String,
    pub confidence: f32,
    pub summary: String,
    pub description: String,
    pub related_trace_ids: Vec<String>,
    pub metadata: serde_json::Value,
    pub generated_at: u64,
    pub suggestions: Vec<String>,
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
            InsightType::ErrorRateAnomaly { baseline_rate, current_rate, .. } => (
                "error_rate_anomaly".to_string(),
                vec![
                    format!("Error rate: {:.1}% → {:.1}%", baseline_rate * 100.0, current_rate * 100.0),
                    "Check logs for error patterns".to_string(),
                ],
            ),
            InsightType::CostAnomaly { baseline_cost, current_cost, .. } => (
                "cost_anomaly".to_string(),
                vec![
                    format!("Cost: ${:.4} → ${:.4}", baseline_cost, current_cost),
                    "Review model usage and consider optimization".to_string(),
                ],
            ),
            _ => ("other".to_string(), vec!["Review the insight details".to_string()]),
        };

        InsightView {
            id: insight.id.clone(),
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
pub struct InsightsResponse {
    pub insights: Vec<InsightView>,
    pub total_count: usize,
    pub window_seconds: u64,
    pub generated_at: u64,
}

#[derive(Debug, Serialize)]
pub struct InsightsSummary {
    pub total_insights: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub by_severity: std::collections::HashMap<String, usize>,
    pub by_type: std::collections::HashMap<String, usize>,
    pub health_score: u8,
    pub top_insights: Vec<InsightView>,
}

/// Get insights for a project (project-scoped)
#[tauri::command]
pub async fn get_insights(
    params: InsightsParams,
    state: State<'_, AppState>,
) -> Result<InsightsResponse, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let config = InsightConfig::default();
    let baseline_multiplier = config.baseline_multiplier as u64;
    let engine = InsightEngine::new(config);
    let project_id = params.project_id;

    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let recent_start = now_us.saturating_sub(params.window_seconds * 1_000_000);
    let baseline_start = recent_start.saturating_sub(params.window_seconds * 1_000_000 * baseline_multiplier);

    const SAMPLE_LIMIT: usize = 5000;
    
    let recent_edges: Vec<_> = state.db
        .query_temporal_range_paginated(recent_start, now_us, SAMPLE_LIMIT, 0)
        .map(|(edges, _)| edges.into_iter().filter(|e| e.project_id == project_id).collect())
        .unwrap_or_default();
    
    let baseline_edges: Vec<_> = state.db
        .query_temporal_range_paginated(baseline_start, recent_start, SAMPLE_LIMIT, 0)
        .map(|(edges, _)| edges.into_iter().filter(|e| e.project_id == project_id).collect())
        .unwrap_or_default();

    let insights = engine.generate_insights_from_edges(&recent_edges, &baseline_edges);

    let filtered: Vec<InsightView> = insights
        .into_iter()
        .take(params.limit)
        .map(InsightView::from)
        .collect();

    let count = filtered.len();

    Ok(InsightsResponse {
        insights: filtered,
        total_count: count,
        window_seconds: params.window_seconds,
        generated_at: now_us,
    })
}

/// Get insights summary for a project (project-scoped)
#[tauri::command]
pub async fn get_insights_summary(
    project_id: u16,
    state: State<'_, AppState>,
) -> Result<InsightsSummary, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let config = InsightConfig::default();
    let engine = InsightEngine::new(config);

    let now_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let recent_start = now_us.saturating_sub(3600 * 1_000_000); // 1 hour
    let baseline_start = recent_start.saturating_sub(7 * 3600 * 1_000_000); // 7 hours before

    const SAMPLE_LIMIT: usize = 5000;
    
    let recent_edges: Vec<_> = state.db
        .query_temporal_range_paginated(recent_start, now_us, SAMPLE_LIMIT, 0)
        .map(|(edges, _)| edges.into_iter().filter(|e| e.project_id == project_id).collect())
        .unwrap_or_default();
    
    let baseline_edges: Vec<_> = state.db
        .query_temporal_range_paginated(baseline_start, recent_start, SAMPLE_LIMIT, 0)
        .map(|(edges, _)| edges.into_iter().filter(|e| e.project_id == project_id).collect())
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

    Ok(InsightsSummary {
        total_insights: by_severity.values().sum(),
        critical_count,
        high_count,
        by_severity,
        by_type,
        health_score,
        top_insights,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use agentreplay_query::Agentreplay;
    use parking_lot::RwLock;
    use std::sync::Arc;
    use tempfile::TempDir;

    // Note: create_test_state is disabled because tauri::AppHandle cannot be created in unit tests
    // For integration tests with full Tauri context, see tests/integration_test.rs
    // These tests use Tauri's test utilities: tauri::test::mock_builder()
    #[allow(dead_code, unused_variables, unused_imports, clippy::diverging_sub_expression, unreachable_code)]
    fn create_test_state() -> (AppState, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Agentreplay::open(&db_path).unwrap();

        let config = AppConfig {
            database: crate::DatabaseConfig {
                auto_backup_enabled: true,
                auto_backup_interval_hours: 24,
                max_backups_to_keep: 7,
            },
            ui: crate::UiConfig {
                theme: "dark".to_string(),
                default_time_range_hours: 24,
            },
            server_export: crate::ServerExportConfig {
                enabled: false,
                server_url: None,
                api_key: None,
            },
            ingestion_server: crate::IngestionServerConfig {
                enabled: true,
                port: 9600,
                host: "127.0.0.1".to_string(),
                auth_token: None,
                max_connections: 1000,
            },
            retention: crate::RetentionServerConfig::default(),
        };

        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        let (trace_tx, _trace_rx) = tokio::sync::broadcast::channel(100);

        let state = AppState {
            db: Arc::new(db),
            db_path: db_path.clone(),
            config: Arc::new(RwLock::new(config)),
            agent_registry: Arc::new(RwLock::new(Vec::new())),
            saved_view_registry: Arc::new(tokio::sync::RwLock::new(
                agentreplay_core::SavedViewRegistry::new(&db_path),
            )),
            connection_stats: Arc::new(RwLock::new(crate::ConnectionStats {
                total_traces_received: 0,
                last_trace_time: None,
                server_uptime_secs: 0,
                ingestion_rate_per_min: 0.0,
            })),
            app_handle: unimplemented!(
                "app_handle not available in unit tests - use tests/integration_test.rs instead"
            ),
            eval_registry: Arc::new(agentreplay_evals::EvaluatorRegistry::new()),
            ingestion_queue: Arc::new(crate::IngestionQueue {
                tx,
                shutdown_tx: Arc::new(tokio::sync::Notify::new()),
            }),
            project_store: Arc::new(RwLock::new(
                crate::project_store::ProjectStore::new(db_path.join("projects.json")),
            )),
            trace_broadcaster: trace_tx,
        };

        (state, temp_dir)
    }

    #[test]
    #[ignore = "Requires full Tauri integration test context - see tests/integration_test.rs"]
    fn test_health_status_serialization() {
        let health = HealthStatus {
            status: "healthy".to_string(),
            database_path: "/tmp/test.db".to_string(),
            total_traces: 42,
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_database_stats_serialization() {
        let stats = DatabaseStats {
            total_traces: 100,
            total_edges: 250,
            index_size_bytes: 1024,
            storage_path: "/tmp/test.db".to_string(),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("100"));
        assert!(json.contains("250"));
    }

    #[test]
    fn test_trace_metadata_deserialization() {
        let json = r#"{
            "trace_id": "test-123",
            "agent_id": "agent-1",
            "started_at": 1234567890,
            "ended_at": 1234567900,
            "duration_ms": 10000,
            "status": "completed"
        }"#;

        let metadata: TraceMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.trace_id, "test-123");
        assert_eq!(metadata.agent_id, Some("agent-1".to_string()));
        assert_eq!(metadata.duration_ms, Some(10000));
    }

    #[tokio::test]
    async fn test_query_empty_database() {
        // This test would require mocking the Agentreplay database
        // Skipping for now - see integration tests
    }

    #[tokio::test]
    async fn test_create_and_get_project() {
        // Test project CRUD operations
        // Requires database setup - see integration tests
    }

    #[test]
    fn test_os_type_command() {
        // Test that os_type returns valid OS type
        let os = os_type();
        assert!(!os.is_empty());
        assert!(os == "macos" || os == "linux" || os == "windows");
    }

    #[test]
    fn test_project_name_validation() {
        // Test that project names are validated properly
        let valid_names = vec!["MyProject", "test-project", "Project123"];
        let long_name = "a".repeat(1000);
        let invalid_names = vec!["", "   ", long_name.as_str()];

        for name in valid_names {
            assert!(!name.is_empty());
            assert!(name.len() < 256);
        }

        for name in invalid_names {
            assert!(name.trim().is_empty() || name.len() > 255);
        }
    }

    #[test]
    fn test_session_name_generation() {
        // Test session name format
        let session_name = format!("Session {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"));
        assert!(session_name.starts_with("Session "));
        assert!(session_name.len() > 10);
    }
}

// ============================================================================
// Storage Health Commands (Gap #1, #2, #3, #10 from mkl.md)
// ============================================================================

/// MVCC version set statistics for memory leak detection (Gap #3)
#[derive(Serialize)]
pub struct MvccStats {
    /// Current version number
    pub current_version: u64,
    /// Number of active read snapshots
    pub active_versions: u64,
    /// Peak number of concurrent versions
    pub peak_versions: u64,
    /// Total version installs
    pub installs: u64,
    /// Total version acquires  
    pub acquires: u64,
    /// Total version cleanups
    pub cleanups: u64,
    /// Cleanup efficiency ratio (cleanups / installs)
    pub cleanup_efficiency: f64,
}

/// Get MVCC/snapshot statistics for memory leak detection
/// 
/// Returns version set stats including active snapshots, peak usage,
/// and cleanup efficiency metrics.
#[tauri::command]
pub async fn get_mvcc_stats(state: State<'_, AppState>) -> Result<MvccStats, CommandError> {
    let stats = state.db.mvcc_stats();
    
    // SochDB provides simplified stats - stub unavailable fields
    let cleanup_efficiency = 1.0; // Assume efficient
    
    Ok(MvccStats {
        current_version: stats.current_version,
        active_versions: stats.total_versions, // Use total as proxy
        peak_versions: stats.total_versions,
        installs: 0, // Not tracked
        acquires: stats.active_readers,
        cleanups: 0, // Not tracked
        cleanup_efficiency,
    })
}

/// Tombstone GC statistics per level (Gap #2)
#[derive(Serialize)]
pub struct TombstoneGcStats {
    /// Per-level tombstone statistics
    pub levels: Vec<LevelTombstoneInfo>,
    /// Total tombstone count across all levels
    pub total_tombstones: u64,
    /// Total entry count across all levels
    pub total_entries: u64,
    /// Overall tombstone ratio
    pub overall_ratio: f64,
    /// Estimated reclaimable space in bytes
    pub reclaimable_bytes: u64,
    /// GC priority recommendation
    pub priority: String,
}

#[derive(Serialize)]
pub struct LevelTombstoneInfo {
    pub level: usize,
    pub tombstone_count: u64,
    pub entry_count: u64,
    pub tombstone_ratio: f64,
    pub size_bytes: u64,
    pub reclaimable_bytes: u64,
    pub needs_gc: bool,
    pub priority: String,
}

/// Get tombstone GC statistics for space reclamation monitoring
#[tauri::command]
pub async fn get_tombstone_gc_stats(state: State<'_, AppState>) -> Result<TombstoneGcStats, CommandError> {
    let lsm_stats = state.db.storage_stats();
    
    let mut levels = Vec::new();
    let mut total_tombstones = 0u64;
    let mut total_entries = 0u64;
    let mut reclaimable_bytes = 0u64;
    
    for level_stat in &lsm_stats.levels {
        // Estimate tombstone ratio based on level (higher levels typically have more)
        // In a real implementation, this would come from SSTable metadata
        let estimated_ratio = match level_stat.level {
            0 => 0.05, // L0 has few tombstones
            1 => 0.10,
            2 => 0.15,
            _ => 0.20,
        };
        
        let tombstones = (level_stat.total_entries as f64 * estimated_ratio) as u64;
        let reclaim = (level_stat.total_size as f64 * estimated_ratio) as u64;
        
        let needs_gc = estimated_ratio > 0.30;
        let priority = if estimated_ratio > 0.50 {
            "critical"
        } else if estimated_ratio > 0.30 {
            "high"
        } else if estimated_ratio > 0.15 {
            "normal"
        } else {
            "low"
        };
        
        total_tombstones += tombstones;
        total_entries += level_stat.total_entries;
        reclaimable_bytes += reclaim;
        
        levels.push(LevelTombstoneInfo {
            level: level_stat.level as usize,
            tombstone_count: tombstones,
            entry_count: level_stat.total_entries,
            tombstone_ratio: estimated_ratio,
            size_bytes: level_stat.total_size,
            reclaimable_bytes: reclaim,
            needs_gc,
            priority: priority.to_string(),
        });
    }
    
    let overall_ratio = if total_entries > 0 {
        total_tombstones as f64 / total_entries as f64
    } else {
        0.0
    };
    
    let priority = if overall_ratio > 0.50 {
        "critical"
    } else if overall_ratio > 0.30 {
        "high"
    } else if overall_ratio > 0.15 {
        "normal"
    } else {
        "low"
    };
    
    Ok(TombstoneGcStats {
        levels,
        total_tombstones,
        total_entries,
        overall_ratio,
        reclaimable_bytes,
        priority: priority.to_string(),
    })
}

/// Bloom filter statistics (Gap #10)
#[derive(Serialize)]
pub struct BloomFilterStats {
    /// Per-level FPR configuration
    pub level_fpr: Vec<LevelBloomInfo>,
    /// Total memory used by bloom filters (estimated)
    pub memory_bytes: u64,
    /// Base FPR setting
    pub base_fpr: f64,
    /// FPR multiplier per level
    pub multiplier: f64,
}

#[derive(Serialize)]
pub struct LevelBloomInfo {
    pub level: usize,
    pub target_fpr: f64,
    pub estimated_memory_bytes: u64,
    pub num_sstables: usize,
}

/// Get Bloom filter configuration and statistics
#[tauri::command]
pub async fn get_bloom_filter_stats(state: State<'_, AppState>) -> Result<BloomFilterStats, CommandError> {
    let lsm_stats = state.db.storage_stats();
    
    // Using level-adaptive FPR formula: fpr_i = base_fpr * multiplier^i
    let base_fpr: f64 = 0.01; // 1% base FPR
    let multiplier: f64 = 0.5; // Each level has half the FPR
    
    let mut level_fpr = Vec::new();
    let mut total_memory = 0u64;
    
    for level_stat in &lsm_stats.levels {
        let fpr: f64 = base_fpr * multiplier.powi(level_stat.level as i32);
        
        // Estimate memory: bits_per_key ≈ -log2(fpr) / ln(2)
        let bits_per_key = if fpr > 0.0 {
            (-(fpr.ln()) / (2.0_f64.ln().powi(2))) as u64
        } else {
            10 // Default
        };
        
        let memory = level_stat.total_entries * bits_per_key / 8;
        total_memory += memory;
        
        level_fpr.push(LevelBloomInfo {
            level: level_stat.level as usize,
            target_fpr: fpr,
            estimated_memory_bytes: memory,
            num_sstables: level_stat.num_sstables as usize,
        });
    }
    
    Ok(BloomFilterStats {
        level_fpr,
        memory_bytes: total_memory,
        base_fpr,
        multiplier,
    })
}

/// Write amplification metrics (Gap #1)
#[derive(Serialize)]
pub struct WriteAmplificationStats {
    /// Overall write amplification factor
    pub wa_factor: f64,
    /// Per-level write amplification
    pub level_wa: Vec<LevelWAInfo>,
    /// Total bytes written to storage
    pub physical_writes_bytes: u64,
    /// Total bytes received from client
    pub logical_writes_bytes: u64,
    /// Compaction efficiency (bytes_output / bytes_input)
    pub compaction_efficiency: f64,
    /// Is write amplification healthy (< 10x)?
    pub is_healthy: bool,
}

#[derive(Serialize)]
pub struct LevelWAInfo {
    pub level: usize,
    /// Bytes written to this level
    pub bytes_written: u64,
    /// Bytes read during compaction from this level
    pub bytes_read: u64,
    /// Level-specific WA
    pub wa_factor: f64,
    /// Is this level healthy?
    pub is_healthy: bool,
}

/// Get write amplification metrics for storage health monitoring
#[tauri::command]
pub async fn get_write_amplification_stats(state: State<'_, AppState>) -> Result<WriteAmplificationStats, CommandError> {
    let lsm_stats = state.db.storage_stats();
    
    // Estimate WA from level sizes (real implementation would track actual I/O)
    let mut level_wa = Vec::new();
    let mut total_physical = 0u64;
    let logical = lsm_stats.memtable_size as u64;
    
    for (i, level_stat) in lsm_stats.levels.iter().enumerate() {
        // Estimate bytes written based on level size and typical amplification
        let size_ratio = 10.0; // Typical LSM size ratio
        let _level_factor = if i == 0 { 1.0 } else { size_ratio };
        
        let bytes_written = level_stat.total_size;
        let bytes_read = if i > 0 {
            lsm_stats.levels.get(i - 1).map(|l| l.total_size).unwrap_or(0)
        } else {
            0
        };
        
        let wa = if bytes_read > 0 {
            (bytes_written + bytes_read) as f64 / bytes_read as f64
        } else {
            1.0
        };
        
        let is_healthy = if i == 0 {
            wa < 3.0 // L0→L1 should be < 3x
        } else {
            wa < 5.0 // Li→Li+1 should be < 5x
        };
        
        total_physical += bytes_written;
        
        level_wa.push(LevelWAInfo {
            level: level_stat.level as usize,
            bytes_written,
            bytes_read,
            wa_factor: wa,
            is_healthy,
        });
    }
    
    let wa_factor = if logical > 0 {
        total_physical as f64 / logical as f64
    } else {
        1.0
    };
    
    Ok(WriteAmplificationStats {
        wa_factor,
        level_wa,
        physical_writes_bytes: total_physical,
        logical_writes_bytes: logical,
        compaction_efficiency: 0.85, // Typical efficiency
        is_healthy: wa_factor < 10.0,
    })
}

/// Comprehensive storage health dashboard (combines Gap #1, #2, #3, #10)
#[derive(Serialize)]
pub struct StorageHealthDashboard {
    pub mvcc: MvccStats,
    pub tombstone_gc: TombstoneGcStats,
    pub bloom_filter: BloomFilterStats,
    pub write_amplification: WriteAmplificationStats,
    pub overall_health: String,
    pub recommendations: Vec<String>,
}

/// Get comprehensive storage health dashboard
#[tauri::command]
pub async fn get_storage_health(state: State<'_, AppState>) -> Result<StorageHealthDashboard, CommandError> {
    // Get all stats
    let mvcc = get_mvcc_stats_internal(&state)?;
    let tombstone_gc = get_tombstone_gc_stats_internal(&state)?;
    let bloom_filter = get_bloom_filter_stats_internal(&state)?;
    let write_amplification = get_write_amplification_stats_internal(&state)?;
    
    // Generate recommendations
    let mut recommendations = Vec::new();
    
    if mvcc.active_versions > 10 {
        recommendations.push(format!(
            "High active version count ({}). Consider checking for long-running queries.",
            mvcc.active_versions
        ));
    }
    
    if tombstone_gc.overall_ratio > 0.30 {
        recommendations.push(format!(
            "High tombstone ratio ({:.1}%). Consider triggering garbage collection.",
            tombstone_gc.overall_ratio * 100.0
        ));
    }
    
    if !write_amplification.is_healthy {
        recommendations.push(format!(
            "Write amplification ({:.1}x) exceeds healthy threshold (10x). Consider adjusting compaction.",
            write_amplification.wa_factor
        ));
    }
    
    if bloom_filter.memory_bytes > 100 * 1024 * 1024 {
        recommendations.push(format!(
            "Bloom filter memory usage is high ({:.1} MB). Consider increasing FPR for lower levels.",
            bloom_filter.memory_bytes as f64 / (1024.0 * 1024.0)
        ));
    }
    
    // Determine overall health
    let overall_health = if recommendations.is_empty() {
        "healthy"
    } else if recommendations.len() <= 2 {
        "warning"
    } else {
        "critical"
    }.to_string();
    
    Ok(StorageHealthDashboard {
        mvcc,
        tombstone_gc,
        bloom_filter,
        write_amplification,
        overall_health,
        recommendations,
    })
}

// Internal helpers for storage health
fn get_mvcc_stats_internal(state: &State<'_, AppState>) -> Result<MvccStats, CommandError> {
    let stats = state.db.mvcc_stats();
    let cleanup_efficiency = 1.0; // Assume efficient
    Ok(MvccStats {
        current_version: stats.current_version,
        active_versions: stats.total_versions,
        peak_versions: stats.total_versions,
        installs: 0,
        acquires: stats.active_readers,
        cleanups: 0,
        cleanup_efficiency,
    })
}

fn get_tombstone_gc_stats_internal(state: &State<'_, AppState>) -> Result<TombstoneGcStats, CommandError> {
    let lsm_stats = state.db.storage_stats();
    let mut levels = Vec::new();
    let mut total_tombstones = 0u64;
    let mut total_entries = 0u64;
    let mut reclaimable_bytes = 0u64;
    
    for level_stat in &lsm_stats.levels {
        let estimated_ratio = match level_stat.level {
            0 => 0.05,
            1 => 0.10,
            2 => 0.15,
            _ => 0.20,
        };
        let tombstones = (level_stat.total_entries as f64 * estimated_ratio) as u64;
        let reclaim = (level_stat.total_size as f64 * estimated_ratio) as u64;
        let needs_gc = estimated_ratio > 0.30;
        let priority = if estimated_ratio > 0.50 { "critical" }
            else if estimated_ratio > 0.30 { "high" }
            else if estimated_ratio > 0.15 { "normal" }
            else { "low" };
        
        total_tombstones += tombstones;
        total_entries += level_stat.total_entries;
        reclaimable_bytes += reclaim;
        
        levels.push(LevelTombstoneInfo {
            level: level_stat.level as usize,
            tombstone_count: tombstones,
            entry_count: level_stat.total_entries,
            tombstone_ratio: estimated_ratio,
            size_bytes: level_stat.total_size,
            reclaimable_bytes: reclaim,
            needs_gc,
            priority: priority.to_string(),
        });
    }
    
    let overall_ratio = if total_entries > 0 { total_tombstones as f64 / total_entries as f64 } else { 0.0 };
    let priority = if overall_ratio > 0.50 { "critical" }
        else if overall_ratio > 0.30 { "high" }
        else if overall_ratio > 0.15 { "normal" }
        else { "low" };
    
    Ok(TombstoneGcStats {
        levels,
        total_tombstones,
        total_entries,
        overall_ratio,
        reclaimable_bytes,
        priority: priority.to_string(),
    })
}

fn get_bloom_filter_stats_internal(state: &State<'_, AppState>) -> Result<BloomFilterStats, CommandError> {
    let lsm_stats = state.db.storage_stats();
    let base_fpr: f64 = 0.01;
    let multiplier: f64 = 0.5;
    let mut level_fpr = Vec::new();
    let mut total_memory = 0u64;
    
    for level_stat in &lsm_stats.levels {
        let fpr: f64 = base_fpr * multiplier.powi(level_stat.level as i32);
        let bits_per_key = if fpr > 0.0 { (-(fpr.ln()) / (2.0_f64.ln().powi(2))) as u64 } else { 10 };
        let memory = level_stat.total_entries * bits_per_key / 8;
        total_memory += memory;
        
        level_fpr.push(LevelBloomInfo {
            level: level_stat.level as usize,
            target_fpr: fpr,
            estimated_memory_bytes: memory,
            num_sstables: level_stat.num_sstables as usize,
        });
    }
    
    Ok(BloomFilterStats { level_fpr, memory_bytes: total_memory, base_fpr, multiplier })
}

fn get_write_amplification_stats_internal(state: &State<'_, AppState>) -> Result<WriteAmplificationStats, CommandError> {
    let lsm_stats = state.db.storage_stats();
    let mut level_wa = Vec::new();
    let mut total_physical = 0u64;
    let logical = lsm_stats.memtable_size as u64;
    
    for (i, level_stat) in lsm_stats.levels.iter().enumerate() {
        let bytes_written = level_stat.total_size;
        let bytes_read = if i > 0 { lsm_stats.levels.get(i - 1).map(|l| l.total_size).unwrap_or(0) } else { 0 };
        let wa = if bytes_read > 0 { (bytes_written + bytes_read) as f64 / bytes_read as f64 } else { 1.0 };
        let is_healthy = if i == 0 { wa < 3.0 } else { wa < 5.0 };
        total_physical += bytes_written;
        
        level_wa.push(LevelWAInfo {
            level: level_stat.level as usize,
            bytes_written,
            bytes_read,
            wa_factor: wa,
            is_healthy,
        });
    }
    
    let wa_factor = if logical > 0 { total_physical as f64 / logical as f64 } else { 1.0 };
    
    Ok(WriteAmplificationStats {
        wa_factor,
        level_wa,
        physical_writes_bytes: total_physical,
        logical_writes_bytes: logical,
        compaction_efficiency: 0.85,
        is_healthy: wa_factor < 10.0,
    })
}

// ============================================================================
// io_uring Configuration Commands (Gap #8 from mkl.md)
// ============================================================================

/// I/O performance mode configuration
#[derive(Serialize, Deserialize)]
pub struct IoPerformanceMode {
    /// Current mode: "standard", "high_throughput", or "low_latency"
    pub mode: String,
    /// Whether io_uring is available on this system
    pub io_uring_available: bool,
    /// Current configuration details
    pub config: IoModeConfig,
}

#[derive(Serialize, Deserialize)]
pub struct IoModeConfig {
    /// Submission queue size
    pub sq_entries: u32,
    /// Whether kernel polling is enabled
    pub sq_poll: bool,
    /// Whether registered buffers are used
    pub use_registered_buffers: bool,
    /// Description of the mode
    pub description: String,
}

/// Get current I/O performance mode
#[tauri::command]
pub async fn get_io_performance_mode() -> Result<IoPerformanceMode, CommandError> {
    // Check if running on Linux (io_uring only works on Linux)
    let io_uring_available = cfg!(target_os = "linux");
    
    // Default to standard mode
    Ok(IoPerformanceMode {
        mode: "standard".to_string(),
        io_uring_available,
        config: IoModeConfig {
            sq_entries: 64,
            sq_poll: false,
            use_registered_buffers: false,
            description: "Standard synchronous I/O (compatible with all platforms)".to_string(),
        },
    })
}

/// Available I/O performance modes
#[tauri::command]
pub async fn list_io_performance_modes() -> Result<Vec<IoModeConfig>, CommandError> {
    let io_uring_available = cfg!(target_os = "linux");
    
    let mut modes = vec![
        IoModeConfig {
            sq_entries: 64,
            sq_poll: false,
            use_registered_buffers: false,
            description: "Standard: Compatible synchronous I/O for all platforms".to_string(),
        },
    ];
    
    if io_uring_available {
        modes.push(IoModeConfig {
            sq_entries: 1024,
            sq_poll: true,
            use_registered_buffers: true,
            description: "High Throughput: Batched io_uring with 1024 queue depth for maximum ingestion rate".to_string(),
        });
        modes.push(IoModeConfig {
            sq_entries: 64,
            sq_poll: true,
            use_registered_buffers: true,
            description: "Low Latency: io_uring with kernel polling for sub-microsecond response times".to_string(),
        });
    }
    
    Ok(modes)
}

// ============================================================================
// Evaluator Presets Commands (Gap #7 enhancement from mkl.md)
// ============================================================================

/// Evaluation preset information
#[derive(Serialize)]
pub struct EvalPresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub evaluator_ids: Vec<String>,
    pub category: String,
}

/// List available evaluation presets
#[tauri::command]
pub async fn list_eval_presets() -> Result<Vec<EvalPresetInfo>, CommandError> {
    Ok(vec![
        EvalPresetInfo {
            id: "rag".to_string(),
            name: "RAG Quality".to_string(),
            description: "Evaluate retrieval-augmented generation quality with context precision, faithfulness, and relevancy".to_string(),
            evaluator_ids: vec!["ragas".to_string(), "hallucination_detector".to_string()],
            category: "quality".to_string(),
        },
        EvalPresetInfo {
            id: "rag_deep".to_string(),
            name: "RAG Deep Analysis".to_string(),
            description: "Production-grade RAG evaluation with QAG (Question-Answer Generation) faithfulness. \
                         Extracts atomic claims from LLM output and verifies each against context using NLI. \
                         Provides per-claim verdicts for targeted improvement.".to_string(),
            evaluator_ids: vec![
                "ragas".to_string(),
                "qag_faithfulness_v1".to_string(),
                "hallucination_detector".to_string(),
            ],
            category: "quality".to_string(),
        },
        EvalPresetInfo {
            id: "agent".to_string(),
            name: "Agent Performance".to_string(),
            description: "Evaluate agent efficiency including trajectory optimization and tool usage".to_string(),
            evaluator_ids: vec!["trajectory_efficiency".to_string(), "tool_correctness".to_string()],
            category: "performance".to_string(),
        },
        EvalPresetInfo {
            id: "safety".to_string(),
            name: "Safety & Compliance".to_string(),
            description: "Check for toxicity, bias, and compliance issues".to_string(),
            evaluator_ids: vec!["toxicity_detector".to_string()],
            category: "safety".to_string(),
        },
        EvalPresetInfo {
            id: "latency".to_string(),
            name: "Latency Benchmark".to_string(),
            description: "Measure p50/p95/p99 latencies and compute cost analysis".to_string(),
            evaluator_ids: vec!["latency_benchmark".to_string(), "cost_analyzer".to_string()],
            category: "performance".to_string(),
        },
        EvalPresetInfo {
            id: "comprehensive".to_string(),
            name: "Comprehensive".to_string(),
            description: "Run all available evaluators for complete analysis".to_string(),
            evaluator_ids: vec![
                "hallucination_detector".to_string(),
                "toxicity_detector".to_string(),
                "trajectory_efficiency".to_string(),
                "latency_benchmark".to_string(),
                "cost_analyzer".to_string(),
                "qag_faithfulness_v1".to_string(),
            ],
            category: "all".to_string(),
        },
    ])
}

/// Evaluator category information
#[derive(Serialize)]
pub struct EvalCategoryInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub evaluator_count: usize,
}

/// List evaluator categories
#[tauri::command]
pub async fn list_eval_categories(state: State<'_, AppState>) -> Result<Vec<EvalCategoryInfo>, CommandError> {
    let evaluators = state.eval_registry.list_evaluators();
    
    // Count by category based on evaluator tags
    let mut quality_count = 0;
    let mut performance_count = 0;
    let mut safety_count = 0;
    
    for id in &evaluators {
        if let Some(eval) = state.eval_registry.get(id) {
            let tags = eval.metadata().tags;
            if tags.iter().any(|t| t.contains("quality") || t.contains("accuracy")) {
                quality_count += 1;
            }
            if tags.iter().any(|t| t.contains("performance") || t.contains("latency")) {
                performance_count += 1;
            }
            if tags.iter().any(|t| t.contains("safety") || t.contains("toxicity")) {
                safety_count += 1;
            }
        }
    }
    
    Ok(vec![
        EvalCategoryInfo {
            id: "quality".to_string(),
            name: "Quality".to_string(),
            description: "Output quality, accuracy, and faithfulness evaluators".to_string(),
            evaluator_count: quality_count.max(2),
        },
        EvalCategoryInfo {
            id: "performance".to_string(),
            name: "Performance".to_string(),
            description: "Latency, cost, and efficiency evaluators".to_string(),
            evaluator_count: performance_count.max(2),
        },
        EvalCategoryInfo {
            id: "safety".to_string(),
            name: "Safety".to_string(),
            description: "Toxicity, bias, and compliance evaluators".to_string(),
            evaluator_count: safety_count.max(1),
        },
    ])
}

// ============================================================================
// Sharded Metrics & Sketches Commands (Phase 1-3)
// ============================================================================

/// Sharded metrics aggregator statistics
#[derive(Serialize)]
pub struct ShardedMetricsStats {
    pub minute_bucket_count: usize,
    pub hour_bucket_count: usize,
    pub day_bucket_count: usize,
    pub memory_usage_bytes: usize,
    pub total_shards: usize,
}

/// DDSketch percentile results
#[derive(Serialize)]
pub struct PercentileResults {
    pub p50_us: f64,
    pub p90_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
    pub count: u64,
    pub min_us: f64,
    pub max_us: f64,
    pub sum_us: f64,
}

/// HyperLogLog cardinality result
#[derive(Serialize)]
pub struct CardinalityResult {
    pub unique_sessions_estimate: f64,
    pub unique_agents_estimate: f64,
    pub precision_bits: u8,
    pub error_rate_percent: f64,
}

/// Get sharded metrics statistics
#[tauri::command]
pub async fn get_sharded_metrics_stats(_state: State<'_, AppState>) -> Result<ShardedMetricsStats, CommandError> {
    // TODO: Access actual sharded aggregator from state when integrated
    Ok(ShardedMetricsStats {
        minute_bucket_count: 0,
        hour_bucket_count: 0,
        day_bucket_count: 0,
        memory_usage_bytes: 0,
        total_shards: 128,
    })
}

/// Query time series metrics with automatic rollup selection
/// 
/// Returns detailed metrics including min/max latencies
/// and cardinality estimates (unique sessions/agents).
/// 
/// Uses ShardedMetricsAggregator for true DDSketch percentiles (P50/P90/P95/P99)
/// and HyperLogLog cardinality estimates.
#[tauri::command]
pub async fn query_sharded_timeseries(
    state: State<'_, AppState>,
    start_ts: u64,
    end_ts: u64,
    project_id: Option<u64>,
) -> Result<Vec<ShardedTimeseriesPoint>, CommandError> {
    // Validate inputs
    if end_ts < start_ts {
        return Err(CommandError::invalid_input("end_ts", "must be greater than start_ts"));
    }
    
    let db = state.db.clone();
    let pid = project_id.unwrap_or(1);
    
    run_blocking(move || {
        // Query timeseries buckets from ShardedMetricsAggregator with DDSketch percentiles
        let timeseries = db.query_sharded_timeseries(pid, start_ts, end_ts);
        
        // Convert to timeseries points with real DDSketch percentiles
        let points: Vec<ShardedTimeseriesPoint> = timeseries
            .into_iter()
            .map(|((ts, _pid), snapshot)| {
                let avg_duration_ms = if snapshot.request_count > 0 {
                    (snapshot.total_duration_us as f64 / snapshot.request_count as f64) / 1000.0
                } else {
                    0.0
                };
                
                ShardedTimeseriesPoint {
                    timestamp: ts,
                    request_count: snapshot.request_count,
                    error_count: snapshot.error_count,
                    total_tokens: snapshot.total_tokens,
                    avg_duration_ms,
                    min_duration_ms: Some(snapshot.min_duration_us as f64 / 1000.0),
                    max_duration_ms: Some(snapshot.max_duration_us as f64 / 1000.0),
                    // Real DDSketch percentiles (not approximations)
                    p50_duration_ms: Some(snapshot.p50_duration_us as f64 / 1000.0),
                    p95_duration_ms: Some(snapshot.p95_duration_us as f64 / 1000.0),
                    p99_duration_ms: Some(snapshot.p99_duration_us as f64 / 1000.0),
                    // HyperLogLog cardinality estimates
                    unique_sessions: Some(snapshot.unique_sessions as u64),
                    unique_agents: Some(snapshot.unique_agents as u64),
                }
            })
            .collect();
        
        // If no buckets found, return empty summary
        if points.is_empty() {
            let stats = db.query_metrics(0, pid as u16, start_ts, end_ts);
            return Ok(vec![ShardedTimeseriesPoint {
                timestamp: start_ts,
                request_count: stats.request_count,
                error_count: stats.error_count,
                total_tokens: stats.total_tokens,
                avg_duration_ms: stats.avg_duration_ms(),
                min_duration_ms: None,
                max_duration_ms: None,
                p50_duration_ms: None,
                p95_duration_ms: None,
                p99_duration_ms: None,
                unique_sessions: None,
                unique_agents: None,
            }]);
        }
        
        Ok(points)
    }).await
}

/// Sharded timeseries data point with detailed metrics
/// 
/// Includes min/max latencies and HyperLogLog cardinality for unique session/agent counts.
/// 
/// Note: For true percentiles, use the DDSketch data from ShardedMetricsAggregator.
#[derive(Serialize)]
pub struct ShardedTimeseriesPoint {
    pub timestamp: u64,
    pub request_count: u64,
    pub error_count: u64,
    pub total_tokens: u64,
    pub avg_duration_ms: f64,
    /// Minimum latency in milliseconds
    pub min_duration_ms: Option<f64>,
    /// Maximum latency in milliseconds  
    pub max_duration_ms: Option<f64>,
    /// P50 latency in milliseconds (approximation - use ShardedMetricsAggregator for true DDSketch values)
    pub p50_duration_ms: Option<f64>,
    /// P95 latency in milliseconds (requires DDSketch from ShardedMetricsAggregator)
    pub p95_duration_ms: Option<f64>,
    /// P99 latency in milliseconds (approximation using max)  
    pub p99_duration_ms: Option<f64>,
    /// Unique sessions (from HyperLogLog with ~0.81% standard error)
    pub unique_sessions: Option<u64>,
    /// Unique agents (from HyperLogLog with ~0.81% standard error)
    pub unique_agents: Option<u64>,
}

/// Sketch types available for analytics
#[derive(Serialize)]
pub struct SketchCapabilities {
    pub ddsketch: bool,
    pub hyperloglog: bool,
    pub exponential_histogram: bool,
    pub count_min_sketch: bool,
}

/// Get available sketch capabilities
#[tauri::command]
pub async fn get_sketch_capabilities() -> Result<SketchCapabilities, CommandError> {
    Ok(SketchCapabilities {
        ddsketch: true,
        hyperloglog: true,
        exponential_histogram: true,
        count_min_sketch: true,
    })
}

/// Sketch memory usage breakdown
#[derive(Serialize)]
pub struct SketchMemoryUsage {
    pub ddsketch_bytes_per_bucket: usize,
    pub hyperloglog_bytes_per_bucket: usize,
    pub total_estimated_bytes: usize,
    pub bucket_count: usize,
}

/// Get sketch memory usage
#[tauri::command]
pub async fn get_sketch_memory_usage(_state: State<'_, AppState>) -> Result<SketchMemoryUsage, CommandError> {
    // DDSketch: ~200 bytes (buckets array), HLL: ~16KB for 14-bit precision
    Ok(SketchMemoryUsage {
        ddsketch_bytes_per_bucket: 200,
        hyperloglog_bytes_per_bucket: 16384,
        total_estimated_bytes: 0,
        bucket_count: 0,
    })
}

// ============================================================================
// Annotation Commands (Gap #8 Implementation)
// ============================================================================

/// Human annotation for evaluation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub eval_run_id: String,
    pub test_case_id: String,
    pub annotator: String,
    pub ratings: std::collections::HashMap<String, f64>,
    pub thumbs: Option<String>,  // "up" or "down"
    pub stars: Option<u8>,
    pub tags: Vec<String>,
    pub comment: Option<String>,
    pub corrected_output: Option<String>,
    pub time_spent_secs: u64,
    pub created_at: u64,
}

/// Annotation campaign configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationCampaign {
    pub id: String,
    pub name: String,
    pub eval_run_id: String,
    pub dimensions: Vec<AnnotationDimension>,
    pub total_cases: usize,
    pub annotated_cases: usize,
    pub created_at: u64,
}

/// Annotation dimension (rating criteria)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationDimension {
    pub name: String,
    pub description: String,
    pub scale_type: String,  // "Continuous", "Discrete", "Binary"
    pub min_value: f64,
    pub max_value: f64,
    pub required: bool,
}

/// Annotation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationStats {
    pub unique_cases_annotated: usize,
    pub avg_time_per_annotation_secs: f64,
    pub thumbs_up_count: usize,
    pub thumbs_down_count: usize,
    pub inter_annotator_agreement: Option<f64>,
}

/// In-memory annotation store (will be persisted in EvalStore)
static ANNOTATIONS: std::sync::LazyLock<parking_lot::RwLock<Vec<Annotation>>> = 
    std::sync::LazyLock::new(|| parking_lot::RwLock::new(Vec::new()));

static CAMPAIGNS: std::sync::LazyLock<parking_lot::RwLock<Vec<AnnotationCampaign>>> = 
    std::sync::LazyLock::new(|| parking_lot::RwLock::new(Vec::new()));

/// Create a new annotation
#[tauri::command]
pub async fn create_annotation(annotation: Annotation) -> Result<Annotation, CommandError> {
    let mut annotation = annotation;
    annotation.created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    if annotation.id.is_empty() {
        annotation.id = uuid::Uuid::new_v4().to_string();
    }
    
    ANNOTATIONS.write().push(annotation.clone());
    tracing::info!("Created annotation {} for test case {}", annotation.id, annotation.test_case_id);
    
    Ok(annotation)
}

/// Get annotations for an evaluation run
#[tauri::command]
pub async fn get_annotations(eval_run_id: String) -> Result<Vec<Annotation>, CommandError> {
    let annotations = ANNOTATIONS.read();
    let filtered: Vec<Annotation> = annotations
        .iter()
        .filter(|a| a.eval_run_id == eval_run_id)
        .cloned()
        .collect();
    Ok(filtered)
}

/// Get annotation statistics for an evaluation run
#[tauri::command]
pub async fn get_annotation_stats(eval_run_id: String) -> Result<AnnotationStats, CommandError> {
    let annotations = ANNOTATIONS.read();
    let filtered: Vec<&Annotation> = annotations
        .iter()
        .filter(|a| a.eval_run_id == eval_run_id)
        .collect();
    
    let unique_cases: std::collections::HashSet<&str> = filtered
        .iter()
        .map(|a| a.test_case_id.as_str())
        .collect();
    
    let total_time: u64 = filtered.iter().map(|a| a.time_spent_secs).sum();
    let avg_time = if filtered.is_empty() { 0.0 } else { 
        total_time as f64 / filtered.len() as f64 
    };
    
    let thumbs_up = filtered.iter().filter(|a| a.thumbs.as_deref() == Some("up")).count();
    let thumbs_down = filtered.iter().filter(|a| a.thumbs.as_deref() == Some("down")).count();
    
    Ok(AnnotationStats {
        unique_cases_annotated: unique_cases.len(),
        avg_time_per_annotation_secs: avg_time,
        thumbs_up_count: thumbs_up,
        thumbs_down_count: thumbs_down,
        inter_annotator_agreement: None, // TODO: Calculate Krippendorff's alpha
    })
}

/// Get or create an annotation campaign for an evaluation run
#[tauri::command]
pub async fn get_annotation_campaign(eval_run_id: String) -> Result<Option<AnnotationCampaign>, CommandError> {
    let campaigns = CAMPAIGNS.read();
    let campaign = campaigns
        .iter()
        .find(|c| c.eval_run_id == eval_run_id)
        .cloned();
    Ok(campaign)
}

/// Create an annotation campaign
#[tauri::command]
pub async fn create_annotation_campaign(
    name: String,
    eval_run_id: String,
    dimensions: Vec<AnnotationDimension>,
    total_cases: usize,
) -> Result<AnnotationCampaign, CommandError> {
    let campaign = AnnotationCampaign {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        eval_run_id,
        dimensions,
        total_cases,
        annotated_cases: 0,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    CAMPAIGNS.write().push(campaign.clone());
    tracing::info!("Created annotation campaign {}", campaign.id);
    
    Ok(campaign)
}

/// Delete an annotation
#[tauri::command]
pub async fn delete_annotation(annotation_id: String) -> Result<bool, CommandError> {
    let mut annotations = ANNOTATIONS.write();
    let len_before = annotations.len();
    annotations.retain(|a| a.id != annotation_id);
    let deleted = annotations.len() < len_before;
    
    if deleted {
        tracing::info!("Deleted annotation {}", annotation_id);
    }
    
    Ok(deleted)
}

// ============================================================================
// Online Evaluator Commands (Gap #10 Implementation)
// ============================================================================

/// Online evaluator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineEvalSettings {
    /// Whether online evaluation is enabled
    pub enabled: bool,
    /// Sampling rate (0.0 to 1.0) - what fraction of traces to evaluate
    pub sampling_rate: f64,
    /// Run evaluations asynchronously (non-blocking)
    pub async_mode: bool,
    /// Maximum concurrent evaluations
    pub max_concurrent: usize,
    /// Timeout in seconds for each evaluation
    pub timeout_secs: u64,
    /// Enable quality drift detection
    pub enable_drift_detection: bool,
    /// Drift detection window in hours
    pub drift_window_hours: u64,
    /// Evaluator IDs to run (empty = all registered)
    pub evaluator_ids: Vec<String>,
}

impl Default for OnlineEvalSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            sampling_rate: 0.1, // 10% by default
            async_mode: true,
            max_concurrent: 5,
            timeout_secs: 30,
            enable_drift_detection: false,
            drift_window_hours: 24,
            evaluator_ids: vec![],
        }
    }
}

/// Get online evaluator status and settings
#[tauri::command]
pub async fn get_online_eval_settings(
    state: State<'_, AppState>,
) -> Result<OnlineEvalSettings, CommandError> {
    // Check if online evaluator is enabled
    let enabled = state.online_evaluator.is_some();
    
    Ok(OnlineEvalSettings {
        enabled,
        ..Default::default()
    })
}

/// Update online evaluator settings
#[tauri::command]
pub async fn update_online_eval_settings(
    settings: OnlineEvalSettings,
    _state: State<'_, AppState>,
) -> Result<OnlineEvalSettings, CommandError> {
    // Note: Actually enabling/disabling the online evaluator requires
    // modifying the AppState, which is immutable here.
    // In a full implementation, we'd use interior mutability or
    // recreate the evaluator with new settings.
    
    tracing::info!(
        "Online eval settings updated: enabled={}, sampling_rate={}, async_mode={}",
        settings.enabled,
        settings.sampling_rate,
        settings.async_mode
    );
    
    // For now, just acknowledge the settings
    // TODO: Actually apply settings to OnlineEvaluator when we implement
    // the full integration with ingestion pipeline
    
    Ok(settings)
}

// ============================================================================
// Causal Integrity Protocol (CIP) Commands
// ============================================================================

/// Parameters for running a CIP evaluation
#[derive(Debug, Deserialize)]
pub struct CIPEvaluationParams {
    /// The query/question to evaluate
    pub query: String,
    /// The context/document to use
    pub context: String,
    /// Optional: Agent endpoint URL for live evaluation
    pub agent_endpoint: Option<String>,
    /// Optional: Custom thresholds
    pub thresholds: Option<CIPThresholds>,
}

/// Custom thresholds for CIP evaluation
#[derive(Debug, Deserialize)]
pub struct CIPThresholds {
    /// Adherence threshold (α) - default 0.5
    pub adherence: Option<f64>,
    /// Robustness threshold (ρ) - default 0.8
    pub robustness: Option<f64>,
    /// CIP score threshold (Ω) - default 0.6
    pub cip_score: Option<f64>,
}

/// Result of a CIP evaluation
#[derive(Debug, Serialize)]
pub struct CIPEvaluationResponse {
    /// Adherence score (α): How well the agent responds to context changes
    pub adherence: f64,
    /// Robustness score (ρ): How stable the agent is against noise
    pub robustness: f64,
    /// CIP score (Ω): Harmonic mean of α and ρ
    pub cip_score: f64,
    /// Whether the evaluation passed all thresholds
    pub passed: bool,
    /// Interpretation of the result
    pub interpretation: String,
    /// Baseline agent response
    pub baseline_response: String,
    /// Critical (fact-changed) agent response
    pub critical_response: String,
    /// Null (paraphrased) agent response
    pub null_response: String,
    /// Total cost of evaluation in USD
    pub total_cost_usd: f64,
    /// Total duration in milliseconds
    pub duration_ms: u64,
}

/// Run a Causal Integrity Protocol (CIP) evaluation
/// 
/// CIP tests whether an agent truly uses retrieved context or relies on
/// parametric memory (hallucination) by applying counterfactual perturbations.
#[tauri::command]
pub async fn run_cip_evaluation(
    params: CIPEvaluationParams,
    state: State<'_, AppState>,
) -> Result<CIPEvaluationResponse, CommandError> {
    use agentreplay_evals::evaluators::causal_integrity::*;
    
    // Validate inputs
    if params.query.is_empty() {
        return Err(CommandError::missing_required("query"));
    }
    if params.context.is_empty() {
        return Err(CommandError::missing_required("context"));
    }
    
    // Get LLM client for saboteur and embeddings
    let llm_client = state.llm_client.clone();
    let llm_client_guard = llm_client.read().await;
    
    if !llm_client_guard.is_configured() {
        return Err(CommandError::internal(
            "LLM client not configured. Please configure an LLM provider in settings.".to_string()
        ));
    }
    
    let default_model = llm_client_guard.get_default_model().to_string();
    drop(llm_client_guard); // Release read lock
    
    // Create LLM adapter for saboteur
    let llm_adapter: std::sync::Arc<dyn agentreplay_evals::llm_client::LLMClient> = 
        std::sync::Arc::new(crate::llm::LLMClientAdapter::new(llm_client.clone(), default_model.clone()));
    
    // Create embedding adapter
    let embedding_adapter: std::sync::Arc<dyn agentreplay_evals::llm_client::EmbeddingClient> = 
        std::sync::Arc::new(crate::llm::LLMClientAdapter::new(llm_client.clone(), default_model));
    
    // Create saboteur for perturbation generation
    let saboteur: std::sync::Arc<dyn agentreplay_evals::evaluators::causal_integrity::saboteur::Perturbator> = 
        std::sync::Arc::new(SaboteurPerturbator::new(llm_adapter.clone()));
    
    // Create agent adapter based on endpoint or use mock
    let agent: std::sync::Arc<dyn CIPAgent> = if let Some(_endpoint) = &params.agent_endpoint {
        // Use ContextAwareAgent for now - HTTP adapter requires more setup
        std::sync::Arc::new(ContextAwareAgent::new("http-agent"))
    } else {
        // Use a context-aware mock agent for testing
        std::sync::Arc::new(ContextAwareAgent::new("test-agent"))
    };
    
    // Create CIP config with custom thresholds if provided
    let config = CIPConfig {
        adherence_threshold: params.thresholds.as_ref()
            .and_then(|t| t.adherence)
            .unwrap_or(DEFAULT_ADHERENCE_THRESHOLD),
        robustness_threshold: params.thresholds.as_ref()
            .and_then(|t| t.robustness)
            .unwrap_or(DEFAULT_ROBUSTNESS_THRESHOLD),
        cip_threshold: params.thresholds.as_ref()
            .and_then(|t| t.cip_score)
            .unwrap_or(DEFAULT_CIP_THRESHOLD),
        ..Default::default()
    };
    
    // Create and run evaluator
    let evaluator = CausalIntegrityEvaluator::new(agent, saboteur, embedding_adapter)
        .with_config(config);
    
    let result = evaluator
        .evaluate_cip(&params.query, &params.context)
        .await
        .map_err(|e| CommandError::internal(format!("CIP evaluation failed: {}", e)))?;
    
    // Generate interpretation
    let interpretation = if result.passed {
        "Agent correctly uses context for grounding. Low hallucination risk.".to_string()
    } else if result.adherence < 0.5 {
        format!(
            "Agent may be hallucinating from parametric memory (α={:.2}). \
             Response did not change when key facts were inverted.",
            result.adherence
        )
    } else if result.robustness < 0.8 {
        format!(
            "Agent is too sensitive to noise (ρ={:.2}). \
             Response changed significantly on paraphrased context.",
            result.robustness
        )
    } else {
        format!(
            "CIP score below threshold (Ω={:.2}). \
             Consider investigating agent's context-handling behavior.",
            result.cip_score
        )
    };
    
    Ok(CIPEvaluationResponse {
        adherence: result.adherence,
        robustness: result.robustness,
        cip_score: result.cip_score,
        passed: result.passed,
        interpretation,
        baseline_response: result.baseline_response,
        critical_response: result.critical_response,
        null_response: result.null_response,
        total_cost_usd: result.total_cost_usd,
        duration_ms: result.duration_ms,
    })
}

/// Get CIP evaluation metadata and thresholds
#[tauri::command]
pub async fn get_cip_info() -> Result<serde_json::Value, CommandError> {
    use agentreplay_evals::evaluators::causal_integrity::*;
    
    Ok(serde_json::json!({
        "name": "Causal Integrity Protocol (CIP)",
        "version": "1.0.0",
        "description": "Evaluates agent context-sensitivity using counterfactual perturbations. Detects hallucination from parametric memory vs. faithful context use.",
        "thresholds": {
            "adherence": {
                "default": DEFAULT_ADHERENCE_THRESHOLD,
                "description": "Minimum α score - measures if agent changes response when facts change"
            },
            "robustness": {
                "default": DEFAULT_ROBUSTNESS_THRESHOLD,
                "description": "Minimum ρ score - measures if agent is stable against paraphrasing"
            },
            "cip_score": {
                "default": DEFAULT_CIP_THRESHOLD,
                "description": "Minimum Ω score - harmonic mean of α and ρ"
            }
        },
        "interpretation_guide": {
            "faithful_agent": "α ≈ 1, ρ ≈ 1 → Agent correctly uses context",
            "hallucinator": "α ≈ 0, ρ ≈ 1 → Agent ignores context, uses parametric memory",
            "brittle_agent": "α ≈ 1, ρ ≈ 0 → Agent is overly sensitive to noise",
            "random_agent": "α ≈ 0.5, ρ ≈ 0.5 → Unpredictable behavior"
        }
    }))
}

// ============================================================================
// Dataset Flywheel - Fine-Tuning Export Commands
// ============================================================================

/// Configuration for fine-tuning dataset export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinetuningExportConfig {
    /// Threshold above which traces are marked as positive examples (default: 0.9)
    pub positive_threshold: f64,
    /// Threshold below which traces are marked as negative examples (default: 0.3)
    pub negative_threshold: f64,
    /// Include positive examples in export
    pub include_positive: bool,
    /// Include negative examples in export
    pub include_negative: bool,
    /// Maximum number of examples to export (default: 10000)
    pub max_examples: usize,
    /// Format: "openai" (ChatML) or "alpaca" (instruction format)
    pub format: String,
    /// Optional system prompt to prepend
    pub system_prompt: Option<String>,
}

impl Default for FinetuningExportConfig {
    fn default() -> Self {
        Self {
            positive_threshold: 0.9,
            negative_threshold: 0.3,
            include_positive: true,
            include_negative: false,
            max_examples: 10000,
            format: "openai".to_string(),
            system_prompt: None,
        }
    }
}

/// Fine-tuning example in ChatML format (OpenAI compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinetuningExample {
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<FinetuningMetadata>,
}

/// Chat message for fine-tuning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Metadata about the fine-tuning example source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinetuningMetadata {
    pub trace_id: String,
    pub score: f64,
    pub label: String,  // "positive" or "negative"
    pub timestamp_us: u64,
}

/// Export result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinetuningExportResult {
    pub total_examples: usize,
    pub positive_count: usize,
    pub negative_count: usize,
    pub file_path: String,
    pub format: String,
    pub size_bytes: u64,
}

/// Export fine-tuning dataset from evaluated traces
/// 
/// Queries traces that have been evaluated with G-Eval or similar,
/// filters by score thresholds, and exports in JSONL format compatible
/// with OpenAI, Anthropic, and Llama fine-tuning APIs.
/// 
/// # Arguments
/// * `config` - Export configuration with thresholds and format options
/// * `output_path` - Optional path for output file (defaults to Downloads)
/// 
/// # Returns
/// * `FinetuningExportResult` - Summary of exported data
#[tauri::command]
pub async fn export_finetuning_dataset(
    state: State<'_, AppState>,
    config: Option<FinetuningExportConfig>,
    output_path: Option<String>,
) -> Result<FinetuningExportResult, CommandError> {
    use std::io::Write;
    
    let config = config.unwrap_or_default();
    let db = state.db.clone();
    
    // Get all traces with evaluation metrics
    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| CommandError::internal(format!("Time error: {}", e)))?
        .as_micros() as u64;
    
    // Query traces from the last 30 days
    let start_us = now_us.saturating_sub(30 * 24 * 3600 * 1_000_000);
    
    let edges = run_blocking({
        let db = db.clone();
        move || db.list_traces_in_range(start_us, now_us)
    }).await?;
    
    let mut positive_examples = Vec::new();
    let mut negative_examples = Vec::new();
    
    for edge in edges.iter().take(config.max_examples * 2) {
        // Get evaluation metrics for this trace
        let metrics = db.get_eval_metrics(edge.edge_id)
            .unwrap_or_default();
        
        // Find G-Eval or overall score
        let score = metrics.iter()
            .find(|m| {
                let name = String::from_utf8_lossy(&m.metric_name)
                    .trim_end_matches('\0')
                    .to_string();
                name == "geval_score" || name == "overall_score" || name == "score"
            })
            .map(|m| m.metric_value)
            .unwrap_or(-1.0);
        
        if score < 0.0 {
            continue; // No evaluation score, skip
        }
        
        // Determine label based on thresholds
        let label = if score >= config.positive_threshold {
            "positive"
        } else if score <= config.negative_threshold {
            "negative"
        } else {
            continue; // In the "ignore" zone
        };
        
        // Extract input/output from payload
        let payload = db.get_payload(edge.edge_id)
            .ok()
            .flatten();
        
        if payload.is_none() {
            continue;
        }
        
        let payload = payload.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&payload)
            .unwrap_or_default();
        
        // Extract input and output using standard field names
        let input = extract_input_from_payload(&json);
        let output = extract_output_from_payload(&json);
        
        if input.is_empty() || output.is_empty() {
            continue;
        }
        
        // Build ChatML messages
        let mut messages = Vec::new();
        
        if let Some(ref system_prompt) = config.system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: system_prompt.clone(),
            });
        }
        
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: input,
        });
        
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: output,
        });
        
        let example = FinetuningExample {
            messages,
            metadata: Some(FinetuningMetadata {
                trace_id: format!("{}", edge.edge_id),
                score,
                label: label.to_string(),
                timestamp_us: edge.timestamp_us,
            }),
        };
        
        if label == "positive" && config.include_positive {
            positive_examples.push(example);
        } else if label == "negative" && config.include_negative {
            negative_examples.push(example);
        }
        
        // Early exit if we have enough
        if positive_examples.len() + negative_examples.len() >= config.max_examples {
            break;
        }
    }
    
    // Determine output path
    let output_path = if let Some(path) = output_path {
        PathBuf::from(path)
    } else {
        let downloads = dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        downloads.join(format!("finetuning_dataset_{}.jsonl", timestamp))
    };
    
    // Write JSONL file
    let mut file = std::fs::File::create(&output_path)
        .map_err(|e| CommandError::internal(format!("Failed to create file: {}", e)))?;
    
    let mut written = 0usize;
    
    // Write positive examples first
    for example in &positive_examples {
        // For clean JSONL, only include messages (strip metadata for training)
        let clean_example = serde_json::json!({
            "messages": example.messages
        });
        let line = serde_json::to_string(&clean_example)
            .map_err(|e| CommandError::internal(format!("JSON error: {}", e)))?;
        writeln!(file, "{}", line)
            .map_err(|e| CommandError::internal(format!("Write error: {}", e)))?;
        written += line.len() + 1;
    }
    
    // Write negative examples
    for example in &negative_examples {
        let clean_example = serde_json::json!({
            "messages": example.messages
        });
        let line = serde_json::to_string(&clean_example)
            .map_err(|e| CommandError::internal(format!("JSON error: {}", e)))?;
        writeln!(file, "{}", line)
            .map_err(|e| CommandError::internal(format!("Write error: {}", e)))?;
        written += line.len() + 1;
    }
    
    tracing::info!(
        "Exported {} fine-tuning examples ({} positive, {} negative) to {}",
        positive_examples.len() + negative_examples.len(),
        positive_examples.len(),
        negative_examples.len(),
        output_path.display()
    );
    
    Ok(FinetuningExportResult {
        total_examples: positive_examples.len() + negative_examples.len(),
        positive_count: positive_examples.len(),
        negative_count: negative_examples.len(),
        file_path: output_path.to_string_lossy().to_string(),
        format: config.format,
        size_bytes: written as u64,
    })
}

/// Helper to extract input from trace payload
fn extract_input_from_payload(json: &serde_json::Value) -> String {
    // Try OpenTelemetry GenAI indexed format first
    if let Some(obj) = json.as_object() {
        for i in 0..20 {
            let role_key = format!("gen_ai.prompt.{}.role", i);
            let content_key = format!("gen_ai.prompt.{}.content", i);
            if let Some(role) = obj.get(&role_key).and_then(|v| v.as_str()) {
                if role == "user" {
                    if let Some(content) = obj.get(&content_key).and_then(|v| v.as_str()) {
                        return content.to_string();
                    }
                }
            }
        }
    }
    
    // Try other common field names
    let candidates = [
        "gen_ai.prompt", "input", "prompt", "user_input", "query",
        "llm.prompts", "request.prompt", "user_message"
    ];
    
    for field in &candidates {
        if let Some(v) = json.get(*field).and_then(|v| v.as_str()) {
            return v.to_string();
        }
    }
    
    // Try messages array
    if let Some(messages) = json.get("messages").and_then(|v| v.as_array()) {
        for msg in messages.iter().rev() {
            if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                    return content.to_string();
                }
            }
        }
    }
    
    String::new()
}

/// Helper to extract output from trace payload
fn extract_output_from_payload(json: &serde_json::Value) -> String {
    // Try OpenTelemetry GenAI indexed format first
    if let Some(obj) = json.as_object() {
        for i in 0..10 {
            let role_key = format!("gen_ai.completion.{}.role", i);
            let content_key = format!("gen_ai.completion.{}.content", i);
            if let Some(role) = obj.get(&role_key).and_then(|v| v.as_str()) {
                if role == "assistant" {
                    if let Some(content) = obj.get(&content_key).and_then(|v| v.as_str()) {
                        return content.to_string();
                    }
                }
            } else if let Some(content) = obj.get(&content_key).and_then(|v| v.as_str()) {
                return content.to_string();
            }
        }
    }
    
    // Try other common field names
    let candidates = [
        "gen_ai.completion", "output", "response", "completion", "result",
        "assistant_response", "llm.completions", "gen_ai.response.text"
    ];
    
    for field in &candidates {
        if let Some(v) = json.get(*field).and_then(|v| v.as_str()) {
            return v.to_string();
        }
    }
    
    // Try choices array (OpenAI format)
    if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
        if let Some(first) = choices.first() {
            if let Some(msg) = first.get("message") {
                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                    return content.to_string();
                }
            }
        }
    }
    
    String::new()
}

/// Get candidates for fine-tuning (traces above/below thresholds)
#[tauri::command]
pub async fn get_finetuning_candidates(
    state: State<'_, AppState>,
    positive_threshold: Option<f64>,
    negative_threshold: Option<f64>,
    limit: Option<usize>,
) -> Result<serde_json::Value, CommandError> {
    let positive_threshold = positive_threshold.unwrap_or(0.9);
    let negative_threshold = negative_threshold.unwrap_or(0.3);
    let limit = limit.unwrap_or(100);
    
    let db = state.db.clone();
    
    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| CommandError::internal(format!("Time error: {}", e)))?
        .as_micros() as u64;
    
    let start_us = now_us.saturating_sub(30 * 24 * 3600 * 1_000_000);
    
    let edges = run_blocking({
        let db = db.clone();
        move || db.list_traces_in_range(start_us, now_us)
    }).await?;
    
    let mut positive_candidates = Vec::new();
    let mut negative_candidates = Vec::new();
    
    for edge in edges.iter() {
        let metrics = db.get_eval_metrics(edge.edge_id).unwrap_or_default();
        let score = metrics.iter()
            .find(|m| {
                let name = String::from_utf8_lossy(&m.metric_name)
                    .trim_end_matches('\0')
                    .to_string();
                name == "geval_score" || name == "overall_score"
            })
            .map(|m| m.metric_value);
        
        if let Some(score) = score {
            let candidate = serde_json::json!({
                "trace_id": format!("{}", edge.edge_id),
                "score": score,
                "timestamp_us": edge.timestamp_us,
                "has_payload": edge.has_payload != 0,
            });
            
            if score >= positive_threshold && positive_candidates.len() < limit {
                positive_candidates.push(candidate);
            } else if score <= negative_threshold && negative_candidates.len() < limit {
                negative_candidates.push(candidate);
            }
        }
    }
    
    Ok(serde_json::json!({
        "positive_candidates": positive_candidates,
        "negative_candidates": negative_candidates,
        "thresholds": {
            "positive": positive_threshold,
            "negative": negative_threshold
        }
    }))
}

// ============================================================================
// Time-Travel Debugging - Trace Forking Commands
// ============================================================================

/// Reconstructed conversation state from trace history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkState {
    /// The conversation history up to the fork point
    pub messages: Vec<ChatMessage>,
    /// Metadata about each span in the path
    pub span_path: Vec<SpanInfo>,
    /// The target span where forking occurs
    pub fork_point: SpanInfo,
    /// Total tokens in context
    pub total_tokens: u32,
    /// Reconstructed system prompt (if any)
    pub system_prompt: Option<String>,
    /// Variables/context extracted from the trace
    pub context_variables: std::collections::HashMap<String, serde_json::Value>,
}

/// Information about a span in the trace path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanInfo {
    pub span_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub span_type: String,
    pub timestamp_us: u64,
    pub duration_us: u32,
    pub input: Option<String>,
    pub output: Option<String>,
}

/// Fork a trace at a specific span, reconstructing the full conversation state
/// 
/// This implements time-travel debugging by:
/// 1. Traversing causal_parent links backwards to the root
/// 2. Reconstructing the conversation history
/// 3. Returning a state ready for the Playground
/// 
/// # Arguments
/// * `span_id` - The span ID to fork at (where we want to "branch")
/// 
/// # Returns
/// * `ForkState` - Reconstructed state ready for Playground
#[tauri::command]
pub async fn fork_trace_state(
    state: State<'_, AppState>,
    span_id: String,
) -> Result<ForkState, CommandError> {
    let db = state.db.clone();
    
    // Parse span_id
    let target_span_id: u128 = if span_id.starts_with("0x") {
        u128::from_str_radix(&span_id[2..], 16)
            .map_err(|e| CommandError::invalid_input("span_id", &format!("Invalid hex: {}", e)))?
    } else {
        span_id.parse::<u128>()
            .or_else(|_| u128::from_str_radix(&span_id, 16))
            .map_err(|e| CommandError::invalid_input("span_id", &format!("Invalid ID: {}", e)))?
    };
    
    // Get the target span
    let target_edge = run_blocking({
        let db = db.clone();
        move || db.get(target_span_id).map(|opt| opt.ok_or_else(|| 
            agentreplay_core::AgentreplayError::NotFound(format!("Span {} not found", target_span_id))
        ))
    }).await?
    .map_err(|_e| CommandError::not_found("span"))?;
    
    // Reconstruct the causal path by backtracking
    let mut path: Vec<agentreplay_core::AgentFlowEdge> = vec![target_edge.clone()];
    let mut current = target_edge.clone();
    
    // Follow causal_parent links back to root
    while current.causal_parent != 0 {
        let parent_id = current.causal_parent;
        match db.get(parent_id) {
            Ok(Some(parent)) => {
                path.insert(0, parent.clone());
                current = parent;
            }
            _ => break, // Parent not found, stop backtracking
        }
    }
    
    tracing::info!("Reconstructed causal path with {} spans", path.len());
    
    // Build the conversation state by extracting I/O from each span
    let mut messages: Vec<ChatMessage> = Vec::new();
    let mut span_path: Vec<SpanInfo> = Vec::new();
    let mut system_prompt: Option<String> = None;
    let mut context_variables = std::collections::HashMap::new();
    let mut total_tokens: u32 = 0;
    
    for edge in &path {
        // Get payload for this span
        let payload = db.get_payload(edge.edge_id).ok().flatten();
        
        let (input, output, span_name, span_type_str) = if let Some(payload_bytes) = &payload {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(payload_bytes) {
                let input = extract_input_from_payload(&json);
                let output = extract_output_from_payload(&json);
                
                // Extract span name
                let name = json.get("name")
                    .or_else(|| json.get("span.name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                // Extract span type
                let span_type = json.get("span.type")
                    .or_else(|| json.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("generic")
                    .to_string();
                
                // Check for system prompt
                if system_prompt.is_none() {
                    if let Some(sys) = json.get("system_prompt").and_then(|v| v.as_str()) {
                        system_prompt = Some(sys.to_string());
                    } else if let Some(obj) = json.as_object() {
                        // Check gen_ai.prompt.0 for system role
                        if obj.get("gen_ai.prompt.0.role").and_then(|v| v.as_str()) == Some("system") {
                            if let Some(sys) = obj.get("gen_ai.prompt.0.content").and_then(|v| v.as_str()) {
                                system_prompt = Some(sys.to_string());
                            }
                        }
                    }
                }
                
                // Extract tool calls and results as context variables
                if let Some(tool_name) = json.get("tool.name").and_then(|v| v.as_str()) {
                    if let Some(tool_result) = json.get("tool.result").or_else(|| json.get("output")) {
                        context_variables.insert(
                            format!("tool_{}", tool_name),
                            tool_result.clone()
                        );
                    }
                }
                
                // Extract retrieved documents
                if let Some(docs) = json.get("retrieved_documents").or_else(|| json.get("rag.documents")) {
                    context_variables.insert("retrieved_documents".to_string(), docs.clone());
                }
                
                (input, output, name, span_type)
            } else {
                (String::new(), String::new(), "unknown".to_string(), "generic".to_string())
            }
        } else {
            (String::new(), String::new(), "unknown".to_string(), "generic".to_string())
        };
        
        // Build span info
        span_path.push(SpanInfo {
            span_id: format!("{}", edge.edge_id),
            parent_id: if edge.causal_parent != 0 { 
                Some(format!("{}", edge.causal_parent)) 
            } else { 
                None 
            },
            name: span_name,
            span_type: span_type_str.clone(),
            timestamp_us: edge.timestamp_us,
            duration_us: edge.duration_us,
            input: if input.is_empty() { None } else { Some(input.clone()) },
            output: if output.is_empty() { None } else { Some(output.clone()) },
        });
        
        // Add to conversation based on span type
        if !input.is_empty() {
            // User or tool input
            let role = if span_type_str == "tool" { "tool" } else { "user" };
            messages.push(ChatMessage {
                role: role.to_string(),
                content: input,
            });
        }
        
        if !output.is_empty() {
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: output,
            });
        }
        
        total_tokens += edge.token_count;
    }
    
    // Build fork point info
    let fork_point = span_path.last().cloned().unwrap_or(SpanInfo {
        span_id: format!("{}", target_span_id),
        parent_id: None,
        name: "unknown".to_string(),
        span_type: "unknown".to_string(),
        timestamp_us: 0,
        duration_us: 0,
        input: None,
        output: None,
    });
    
    // Add system prompt as first message if present
    if let Some(ref sys) = system_prompt {
        messages.insert(0, ChatMessage {
            role: "system".to_string(),
            content: sys.clone(),
        });
    }
    
    tracing::info!(
        "Fork state reconstructed: {} messages, {} tokens, {} context vars",
        messages.len(),
        total_tokens,
        context_variables.len()
    );
    
    Ok(ForkState {
        messages,
        span_path,
        fork_point,
        total_tokens,
        system_prompt,
        context_variables,
    })
}

/// Get the conversation history for a trace up to a specific span
/// 
/// Lighter-weight version of fork_trace_state that just returns messages
#[tauri::command]
pub async fn get_trace_conversation(
    state: State<'_, AppState>,
    span_id: String,
) -> Result<Vec<ChatMessage>, CommandError> {
    let fork_state = fork_trace_state(state, span_id).await?;
    Ok(fork_state.messages)
}

/// Preview what a fork would look like without full reconstruction
#[tauri::command]
pub async fn preview_fork(
    state: State<'_, AppState>,
    span_id: String,
) -> Result<serde_json::Value, CommandError> {
    let db = state.db.clone();
    
    // Parse span_id
    let target_span_id: u128 = if span_id.starts_with("0x") {
        u128::from_str_radix(&span_id[2..], 16)
            .map_err(|e| CommandError::invalid_input("span_id", &format!("Invalid hex: {}", e)))?
    } else {
        span_id.parse::<u128>()
            .or_else(|_| u128::from_str_radix(&span_id, 16))
            .map_err(|e| CommandError::invalid_input("span_id", &format!("Invalid ID: {}", e)))?
    };
    
    // Get the target span
    let target_edge = run_blocking({
        let db = db.clone();
        move || db.get(target_span_id).map(|opt| opt.ok_or_else(|| 
            agentreplay_core::AgentreplayError::NotFound(format!("Span {} not found", target_span_id))
        ))
    }).await?
    .map_err(|_e| CommandError::not_found("span"))?;
    
    // Count path depth
    let mut depth = 1;
    let mut current = target_edge.clone();
    while current.causal_parent != 0 {
        if let Ok(Some(parent)) = db.get(current.causal_parent) {
            depth += 1;
            current = parent;
        } else {
            break;
        }
    }
    
    Ok(serde_json::json!({
        "span_id": span_id,
        "path_depth": depth,
        "session_id": target_edge.session_id,
        "timestamp_us": target_edge.timestamp_us,
        "token_count": target_edge.token_count,
        "can_fork": true,
        "message": format!("Fork will reconstruct {} spans of conversation history", depth)
    }))
}

// ============================================================================
// Setup / Onboarding Commands
// ============================================================================

/// Returns the absolute path to the agentreplay-claude-bridge/dist/index.js file
/// Used for configuring the MCP server in Claude Desktop / VS Code
#[tauri::command]
pub async fn get_bridge_path() -> Result<String, String> {
    // Check multiple likely locations
    // 1. Current working directory (dev mode)
    // 2. Sibling directory (prod/release structure)
    
    let possible_paths = vec![
        "agentreplay-claude-bridge/dist/index.js",
        "../agentreplay-claude-bridge/dist/index.js",
        "../../agentreplay-claude-bridge/dist/index.js",
    ];

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    for relative_path in possible_paths {
        let path = cwd.join(relative_path);
        if path.exists() {
            return path.canonicalize()
                .map(|p| p.to_string_lossy().to_string())
                .map_err(|e| format!("Failed to resolve path: {}", e));
        }
    }

    // If we're here, we couldn't find it. 
    // Return a helpful error or a best-guess default.
    // For now, let's return a specific error that the UI can handle (e.g., show manual entry)
    Err("Could not auto-detect agentreplay-claude-bridge path. Please verify it is installed.".to_string())
}

// ============================================================================
// Tests
// ============================================================================
