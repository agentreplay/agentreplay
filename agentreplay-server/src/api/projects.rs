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

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::api::{ApiError, AppState};
use crate::auth::AuthContext;

/// Project (Collection) metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub project_id: u16,
    pub name: String,
    pub description: Option<String>,
    pub created_at: u64,
    pub trace_count: usize,
    pub favorite: bool,
}

/// Request to create a new project
#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
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

/// Request to update project metadata
#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub favorite: Option<bool>,
}

/// List of projects response
#[derive(Debug, Serialize)]
pub struct ProjectsResponse {
    pub projects: Vec<Project>,
    pub total: usize,
}

/// GET /api/v1/projects
/// List all projects (collections) for the authenticated tenant
pub async fn list_projects(
    State(state): State<AppState>,
    auth: axum::Extension<AuthContext>,
) -> Result<Json<ProjectsResponse>, ApiError> {
    let projects = if let Some(ref registry) = state.project_registry {
        // Use cached project list from registry - FAST!
        let cached_projects = registry.list_projects();

        let mut projects: Vec<Project> = Vec::new();

        for metadata in cached_projects {
            let project_id = metadata.project_id;

            // Try to get cached stats (max age: 30 seconds)
            let trace_count = if let Some(count) = registry.get_cached_stats(project_id, 30) {
                count
            } else {
                // Cache miss - query the project (only if needed)
                let count = if let Some(ref pm) = state.project_manager {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;

                    let start_ts = now.saturating_sub(30 * 24 * 60 * 60 * 1_000_000);

                    let edges = pm
                        .query_project(project_id, auth.tenant_id, start_ts, now)
                        .unwrap_or_default();

                    edges.len()
                } else {
                    0
                };

                // Cache the result for next time
                registry.cache_stats(project_id, count);
                count
            };

            projects.push(Project {
                project_id: metadata.project_id,
                name: metadata.name.clone(),
                description: metadata.description.clone(),
                created_at: metadata.created_at,
                trace_count,
                favorite: metadata.favorite,
            });
        }

        // Sort by project_id
        projects.sort_by_key(|p| p.project_id);
        projects
    } else {
        // Fallback: scan the single database to find unique project_ids
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let start_ts = now.saturating_sub(30 * 24 * 60 * 60 * 1_000_000);

        let edges = state
            .db
            .query_temporal_range_for_tenant(start_ts, now, auth.tenant_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Group by project_id and count traces
        let mut project_stats: HashMap<u16, usize> = HashMap::new();
        for edge in &edges {
            *project_stats.entry(edge.project_id).or_insert(0) += 1;
        }

        // Create project objects
        let mut projects: Vec<Project> = project_stats
            .into_iter()
            .map(|(project_id, trace_count)| Project {
                project_id,
                name: format!("Project {}", project_id),
                description: Some(format!("Collection with {} traces", trace_count)),
                created_at: 0, // TODO: Store actual creation time
                trace_count,
                favorite: project_id == 0, // Default project is favorite
            })
            .collect();

        // Sort by project_id
        projects.sort_by_key(|p| p.project_id);
        projects
    };

    let total = projects.len();

    Ok(Json(ProjectsResponse { projects, total }))
}

/// GET /api/v1/projects/:project_id
/// Get details about a specific project
pub async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<u16>,
    auth: axum::Extension<AuthContext>,
) -> Result<Json<Project>, ApiError> {
    // Get trace count for this project
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    let start_ts = now.saturating_sub(30 * 24 * 60 * 60 * 1_000_000);

    let trace_count = if let Some(ref pm) = state.project_manager {
        // Query project using ProjectManager
        let edges = pm
            .query_project(project_id, auth.tenant_id, start_ts, now)
            .map_err(|e| ApiError::Internal(format!("Failed to query project: {}", e)))?;

        if edges.is_empty() {
            return Err(ApiError::NotFound(format!(
                "Project {} not found or has no traces",
                project_id
            )));
        }

        edges.len()
    } else {
        // Fallback to single database
        let edges = state
            .db
            .query_temporal_range_for_tenant(start_ts, now, auth.tenant_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let count = edges.iter().filter(|e| e.project_id == project_id).count();

        if count == 0 {
            return Err(ApiError::NotFound(format!(
                "Project {} not found or has no traces",
                project_id
            )));
        }

        count
    };

    Ok(Json(Project {
        project_id,
        name: format!("Project {}", project_id),
        description: Some(format!("Collection with {} traces", trace_count)),
        created_at: 0,
        trace_count,
        favorite: project_id == 0,
    }))
}

/// POST /api/v1/projects
/// Create a new project with environment variables for SDK setup
pub async fn create_project(
    State(_state): State<AppState>,
    State(state): State<AppState>,
    auth: axum::Extension<AuthContext>,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<Json<CreateProjectResponse>, ApiError> {
    // Generate new project_id (timestamp-based for now)
    let project_id = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        % 65535) as u16;

    // Register project in registry if available
    if let Some(ref registry) = state.project_registry {
        registry
            .register_project(
                project_id,
                payload.name.clone(),
                payload.description.clone(),
            )
            .map_err(|e| ApiError::Internal(format!("Failed to register project: {}", e)))?;
    }

    let env_vars = EnvVariables {
        agentreplay_url: "http://*********:47100".to_string(),
        tenant_id: auth.tenant_id.to_string(),
        project_id: project_id.to_string(),
    };

    Ok(Json(CreateProjectResponse {
        project_id,
        name: payload.name,
        description: payload.description,
        env_vars,
    }))
}

/// PATCH /api/v1/projects/:project_id
/// Update project metadata (name, description, favorite status)
pub async fn update_project(
    State(_state): State<AppState>,
    Path(project_id): Path<u16>,
    _auth: axum::Extension<AuthContext>,
    Json(payload): Json<UpdateProjectRequest>,
) -> Result<Json<Project>, ApiError> {
    // TODO: Load existing project metadata from registry
    // TODO: Update and persist to registry

    Ok(Json(Project {
        project_id,
        name: payload
            .name
            .unwrap_or_else(|| format!("Project {}", project_id)),
        description: payload.description,
        created_at: 0,
        trace_count: 0,
        favorite: payload.favorite.unwrap_or(false),
    }))
}

/// POST /api/v1/projects/:project_id/favorite
/// Toggle project favorite status
pub async fn toggle_favorite(
    State(_state): State<AppState>,
    Path(_project_id): Path<u16>,
    _auth: axum::Extension<AuthContext>,
) -> Result<StatusCode, ApiError> {
    // TODO: Load existing project metadata from registry
    // TODO: Toggle favorite status and persist

    Ok(StatusCode::NO_CONTENT)
}
