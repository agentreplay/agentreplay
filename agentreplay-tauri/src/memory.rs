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
    pub collection: String,
    pub status: String,
}

/// Request to retrieve from memory
#[derive(Debug, Deserialize)]
pub struct RetrieveMemoryRequest {
    /// Collection to search in
    pub collection: String,
    /// Query text
    pub query: String,
    /// Number of results to return
    #[serde(default = "default_k")]
    pub k: usize,
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
    let payload = MemoryPayload {
        collection: req.collection.clone(),
        content: req.content,
        metadata: req.metadata,
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
        id: id_str,
        collection: req.collection,
        status: "stored".to_string(),
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

    let k = req.k.min(MAX_K);

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

                results.push(MemoryResult {
                    id: format!("{:#x}", id),
                    collection: payload.collection,
                    content: payload.content,
                    metadata: payload.metadata,
                    score: score,
                    timestamp: 0, // Timestamp not tracked in payload
                });
            } else {
                tracing::warn!("Failed to deserialize payload for {:#x}", id);
            }
        } else {
            tracing::warn!("Payload not found for neighbor {:#x}", id);
        }
    }

    Json(RetrieveMemoryResponse {
        results,
        query: req.query,
        collection: req.collection,
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
