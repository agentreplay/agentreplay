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

//! Git-Style Response Versioning API
//!
//! HTTP endpoints for Git-like operations on LLM response versions.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use agentreplay_storage::response_git::{
    LogEntry, RepositoryError, ResponseRepository, ResponseSnapshot,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

/// Repository state shared across handlers
pub struct GitVersioningState {
    pub repo: RwLock<ResponseRepository>,
}

impl GitVersioningState {
    pub fn new(author_name: &str) -> Self {
        Self {
            repo: RwLock::new(ResponseRepository::new(author_name)),
        }
    }
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// === Request/Response types ===

#[derive(Debug, Deserialize, Serialize)]
pub struct CommitRequest {
    pub prompt: String,
    pub response: String,
    pub message: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CommitResponse {
    pub commit_id: String,
    pub short_id: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct LogQuery {
    #[serde(default)]
    pub max_count: Option<usize>,
    #[serde(default)]
    pub from_ref: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LogResponse {
    pub entries: Vec<LogEntryDto>,
}

#[derive(Debug, Serialize)]
pub struct LogEntryDto {
    pub commit_id: String,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub timestamp: String,
    pub parents: Vec<String>,
}

impl From<LogEntry> for LogEntryDto {
    fn from(entry: LogEntry) -> Self {
        Self {
            commit_id: entry.commit_id.to_string(),
            short_id: entry.commit_id.short(),
            message: entry.message,
            author: entry.author,
            timestamp: entry.timestamp.to_rfc3339(),
            parents: entry.parents.iter().map(|p| p.to_string()).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ShowResponse {
    pub commit_id: String,
    pub snapshot: SnapshotDto,
}

#[derive(Debug, Serialize)]
pub struct SnapshotDto {
    pub prompt: String,
    pub response: String,
    pub model: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl From<ResponseSnapshot> for SnapshotDto {
    fn from(s: ResponseSnapshot) -> Self {
        Self {
            prompt: s.prompt,
            response: s.response,
            model: s.model,
            metadata: s.metadata,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
    #[serde(default)]
    pub from_ref: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BranchResponse {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct BranchListResponse {
    pub branches: Vec<BranchResponse>,
    pub current: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub commit_id: String,
    pub branch: Option<String>,
    pub detached: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TagResponse {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct TagListResponse {
    pub tags: Vec<TagResponse>,
}

#[derive(Debug, Deserialize)]
pub struct DiffRequest {
    pub old_ref: String,
    pub new_ref: String,
}

#[derive(Debug, Serialize)]
pub struct DiffResponse {
    pub old_commit: String,
    pub new_commit: String,
    pub files_added: usize,
    pub files_removed: usize,
    pub files_modified: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub diffs: Vec<FileDiffDto>,
}

#[derive(Debug, Serialize)]
pub struct FileDiffDto {
    pub path: String,
    pub change_type: String,
    pub similarity: Option<f64>,
    pub unified_diff: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StartExperimentRequest {
    pub name: String,
    pub variants: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExperimentResponse {
    pub id: String,
    pub name: String,
    pub base_branch: String,
    pub variants: Vec<ExperimentVariantDto>,
}

#[derive(Debug, Serialize)]
pub struct ExperimentVariantDto {
    pub id: String,
    pub name: String,
    pub branch_name: String,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_objects: u64,
    pub blob_count: u64,
    pub tree_count: u64,
    pub commit_count: u64,
    pub total_size_bytes: u64,
}

// === Handlers ===

/// POST /api/v1/git/commit - Create a new commit
pub async fn create_commit(
    State(state): State<Arc<GitVersioningState>>,
    Json(req): Json<CommitRequest>,
) -> impl IntoResponse {
    let snapshot = ResponseSnapshot {
        prompt: req.prompt,
        response: req.response,
        model: req.model,
        temperature: req.temperature,
        tokens: None,
        metadata: req.metadata,
    };

    let repo = state.repo.write();
    match repo.commit(&snapshot, &req.message) {
        Ok(oid) => {
            info!(commit_id = %oid.short(), "Created new commit");
            (
                StatusCode::CREATED,
                Json(ApiResponse::ok(CommitResponse {
                    commit_id: oid.to_string(),
                    short_id: oid.short(),
                })),
            )
        }
        Err(e) => {
            error!(error = %e, "Failed to create commit");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<CommitResponse>::err(e.to_string())),
            )
        }
    }
}

/// GET /api/v1/git/log - Get commit history
pub async fn get_log(
    State(state): State<Arc<GitVersioningState>>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let repo = state.repo.read();

    let result = if let Some(ref from_ref) = query.from_ref {
        match repo.refs().resolve(from_ref) {
            Ok(oid) => repo.log_from(oid, query.max_count),
            Err(_) => Err(RepositoryError::RefNotFound(from_ref.clone())),
        }
    } else {
        repo.log(query.max_count)
    };

    match result {
        Ok(entries) => (
            StatusCode::OK,
            Json(ApiResponse::ok(LogResponse {
                entries: entries.into_iter().map(LogEntryDto::from).collect(),
            })),
        ),
        Err(RepositoryError::EmptyRepository) => (
            StatusCode::OK,
            Json(ApiResponse::ok(LogResponse { entries: vec![] })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<LogResponse>::err(e.to_string())),
        ),
    }
}

/// GET /api/v1/git/show/:ref - Show commit details
pub async fn show_commit(
    State(state): State<Arc<GitVersioningState>>,
    Path(ref_or_oid): Path<String>,
) -> impl IntoResponse {
    let repo = state.repo.read();

    match repo.get_snapshot(&ref_or_oid) {
        Ok(snapshot) => match repo.refs().resolve(&ref_or_oid) {
            Ok(oid) => (
                StatusCode::OK,
                Json(ApiResponse::ok(ShowResponse {
                    commit_id: oid.to_string(),
                    snapshot: snapshot.into(),
                })),
            ),
            Err(e) => (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<ShowResponse>::err(e.to_string())),
            ),
        },
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ShowResponse>::err(e.to_string())),
        ),
    }
}

/// GET /api/v1/git/branches - List branches
pub async fn list_branches(State(state): State<Arc<GitVersioningState>>) -> impl IntoResponse {
    let repo = state.repo.read();
    let branches: Vec<BranchResponse> = repo
        .list_branches()
        .into_iter()
        .map(|(name, oid)| BranchResponse {
            name,
            target: oid.to_string(),
        })
        .collect();

    let current = repo.current_branch_name();

    (
        StatusCode::OK,
        Json(ApiResponse::ok(BranchListResponse { branches, current })),
    )
}

/// POST /api/v1/git/branches - Create branch
pub async fn create_branch(
    State(state): State<Arc<GitVersioningState>>,
    Json(req): Json<CreateBranchRequest>,
) -> impl IntoResponse {
    let repo = state.repo.write();

    let result = if let Some(ref from_ref) = req.from_ref {
        repo.create_branch_at(&req.name, from_ref)
    } else {
        repo.create_branch(&req.name)
    };

    match result {
        Ok(()) => match repo.refs().resolve(&req.name) {
            Ok(target) => {
                info!(branch = %req.name, "Created branch");
                (
                    StatusCode::CREATED,
                    Json(ApiResponse::ok(BranchResponse {
                        name: req.name,
                        target: target.to_string(),
                    })),
                )
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<BranchResponse>::err(e.to_string())),
            ),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<BranchResponse>::err(e.to_string())),
        ),
    }
}

/// DELETE /api/v1/git/branches/:name - Delete branch
pub async fn delete_branch(
    State(state): State<Arc<GitVersioningState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let repo = state.repo.write();

    match repo.delete_branch(&name) {
        Ok(()) => {
            info!(branch = %name, "Deleted branch");
            (StatusCode::NO_CONTENT, Json(ApiResponse::<()>::ok(())))
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err(e.to_string())),
        ),
    }
}

/// POST /api/v1/git/checkout - Switch branch or checkout commit
pub async fn checkout(
    State(state): State<Arc<GitVersioningState>>,
    Json(req): Json<CheckoutRequest>,
) -> impl IntoResponse {
    let repo = state.repo.write();

    match repo.checkout(&req.target) {
        Ok(oid) => {
            let branch = repo.current_branch_name();
            info!(target = %req.target, branch = ?branch, "Checkout complete");
            (
                StatusCode::OK,
                Json(ApiResponse::ok(CheckoutResponse {
                    commit_id: oid.to_string(),
                    branch: branch.clone(),
                    detached: branch.is_none(),
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CheckoutResponse>::err(e.to_string())),
        ),
    }
}

/// GET /api/v1/git/tags - List tags
pub async fn list_tags(State(state): State<Arc<GitVersioningState>>) -> impl IntoResponse {
    let repo = state.repo.read();
    let tags: Vec<TagResponse> = repo
        .list_tags()
        .into_iter()
        .map(|(name, oid)| TagResponse {
            name,
            target: oid.to_string(),
        })
        .collect();

    (
        StatusCode::OK,
        Json(ApiResponse::ok(TagListResponse { tags })),
    )
}

/// POST /api/v1/git/tags - Create tag
pub async fn create_tag(
    State(state): State<Arc<GitVersioningState>>,
    Json(req): Json<CreateTagRequest>,
) -> impl IntoResponse {
    let repo = state.repo.write();

    let result = if let Some(ref target) = req.target {
        repo.tag_at(&req.name, target, req.message.as_deref())
    } else {
        repo.tag(&req.name, req.message.as_deref())
    };

    match result {
        Ok(()) => match repo.refs().resolve(&req.name) {
            Ok(target) => {
                info!(tag = %req.name, "Created tag");
                (
                    StatusCode::CREATED,
                    Json(ApiResponse::ok(TagResponse {
                        name: req.name,
                        target: target.to_string(),
                    })),
                )
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<TagResponse>::err(e.to_string())),
            ),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<TagResponse>::err(e.to_string())),
        ),
    }
}

/// POST /api/v1/git/diff - Compare two refs
pub async fn diff_refs(
    State(state): State<Arc<GitVersioningState>>,
    Json(req): Json<DiffRequest>,
) -> impl IntoResponse {
    let repo = state.repo.read();

    match repo.diff(&req.old_ref, &req.new_ref) {
        Ok(commit_diff) => {
            let mut diffs = Vec::new();

            // Added files
            for added in &commit_diff.tree_diff.added {
                diffs.push(FileDiffDto {
                    path: added.path.clone(),
                    change_type: "added".to_string(),
                    similarity: None,
                    unified_diff: None,
                });
            }

            // Removed files
            for removed in &commit_diff.tree_diff.removed {
                diffs.push(FileDiffDto {
                    path: removed.path.clone(),
                    change_type: "removed".to_string(),
                    similarity: None,
                    unified_diff: None,
                });
            }

            // Modified files
            for modified in &commit_diff.tree_diff.modified {
                let (similarity, unified_diff) = match &modified.blob_diff {
                    Some(bd) => (
                        Some(bd.similarity),
                        Some(bd.to_unified(
                            &format!("a/{}", modified.path),
                            &format!("b/{}", modified.path),
                        )),
                    ),
                    None => (None, None),
                };

                diffs.push(FileDiffDto {
                    path: modified.path.clone(),
                    change_type: "modified".to_string(),
                    similarity,
                    unified_diff,
                });
            }

            (
                StatusCode::OK,
                Json(ApiResponse::ok(DiffResponse {
                    old_commit: commit_diff.old_commit.to_string(),
                    new_commit: commit_diff.new_commit.to_string(),
                    files_added: commit_diff.stats.files_added,
                    files_removed: commit_diff.stats.files_removed,
                    files_modified: commit_diff.stats.files_modified,
                    lines_added: commit_diff.stats.lines_added,
                    lines_removed: commit_diff.stats.lines_removed,
                    diffs,
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<DiffResponse>::err(e.to_string())),
        ),
    }
}

/// POST /api/v1/git/experiments - Start an experiment
pub async fn start_experiment(
    State(state): State<Arc<GitVersioningState>>,
    Json(req): Json<StartExperimentRequest>,
) -> impl IntoResponse {
    let repo = state.repo.write();
    let variants: Vec<&str> = req.variants.iter().map(|s| s.as_str()).collect();

    match repo.start_experiment(&req.name, &variants) {
        Ok(exp) => {
            info!(experiment = %req.name, variants = ?req.variants, "Started experiment");
            (
                StatusCode::CREATED,
                Json(ApiResponse::ok(ExperimentResponse {
                    id: exp.id,
                    name: exp.name,
                    base_branch: exp.base_branch,
                    variants: exp
                        .variants
                        .into_iter()
                        .map(|v| ExperimentVariantDto {
                            id: v.id,
                            name: v.name,
                            branch_name: v.branch_name,
                        })
                        .collect(),
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ExperimentResponse>::err(e.to_string())),
        ),
    }
}

/// GET /api/v1/git/stats - Get repository statistics
pub async fn get_stats(State(state): State<Arc<GitVersioningState>>) -> impl IntoResponse {
    let repo = state.repo.read();
    let stats = repo.store().stats();

    (
        StatusCode::OK,
        Json(ApiResponse::ok(StatsResponse {
            total_objects: stats.total_objects,
            blob_count: stats.blob_count,
            tree_count: stats.tree_count,
            commit_count: stats.commit_count,
            total_size_bytes: stats.total_size_bytes,
        })),
    )
}

/// Create router for git versioning endpoints
pub fn git_versioning_router() -> Router<Arc<GitVersioningState>> {
    Router::new()
        .route("/commit", post(create_commit))
        .route("/log", get(get_log))
        .route("/show/:ref", get(show_commit))
        .route("/branches", get(list_branches).post(create_branch))
        .route("/branches/:name", delete(delete_branch))
        .route("/checkout", post(checkout))
        .route("/tags", get(list_tags).post(create_tag))
        .route("/diff", post(diff_refs))
        .route("/experiments", post(start_experiment))
        .route("/stats", get(get_stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn create_test_state() -> Arc<GitVersioningState> {
        Arc::new(GitVersioningState::new("test"))
    }

    #[tokio::test]
    async fn test_empty_log() {
        let state = create_test_state();
        let app = git_versioning_router().with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/log").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_commit_and_log() {
        let state = create_test_state();
        let app = git_versioning_router().with_state(state.clone());

        // Create commit
        let commit_req = CommitRequest {
            prompt: "Hello".to_string(),
            response: "World".to_string(),
            message: "Initial".to_string(),
            model: None,
            temperature: None,
            metadata: HashMap::new(),
        };

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/commit")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&commit_req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Check log
        let app = git_versioning_router().with_state(state);
        let response = app
            .oneshot(Request::builder().uri("/log").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_stats() {
        let state = create_test_state();
        let app = git_versioning_router().with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
