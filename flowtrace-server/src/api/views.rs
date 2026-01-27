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
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use flowtrace_core::SavedView;
use serde::{Deserialize, Serialize};

use super::AppState;

/// Request to create a new saved view
#[derive(Debug, Deserialize)]
pub struct CreateViewRequest {
    pub name: String,
    pub description: Option<String>,
    pub filters: serde_json::Value,
    pub columns: Vec<String>,
    pub tags: Option<Vec<String>>,
    pub is_shared: Option<bool>,
}

/// Request to update an existing view
#[derive(Debug, Deserialize)]
pub struct UpdateViewRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub filters: Option<serde_json::Value>,
    pub columns: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub is_shared: Option<bool>,
}

/// Query parameters for listing views
#[derive(Debug, Deserialize)]
pub struct ListViewsQuery {
    pub search: Option<String>,
}

/// Request to import views
#[derive(Debug, Deserialize)]
pub struct ImportViewsRequest {
    pub views_json: String,
    pub overwrite: Option<bool>,
}

/// Response for import operation
#[derive(Debug, Serialize)]
pub struct ImportViewsResponse {
    pub success: bool,
    pub imported_count: usize,
    pub message: String,
}

/// Response for export operation
#[derive(Debug, Serialize)]
pub struct ExportViewsResponse {
    pub views_json: String,
}

/// POST /api/v1/views - Create a new saved view
pub async fn create_view(
    State(state): State<AppState>,
    Json(req): Json<CreateViewRequest>,
) -> Result<Json<SavedView>, (StatusCode, String)> {
    let mut view = SavedView::new(req.name, req.filters, req.columns);

    if let Some(desc) = req.description {
        view.description = Some(desc);
    }

    if let Some(tags) = req.tags {
        view.tags = tags;
    }

    if let Some(is_shared) = req.is_shared {
        view.is_shared = is_shared;
    }

    let mut registry = state.saved_view_registry.write().await;

    match registry.add_view(view) {
        Ok(saved_view) => Ok(Json(saved_view)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

/// GET /api/v1/views - List all saved views
pub async fn list_views(
    State(state): State<AppState>,
    Query(query): Query<ListViewsQuery>,
) -> Result<Json<Vec<SavedView>>, (StatusCode, String)> {
    let registry = state.saved_view_registry.read().await;

    let views = if let Some(search_query) = query.search {
        registry.search_views(&search_query)
    } else {
        registry.list_views()
    };

    // Convert from Vec<&SavedView> to Vec<SavedView>
    let views: Vec<SavedView> = views.into_iter().cloned().collect();

    Ok(Json(views))
}

/// GET /api/v1/views/:id - Get a specific saved view
pub async fn get_view(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SavedView>, (StatusCode, String)> {
    let registry = state.saved_view_registry.read().await;

    match registry.get_view(&id) {
        Some(view) => Ok(Json(view.clone())),
        None => Err((StatusCode::NOT_FOUND, format!("View {} not found", id))),
    }
}

/// PUT /api/v1/views/:id - Update a saved view
pub async fn update_view(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateViewRequest>,
) -> Result<Json<SavedView>, (StatusCode, String)> {
    let mut registry = state.saved_view_registry.write().await;

    // Get existing view
    let mut view = match registry.get_view(&id) {
        Some(v) => v.clone(),
        None => return Err((StatusCode::NOT_FOUND, format!("View {} not found", id))),
    };

    // Update fields
    if let Some(name) = req.name {
        view.name = name;
    }

    if let Some(description) = req.description {
        view.description = Some(description);
    }

    if let Some(filters) = req.filters {
        view.filters = filters;
    }

    if let Some(columns) = req.columns {
        view.columns = columns;
    }

    if let Some(tags) = req.tags {
        view.tags = tags;
    }

    if let Some(is_shared) = req.is_shared {
        view.is_shared = is_shared;
    }

    match registry.update_view(view) {
        Ok(updated_view) => Ok(Json(updated_view)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

/// DELETE /api/v1/views/:id - Delete a saved view
pub async fn delete_view(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut registry = state.saved_view_registry.write().await;

    match registry.delete_view(&id) {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

/// POST /api/v1/views/export - Export views to JSON
pub async fn export_views(
    State(state): State<AppState>,
    Json(view_ids): Json<Option<Vec<String>>>,
) -> Result<Json<ExportViewsResponse>, (StatusCode, String)> {
    let registry = state.saved_view_registry.read().await;

    match registry.export_views(view_ids) {
        Ok(json) => Ok(Json(ExportViewsResponse { views_json: json })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

/// POST /api/v1/views/import - Import views from JSON
pub async fn import_views(
    State(state): State<AppState>,
    Json(req): Json<ImportViewsRequest>,
) -> Result<Json<ImportViewsResponse>, (StatusCode, String)> {
    let mut registry = state.saved_view_registry.write().await;

    let overwrite = req.overwrite.unwrap_or(false);

    match registry.import_views(&req.views_json, overwrite) {
        Ok(count) => Ok(Json(ImportViewsResponse {
            success: true,
            imported_count: count,
            message: format!("Successfully imported {} views", count),
        })),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}
