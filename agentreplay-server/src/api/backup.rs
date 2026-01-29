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

use super::query::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use agentreplay_storage::{BackupManager, BackupMetadata};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateBackupRequest {
    /// Destination path for the backup
    pub destination: String,

    /// Optional backup name (defaults to timestamp-based name)
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RestoreBackupRequest {
    /// Path to the backup to restore from
    pub backup_path: String,
}

#[derive(Debug, Deserialize)]
pub struct ListBackupsQuery {
    /// Directory containing backups
    pub location: String,
}

#[derive(Debug, Serialize)]
pub struct BackupResponse {
    pub timestamp_us: u64,
    pub created_at: String,
    pub size_bytes: u64,
    pub file_count: usize,
    pub database_version: String,
    pub backup_path: String,
}

#[derive(Debug, Serialize)]
pub struct BackupListResponse {
    pub backups: Vec<BackupResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct BackupOperationResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BackupMetadata>,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn metadata_to_response(metadata: &BackupMetadata, backup_path: String) -> BackupResponse {
    BackupResponse {
        timestamp_us: metadata.timestamp_us,
        created_at: metadata.created_at.clone(),
        size_bytes: metadata.size_bytes,
        file_count: metadata.file_count,
        database_version: metadata.database_version.clone(),
        backup_path,
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/v1/backup
/// Create a backup of the database
pub async fn create_backup(
    State(state): State<AppState>,
    Json(req): Json<CreateBackupRequest>,
) -> Result<(StatusCode, Json<BackupOperationResponse>), (StatusCode, String)> {
    // Get database path from config
    let db_path = state.db_path.clone();

    let backup_manager = BackupManager::new(&db_path);

    let destination = PathBuf::from(&req.destination);

    // If name provided, append it to destination
    let final_destination = if let Some(name) = req.name {
        destination.join(name)
    } else {
        destination
    };

    match backup_manager.create_backup(&final_destination) {
        Ok(metadata) => Ok((
            StatusCode::CREATED,
            Json(BackupOperationResponse {
                success: true,
                message: format!(
                    "Backup created successfully at {}",
                    final_destination.display()
                ),
                metadata: Some(metadata),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create backup: {}", e),
        )),
    }
}

/// POST /api/v1/backup/restore
/// Restore database from a backup
///
/// **WARNING**: This operation requires the server to be restarted after restore.
pub async fn restore_backup(
    State(state): State<AppState>,
    Json(req): Json<RestoreBackupRequest>,
) -> Result<Json<BackupOperationResponse>, (StatusCode, String)> {
    let db_path = state.db_path.clone();

    let backup_manager = BackupManager::new(&db_path);

    match backup_manager.restore_backup(&req.backup_path) {
        Ok(()) => Ok(Json(BackupOperationResponse {
            success: true,
            message:
                "Backup restored successfully. Please restart the server to load the restored data."
                    .to_string(),
            metadata: None,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to restore backup: {}", e),
        )),
    }
}

/// GET /api/v1/backup
/// List available backups in a directory
pub async fn list_backups(
    Query(params): Query<ListBackupsQuery>,
) -> Result<Json<BackupListResponse>, (StatusCode, String)> {
    match BackupManager::list_backups(&params.location) {
        Ok(backups) => {
            let total = backups.len();
            let backup_responses: Vec<BackupResponse> = backups
                .iter()
                .enumerate()
                .map(|(i, metadata)| {
                    let backup_path = PathBuf::from(&params.location)
                        .join(format!("backup-{}", i))
                        .display()
                        .to_string();
                    metadata_to_response(metadata, backup_path)
                })
                .collect();

            Ok(Json(BackupListResponse {
                backups: backup_responses,
                total,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list backups: {}", e),
        )),
    }
}

/// GET /api/v1/backup/verify
/// Verify the integrity of a backup
pub async fn verify_backup(
    Query(params): Query<RestoreBackupRequest>,
) -> Result<Json<BackupOperationResponse>, (StatusCode, String)> {
    match BackupManager::verify_backup(&params.backup_path) {
        Ok(valid) => {
            if valid {
                Ok(Json(BackupOperationResponse {
                    success: true,
                    message: "Backup is valid and intact".to_string(),
                    metadata: None,
                }))
            } else {
                Ok(Json(BackupOperationResponse {
                    success: false,
                    message: "Backup verification failed: missing or corrupted files".to_string(),
                    metadata: None,
                }))
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to verify backup: {}", e),
        )),
    }
}
