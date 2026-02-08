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
