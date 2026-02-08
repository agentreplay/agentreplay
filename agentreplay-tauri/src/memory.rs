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

//! Memory/RAG API endpoints
//!  
//! Provides semantic memory storage and retrieval using the HNSW vector index.
//! Allows agents to have "long-term memory" without needing a separate vector DB.

use axum::{
    extract::{Json, State as AxumState},
    http::StatusCode,
    response::IntoResponse,
};
use agentreplay_index::embedding::{EmbeddingProvider, LocalEmbeddingProvider};
use agentreplay_index::Embedding;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::server::ServerState;

/// Request to ingest content into memory
#[derive(Debug, Deserialize)]
pub struct IngestMemoryRequest {
    /// Collection name (e.g., "agent_knowledge", "user_docs")
    pub collection: String,
    /// Content to store
    pub content: String,
    /// Optional metadata (source, tags, etc.)
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Response from memory ingestion
#[derive(Debug, Serialize)]
pub struct IngestMemoryResponse {
    pub id: String,
    /// Alias for plugin compatibility (same as `id`)
    pub document_id: String,
    pub collection: String,
    pub status: String,
    /// Plugin compatibility: true when status == "stored"
    pub success: bool,
    /// Plugin compatibility: always 1 for single-chunk ingestion
    pub chunks_created: usize,
}

/// Request to retrieve from memory
#[derive(Debug, Deserialize)]
pub struct RetrieveMemoryRequest {
    /// Collection to search in (empty or missing = search all collections)
    #[serde(default)]
    pub collection: String,
    /// Query text
    pub query: String,
    /// Number of results to return
    #[serde(default = "default_k")]
    pub k: usize,
    /// Alias for k (plugin compatibility) - if set, overrides k
    #[serde(default)]
    pub limit: Option<usize>,
    /// Minimum similarity score (plugin compatibility, currently unused)
    #[serde(default)]
    pub min_score: Option<f32>,
}

fn default_k() -> usize {
    5
}

/// Single memory retrieval result
#[derive(Debug, Serialize)]
pub struct MemoryResult {
    pub id: String,
    pub collection: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub score: f32,
    pub timestamp: u64,
}

/// Response from memory retrieval
#[derive(Debug, Serialize)]
pub struct RetrieveMemoryResponse {
    pub results: Vec<MemoryResult>,
    pub query: String,
    pub collection: String,
    /// Total number of results returned (plugin compatibility)
    pub total_results: usize,
}


/// Error response for memory operations
#[derive(Debug, Serialize)]
pub struct MemoryError {
    pub error: String,
    pub code: String,
}

/// Helper type for storing memory payloads
#[derive(Debug, Serialize, Deserialize)]
struct MemoryPayload {
    collection: String,
    content: String,
    metadata: HashMap<String, String>,
    /// Epoch seconds when this memory was created
    #[serde(default)]
    created_at: f64,
}

/// Request for listing memories (GET with query params)
#[derive(Debug, Deserialize)]
pub struct ListMemoryQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: usize,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: usize,
    /// Optional collection filter
    #[serde(default)]
    pub collection: Option<String>,
}

fn default_page() -> usize { 1 }
fn default_per_page() -> usize { 10 }

/// A memory item in the list response
#[derive(Debug, Serialize)]
pub struct MemoryListItem {
    pub id: String,
    pub collection: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub created_at: f64,
}

