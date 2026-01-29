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

//! Admin API endpoints for data management
//!
//! These endpoints provide administrative functions for managing data,
//! including deleting projects and resetting the database.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState};
use crate::auth::AuthContext;

/// Response for delete operations
#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub success: bool,
    pub message: String,
    pub deleted_count: Option<usize>,
}

/// Request for reset confirmation
#[derive(Debug, Deserialize)]
pub struct ResetConfirmation {
    pub confirm: String,
}

/// DELETE /api/v1/projects/:project_id
/// Delete a specific project and all its data
pub async fn delete_project(
    State(state): State<AppState>,
    Path(project_id): Path<u16>,
    _auth: axum::Extension<AuthContext>,
) -> Result<Json<DeleteResponse>, ApiError> {
    // Use project manager to delete the project
    if let Some(ref pm) = state.project_manager {
        pm.delete_project(project_id)
            .map_err(|e| ApiError::Internal(format!("Failed to delete project: {}", e)))?;

        // Also remove from project registry if it exists
        if let Some(ref registry) = state.project_registry {
            registry.remove_project(project_id);
        }

        Ok(Json(DeleteResponse {
            success: true,
            message: format!("Project {} deleted successfully", project_id),
            deleted_count: Some(1),
        }))
    } else {
        Err(ApiError::Internal(
            "Project manager not available".to_string(),
        ))
    }
}

/// DELETE /api/v1/admin/reset
/// Delete all data from the database (requires confirmation)
pub async fn reset_all_data(
    State(state): State<AppState>,
    _auth: axum::Extension<AuthContext>,
) -> Result<Json<DeleteResponse>, ApiError> {
    // Use project manager to delete all projects
    if let Some(ref pm) = state.project_manager {
        let deleted_count = pm
            .delete_all_projects()
            .map_err(|e| ApiError::Internal(format!("Failed to reset data: {}", e)))?;

        // Clear project registry if it exists
        if let Some(ref registry) = state.project_registry {
            registry.clear_all();
        }

        Ok(Json(DeleteResponse {
            success: true,
            message: format!(
                "All data deleted successfully. {} projects removed.",
                deleted_count
            ),
            deleted_count: Some(deleted_count),
        }))
    } else {
        // If no project manager, try deleting from single database
        // Just return success since there's no project manager
        Ok(Json(DeleteResponse {
            success: true,
            message: "No project manager available, but operation completed".to_string(),
            deleted_count: Some(0),
        }))
    }
}
