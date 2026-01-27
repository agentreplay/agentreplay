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

use crate::agent_registry::AgentMetadata;
use crate::api::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

/// Request body for agent registration
#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub agent_id: u64,
    pub name: String,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// Response for successful agent registration
#[derive(Debug, Serialize)]
pub struct RegisterAgentResponse {
    pub success: bool,
    pub agent: AgentMetadata,
}

/// Response for listing agents
#[derive(Debug, Serialize)]
pub struct ListAgentsResponse {
    pub agents: Vec<AgentMetadata>,
    pub total: usize,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

/// POST /api/v1/agents/register
///
/// Register a new agent or update existing agent metadata.
///
/// # Edge Cases Handled:
/// - Empty name: Returns 400 error
/// - Duplicate agent_id: Updates existing entry
/// - Invalid JSON: Returns 400 error
/// - Lock contention: Handled by RwLock in registry
pub async fn register_agent(
    State(state): State<AppState>,
    Json(req): Json<RegisterAgentRequest>,
) -> Result<Json<RegisterAgentResponse>, ErrorResponse> {
    let registry = &state.agent_registry;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let metadata = AgentMetadata {
        agent_id: req.agent_id,
        name: req.name,
        namespace: req.namespace,
        version: req.version,
        description: req.description,
        created_at: now,
        updated_at: now,
        metadata: req.metadata,
    };

    match registry.register(metadata) {
        Ok(agent) => Ok(Json(RegisterAgentResponse {
            success: true,
            agent,
        })),
        Err(e) => Err(ErrorResponse { error: e }),
    }
}

/// GET /api/v1/agents
///
/// List all registered agents.
///
/// Returns empty array if no agents registered.
pub async fn list_agents(
    State(state): State<AppState>,
) -> Result<Json<ListAgentsResponse>, ErrorResponse> {
    let registry = &state.agent_registry;
    match registry.list() {
        Ok(mut agents) => {
            // Sort by agent_id for consistent ordering
            agents.sort_by_key(|a| a.agent_id);

            let total = agents.len();
            Ok(Json(ListAgentsResponse { agents, total }))
        }
        Err(e) => Err(ErrorResponse { error: e }),
    }
}

/// GET /api/v1/agents/:agent_id
///
/// Get metadata for a specific agent.
///
/// # Edge Cases:
/// - Agent not found: Returns 404
/// - Invalid agent_id format: Returns 400
pub async fn get_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
) -> Result<Json<AgentMetadata>, StatusCode> {
    let registry = &state.agent_registry;
    registry
        .get(agent_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// PUT /api/v1/agents/:agent_id
///
/// Update agent metadata.
///
/// # Edge Cases:
/// - Agent not found: Creates new registration
/// - Partial update: Only provided fields are updated
pub async fn update_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Json(req): Json<UpdateAgentRequest>,
) -> Result<Json<RegisterAgentResponse>, ErrorResponse> {
    let registry = &state.agent_registry;
    // Get existing metadata or create default
    let mut metadata = registry.get(agent_id).unwrap_or_else(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        AgentMetadata {
            agent_id,
            name: format!("agent_{}", agent_id),
            namespace: None,
            version: None,
            description: None,
            created_at: now,
            updated_at: now,
            metadata: std::collections::HashMap::new(),
        }
    });

    // Update fields if provided
    if let Some(name) = req.name {
        metadata.name = name;
    }
    if let Some(namespace) = req.namespace {
        metadata.namespace = Some(namespace);
    }
    if let Some(version) = req.version {
        metadata.version = Some(version);
    }
    if let Some(description) = req.description {
        metadata.description = Some(description);
    }
    if let Some(new_metadata) = req.metadata {
        metadata.metadata.extend(new_metadata);
    }

    // Update timestamp
    metadata.updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match registry.register(metadata) {
        Ok(agent) => Ok(Json(RegisterAgentResponse {
            success: true,
            agent,
        })),
        Err(e) => Err(ErrorResponse { error: e }),
    }
}

/// Request body for updating agent metadata
#[derive(Debug, Deserialize)]
pub struct UpdateAgentRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// DELETE /api/v1/agents/:agent_id
///
/// Delete an agent registration.
///
/// # Note:
/// This doesn't delete traces from that agent, just the metadata.
/// Traces will show fallback name after deletion.
pub async fn delete_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
) -> Result<StatusCode, ErrorResponse> {
    let registry = &state.agent_registry;
    match registry.delete(agent_id) {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Ok(StatusCode::NOT_FOUND),
        Err(e) => Err(ErrorResponse { error: e }),
    }
}