/// Response for memory listing
#[derive(Debug, Serialize)]
pub struct ListMemoryResponse {
    pub memories: Vec<MemoryListItem>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

/// Maximum content length for ingestion
const MAX_CONTENT_LENGTH: usize = 100_000;
/// Maximum query length for retrieval
const MAX_QUERY_LENGTH: usize = 2000;
/// Maximum k for retrieval
const MAX_K: usize = 100;

/// POST /api/v1/memory/ingest - Store content in memory
///
/// This endpoint:
/// 1. Accepts text content + metadata
/// 2. Generates embedding using local embedding provider
/// 3. Stores in HNSW vector index
/// 4. Stores raw content in LSM tree
pub async fn ingest_memory(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<IngestMemoryRequest>,
) -> impl IntoResponse {
    // Input validation
    if req.content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(MemoryError {
                error: "Content cannot be empty".to_string(),
                code: "INVALID_CONTENT".to_string(),
            }),
        )
            .into_response();
    }

    if req.content.len() > MAX_CONTENT_LENGTH {
        return (
            StatusCode::BAD_REQUEST,
            Json(MemoryError {
                error: format!(
                    "Content too long: {} characters (max {})",
                    req.content.len(),
                    MAX_CONTENT_LENGTH
                ),
                code: "CONTENT_TOO_LONG".to_string(),
            }),
        )
            .into_response();
    }

    if req.collection.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(MemoryError {
                error: "Collection name cannot be empty".to_string(),
                code: "INVALID_COLLECTION".to_string(),
            }),
        )
            .into_response();
    }

    tracing::info!(
        "Memory ingest request: collection='{}', content_len={}, metadata={:?}",
        req.collection,
        req.content.len(),
        req.metadata
    );

    // Initialize embedding provider
    let provider = match LocalEmbeddingProvider::default_provider() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to initialize embedding provider: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MemoryError {
                    error: format!("Embedding provider initialization failed: {}", e),
                    code: "EMBEDDING_INIT_FAILED".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Generate embedding
    let embedding = match provider.embed(&req.content) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to generate embedding: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MemoryError {
                    error: format!("Embedding generation failed: {}", e),
                    code: "EMBEDDING_FAILED".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Generate unique ID
    let id = agentreplay_core::edge::AgentFlowEdge::generate_id();
    let id_str = format!("{:#x}", id);

    // Store payload in LSM tree
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let payload = MemoryPayload {
        collection: req.collection.clone(),
        content: req.content,
        metadata: req.metadata,
        created_at: now,
    };

    let payload_bytes = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to serialize payload: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MemoryError {
                    error: "Failed to serialize payload".to_string(),
                    code: "SERIALIZATION_FAILED".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Store in database
    if let Err(e) = state.tauri_state.db.put_payload(id, &payload_bytes) {
        tracing::error!("Failed to store payload: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(MemoryError {
                error: format!("Storage failed: {}", e),
                code: "STORAGE_FAILED".to_string(),
            }),
        )
            .into_response();
    }

    // Store embedding in vector index
    if let Err(e) = state.tauri_state.db.store_embedding(id, &embedding) {
        tracing::error!("Failed to store embedding: {}", e);
        // Clean up the payload we just stored
        let _ = state.tauri_state.db.delete_payload(id);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(MemoryError {
                error: format!("Vector index storage failed: {}", e),
                code: "VECTOR_INDEX_FAILED".to_string(),
            }),
        )
            .into_response();
    }

    // **PERSISTENCE FIX**: Sync vector index to disk after ingest
    // This ensures memory data survives app restarts
    if let Err(e) = state.tauri_state.db.sync_vector_index() {
        tracing::warn!("Failed to sync vector index after ingest: {}", e);
        // Non-fatal: data is in memory and will be saved on graceful shutdown
    }

    Json(IngestMemoryResponse {
        id: id_str.clone(),
        document_id: id_str,
        collection: req.collection,
        status: "stored".to_string(),
        success: true,
        chunks_created: 1,
    })
    .into_response()
}

