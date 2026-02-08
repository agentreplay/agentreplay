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

// agentreplay-server/src/api/experiments.rs
//
// A/B testing and experiments API endpoints

use super::query::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Data Models - Use core types with API enums
// ============================================================================

use agentreplay_core::enterprise::{
    Experiment as CoreExperiment, ExperimentResult as CoreResult, ExperimentVariant as CoreVariant,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExperimentStatus {
    Draft,
    Running,
    Paused,
    Completed,
    Stopped,
}

impl ExperimentStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ExperimentStatus::Draft => "draft",
            ExperimentStatus::Running => "running",
            ExperimentStatus::Paused => "paused",
            ExperimentStatus::Completed => "completed",
            ExperimentStatus::Stopped => "stopped",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "draft" => ExperimentStatus::Draft,
            "running" => ExperimentStatus::Running,
            "paused" => ExperimentStatus::Paused,
            "completed" => ExperimentStatus::Completed,
            "stopped" => ExperimentStatus::Stopped,
            _ => ExperimentStatus::Draft,
        }
    }
}

// API representation with typed enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: u128,
    pub name: String,
    pub description: String,
    pub variants: Vec<Variant>,
    pub status: ExperimentStatus,
    pub traffic_split: HashMap<String, f64>,
    pub metrics: Vec<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub id: String,
    pub name: String,
    pub description: String,
    pub config: HashMap<String, serde_json::Value>,
}

// Conversion from API type to Core type
impl From<Experiment> for CoreExperiment {
    fn from(exp: Experiment) -> Self {
        CoreExperiment {
            id: exp.id,
            name: exp.name,
            description: exp.description,
            variants: exp
                .variants
                .into_iter()
                .map(|v| CoreVariant {
                    id: v.id,
                    name: v.name,
                    description: v.description,
                    config: v.config,
                })
                .collect(),
            status: exp.status.as_str().to_string(),
            traffic_split: exp.traffic_split,
            metrics: exp.metrics,
            start_time: exp.start_time,
            end_time: exp.end_time,
            created_at: exp.created_at,
            updated_at: exp.updated_at,
        }
    }
}

