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

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::api::{ApiError, AppState};
use crate::auth::AuthContext;

/// Request body for trace feedback
#[derive(Debug, Deserialize)]
pub struct FeedbackRequest {
    pub feedback: i8, // 1 for thumbs up, -1 for thumbs down
}

/// Request body for adding trace to dataset
#[derive(Debug, Deserialize)]
pub struct AddToDatasetRequest {
    pub trace_id: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
}

/// Response for feedback submission
#[derive(Debug, Serialize)]
pub struct FeedbackResponse {
    pub success: bool,
    pub message: String,
}

/// Response for dataset addition
#[derive(Debug, Serialize)]
pub struct DatasetResponse {
    pub success: bool,
    pub message: String,
    pub dataset_name: String,
}

/// POST /api/v1/traces/:trace_id/feedback - Submit thumbs up/down feedback for a trace
pub async fn submit_trace_feedback(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(trace_id): Path<String>,
    Json(req): Json<FeedbackRequest>,
) -> Result<impl IntoResponse, ApiError> {
    debug!(
        "Submitting feedback for trace {}: {}",
        trace_id, req.feedback
    );

    // Validate feedback value
    if req.feedback != 1 && req.feedback != -1 {
        return Err(ApiError::BadRequest(
            "Feedback must be 1 (positive) or -1 (negative)".to_string(),
        ));
    }

    // Parse trace_id as edge_id
    let edge_id = u128::from_str_radix(&trace_id, 16)
        .map_err(|_| ApiError::BadRequest(format!("Invalid trace_id format: {}", trace_id)))?;

    // Store feedback as metadata on the edge
    let mut feedback_data = serde_json::Map::new();
    feedback_data.insert("feedback".to_string(), serde_json::json!(req.feedback));
    feedback_data.insert("tenant_id".to_string(), serde_json::json!(auth.tenant_id));
    feedback_data.insert(
        "timestamp".to_string(),
        serde_json::json!(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()),
    );

    let feedback_json = serde_json::to_vec(&serde_json::Value::Object(feedback_data))
        .map_err(|e| ApiError::Internal(format!("Failed to serialize feedback: {}", e)))?;

    // Store as payload with special prefix (feedback_)
    let feedback_key = format!("feedback_{}", edge_id);
    let feedback_edge_id = crate::api::ingest::hash_string_to_u64(&feedback_key) as u128;

    state
        .db
        .put_payload(feedback_edge_id, &feedback_json)
        .map_err(|e| ApiError::Internal(format!("Failed to store feedback: {}", e)))?;

    Ok((
        StatusCode::OK,
        Json(FeedbackResponse {
            success: true,
            message: "Feedback recorded successfully".to_string(),
        }),
    ))
}

/// POST /api/v1/datasets/:name/add - Add trace to evaluation dataset
pub async fn add_trace_to_dataset(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(dataset_name): Path<String>,
    Json(req): Json<AddToDatasetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    debug!("Adding trace {} to dataset {}", req.trace_id, dataset_name);

    // Parse trace_id
    let edge_id = u128::from_str_radix(&req.trace_id, 16)
        .map_err(|_| ApiError::BadRequest(format!("Invalid trace_id format: {}", req.trace_id)))?;

    // Create dataset entry
    let mut dataset_entry = serde_json::Map::new();
    dataset_entry.insert("trace_id".to_string(), serde_json::json!(req.trace_id));
    dataset_entry.insert("edge_id".to_string(), serde_json::json!(edge_id));
    dataset_entry.insert("input".to_string(), req.input);
    dataset_entry.insert("output".to_string(), req.output);
    dataset_entry.insert("tenant_id".to_string(), serde_json::json!(auth.tenant_id));
    dataset_entry.insert("dataset_name".to_string(), serde_json::json!(dataset_name));
    dataset_entry.insert(
        "added_at".to_string(),
        serde_json::json!(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()),
    );

    let dataset_json = serde_json::to_vec(&serde_json::Value::Object(dataset_entry))
        .map_err(|e| ApiError::Internal(format!("Failed to serialize dataset entry: {}", e)))?;

    // Store as payload with dataset prefix
    let dataset_key = format!("dataset_{}_{}", dataset_name, edge_id);
    let dataset_entry_id = crate::api::ingest::hash_string_to_u64(&dataset_key) as u128;

    state
        .db
        .put_payload(dataset_entry_id, &dataset_json)
        .map_err(|e| ApiError::Internal(format!("Failed to store dataset entry: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(DatasetResponse {
            success: true,
            message: "Trace added to dataset successfully".to_string(),
            dataset_name,
        }),
    ))
}
