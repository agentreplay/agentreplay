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

//! Memory & RAG API Endpoints
//!
//! API endpoints for the Memory & RAG page in the UI.
//! These endpoints expose MCP project information and collection management.

use crate::api::AppState;
use crate::mcp::{
    MCPCollection, MCPContext, MCPProjectInfo, MCP_DEFAULT_PROJECT_ID, MCP_TENANT_ID,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use agentreplay_core::{AgentFlowEdge, SpanType};
use agentreplay_index::embedding::{LocalEmbeddingProvider, EmbeddingProvider};
use agentreplay_index::Embedding;

/// Response for MCP project info
#[derive(Debug, Serialize)]
pub struct MCPInfoResponse {
    pub project: MCPProjectInfo,
    pub collections: Vec<MCPCollection>,
    pub status: MCPStatus,
}

/// MCP system status
#[derive(Debug, Serialize)]
pub struct MCPStatus {
    pub initialized: bool,
    pub server_running: bool,
    pub tenant_id: u64,
    pub project_id: u16,
    pub isolation_mode: String,
}

/// Memory stats response
#[derive(Debug, Serialize)]
pub struct MemoryStatsResponse {
    pub total_vectors: usize,
    pub total_documents: usize,
    pub total_collections: usize,
    pub embedding_dimension: usize,
    pub storage_size_bytes: u64,
    pub index_type: String,
}

/// Create a new collection request
#[derive(Debug, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub description: Option<String>,
    pub embedding_dimension: Option<usize>,
}

/// Ingest document request
#[derive(Debug, Deserialize)]
pub struct IngestDocumentRequest {
    pub collection: Option<String>,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
}

/// Ingest response
#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub success: bool,
    pub document_id: String,
    pub chunks_created: usize,
    pub vectors_stored: usize,
}

/// Retrieve/search request
#[derive(Debug, Deserialize)]
pub struct RetrieveRequest {
    pub query: String,
    pub collection: Option<String>,
    pub limit: Option<usize>,
    pub min_score: Option<f32>,
}

/// Retrieve result
#[derive(Debug, Serialize)]
pub struct RetrieveResult {
    pub content: String,
    pub score: f32,
    pub metadata: Option<serde_json::Value>,
    pub document_id: String,
    pub chunk_index: usize,
}

/// Retrieve response
#[derive(Debug, Serialize)]
pub struct RetrieveResponse {
    pub results: Vec<RetrieveResult>,
    pub query: String,
    pub collection: String,
    pub total_results: usize,
}

/// Create the memory API router
pub fn memory_router() -> Router<AppState> {
    Router::new()
        .route("/info", get(get_memory_info))
        .route("/stats", get(get_memory_stats))
        .route("/collections", get(list_collections))
        .route("/collections", post(create_collection))
        .route("/ingest", post(ingest_document))
        .route("/retrieve", post(retrieve_documents))
}