/// POST /api/v1/memory/retrieve - Retrieve similar content from memory
///
/// This endpoint:
/// 1. Accepts query text
/// 2. Generates embedding for query
/// 3. Searches HNSW index for nearest neighbors
/// 4. Fetches full content from LSM tree
/// 5. Returns top-k results with similarity scores
pub async fn retrieve_memory(
    AxumState(state): AxumState<ServerState>,
    Json(req): Json<RetrieveMemoryRequest>,
) -> impl IntoResponse {
    // Input validation
    if req.query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(MemoryError {
                error: "Query cannot be empty".to_string(),
                code: "INVALID_QUERY".to_string(),
            }),
        )
            .into_response();
    }

    if req.query.len() > MAX_QUERY_LENGTH {
        return (
            StatusCode::BAD_REQUEST,
            Json(MemoryError {
                error: format!(
                    "Query too long: {} characters (max {})",
                    req.query.len(),
                    MAX_QUERY_LENGTH
                ),
                code: "QUERY_TOO_LONG".to_string(),
            }),
        )
            .into_response();
    }

    // Plugin sends `limit`, direct API sends `k` — use limit if provided
    let k = req.limit.unwrap_or(req.k).min(MAX_K);

    tracing::info!(
        "Memory retrieval request: collection='{}', query='{}', k={}",
        req.collection,
        req.query,
        k
    );

    // Initialize embedding provider
    let provider = match LocalEmbeddingProvider::default_provider() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to initialize embedding provider: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MemoryError {
                    error: format!("Embedding provider initialization failed: {}", e),
                    code: "EMBEDDING_INIT_FAILED".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Generate query embedding and convert to ndarray
    let query_vec = match provider.embed(&req.query) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to generate query embedding: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MemoryError {
                    error: format!("Embedding generation failed: {}", e),
                    code: "EMBEDDING_FAILED".to_string(),
                }),
            )
                .into_response();
        }
    };
    let query_embedding = Embedding::from_vec(query_vec);

    // Search vector index
    let neighbors = match state.tauri_state.db.search_vectors(&query_embedding, k) {
        Ok(results) => {
            tracing::info!("Vector search found {} neighbors", results.len());
            results
        },
        Err(e) => {
            tracing::error!("Vector search failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MemoryError {
                    error: format!("Search failed: {}", e),
                    code: "SEARCH_FAILED".to_string(),
                }),
            )
            .into_response();
        }
    };

    // Fetch payloads and filter by collection
    let mut results: Vec<MemoryResult> = Vec::new();
    for (id, score) in neighbors {
        tracing::debug!("Checking neighbor {:#x} (score: {:.4})", id, score);
        
        if let Ok(Some(payload_bytes)) = state.tauri_state.db.get_payload(id) {
            if let Ok(payload) = serde_json::from_slice::<MemoryPayload>(&payload_bytes) {
                tracing::debug!("Found payload for {:#x}: collection='{}'", id, payload.collection);
                // Filter by collection if specified
                if !req.collection.is_empty() && payload.collection != req.collection {
                    tracing::debug!("Skipping {:#x}: collection mismatch ('{}' != '{}')", id, payload.collection, req.collection);
                    continue;
                }

                // Apply min_score filter: score is distance (lower = more similar)
                // min_score is a threshold — skip results with distance > (1.0 - min_score)
                if let Some(min_score) = req.min_score {
                    if min_score > 0.0 && score > (1.5 - min_score) {
                        continue;
                    }
                }

                results.push(MemoryResult {
                    id: format!("{:#x}", id),
                    collection: payload.collection,
                    content: payload.content,
                    metadata: payload.metadata,
                    score,
                    timestamp: payload.created_at as u64,
                });
            } else {
                tracing::warn!("Failed to deserialize payload for {:#x}", id);
            }
        } else {
            tracing::warn!("Payload not found for neighbor {:#x}", id);
        }
    }

    let total_results = results.len();
    Json(RetrieveMemoryResponse {
        results,
        query: req.query,
        collection: req.collection,
        total_results,
    })
    .into_response()
}

/// MCP Tenant and Project IDs for isolation
const MCP_TENANT_ID: i64 = 2;
const MCP_PROJECT_ID: i64 = 1000;

/// Response for MCP project info
#[derive(Debug, Serialize)]
pub struct MCPInfoResponse {
    pub project: MCPProjectInfo,
    pub collections: Vec<MCPCollection>,
    pub status: MCPStatus,
}

/// MCP project info
#[derive(Debug, Serialize)]
pub struct MCPProjectInfo {
    pub project_id: i64,
    pub project_name: String,
    pub tenant_id: i64,
    pub description: String,
    pub created_at: f64,
    pub vector_count: usize,
    pub collection_count: usize,
    pub last_activity: Option<f64>,
    pub storage_path: String,
}

/// MCP collection info
#[derive(Debug, Serialize)]
pub struct MCPCollection {
    pub name: String,
    pub document_count: usize,
    pub vector_count: usize,
    pub embedding_dimension: usize,
    pub created_at: f64,
    pub last_updated: f64,
}

/// MCP status
#[derive(Debug, Serialize)]
pub struct MCPStatus {
    pub initialized: bool,
    pub server_running: bool,
    pub tenant_id: i64,
    pub project_id: i64,
    pub isolation_mode: String,
}

/// GET /api/v1/memory/info - Get MCP project info and status
pub async fn get_memory_info(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    // MCP Memory has its own isolated storage - not shared with observability traces
    // TODO: Implement actual memory-specific vector count from MCP storage
    // Get stats from DB
    let stats = state.tauri_state.db.stats();
    let memory_vector_count = stats.vector_count;

    
    // Current timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    
    let response = MCPInfoResponse {
        project: MCPProjectInfo {
            project_id: MCP_PROJECT_ID,
            project_name: "MCP Memory".to_string(),
            tenant_id: MCP_TENANT_ID,
            description: "Dedicated project for MCP vector storage and memory operations".to_string(),
            created_at: now,
            vector_count: memory_vector_count,
            collection_count: 1,
            last_activity: if memory_vector_count > 0 { Some(now) } else { None },
            storage_path: format!("project_{}", MCP_PROJECT_ID),
        },
        collections: vec![MCPCollection {
            name: "default".to_string(),
            document_count: 0,
            vector_count: memory_vector_count,
            embedding_dimension: 384,
            created_at: now,
            last_updated: now,
        }],
        status: MCPStatus {
            initialized: true,
            server_running: true,
            tenant_id: MCP_TENANT_ID,
            project_id: MCP_PROJECT_ID,
            isolation_mode: "tenant_project".to_string(),
        },
    };
    
    Json(response).into_response()
}

