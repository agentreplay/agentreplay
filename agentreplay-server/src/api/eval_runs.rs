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
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use agentreplay_core::{
    EvalRun, EvalTraceV1, GraderResult, OverallResult, RunResult, TaskAggregate, TraceRefV1,
    TranscriptEventV1,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    pub dataset_id: String,
    pub name: String,
    pub agent_id: String,
    pub model: String,
    #[serde(default)]
    pub config: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct AddRunResultRequest {
    pub test_case_id: String,
    #[serde(default)]
    pub trial_id: Option<u32>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub eval_metrics: HashMap<String, f64>,
    #[serde(default)]
    pub grader_results: Vec<GraderResult>,
    #[serde(default)]
    pub overall: Option<OverallResult>,
    pub passed: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRunStatusRequest {
    pub status: String, // "completed", "failed", "stopped"
}

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub dataset_id: Option<String>,
    /// Filter by run status (running, completed, failed, stopped)
    pub status: Option<String>,
    /// Filter by model name (substring match)
    pub model: Option<String>,
    /// Filter by agent ID
    pub agent_id: Option<String>,
    /// Start time for date range filter (microseconds since epoch)
    pub start_time: Option<u64>,
    /// End time for date range filter (microseconds since epoch)
    pub end_time: Option<u64>,
    /// Filter by tags (comma-separated)
    pub tags: Option<String>,
    /// Sort field (started_at, completed_at, pass_rate, name)
    #[serde(default = "default_sort_field")]
    pub sort_by: String,
    /// Sort order (asc, desc)
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
    /// Page number for pagination
    #[serde(default = "default_page")]
    pub page: usize,
    /// Page size for pagination
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_sort_field() -> String {
    "started_at".to_string()
}

fn default_sort_order() -> String {
    "desc".to_string()
}

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    20
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    pub id: String,
    pub dataset_id: String,
    pub name: String,
    pub agent_id: String,
    pub model: String,
    pub schema_version: String,
    pub status: String,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub test_case_count: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub pass_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct RunDetailResponse {
    pub id: String,
    pub dataset_id: String,
    pub name: String,
    pub agent_id: String,
    pub model: String,
    pub schema_version: String,
    pub status: String,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub results: Vec<RunResultOutput>,
    pub task_aggregates: Vec<TaskAggregate>,
    pub aggregated_metrics: HashMap<String, f64>,
    pub passed_count: usize,
    pub failed_count: usize,
    pub pass_rate: f64,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct RunResultOutput {
    pub test_case_id: String,
    pub trial_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_ref: Option<TraceRefV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_trace: Option<EvalTraceV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_summary: Option<TraceSummaryOutput>,
    pub eval_metrics: HashMap<String, f64>,
    #[serde(default)]
    pub grader_results: Vec<GraderResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall: Option<OverallResult>,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp_us: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TraceSummaryOutput {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_output_text: Option<String>,
    pub tool_call_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_diff_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunListResponse {
    pub runs: Vec<RunResponse>,
    pub total: usize,
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

    let random = (rand::random::<u64>() as u128) << 64;
    timestamp ^ random
}

fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn parse_id(id_str: &str) -> Result<u128, String> {
    let id_str = id_str.trim_start_matches("0x");
    u128::from_str_radix(id_str, 16).map_err(|e| format!("Invalid ID: {}", e))
}

fn run_to_response(run: &EvalRun) -> RunResponse {
    RunResponse {
        id: format!("0x{:x}", run.id),
        dataset_id: format!("0x{:x}", run.dataset_id),
        name: run.name.clone(),
        agent_id: run.agent_id.clone(),
        model: run.model.clone(),
        schema_version: run.schema_version.clone(),
        status: run.status.as_str().to_string(),
        started_at: run.started_at,
        completed_at: run.completed_at,
        test_case_count: run.results.len(),
        passed_count: run.passed_count(),
        failed_count: run.failed_count(),
        pass_rate: run.pass_rate(),
    }
}

fn run_to_detail_response(state: &AppState, run: &EvalRun) -> RunDetailResponse {
    let task_aggregates = run.task_aggregates(&[1, 3, 5]);

    RunDetailResponse {
        id: format!("0x{:x}", run.id),
        dataset_id: format!("0x{:x}", run.dataset_id),
        name: run.name.clone(),
        agent_id: run.agent_id.clone(),
        model: run.model.clone(),
        schema_version: run.schema_version.clone(),
        status: run.status.as_str().to_string(),
        started_at: run.started_at,
        completed_at: run.completed_at,
        results: run
            .results
            .iter()
            .map(|r| {
                let (eval_trace, trace_ref, trace_summary) = build_trace_artifacts(state, r.trace_id);
                RunResultOutput {
                test_case_id: format!("0x{:x}", r.test_case_id),
                trial_id: r.trial_id,
                seed: r.seed,
                trace_id: r.trace_id.map(|id| format!("0x{:x}", id)),
                    trace_ref,
                    eval_trace,
                    trace_summary,
                    eval_metrics: r.eval_metrics.clone(),
                    grader_results: r.grader_results.clone(),
                    overall: r.overall.clone(),
                    passed: r.passed,
                    error: r.error.clone(),
                    timestamp_us: r.timestamp_us,
                    cost_usd: r.cost_usd,
                    latency_ms: r.latency_ms,
                }
            })
            .collect(),
        task_aggregates,
        aggregated_metrics: run.aggregate_metrics(),
        passed_count: run.passed_count(),
        failed_count: run.failed_count(),
        pass_rate: run.pass_rate(),
        config: run.config.clone(),
    }
}

fn build_trace_artifacts(
    state: &AppState,
    trace_id: Option<u128>,
) -> (Option<EvalTraceV1>, Option<TraceRefV1>, Option<TraceSummaryOutput>) {
    let Some(trace_id) = trace_id else {
        return (None, None, None);
    };

    let trace_id_hex = format!("0x{:x}", trace_id);
    let edge = state.db.get(trace_id).ok().flatten();
    let eval_trace = edge.map(|edge| crate::api::build_eval_trace_v1(state, &edge));

    let trace_ref = eval_trace.as_ref().map(|trace| TraceRefV1 {
        schema_version: trace.schema_version.clone(),
        trace_id: trace.trace_id.clone(),
        export_uri: Some(format!("/api/v1/traces/{}/detailed", trace.trace_id)),
        hash: trace.trace_ref.as_ref().and_then(|r| r.hash.clone()),
    }).or_else(|| {
        Some(TraceRefV1 {
            schema_version: "eval_trace_v1".to_string(),
            trace_id: trace_id_hex.clone(),
            export_uri: Some(format!("/api/v1/traces/{}/detailed", trace_id_hex)),
            hash: None,
        })
    });

    let trace_summary = eval_trace.as_ref().map(build_trace_summary);

    (eval_trace, trace_ref, trace_summary)
}

fn build_trace_summary(trace: &EvalTraceV1) -> TraceSummaryOutput {
    let tool_call_count = trace
        .transcript
        .iter()
        .filter(|event| matches!(event, TranscriptEventV1::ToolCall { .. }))
        .count();

    let state_diff_hash = trace
        .outcome_v2
        .as_ref()
        .and_then(|outcome| outcome.state_before.as_ref().zip(outcome.state_after.as_ref()))
        .map(|(before, after)| {
            let payload = serde_json::json!({"before": before, "after": after});
            blake3::hash(&serde_json::to_vec(&payload).unwrap_or_default())
                .to_hex()
                .to_string()
        });

    TraceSummaryOutput {
        status: trace.outcome.status.clone(),
        final_output_text: trace.outcome.output_text.clone(),
        tool_call_count,
        state_diff_hash,
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/v1/evals/runs
/// Create a new evaluation run (experiment)
pub async fn create_run(
    State(state): State<AppState>,
    Json(req): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<RunDetailResponse>), (StatusCode, String)> {
    let dataset_id = parse_id(&req.dataset_id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Verify dataset exists
    state
        .db
        .get_eval_dataset(dataset_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Dataset not found".to_string()))?;

    let run_id = generate_id();
    let timestamp = current_timestamp_us();

    let mut run = EvalRun::new(
        run_id,
        dataset_id,
        req.name,
        req.agent_id,
        req.model,
        timestamp,
    );
    run.config = req.config;

    state
        .db
        .store_eval_run(run.clone())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(run_to_detail_response(&state, &run))))
}

/// GET /api/v1/evals/runs
/// List evaluation runs with advanced filtering, sorting, and pagination
pub async fn list_runs(
    State(state): State<AppState>,
    Query(params): Query<ListRunsQuery>,
) -> Result<Json<RunListResponse>, (StatusCode, String)> {
    let dataset_id = if let Some(id_str) = &params.dataset_id {
        Some(parse_id(id_str).map_err(|e| (StatusCode::BAD_REQUEST, e))?)
    } else {
        None
    };

    let mut runs = state
        .db
        .list_eval_runs(dataset_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Apply status filter
    if let Some(ref status_filter) = params.status {
        let status_lower = status_filter.to_lowercase();
        runs.retain(|r| {
            let run_status = match r.status {
                agentreplay_core::eval_dataset::RunStatus::Running => "running",
                agentreplay_core::eval_dataset::RunStatus::Completed => "completed",
                agentreplay_core::eval_dataset::RunStatus::Failed => "failed",
                agentreplay_core::eval_dataset::RunStatus::Stopped => "stopped",
            };
            run_status == status_lower
        });
    }

    // Apply model filter (substring match)
    if let Some(ref model_filter) = params.model {
        let model_lower = model_filter.to_lowercase();
        runs.retain(|r| r.model.to_lowercase().contains(&model_lower));
    }

    // Apply agent_id filter
    if let Some(ref agent_filter) = params.agent_id {
        runs.retain(|r| r.agent_id == *agent_filter);
    }

    // Apply date range filter
    if let Some(start_time) = params.start_time {
        runs.retain(|r| r.started_at >= start_time);
    }
    if let Some(end_time) = params.end_time {
        runs.retain(|r| r.started_at <= end_time);
    }

    // Apply tags filter
    if let Some(ref tags_str) = params.tags {
        let tags: Vec<&str> = tags_str.split(',').map(|s| s.trim()).collect();
        runs.retain(|r| {
            // Check if any tag matches a config key prefixed with "tag:"
            tags.iter().any(|tag| {
                r.config.get(&format!("tag:{}", tag)).is_some()
                    || r.config.values().any(|v| v == *tag)
            })
        });
    }

    // Sort results
    match params.sort_by.as_str() {
        "started_at" => {
            if params.sort_order == "asc" {
                runs.sort_by(|a, b| a.started_at.cmp(&b.started_at));
            } else {
                runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
            }
        }
        "completed_at" => {
            runs.sort_by(|a, b| {
                let order = a.completed_at.cmp(&b.completed_at);
                if params.sort_order == "asc" {
                    order
                } else {
                    order.reverse()
                }
            });
        }
        "pass_rate" => {
            if params.sort_order == "asc" {
                runs.sort_by(|a, b| {
                    a.pass_rate()
                        .partial_cmp(&b.pass_rate())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            } else {
                runs.sort_by(|a, b| {
                    b.pass_rate()
                        .partial_cmp(&a.pass_rate())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
        "name" => {
            if params.sort_order == "asc" {
                runs.sort_by(|a, b| a.name.cmp(&b.name));
            } else {
                runs.sort_by(|a, b| b.name.cmp(&a.name));
            }
        }
        _ => {
            // Default to started_at desc
            runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        }
    }

    let total = runs.len();

    // Apply pagination
    let page = params.page.max(1);
    let page_size = params.page_size.min(100).max(1);
    let start = (page - 1) * page_size;
    let paginated_runs: Vec<_> = runs.into_iter().skip(start).take(page_size).collect();

    let run_responses: Vec<RunResponse> = paginated_runs.iter().map(run_to_response).collect();

    Ok(Json(RunListResponse {
        runs: run_responses,
        total,
    }))
}

/// GET /api/v1/evals/runs/:id
/// Get a specific evaluation run with all results
pub async fn get_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RunDetailResponse>, (StatusCode, String)> {
    let run_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let run = state
        .db
        .get_eval_run(run_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Run not found".to_string()))?;

    Ok(Json(run_to_detail_response(&state, &run)))
}

/// POST /api/v1/evals/runs/:id/results
/// Add a result to an evaluation run
pub async fn add_run_result(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddRunResultRequest>,
) -> Result<Json<RunDetailResponse>, (StatusCode, String)> {
    let run_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let test_case_id = parse_id(&req.test_case_id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let trace_id = if let Some(tid_str) = req.trace_id {
        Some(parse_id(&tid_str).map_err(|e| (StatusCode::BAD_REQUEST, e))?)
    } else {
        None
    };

    let timestamp = current_timestamp_us();
    let mut result = if req.passed {
        RunResult::success(test_case_id, trace_id.unwrap_or(0), timestamp)
    } else {
        RunResult::failure(
            test_case_id,
            req.error.unwrap_or_else(|| "Unknown error".to_string()),
            timestamp,
        )
    };

    result.trace_id = trace_id;
    result.eval_metrics = req.eval_metrics;
    result.trial_id = req.trial_id.unwrap_or(0);
    result.seed = req.seed;
    result.grader_results = req.grader_results;
    result.overall = req.overall;
    result.cost_usd = req.cost_usd;
    result.latency_ms = req.latency_ms;

    state
        .db
        .update_eval_run(run_id, |run| {
            run.add_result(result);
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch updated run
    let run = state
        .db
        .get_eval_run(run_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Run not found".to_string()))?;

    Ok(Json(run_to_detail_response(&state, &run)))
}

/// POST /api/v1/evals/runs/:id/status
/// Update the status of an evaluation run (complete, fail, stop)
pub async fn update_run_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRunStatusRequest>,
) -> Result<Json<RunDetailResponse>, (StatusCode, String)> {
    let run_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let timestamp = current_timestamp_us();

    state
        .db
        .update_eval_run(run_id, |run| {
            match req.status.to_lowercase().as_str() {
                "completed" => run.complete(timestamp),
                "failed" => run.fail(timestamp),
                "stopped" => run.stop(timestamp),
                _ => {} // Ignore unknown statuses
            }
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch updated run
    let run = state
        .db
        .get_eval_run(run_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Run not found".to_string()))?;

    Ok(Json(run_to_detail_response(&state, &run)))
}

/// DELETE /api/v1/evals/runs/:id
/// Delete an evaluation run
pub async fn delete_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, String)> {
    let run_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let deleted = state
        .db
        .delete_eval_run(run_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(DeleteResponse {
            success: true,
            message: "Run deleted successfully".to_string(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Run not found".to_string()))
    }
}

// ============================================================================
// Export/Import Endpoints
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// Format: json or csv
    #[serde(default = "default_export_format")]
    pub format: String,
    /// Include full results or just summary
    #[serde(default)]
    pub include_results: bool,
}

fn default_export_format() -> String {
    "json".to_string()
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub format: String,
    pub data: String,
    pub run_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    /// Format: json or csv
    pub format: String,
    /// The data to import
    pub data: String,
    /// Whether to overwrite existing runs with the same ID
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

/// GET /api/v1/evals/runs/export
/// Export evaluation runs to JSON or CSV
pub async fn export_runs(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Result<Json<ExportResponse>, (StatusCode, String)> {
    let runs = state
        .db
        .list_eval_runs(None)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let run_count = runs.len();

    let data = match query.format.as_str() {
        "csv" => export_runs_csv(&runs, query.include_results)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
        _ => {
            // Default to JSON
            if query.include_results {
                serde_json::to_string_pretty(&runs)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            } else {
                let summaries: Vec<RunResponse> = runs.iter().map(run_to_response).collect();
                serde_json::to_string_pretty(&summaries)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            }
        }
    };

    Ok(Json(ExportResponse {
        format: query.format,
        data,
        run_count,
    }))
}

/// POST /api/v1/evals/runs/import
/// Import evaluation runs from JSON or CSV
pub async fn import_runs(
    State(state): State<AppState>,
    Json(req): Json<ImportRequest>,
) -> Result<Json<ImportResponse>, (StatusCode, String)> {
    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    let runs: Vec<EvalRun> = match req.format.as_str() {
        "csv" => parse_runs_csv(&req.data).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
        _ => {
            // Default to JSON
            serde_json::from_str(&req.data)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?
        }
    };

    for run in runs {
        // Check if run already exists
        match state.db.get_eval_run(run.id) {
            Ok(Some(_)) if !req.overwrite => {
                skipped += 1;
            }
            _ => match state.db.store_eval_run(run.clone()) {
                Ok(_) => imported += 1,
                Err(e) => errors.push(format!("Failed to import run {}: {}", run.id, e)),
            },
        }
    }

    Ok(Json(ImportResponse {
        success: errors.is_empty(),
        imported,
        skipped,
        errors,
    }))
}

/// Export runs to CSV format
fn export_runs_csv(runs: &[EvalRun], include_results: bool) -> Result<String, String> {
    let mut csv = String::new();

    // Header
    if include_results {
        csv.push_str("id,dataset_id,name,agent_id,model,status,started_at,completed_at,test_case_id,passed,error,trace_id\n");

        for run in runs {
            let status = format!("{:?}", run.status);
            let completed = run.completed_at.map(|t| t.to_string()).unwrap_or_default();

            for result in &run.results {
                csv.push_str(&format!(
                    "{:x},{:x},{},{},{},{},{},{},{:x},{},{},{:x}\n",
                    run.id,
                    run.dataset_id,
                    escape_csv(&run.name),
                    escape_csv(&run.agent_id),
                    escape_csv(&run.model),
                    status,
                    run.started_at,
                    completed,
                    result.test_case_id,
                    result.passed,
                    escape_csv(&result.error.clone().unwrap_or_default()),
                    result.trace_id.unwrap_or(0)
                ));
            }
        }
    } else {
        csv.push_str("id,dataset_id,name,agent_id,model,status,started_at,completed_at,test_case_count,passed_count,failed_count,pass_rate\n");

        for run in runs {
            let status = format!("{:?}", run.status);
            let completed = run.completed_at.map(|t| t.to_string()).unwrap_or_default();

            csv.push_str(&format!(
                "{:x},{:x},{},{},{},{},{},{},{},{},{},{:.4}\n",
                run.id,
                run.dataset_id,
                escape_csv(&run.name),
                escape_csv(&run.agent_id),
                escape_csv(&run.model),
                status,
                run.started_at,
                completed,
                run.results.len(),
                run.passed_count(),
                run.failed_count(),
                run.pass_rate()
            ));
        }
    }

    Ok(csv)
}

/// Parse runs from CSV format (simplified - expects JSON-like ID columns)
fn parse_runs_csv(data: &str) -> Result<Vec<EvalRun>, String> {
    let mut runs = Vec::new();
    let lines: Vec<&str> = data.lines().collect();

    if lines.is_empty() {
        return Ok(runs);
    }

    // Skip header
    for line in &lines[1..] {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 8 {
            continue;
        }

        let id = u128::from_str_radix(fields[0].trim_start_matches("0x"), 16)
            .map_err(|_| format!("Invalid ID: {}", fields[0]))?;
        let dataset_id = u128::from_str_radix(fields[1].trim_start_matches("0x"), 16)
            .map_err(|_| format!("Invalid dataset_id: {}", fields[1]))?;

        let run = EvalRun::new(
            id,
            dataset_id,
            unescape_csv(fields[2]),
            unescape_csv(fields[3]),
            unescape_csv(fields[4]),
            fields[6].parse().unwrap_or(0),
        );

        runs.push(run);
    }

    Ok(runs)
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn unescape_csv(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].replace("\"\"", "\"")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_id() {
        assert_eq!(parse_id("0x123").unwrap(), 0x123);
        assert_eq!(parse_id("123").unwrap(), 0x123);
        assert_eq!(parse_id("0xabc").unwrap(), 0xabc);
    }

    #[test]
    fn test_generate_id() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("hello"), "hello");
        assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
        assert_eq!(escape_csv("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_unescape_csv() {
        assert_eq!(unescape_csv("hello"), "hello");
        assert_eq!(unescape_csv("\"hello,world\""), "hello,world");
        assert_eq!(unescape_csv("\"say \"\"hi\"\"\""), "say \"hi\"");
    }
}