/// Get MCP project info and status
async fn get_memory_info(State(state): State<AppState>) -> impl IntoResponse {
    // Check if project manager and registry are available
    let (project_manager, project_registry) = match (
        state.project_manager.as_ref(),
        state.project_registry.as_ref(),
    ) {
        (Some(pm), Some(pr)) => (pm.clone(), pr.clone()),
        _ => {
            // Return minimal info if not fully initialized
            let response = MCPInfoResponse {
                project: MCPProjectInfo {
                    project_id: MCP_DEFAULT_PROJECT_ID,
                    project_name: "MCP Memory".to_string(),
                    tenant_id: MCP_TENANT_ID,
                    description: "MCP Memory Project - Initializing...".to_string(),
                    created_at: 0,
                    vector_count: 0,
                    collection_count: 0,
                    last_activity: None,
                    storage_path: format!("project_{}", MCP_DEFAULT_PROJECT_ID),
                },
                collections: vec![],
                status: MCPStatus {
                    initialized: false,
                    server_running: false,
                    tenant_id: MCP_TENANT_ID,
                    project_id: MCP_DEFAULT_PROJECT_ID,
                    isolation_mode: "tenant_project".to_string(),
                },
            };
            return (StatusCode::OK, Json(response));
        }
    };

    // Create or get MCP context
    match MCPContext::new(project_manager, project_registry) {
        Ok(ctx) => {
            let project_info = ctx.get_project_info();
            let collections = ctx.list_collections();

            let response = MCPInfoResponse {
                project: project_info,
                collections,
                status: MCPStatus {
                    initialized: true,
                    server_running: true,
                    tenant_id: MCP_TENANT_ID,
                    project_id: MCP_DEFAULT_PROJECT_ID,
                    isolation_mode: "tenant_project".to_string(),
                },
            };

            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            let response = MCPInfoResponse {
                project: MCPProjectInfo {
                    project_id: MCP_DEFAULT_PROJECT_ID,
                    project_name: "MCP Memory".to_string(),
                    tenant_id: MCP_TENANT_ID,
                    description: format!("Error: {}", e),
                    created_at: 0,
                    vector_count: 0,
                    collection_count: 0,
                    last_activity: None,
                    storage_path: format!("project_{}", MCP_DEFAULT_PROJECT_ID),
                },
                collections: vec![],
                status: MCPStatus {
                    initialized: false,
                    server_running: false,
                    tenant_id: MCP_TENANT_ID,
                    project_id: MCP_DEFAULT_PROJECT_ID,
                    isolation_mode: "tenant_project".to_string(),
                },
            };
            (StatusCode::OK, Json(response))
        }
    }
}

/// Get memory statistics
async fn get_memory_stats(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.db.stats();

    let response = MemoryStatsResponse {
        total_vectors: stats.vector_count,
        total_documents: stats.causal_nodes, // Nodes are documents
        total_collections: 1,                // Default collection
        embedding_dimension: 384,            // all-MiniLM-L6-v2
        storage_size_bytes: stats.storage.total_size_bytes(),
        index_type: "HNSW".to_string(),
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// List all collections
async fn list_collections(State(state): State<AppState>) -> impl IntoResponse {
    let (project_manager, project_registry) = match (
        state.project_manager.as_ref(),
        state.project_registry.as_ref(),
    ) {
        (Some(pm), Some(pr)) => (pm.clone(), pr.clone()),
        _ => {
            return (StatusCode::OK, Json(Vec::<MCPCollection>::new()));
        }
    };

    match MCPContext::new(project_manager, project_registry) {
        Ok(ctx) => {
            let collections = ctx.list_collections();
            (StatusCode::OK, Json(collections))
        }
        Err(_) => (StatusCode::OK, Json(Vec::<MCPCollection>::new())),
    }
}

/// Create a new collection (placeholder - collections are implicit for now)
async fn create_collection(
    State(_state): State<AppState>,
    Json(request): Json<CreateCollectionRequest>,
) -> impl IntoResponse {
    info!("Create collection request: {:?}", request.name);

    // For now, collections are implicit - return success
    let collection = MCPCollection {
        name: request.name,
        document_count: 0,
        vector_count: 0,
        embedding_dimension: request.embedding_dimension.unwrap_or(384),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        last_updated: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    (StatusCode::CREATED, Json(collection))
}

/// Ingest a document (placeholder)
/// Ingest a document
async fn ingest_document(
    State(state): State<AppState>,
    Json(request): Json<IngestDocumentRequest>,
) -> impl IntoResponse {
    info!("Ingest document request: {} bytes", request.content.len());

    // 1. Generate embedding
    // In production, this should be done in a background task or batched
    let provider = match LocalEmbeddingProvider::default_provider() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IngestResponse {
                    success: false,
                    document_id: "".to_string(),
                    chunks_created: 0,
                    vectors_stored: 0,
                }),
            );
        }
    };

    let embedding_vec = match provider.embed(&request.content) {
        Ok(vec) => vec,
        Err(e) => {
            error!("Embedding generation failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IngestResponse {
                    success: false,
                    document_id: "".to_string(),
                    chunks_created: 0,
                    vectors_stored: 0,
                }),
            );
        }
    };
    
    let embedding = Embedding::from_vec(embedding_vec);

    // 2. Create AgentFlowEdge
    // For documents, we use a specific span type or conventions
    // Here we'll treat them as independent nodes
    let mut edge = AgentFlowEdge::new(
        MCP_TENANT_ID, 
        MCP_DEFAULT_PROJECT_ID as u16, 
        0, // agent_id
        0, // session_id
        SpanType::ToolCall, // Using ToolCall as a generic type for now, or could add a Document type
        0
    );
    
    // Use the timestamp from the edge creation
    let doc_id = format!("{:x}", edge.edge_id);

    // 3. Prepare payload (metadata + content)
    let payload = serde_json::json!({
        "content": request.content,
        "collection": request.collection,
        "metadata": request.metadata
    });
    
    let payload_bytes = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(e) => {
             error!("Payload serialization failed: {}", e);
             return (StatusCode::INTERNAL_SERVER_ERROR, Json(IngestResponse { success: false, document_id: "".to_string(), chunks_created: 0, vectors_stored: 0 }));
        }
    };

    // 4. Insert into DB with vector
    if let Err(e) = state.db.insert_with_vector(edge.clone(), embedding).await {
        error!("DB insert failed: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(IngestResponse { success: false, document_id: "".to_string(), chunks_created: 0, vectors_stored: 0 }));
    }
    
    // 5. Store full payload
    if let Err(e) = state.db.put_payload(edge.edge_id, &payload_bytes) {
         error!("Payload storage failed: {}", e);
         // Try to cleanup? For now just log
    }

    // Success response
    let response = IngestResponse {
        success: true,
        document_id: doc_id,
        chunks_created: 1,
        vectors_stored: 1,
    };

    (StatusCode::OK, Json(response))
}