/// GET /api/v1/memory/health - MCP health check endpoint
pub async fn mcp_health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "protocol_version": "2024-11-05",
        "server_name": "agentreplay-mcp",
        "server_version": env!("CARGO_PKG_VERSION"),
        "tenant_id": MCP_TENANT_ID,
        "project_id": MCP_PROJECT_ID,
        "capabilities": {
            "resources": true,
            "tools": true,
            "prompts": true,
            "memory": true
        }
    })).into_response()
}

/// JSON-RPC request for MCP
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// POST /api/v1/mcp - Handle MCP JSON-RPC requests (including ping)
pub async fn handle_mcp_jsonrpc(
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    // Per MCP protocol, ping returns empty object
    match request.method.as_str() {
        "ping" => {
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.id,
                "result": {}
            })).into_response()
        }
        "initialize" => {
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "resources": {},
                        "tools": {},
                        "prompts": {}
                    },
                    "serverInfo": {
                        "name": "agentreplay-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })).into_response()
        }
        _ => {
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", request.method)
                }
            })).into_response()
        }
    }
}

/// GET /api/v1/memory/collections - List all collections
pub async fn list_collections(
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    // MCP Memory has its own isolated storage
    // Get stats from DB
    let stats = state.tauri_state.db.stats();
    let memory_vector_count = stats.vector_count;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    
    Json(serde_json::json!({
        "collections": [{
            "name": "default",
            "document_count": 0,
            "vector_count": memory_vector_count,
            "embedding_dimension": 384,
            "created_at": now,
            "last_updated": now
        }]
    })).into_response()
}

/// GET /api/v1/memory/list - List all memories with pagination (newest first)
///
/// Unlike `/retrieve` which requires a search query, this endpoint lists
/// all stored memories in reverse chronological order with pagination.
/// Used by the Memory UI for browsing without needing a search query.
pub async fn list_memories(
    AxumState(state): AxumState<ServerState>,
    axum::extract::Query(params): axum::extract::Query<ListMemoryQuery>,
) -> impl IntoResponse {
    let page = params.page.max(1);
    let per_page = params.per_page.min(100).max(1);

    tracing::info!(
        "Memory list request: page={}, per_page={}, collection={:?}",
        page, per_page, params.collection
    );

    // Get all vector IDs from the index
    let all_ids = state.tauri_state.db.list_all_vector_ids();

    // Fetch all payloads and optionally filter by collection
    let mut all_memories: Vec<(u128, MemoryPayload)> = Vec::new();
    for id in all_ids {
        if let Ok(Some(payload_bytes)) = state.tauri_state.db.get_payload(id) {
            if let Ok(payload) = serde_json::from_slice::<MemoryPayload>(&payload_bytes) {
                // Filter by collection if specified
                if let Some(ref col) = params.collection {
                    if !col.is_empty() && payload.collection != *col {
                        continue;
                    }
                }
                all_memories.push((id, payload));
            }
        }
    }

    // Sort by created_at descending (newest first)
    all_memories.sort_by(|a, b| b.1.created_at.partial_cmp(&a.1.created_at).unwrap_or(std::cmp::Ordering::Equal));

    let total = all_memories.len();
    let total_pages = if total == 0 { 1 } else { (total + per_page - 1) / per_page };

    // Paginate
    let start = (page - 1) * per_page;
    let memories: Vec<MemoryListItem> = all_memories
        .into_iter()
        .skip(start)
        .take(per_page)
        .map(|(id, payload)| MemoryListItem {
            id: format!("{:#x}", id),
            collection: payload.collection,
            content: payload.content,
            metadata: payload.metadata,
            created_at: payload.created_at,
        })
        .collect();

    Json(ListMemoryResponse {
        memories,
        total,
        page,
        per_page,
        total_pages,
    }).into_response()
}