// Conversion from Core type to API type
impl From<CoreExperiment> for Experiment {
    fn from(core: CoreExperiment) -> Self {
        Experiment {
            id: core.id,
            name: core.name,
            description: core.description,
            variants: core
                .variants
                .into_iter()
                .map(|v| Variant {
                    id: v.id,
                    name: v.name,
                    description: v.description,
                    config: v.config,
                })
                .collect(),
            status: ExperimentStatus::parse(&core.status),
            traffic_split: core.traffic_split,
            metrics: core.metrics,
            start_time: core.start_time,
            end_time: core.end_time,
            created_at: core.created_at,
            updated_at: core.updated_at,
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateExperimentRequest {
    pub name: String,
    pub description: String,
    pub variants: Vec<VariantInput>,
    #[serde(default)]
    pub metrics: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct VariantInput {
    pub name: String,
    pub description: String,
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExperimentRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub traffic_split: Option<HashMap<String, f64>>,
}

#[derive(Debug, Deserialize)]
pub struct StartExperimentRequest {
    pub traffic_split: HashMap<String, f64>,
}

#[derive(Debug, Deserialize)]
pub struct RecordResultRequest {
    pub variant_id: String,
    pub trace_id: String,
    pub metrics: HashMap<String, f64>,
}

#[derive(Debug, Deserialize)]
pub struct ListExperimentsQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExperimentResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub variants: Vec<Variant>,
    pub status: String,
    pub traffic_split: HashMap<String, f64>,
    pub metrics: Vec<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize)]
pub struct ExperimentListResponse {
    pub experiments: Vec<ExperimentResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct ExperimentStatsResponse {
    pub experiment_id: String,
    pub variant_stats: HashMap<String, VariantStats>,
    pub winner: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct VariantStats {
    pub variant_id: String,
    pub variant_name: String,
    pub sample_count: usize,
    pub metrics: HashMap<String, MetricStats>,
}

#[derive(Debug, Serialize)]
pub struct MetricStats {
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub count: usize,
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

fn generate_variant_id() -> String {
    format!("var_{:x}", rand::random::<u64>())
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

fn experiment_to_response(exp: &Experiment) -> ExperimentResponse {
    ExperimentResponse {
        id: format!("0x{:x}", exp.id),
        name: exp.name.clone(),
        description: exp.description.clone(),
        variants: exp.variants.clone(),
        status: exp.status.as_str().to_string(),
        traffic_split: exp.traffic_split.clone(),
        metrics: exp.metrics.clone(),
        start_time: exp.start_time,
        end_time: exp.end_time,
        created_at: exp.created_at,
        updated_at: exp.updated_at,
    }
}

fn calculate_metric_stats(values: &[f64]) -> MetricStats {
    if values.is_empty() {
        return MetricStats {
            mean: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
            count: 0,
        };
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let std_dev = variance.sqrt();
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    MetricStats {
        mean,
        std_dev,
        min,
        max,
        count: values.len(),
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/v1/experiments
/// Create a new A/B test experiment
pub async fn create_experiment(
    State(state): State<AppState>,
    Json(req): Json<CreateExperimentRequest>,
) -> Result<(StatusCode, Json<ExperimentResponse>), (StatusCode, String)> {
    let experiment_id = generate_id();
    let timestamp = current_timestamp_us();

    let variants: Vec<Variant> = req
        .variants
        .into_iter()
        .map(|v| Variant {
            id: generate_variant_id(),
            name: v.name,
            description: v.description,
            config: v.config,
        })
        .collect();

    let experiment = Experiment {
        id: experiment_id,
        name: req.name,
        description: req.description,
        variants,
        status: ExperimentStatus::Draft,
        traffic_split: HashMap::new(),
        metrics: req.metrics,
        start_time: None,
        end_time: None,
        created_at: timestamp,
        updated_at: timestamp,
    };

    // Convert API type to Core type for storage
    let core_experiment: CoreExperiment = experiment.clone().into();
    state
        .db
        .store_experiment(core_experiment)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(experiment_to_response(&experiment)),
    ))
}

/// GET /api/v1/experiments
/// List all experiments
pub async fn list_experiments(
    State(state): State<AppState>,
    Query(params): Query<ListExperimentsQuery>,
) -> Result<Json<ExperimentListResponse>, (StatusCode, String)> {
    let core_experiments = state
        .db
        .list_experiments()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Convert from Core to API types
    let mut experiments: Vec<Experiment> = core_experiments.into_iter().map(|e| e.into()).collect();

    // Filter by status if provided
    if let Some(status_str) = params.status {
        experiments.retain(|e| e.status.as_str() == status_str.to_lowercase());
    }

    let total = experiments.len();
    let experiment_responses: Vec<ExperimentResponse> =
        experiments.iter().map(experiment_to_response).collect();

    Ok(Json(ExperimentListResponse {
        experiments: experiment_responses,
        total,
    }))
}

/// GET /api/v1/experiments/:id
/// Get a specific experiment
pub async fn get_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExperimentResponse>, (StatusCode, String)> {
    let experiment_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let core_experiment = state
        .db
        .get_experiment(experiment_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Experiment not found".to_string()))?;

    // Convert from Core to API type
    let experiment: Experiment = core_experiment.into();
    Ok(Json(experiment_to_response(&experiment)))
}

/// PUT /api/v1/experiments/:id
/// Update an experiment
pub async fn update_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateExperimentRequest>,
) -> Result<Json<ExperimentResponse>, (StatusCode, String)> {
    let experiment_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let timestamp = current_timestamp_us();

    state
        .db
        .update_experiment(experiment_id, |exp| {
            if let Some(name) = req.name {
                exp.name = name;
            }
            if let Some(description) = req.description {
                exp.description = description;
            }
            if let Some(traffic_split) = req.traffic_split {
                exp.traffic_split = traffic_split;
            }
            exp.updated_at = timestamp;
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch updated experiment
    let core_experiment = state
        .db
        .get_experiment(experiment_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Experiment not found".to_string()))?;

    let experiment: Experiment = core_experiment.into();
    Ok(Json(experiment_to_response(&experiment)))
}

/// POST /api/v1/experiments/:id/start
/// Start an experiment
pub async fn start_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<StartExperimentRequest>,
) -> Result<Json<ExperimentResponse>, (StatusCode, String)> {
    let experiment_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Validate traffic split
    let total: f64 = req.traffic_split.values().sum();
    if (total - 1.0).abs() > 0.001 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Traffic split must sum to 1.0, got {}", total),
        ));
    }

    let timestamp = current_timestamp_us();

    state
        .db
        .update_experiment(experiment_id, |exp| {
            exp.status = ExperimentStatus::Running.as_str().to_string();
            exp.traffic_split = req.traffic_split.clone();
            exp.start_time = Some(timestamp);
            exp.updated_at = timestamp;
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch updated experiment
    let core_experiment = state
        .db
        .get_experiment(experiment_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Experiment not found".to_string()))?;

    let experiment: Experiment = core_experiment.into();
    Ok(Json(experiment_to_response(&experiment)))
}

/// POST /api/v1/experiments/:id/stop
/// Stop an experiment
pub async fn stop_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExperimentResponse>, (StatusCode, String)> {
    let experiment_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let timestamp = current_timestamp_us();

    state
        .db
        .update_experiment(experiment_id, |exp| {
            exp.status = ExperimentStatus::Completed.as_str().to_string();
            exp.end_time = Some(timestamp);
            exp.updated_at = timestamp;
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch updated experiment
    let core_experiment = state
        .db
        .get_experiment(experiment_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Experiment not found".to_string()))?;

    let experiment: Experiment = core_experiment.into();
    Ok(Json(experiment_to_response(&experiment)))
}

/// POST /api/v1/experiments/:id/results
/// Record a result for an experiment
pub async fn record_result(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RecordResultRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let experiment_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let trace_id = parse_id(&req.trace_id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let timestamp = current_timestamp_us();

    let result = CoreResult {
        experiment_id,
        variant_id: req.variant_id,
        trace_id,
        metrics: req.metrics,
        timestamp_us: timestamp,
    };

    state
        .db
        .store_experiment_result(result)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "experiment_id": format!("0x{:x}", experiment_id),
    })))
}

/// GET /api/v1/experiments/:id/stats
/// Get statistics for an experiment
pub async fn get_experiment_stats(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExperimentStatsResponse>, (StatusCode, String)> {
    let experiment_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let core_experiment = state
        .db
        .get_experiment(experiment_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Experiment not found".to_string()))?;

    let experiment: Experiment = core_experiment.into();

    let results = state
        .db
        .get_experiment_results(experiment_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Group results by variant
    let mut variant_data: HashMap<String, Vec<CoreResult>> = HashMap::new();
    for result in results {
        variant_data
            .entry(result.variant_id.clone())
            .or_default()
            .push(result);
    }

    // Calculate stats for each variant
    let mut variant_stats: HashMap<String, VariantStats> = HashMap::new();
    for variant in &experiment.variants {
        let results = variant_data
            .get(&variant.id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let mut metrics: HashMap<String, MetricStats> = HashMap::new();
        for metric_name in &experiment.metrics {
            let values: Vec<f64> = results
                .iter()
                .filter_map(|r| r.metrics.get(metric_name).copied())
                .collect();

            metrics.insert(metric_name.clone(), calculate_metric_stats(&values));
        }

        variant_stats.insert(
            variant.id.clone(),
            VariantStats {
                variant_id: variant.id.clone(),
                variant_name: variant.name.clone(),
                sample_count: results.len(),
                metrics,
            },
        );
    }

    Ok(Json(ExperimentStatsResponse {
        experiment_id: format!("0x{:x}", experiment_id),
        variant_stats,
        winner: None, // TODO: Implement statistical significance testing
        confidence: None,
    }))
}

/// DELETE /api/v1/experiments/:id
/// Delete an experiment
pub async fn delete_experiment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, String)> {
    let experiment_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let deleted = state
        .db
        .delete_experiment(experiment_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(DeleteResponse {
            success: true,
            message: "Experiment deleted successfully".to_string(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Experiment not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_metric_stats() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = calculate_metric_stats(&values);

        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.count, 5);
    }

    #[test]
    fn test_empty_metric_stats() {
        let values: Vec<f64> = vec![];
        let stats = calculate_metric_stats(&values);

        assert_eq!(stats.count, 0);
        assert_eq!(stats.mean, 0.0);
    }
}
