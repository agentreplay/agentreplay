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
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use flowtrace_core::{EvalDataset, TaskDefinitionV2, TestCase};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateDatasetRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub test_cases: Vec<TestCaseInput>,
}

#[derive(Debug, Deserialize)]
pub struct TestCaseInput {
    pub input: String,
    #[serde(default)]
    pub expected_output: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub task_definition_v2: Option<TaskDefinitionV2>,
}

#[derive(Debug, Serialize)]
pub struct DatasetResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub test_case_count: usize,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize)]
pub struct DatasetDetailResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub test_cases: Vec<TestCaseOutput>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize)]
pub struct TestCaseOutput {
    pub id: String,
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
    pub metadata: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_definition_v2: Option<TaskDefinitionV2>,
}

#[derive(Debug, Serialize)]
pub struct DatasetListResponse {
    pub datasets: Vec<DatasetResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct CreateDatasetResponse {
    pub dataset_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_id() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();

    // Use timestamp + random bits for uniqueness
    let random = (rand::random::<u64>() as u128) << 64;
    timestamp ^ random
}

fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn parse_dataset_id(id_str: &str) -> Result<u128, String> {
    let id_str = id_str.trim_start_matches("0x");
    u128::from_str_radix(id_str, 16).map_err(|e| format!("Invalid dataset ID: {}", e))
}

fn dataset_to_response(dataset: &EvalDataset) -> DatasetResponse {
    DatasetResponse {
        id: format!("0x{:x}", dataset.id),
        name: dataset.name.clone(),
        description: dataset.description.clone(),
        test_case_count: dataset.test_case_count(),
        created_at: dataset.created_at,
        updated_at: dataset.updated_at,
    }
}

fn dataset_to_detail_response(dataset: &EvalDataset) -> DatasetDetailResponse {
    DatasetDetailResponse {
        id: format!("0x{:x}", dataset.id),
        name: dataset.name.clone(),
        description: dataset.description.clone(),
        test_cases: dataset
            .test_cases
            .iter()
            .map(|tc| TestCaseOutput {
                id: format!("0x{:x}", tc.id),
                input: tc.input.clone(),
                expected_output: tc.expected_output.clone(),
                metadata: tc.metadata.clone(),
                task_definition_v2: tc.task_definition_v2.clone(),
            })
            .collect(),
        created_at: dataset.created_at,
        updated_at: dataset.updated_at,
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/v1/evals/datasets
/// Create a new evaluation dataset
pub async fn create_dataset(
    State(state): State<AppState>,
    Json(req): Json<CreateDatasetRequest>,
) -> Result<(StatusCode, Json<CreateDatasetResponse>), (StatusCode, String)> {
    let dataset_id = generate_id();
    let timestamp = current_timestamp_us();

    let description = req.description.clone().unwrap_or_default();
    let mut dataset =
        EvalDataset::new(dataset_id, req.name.clone(), description.clone(), timestamp);

    // Add test cases
    for tc_input in req.test_cases {
        let tc_id = generate_id();
        let mut test_case = TestCase::new(tc_id, tc_input.input);
        test_case.expected_output = tc_input.expected_output;
        test_case.metadata = tc_input.metadata;
        if let Some(mut task_definition) = tc_input.task_definition_v2 {
            task_definition.ensure_task_id();
            test_case.task_definition_v2 = Some(task_definition);
        }
        dataset.add_test_case(test_case);
    }

    state
        .db
        .store_eval_dataset(dataset.clone())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateDatasetResponse {
            dataset_id: format!("0x{:x}", dataset_id),
            name: req.name,
            description,
        }),
    ))
}

/// GET /api/v1/evals/datasets
/// List all evaluation datasets
pub async fn list_datasets(
    State(state): State<AppState>,
) -> Result<Json<DatasetListResponse>, (StatusCode, String)> {
    let datasets = state
        .db
        .list_eval_datasets()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = datasets.len();
    let dataset_responses: Vec<DatasetResponse> =
        datasets.iter().map(dataset_to_response).collect();

    Ok(Json(DatasetListResponse {
        datasets: dataset_responses,
        total,
    }))
}

/// GET /api/v1/evals/datasets/:id
/// Get a specific evaluation dataset with all test cases
pub async fn get_dataset(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DatasetDetailResponse>, (StatusCode, String)> {
    let dataset_id = parse_dataset_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let dataset = state
        .db
        .get_eval_dataset(dataset_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Dataset not found".to_string()))?;

    Ok(Json(dataset_to_detail_response(&dataset)))
}

/// DELETE /api/v1/evals/datasets/:id
/// Delete an evaluation dataset
pub async fn delete_dataset(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, String)> {
    let dataset_id = parse_dataset_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let deleted = state
        .db
        .delete_eval_dataset(dataset_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(DeleteResponse {
            success: true,
            message: "Dataset deleted successfully".to_string(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Dataset not found".to_string()))
    }
}

#[derive(Debug, Deserialize)]
pub struct AddExamplesRequest {
    pub examples: Vec<ExampleInput>,
}

#[derive(Debug, Deserialize)]
pub struct ExampleInput {
    #[serde(default)]
    pub example_id: Option<String>,
    pub input: String,
    #[serde(default)]
    pub expected_output: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct AddExamplesResponse {
    pub success: bool,
    pub added_count: usize,
}

/// POST /api/v1/evals/datasets/:id/examples
/// Add examples/test cases to an existing dataset
pub async fn add_examples(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddExamplesRequest>,
) -> Result<Json<AddExamplesResponse>, (StatusCode, String)> {
    let dataset_id = parse_dataset_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Get existing dataset
    let mut dataset = state
        .db
        .get_eval_dataset(dataset_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Dataset not found".to_string()))?;

    let added_count = req.examples.len();

    // Add new test cases
    for example in req.examples {
        let tc_id = generate_id();
        let mut test_case = TestCase::new(tc_id, example.input);
        test_case.expected_output = example.expected_output;
        if let Some(context) = example.context {
            test_case.metadata.insert("context".to_string(), context);
        }
        for (k, v) in example.metadata {
            test_case.metadata.insert(k, v);
        }
        dataset.add_test_case(test_case);
    }

    // Update timestamp
    dataset.updated_at = current_timestamp_us();

    // Save updated dataset
    state
        .db
        .store_eval_dataset(dataset)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AddExamplesResponse {
        success: true,
        added_count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dataset_id() {
        assert_eq!(parse_dataset_id("0x123").unwrap(), 0x123);
        assert_eq!(parse_dataset_id("123").unwrap(), 0x123);
        assert_eq!(parse_dataset_id("0xabc").unwrap(), 0xabc);
    }

    #[test]
    fn test_generate_id() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }
}