/// Retrieve documents via semantic search (placeholder)
/// Retrieve documents via semantic search
async fn retrieve_documents(
    State(state): State<AppState>,
    Json(request): Json<RetrieveRequest>,
) -> impl IntoResponse {
    info!("Retrieve request: {}", request.query);

    // 1. Generate query embedding
    let provider = match LocalEmbeddingProvider::default_provider() {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(RetrieveResponse { results: vec![], query: request.query, collection: "".to_string(), total_results: 0 })),
    };

    let query_vec = match provider.embed(&request.query) {
        Ok(vec) => vec,
        Err(e) => {
             error!("Query embedding failed: {}", e);
             return (StatusCode::INTERNAL_SERVER_ERROR, Json(RetrieveResponse { results: vec![], query: request.query, collection: "".to_string(), total_results: 0 }));
        }
    };
    
    let query_embedding = Embedding::from_vec(query_vec);
    let k = request.limit.unwrap_or(10);

    // 2. Perform semantic search
    let edges = match state.db.semantic_search(&query_embedding, k) {
        Ok(e) => e,
        Err(e) => {
            error!("Semantic search failed: {}", e);
             return (StatusCode::INTERNAL_SERVER_ERROR, Json(RetrieveResponse { results: vec![], query: request.query, collection: "".to_string(), total_results: 0 }));
        }
    };

    // 3. Fetch payloads and format results
    let mut results = Vec::new();
    let target_collection = request.collection.clone().unwrap_or_else(|| "default".to_string());

    for edge in edges {
        if let Ok(Some(payload_bytes)) = state.db.get_payload(edge.edge_id) {
            if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
                // Filter by collection if specified and if payload has collection field
                if let Some(col) = payload.get("collection").and_then(|c| c.as_str()) {
                    if request.collection.is_some() && col != target_collection && target_collection != "all" {
                        continue;
                    }
                }
                
                let content = payload.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                let metadata = payload.get("metadata").cloned();
                
                results.push(RetrieveResult {
                    content,
                    score: 0.0, // We accept lack of score for now from `semantic_search` which returns Vec<Edge>
                    metadata,
                    document_id: format!("{:x}", edge.edge_id),
                    chunk_index: 0,
                });
            }
        }
    }

    let response = RetrieveResponse {
        results,
        query: request.query,
        collection: target_collection,
        total_results: 0, // client calculates length
    };

    (StatusCode::OK, Json(response))
}
