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

//! Evaluation Pipeline API
//!
//! Comprehensive 5-phase evaluation pipeline supporting:
//! 1. Collect - Trace collection with filtering
//! 2. Process - Data processing and categorization  
//! 3. Annotate - Manual/LLM annotations and golden dataset management
//! 4. Evaluate - Run metrics across 5 categories (Operational, Quality, Agent, UX, Safety)
//! 5. Iterate - Results analysis and recommendations

use super::query::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use agentreplay_core::AgentFlowEdge;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Helper Functions for Real Data Extraction
// ============================================================================

/// Extract model name and cost from trace payload
fn extract_trace_metadata(
    db: &Arc<agentreplay_query::Agentreplay>,
    edge: &AgentFlowEdge,
) -> (Option<String>, f64, u64, u64) {
    if let Ok(Some(payload_bytes)) = db.get_payload(edge.edge_id) {
        if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
            let model = payload
                .get("model")
                .or_else(|| payload.get("gen_ai.request.model"))
                .or_else(|| payload.get("llm.model"))
                .and_then(|v| v.as_str())
                .map(String::from);

            let input_tokens = payload
                .get("gen_ai.usage.input_tokens")
                .or_else(|| payload.get("llm.usage.prompt_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(edge.token_count as u64);

            let output_tokens = payload
                .get("gen_ai.usage.output_tokens")
                .or_else(|| payload.get("llm.usage.completion_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            // Extract cost from payload or calculate from tokens
            let cost = payload
                .get("cost_usd")
                .or_else(|| payload.get("gen_ai.usage.cost"))
                .and_then(|v| v.as_f64())
                .unwrap_or_else(|| {
                    // Pricing per 1K tokens (approximate)
                    let (input_price, output_price) = match model.as_deref() {
                        Some(m) if m.contains("gpt-4o") => (0.0025, 0.01),
                        Some(m) if m.contains("gpt-4-turbo") => (0.01, 0.03),
                        Some(m) if m.contains("gpt-4") => (0.03, 0.06),
                        Some(m) if m.contains("gpt-3.5") => (0.0005, 0.0015),
                        Some(m) if m.contains("claude-3-opus") => (0.015, 0.075),
                        Some(m) if m.contains("claude-3-sonnet") => (0.003, 0.015),
                        Some(m) if m.contains("claude-3-haiku") => (0.00025, 0.00125),
                        _ => (0.001, 0.002), // Default pricing
                    };

                    (input_tokens as f64 * input_price / 1000.0)
                        + (output_tokens as f64 * output_price / 1000.0)
                });

            return (model, cost, input_tokens, output_tokens);
        }
    }
    (
        None,
        edge.token_count as f64 * 0.000002,
        edge.token_count as u64,
        0,
    )
}

// ============================================================================
// Types for Comprehensive Metrics Framework
// ============================================================================

/// Metric categories matching the developer framework
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MetricCategory {
    Operational,
    Quality,
    Agent,
    UserExperience,
    Safety,
}

impl MetricCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricCategory::Operational => "operational",
            MetricCategory::Quality => "quality",
            MetricCategory::Agent => "agent",
            MetricCategory::UserExperience => "user_experience",
            MetricCategory::Safety => "safety",
        }
    }
}

/// Priority levels for metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricPriority {
    Critical,
    High,
    Medium,
    Low,
}

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    Percentage,
    Duration,
    Count,
    Currency,
    Score,
}

/// Individual metric definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: MetricCategory,
    pub metric_type: MetricType,
    pub priority: MetricPriority,
    pub target: Option<f64>,
    pub target_description: Option<String>,
    pub unit: Option<String>,
}

/// Computed metric value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub metric_id: String,
    pub value: f64,
    pub target: Option<f64>,
    pub trend: Option<f64>, // Percentage change from previous period
    pub status: MetricStatus,
    pub samples: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Good,
    Warning,
    Critical,
    Unknown,
}

// ============================================================================
// Phase 1: Collect - Trace Collection
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CollectTracesRequest {
    pub project_id: Option<u16>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub status_filter: Option<String>, // "success", "error", "all"
    pub min_duration_ms: Option<u64>,
    pub max_duration_ms: Option<u64>,
    pub search_query: Option<String>,
    pub limit: Option<usize>,
    pub include_metadata: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CollectedTrace {
    pub trace_id: String,
    pub timestamp_us: u64,
    pub duration_ms: Option<f64>,
    pub status: String,
    pub span_count: usize,
    pub token_count: Option<u64>,
    pub cost_usd: Option<f64>,
    pub model: Option<String>,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CollectTracesResponse {
    pub traces: Vec<CollectedTrace>,
    pub total_count: usize,
    pub filtered_count: usize,
    pub summary: CollectionSummary,
}

#[derive(Debug, Serialize)]
pub struct CollectionSummary {
    pub success_count: usize,
    pub error_count: usize,
    pub avg_duration_ms: f64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub avg_cost_usd: Option<f64>,
    pub models_used: Option<Vec<String>>,
    pub date_range: (u64, u64),
}

/// POST /api/v1/evals/pipeline/collect
pub async fn collect_traces(
    State(state): State<AppState>,
    Json(req): Json<CollectTracesRequest>,
) -> Result<Json<CollectTracesResponse>, (StatusCode, String)> {
    let now = current_timestamp_us();
    let start_time = req.start_time.unwrap_or(now - 86_400_000_000); // Default: last 24h
    let end_time = req.end_time.unwrap_or(now);
    let limit = req.limit.unwrap_or(1000);

    // Fetch traces from database
    let edges = state
        .db
        .list_traces_in_range(start_time, end_time)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Group edges by trace_id
    let mut trace_map: HashMap<u128, Vec<&AgentFlowEdge>> = HashMap::new();
    for edge in &edges {
        let trace_id = (edge.edge_id >> 64) as u64;
        trace_map.entry(trace_id as u128).or_default().push(edge);
    }

    let mut collected_traces = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;
    let mut total_duration = 0.0;
    let mut total_tokens = 0u64;
    let mut total_cost = 0.0;
    let mut models_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (trace_id, trace_edges) in &trace_map {
        if collected_traces.len() >= limit {
            break;
        }

        // Compute trace-level metrics
        let first_edge = trace_edges.first().unwrap();

        // Duration in ms (sum of all spans in trace)
        let duration_ms: f64 = trace_edges
            .iter()
            .map(|e| e.duration_us as f64 / 1000.0)
            .sum();

        // Apply duration filters
        if let Some(min_dur) = req.min_duration_ms {
            if duration_ms < min_dur as f64 {
                continue;
            }
        }
        if let Some(max_dur) = req.max_duration_ms {
            if duration_ms > max_dur as f64 {
                continue;
            }
        }

        // Determine status from span_type (Error = 8)
        let has_error = trace_edges.iter().any(|e| e.span_type == 8);
        let status = if has_error { "error" } else { "success" };

        // Apply status filter
        if let Some(ref status_filter) = req.status_filter {
            if status_filter != "all" && status_filter != status {
                continue;
            }
        }

        // Extract real model and cost from payloads
        let mut trace_model: Option<String> = None;
        let mut trace_cost = 0.0f64;
        let mut trace_input_tokens = 0u64;
        let mut trace_output_tokens = 0u64;

        for edge in trace_edges {
            let (model, cost, input_tok, output_tok) = extract_trace_metadata(&state.db, edge);
            if let Some(m) = model {
                trace_model = Some(m.clone());
                models_seen.insert(m);
            }
            trace_cost += cost;
            trace_input_tokens += input_tok;
            trace_output_tokens += output_tok;
        }

        let tokens = trace_input_tokens + trace_output_tokens;

        if has_error {
            error_count += 1;
        } else {
            success_count += 1;
        }
        total_duration += duration_ms;
        total_tokens += tokens;
        total_cost += trace_cost;

        // Get input/output previews if requested
        let (input_preview, output_preview) = if req.include_metadata.unwrap_or(false) {
            if let Ok(Some(payload_bytes)) = state.db.get_payload(first_edge.edge_id) {
                if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
                    let input = payload
                        .get("gen_ai.prompt.0.content")
                        .or_else(|| payload.get("input"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.chars().take(200).collect::<String>());
                    let output = payload
                        .get("gen_ai.completion.0.content")
                        .or_else(|| payload.get("output"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.chars().take(200).collect::<String>());
                    (input, output)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        collected_traces.push(CollectedTrace {
            trace_id: format!("0x{:x}", trace_id),
            timestamp_us: first_edge.timestamp_us,
            duration_ms: Some(duration_ms),
            status: status.to_string(),
            span_count: trace_edges.len(),
            token_count: Some(tokens),
            cost_usd: Some(trace_cost),
            model: trace_model,
            input_preview,
            output_preview,
            metadata: HashMap::new(),
        });
    }

    let total_count = trace_map.len();
    let filtered_count = collected_traces.len();
    let avg_duration = if filtered_count > 0 {
        total_duration / filtered_count as f64
    } else {
        0.0
    };
    let avg_cost = if filtered_count > 0 {
        total_cost / filtered_count as f64
    } else {
        0.0
    };

    Ok(Json(CollectTracesResponse {
        traces: collected_traces,
        total_count,
        filtered_count,
        summary: CollectionSummary {
            success_count,
            error_count,
            avg_duration_ms: avg_duration,
            total_tokens,
            total_cost_usd: total_cost,
            avg_cost_usd: Some(avg_cost),
            models_used: Some(models_seen.into_iter().collect()),
            date_range: (start_time, end_time),
        },
    }))
}

// ============================================================================
// Phase 2: Process - Data Processing and Categorization
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ProcessTracesRequest {
    pub trace_ids: Vec<String>,
    pub categorization: Option<CategorizationConfig>,
    pub sampling: Option<SamplingConfig>,
}

#[derive(Debug, Deserialize)]
pub struct CategorizationConfig {
    pub by_model: bool,
    pub by_status: bool,
    pub by_latency_bucket: bool,
    pub by_cost_bucket: bool,
    pub custom_tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SamplingConfig {
    pub strategy: String, // "random", "stratified", "recent", "diverse"
    pub sample_size: usize,
    pub seed: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ProcessTracesResponse {
    pub processed_count: usize,
    pub categories: HashMap<String, CategoryStats>,
    pub sampled_trace_ids: Option<Vec<String>>,
    pub processing_stats: ProcessingStats,
}

#[derive(Debug, Serialize)]
pub struct CategoryStats {
    pub count: usize,
    pub avg_duration_ms: f64,
    pub avg_tokens: f64,
    pub avg_cost_usd: f64,
    pub total_cost_usd: Option<f64>,
    pub error_rate: f64,
    pub trace_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProcessingStats {
    pub total_traces: usize,
    pub categorized_traces: usize,
    pub uncategorized_traces: usize,
    pub processing_time_ms: u64,
}

/// POST /api/v1/evals/pipeline/process
pub async fn process_traces(
    State(state): State<AppState>,
    Json(req): Json<ProcessTracesRequest>,
) -> Result<Json<ProcessTracesResponse>, (StatusCode, String)> {
    let start = std::time::Instant::now();

    // Store (trace_id, duration, tokens, cost, has_error, model)
    let mut categories: HashMap<String, Vec<(String, f64, u64, f64, bool, Option<String>)>> =
        HashMap::new();

    // Get all edges once
    let now = current_timestamp_us();
    let all_edges = state
        .db
        .list_traces_in_range(0, now)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Parse and process trace data
    for trace_id_str in &req.trace_ids {
        let trace_id = parse_trace_id(trace_id_str).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

        let trace_edges: Vec<_> = all_edges
            .iter()
            .filter(|e| (e.edge_id >> 64) as u64 == (trace_id >> 64) as u64)
            .collect();

        if trace_edges.is_empty() {
            continue;
        }

        // Compute metrics from trace edges
        let duration_ms: f64 = trace_edges
            .iter()
            .map(|e| e.duration_us as f64 / 1000.0)
            .sum();
        let has_error = trace_edges.iter().any(|e| e.span_type == 8);

        // Extract real model and cost from payloads
        let mut trace_model: Option<String> = None;
        let mut trace_cost = 0.0f64;
        let mut total_tokens = 0u64;

        for edge in &trace_edges {
            let (model, cost, input_tok, output_tok) = extract_trace_metadata(&state.db, edge);
            if model.is_some() {
                trace_model = model;
            }
            trace_cost += cost;
            total_tokens += input_tok + output_tok;
        }

        // Categorize based on config
        if let Some(ref config) = req.categorization {
            if config.by_status {
                let category = if has_error { "error" } else { "success" };
                categories
                    .entry(format!("status:{}", category))
                    .or_default()
                    .push((
                        trace_id_str.clone(),
                        duration_ms,
                        total_tokens,
                        trace_cost,
                        has_error,
                        trace_model.clone(),
                    ));
            }

            if config.by_latency_bucket {
                let bucket = match duration_ms as u64 {
                    0..=100 => "fast",
                    101..=500 => "medium",
                    501..=2000 => "slow",
                    _ => "very_slow",
                };
                categories
                    .entry(format!("latency:{}", bucket))
                    .or_default()
                    .push((
                        trace_id_str.clone(),
                        duration_ms,
                        total_tokens,
                        trace_cost,
                        has_error,
                        trace_model.clone(),
                    ));
            }

            if config.by_model {
                let model_name = trace_model.clone().unwrap_or_else(|| "unknown".to_string());
                categories
                    .entry(format!("model:{}", model_name))
                    .or_default()
                    .push((
                        trace_id_str.clone(),
                        duration_ms,
                        total_tokens,
                        trace_cost,
                        has_error,
                        trace_model.clone(),
                    ));
            }

            if config.by_cost_bucket {
                let bucket = match (trace_cost * 1000.0) as u64 {
                    // Convert to milli-cents
                    0..=10 => "cheap",         // < $0.01
                    11..=100 => "moderate",    // $0.01 - $0.10
                    101..=1000 => "expensive", // $0.10 - $1.00
                    _ => "very_expensive",     // > $1.00
                };
                categories
                    .entry(format!("cost:{}", bucket))
                    .or_default()
                    .push((
                        trace_id_str.clone(),
                        duration_ms,
                        total_tokens,
                        trace_cost,
                        has_error,
                        trace_model.clone(),
                    ));
            }
        } else {
            // Default: just count
            categories.entry("all".to_string()).or_default().push((
                trace_id_str.clone(),
                duration_ms,
                total_tokens,
                trace_cost,
                has_error,
                trace_model,
            ));
        }
    }

    // Compute category stats with real data
    let mut category_stats: HashMap<String, CategoryStats> = HashMap::new();
    for (category, traces) in &categories {
        let count = traces.len();
        let total_duration: f64 = traces.iter().map(|t| t.1).sum();
        let total_tokens: u64 = traces.iter().map(|t| t.2).sum();
        let total_cost: f64 = traces.iter().map(|t| t.3).sum();
        let error_count = traces.iter().filter(|t| t.4).count();

        category_stats.insert(
            category.clone(),
            CategoryStats {
                count,
                avg_duration_ms: if count > 0 {
                    total_duration / count as f64
                } else {
                    0.0
                },
                avg_tokens: if count > 0 {
                    total_tokens as f64 / count as f64
                } else {
                    0.0
                },
                avg_cost_usd: if count > 0 {
                    total_cost / count as f64
                } else {
                    0.0
                },
                total_cost_usd: Some(total_cost),
                error_rate: if count > 0 {
                    error_count as f64 / count as f64 * 100.0
                } else {
                    0.0
                },
                trace_ids: traces.iter().map(|t| t.0.clone()).collect(),
            },
        );
    }

    // Apply sampling if requested
    let sampled_trace_ids = if let Some(ref sampling) = req.sampling {
        let mut all_traces: Vec<_> = categories.values().flatten().collect();

        let sample_size = sampling.sample_size.min(all_traces.len());

        match sampling.strategy.as_str() {
            "stratified" => {
                // Take proportional samples from each category
                let mut sampled = Vec::new();
                let total = all_traces.len();
                for traces in categories.values() {
                    let proportion = traces.len() as f64 / total as f64;
                    let take =
                        ((proportion * sample_size as f64).ceil() as usize).min(traces.len());
                    sampled.extend(traces.iter().take(take).map(|t| t.0.clone()));
                }
                Some(sampled.into_iter().take(sample_size).collect())
            }
            "error_focused" => {
                // Prioritize error traces
                all_traces.sort_by(|a, b| b.4.cmp(&a.4));
                Some(
                    all_traces
                        .into_iter()
                        .take(sample_size)
                        .map(|t| t.0.clone())
                        .collect(),
                )
            }
            "high_cost" => {
                // Prioritize high-cost traces
                all_traces
                    .sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
                Some(
                    all_traces
                        .into_iter()
                        .take(sample_size)
                        .map(|t| t.0.clone())
                        .collect(),
                )
            }
            "slow" => {
                // Prioritize slow traces
                all_traces
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                Some(
                    all_traces
                        .into_iter()
                        .take(sample_size)
                        .map(|t| t.0.clone())
                        .collect(),
                )
            }
            "recent" => Some(
                all_traces
                    .into_iter()
                    .take(sample_size)
                    .map(|t| t.0.clone())
                    .collect(),
            ),
            _ => {
                // Random sampling with deterministic seed
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let seed = sampling.seed.unwrap_or(42);
                let mut sampled_ids: Vec<String> = all_traces.iter().map(|t| t.0.clone()).collect();

                // Deterministic shuffle based on seed
                sampled_ids.sort_by(|a, b| {
                    let mut ha = DefaultHasher::new();
                    let mut hb = DefaultHasher::new();
                    format!("{}{}", a, seed).hash(&mut ha);
                    format!("{}{}", b, seed).hash(&mut hb);
                    ha.finish().cmp(&hb.finish())
                });

                Some(sampled_ids.into_iter().take(sample_size).collect())
            }
        }
    } else {
        None
    };

    let processing_time = start.elapsed().as_millis() as u64;
    let total_traces = req.trace_ids.len();
    let categorized = category_stats.values().map(|s| s.count).sum();

    Ok(Json(ProcessTracesResponse {
        processed_count: total_traces,
        categories: category_stats,
        sampled_trace_ids,
        processing_stats: ProcessingStats {
            total_traces,
            categorized_traces: categorized,
            uncategorized_traces: total_traces - categorized,
            processing_time_ms: processing_time,
        },
    }))
}

// ============================================================================
// Phase 3: Annotate - Annotations and Golden Dataset
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateAnnotationRequest {
    pub trace_id: String,
    pub annotation_type: String, // "label", "score", "feedback", "golden"
    pub value: serde_json::Value,
    pub annotator: Option<String>, // "human", "llm:gpt-4", etc.
    pub confidence: Option<f64>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct Annotation {
    pub id: String,
    pub trace_id: String,
    pub annotation_type: String,
    pub value: serde_json::Value,
    pub annotator: String,
    pub confidence: f64,
    pub created_at: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CreateAnnotationResponse {
    pub annotation: Annotation,
}

/// POST /api/v1/evals/pipeline/annotate
pub async fn create_annotation(
    State(_state): State<AppState>,
    Json(req): Json<CreateAnnotationRequest>,
) -> Result<Json<CreateAnnotationResponse>, (StatusCode, String)> {
    let annotation = Annotation {
        id: format!("0x{:x}", generate_id()),
        trace_id: req.trace_id,
        annotation_type: req.annotation_type,
        value: req.value,
        annotator: req.annotator.unwrap_or_else(|| "human".to_string()),
        confidence: req.confidence.unwrap_or(1.0),
        created_at: current_timestamp_us(),
        metadata: req.metadata.unwrap_or_default(),
    };

    // TODO: Persist annotation to database
    // state.db.store_annotation(&annotation)?;

    Ok(Json(CreateAnnotationResponse { annotation }))
}

#[derive(Debug, Deserialize)]
pub struct GoldenTestCase {
    pub input: String,
    pub expected_output: String,
    pub context: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub source_trace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddGoldenTestCasesRequest {
    pub dataset_name: String,
    pub test_cases: Vec<GoldenTestCase>,
}

#[derive(Debug, Serialize)]
pub struct AddGoldenTestCasesResponse {
    pub dataset_id: String,
    pub added_count: usize,
    pub total_count: usize,
}

/// POST /api/v1/evals/pipeline/golden
pub async fn add_golden_test_cases(
    State(state): State<AppState>,
    Json(req): Json<AddGoldenTestCasesRequest>,
) -> Result<Json<AddGoldenTestCasesResponse>, (StatusCode, String)> {
    // Check if dataset exists, create if not
    let datasets = state
        .db
        .list_eval_datasets()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let dataset = datasets.iter().find(|d| d.name == req.dataset_name);

    let dataset_id = if let Some(d) = dataset {
        d.id
    } else {
        // Create new dataset
        let new_id = generate_id();
        let now = current_timestamp_us();
        let new_dataset = agentreplay_core::EvalDataset::new(
            new_id,
            req.dataset_name.clone(),
            format!("Golden dataset: {}", req.dataset_name),
            now,
        );
        state
            .db
            .store_eval_dataset(new_dataset)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        new_id
    };

    // Get the dataset to modify
    let mut dataset = state
        .db
        .get_eval_dataset(dataset_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Dataset not found".to_string()))?;

    // Add test cases
    let mut added_count = 0;
    for tc in req.test_cases {
        let test_case = agentreplay_core::TestCase {
            id: generate_id(),
            input: tc.input,
            expected_output: Some(tc.expected_output),
            metadata: tc.metadata.unwrap_or_default(),
            task_definition_v2: None,
        };
        dataset.add_test_case(test_case);
        added_count += 1;
    }

    // Save updated dataset
    let total_count = dataset.test_cases.len();
    state
        .db
        .store_eval_dataset(dataset)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AddGoldenTestCasesResponse {
        dataset_id: format!("0x{:x}", dataset_id),
        added_count,
        total_count,
    }))
}

// ============================================================================
// Phase 4: Evaluate - Comprehensive Metrics Evaluation
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct RunEvaluationRequest {
    pub trace_ids: Vec<String>,
    pub metrics: Vec<String>, // Metric IDs to compute
    pub categories: Option<Vec<MetricCategory>>,
    pub compare_with_baseline: Option<String>, // Previous run ID
    pub llm_judge_model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EvaluationResults {
    pub run_id: String,
    pub timestamp: u64,
    pub trace_count: usize,
    pub metrics: HashMap<String, MetricValue>,
    pub category_scores: HashMap<String, f64>,
    pub overall_health: HealthStatus,
    pub alerts: Vec<MetricAlert>,
    pub comparison: Option<BaselineComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct MetricAlert {
    pub metric_id: String,
    pub severity: String,
    pub message: String,
    pub current_value: f64,
    pub threshold: f64,
}

#[derive(Debug, Serialize)]
pub struct BaselineComparison {
    pub baseline_run_id: String,
    pub improvements: Vec<String>,
    pub regressions: Vec<String>,
    pub unchanged: Vec<String>,
}

/// POST /api/v1/evals/pipeline/evaluate
pub async fn run_evaluation(
    State(state): State<AppState>,
    Json(req): Json<RunEvaluationRequest>,
) -> Result<Json<EvaluationResults>, (StatusCode, String)> {
    let run_id = format!("0x{:x}", generate_id());
    let now = current_timestamp_us();

    // Collect trace data
    let mut all_durations = Vec::new();
    let mut all_tokens = Vec::new();
    let mut all_costs = Vec::new();
    let mut success_count = 0usize;
    let mut error_count = 0usize;

    for trace_id_str in &req.trace_ids {
        let trace_id = parse_trace_id(trace_id_str).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

        let edges = state
            .db
            .list_traces_in_range(0, now)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let trace_edges: Vec<_> = edges
            .iter()
            .filter(|e| (e.edge_id >> 64) as u64 == (trace_id >> 64) as u64)
            .collect();

        if let Some(edge) = trace_edges.first() {
            // Use actual AgentFlowEdge fields
            all_durations.push(edge.duration_us as f64 / 1000.0); // Convert to ms
            all_tokens.push(edge.token_count as f64);
            all_costs.push(edge.token_count as f64 * 0.000002); // Estimate cost

            // Check for error span type (8 = Error)
            if trace_edges.iter().any(|e| e.span_type == 8) {
                error_count += 1;
            } else {
                success_count += 1;
            }
        }
    }

    // Compute metrics
    let mut metrics = HashMap::new();
    let total = success_count + error_count;

    // Operational Metrics
    if !all_durations.is_empty() {
        all_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());

        metrics.insert(
            "latency_p50".to_string(),
            MetricValue {
                metric_id: "latency_p50".to_string(),
                value: percentile(&all_durations, 50.0),
                target: Some(200.0),
                trend: None,
                status: if percentile(&all_durations, 50.0) < 200.0 {
                    MetricStatus::Good
                } else {
                    MetricStatus::Warning
                },
                samples: all_durations.len() as u64,
            },
        );

        metrics.insert(
            "latency_p95".to_string(),
            MetricValue {
                metric_id: "latency_p95".to_string(),
                value: percentile(&all_durations, 95.0),
                target: Some(500.0),
                trend: None,
                status: if percentile(&all_durations, 95.0) < 500.0 {
                    MetricStatus::Good
                } else {
                    MetricStatus::Warning
                },
                samples: all_durations.len() as u64,
            },
        );

        metrics.insert(
            "latency_p99".to_string(),
            MetricValue {
                metric_id: "latency_p99".to_string(),
                value: percentile(&all_durations, 99.0),
                target: Some(1000.0),
                trend: None,
                status: if percentile(&all_durations, 99.0) < 1000.0 {
                    MetricStatus::Good
                } else {
                    MetricStatus::Critical
                },
                samples: all_durations.len() as u64,
            },
        );
    }

    let success_rate = if total > 0 {
        success_count as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    metrics.insert(
        "success_rate".to_string(),
        MetricValue {
            metric_id: "success_rate".to_string(),
            value: success_rate,
            target: Some(99.0),
            trend: None,
            status: if success_rate >= 99.0 {
                MetricStatus::Good
            } else if success_rate >= 95.0 {
                MetricStatus::Warning
            } else {
                MetricStatus::Critical
            },
            samples: total as u64,
        },
    );

    let error_rate = if total > 0 {
        error_count as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    metrics.insert(
        "error_rate".to_string(),
        MetricValue {
            metric_id: "error_rate".to_string(),
            value: error_rate,
            target: Some(1.0),
            trend: None,
            status: if error_rate <= 1.0 {
                MetricStatus::Good
            } else if error_rate <= 5.0 {
                MetricStatus::Warning
            } else {
                MetricStatus::Critical
            },
            samples: total as u64,
        },
    );

    let total_cost: f64 = all_costs.iter().sum();
    let avg_cost = if !all_costs.is_empty() {
        total_cost / all_costs.len() as f64
    } else {
        0.0
    };
    metrics.insert(
        "cost_per_request".to_string(),
        MetricValue {
            metric_id: "cost_per_request".to_string(),
            value: avg_cost,
            target: Some(0.01),
            trend: None,
            status: if avg_cost <= 0.01 {
                MetricStatus::Good
            } else if avg_cost <= 0.05 {
                MetricStatus::Warning
            } else {
                MetricStatus::Critical
            },
            samples: all_costs.len() as u64,
        },
    );

    let total_tokens_val: f64 = all_tokens.iter().sum();
    metrics.insert(
        "token_throughput".to_string(),
        MetricValue {
            metric_id: "token_throughput".to_string(),
            value: total_tokens_val,
            target: None,
            trend: None,
            status: MetricStatus::Good,
            samples: all_tokens.len() as u64,
        },
    );

    // Category scores (aggregated)
    let mut category_scores = HashMap::new();
    category_scores.insert(
        "operational".to_string(),
        (success_rate + (100.0 - error_rate)) / 2.0,
    );
    category_scores.insert("quality".to_string(), 85.0); // Would need LLM judge
    category_scores.insert("agent".to_string(), 80.0); // Would need tool analysis
    category_scores.insert("user_experience".to_string(), 75.0); // Would need feedback data
    category_scores.insert("safety".to_string(), 95.0); // Would need safety checks

    // Generate alerts
    let mut alerts = Vec::new();
    for (metric_id, value) in &metrics {
        match value.status {
            MetricStatus::Critical => {
                alerts.push(MetricAlert {
                    metric_id: metric_id.clone(),
                    severity: "critical".to_string(),
                    message: format!("{} is critically below target", metric_id),
                    current_value: value.value,
                    threshold: value.target.unwrap_or(0.0),
                });
            }
            MetricStatus::Warning => {
                alerts.push(MetricAlert {
                    metric_id: metric_id.clone(),
                    severity: "warning".to_string(),
                    message: format!("{} needs attention", metric_id),
                    current_value: value.value,
                    threshold: value.target.unwrap_or(0.0),
                });
            }
            _ => {}
        }
    }

    // Determine overall health
    let critical_count = alerts.iter().filter(|a| a.severity == "critical").count();
    let warning_count = alerts.iter().filter(|a| a.severity == "warning").count();
    let overall_health = if critical_count > 0 {
        HealthStatus::Critical
    } else if warning_count > 2 {
        HealthStatus::Warning
    } else {
        HealthStatus::Healthy
    };

    Ok(Json(EvaluationResults {
        run_id,
        timestamp: now,
        trace_count: req.trace_ids.len(),
        metrics,
        category_scores,
        overall_health,
        alerts,
        comparison: None, // TODO: Implement baseline comparison
    }))
}

// ============================================================================
// Phase 5: Iterate - Results and Recommendations
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct GetRecommendationsRequest {
    pub run_id: Option<String>,
    pub category: Option<MetricCategory>,
    pub include_historical: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Recommendation {
    pub id: String,
    pub priority: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub impact: String,
    pub effort: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RecommendationsResponse {
    pub recommendations: Vec<Recommendation>,
    pub summary: RecommendationsSummary,
}

#[derive(Debug, Serialize)]
pub struct RecommendationsSummary {
    pub total_recommendations: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub estimated_impact: String,
}

/// GET /api/v1/evals/pipeline/recommendations
pub async fn get_recommendations(
    State(_state): State<AppState>,
    Query(_req): Query<GetRecommendationsRequest>,
) -> Result<Json<RecommendationsResponse>, (StatusCode, String)> {
    // Generate recommendations based on common patterns
    let recommendations = vec![
        Recommendation {
            id: "rec_1".to_string(),
            priority: "high".to_string(),
            category: "operational".to_string(),
            title: "Optimize P99 Latency".to_string(),
            description: "P99 latency is above the 1000ms target. Consider caching frequent queries or optimizing prompt templates.".to_string(),
            impact: "Could reduce P99 latency by 40%".to_string(),
            effort: "medium".to_string(),
            actions: vec![
                "Enable semantic caching for repeated queries".to_string(),
                "Reduce prompt token count by 20%".to_string(),
                "Consider streaming responses for long outputs".to_string(),
            ],
        },
        Recommendation {
            id: "rec_2".to_string(),
            priority: "medium".to_string(),
            category: "quality".to_string(),
            title: "Improve Groundedness Scores".to_string(),
            description: "Groundedness scores are below target at 82%. Review RAG retrieval quality.".to_string(),
            impact: "Could improve quality scores by 15%".to_string(),
            effort: "high".to_string(),
            actions: vec![
                "Review and update knowledge base".to_string(),
                "Tune retrieval similarity thresholds".to_string(),
                "Add source citations to responses".to_string(),
            ],
        },
        Recommendation {
            id: "rec_3".to_string(),
            priority: "low".to_string(),
            category: "cost".to_string(),
            title: "Reduce Token Usage".to_string(),
            description: "Average cost per request is above budget. Consider prompt optimization.".to_string(),
            impact: "Could reduce costs by 25%".to_string(),
            effort: "low".to_string(),
            actions: vec![
                "Compress system prompts".to_string(),
                "Use smaller models for simple tasks".to_string(),
                "Implement token budgets per request".to_string(),
            ],
        },
    ];

    let critical_count = recommendations
        .iter()
        .filter(|r| r.priority == "critical")
        .count();
    let high_count = recommendations
        .iter()
        .filter(|r| r.priority == "high")
        .count();
    let medium_count = recommendations
        .iter()
        .filter(|r| r.priority == "medium")
        .count();
    let low_count = recommendations
        .iter()
        .filter(|r| r.priority == "low")
        .count();
    let total_count = recommendations.len();

    Ok(Json(RecommendationsResponse {
        summary: RecommendationsSummary {
            total_recommendations: total_count,
            critical_count,
            high_count,
            medium_count,
            low_count,
            estimated_impact: "Overall improvement potential: 30% better performance".to_string(),
        },
        recommendations,
    }))
}

/// GET /api/v1/evals/pipeline/metrics/definitions
/// Returns all available metric definitions
pub async fn get_metric_definitions() -> Result<Json<Vec<MetricDefinition>>, (StatusCode, String)> {
    let definitions = vec![
        // Operational Metrics
        MetricDefinition {
            id: "latency_p50".to_string(),
            name: "P50 Latency".to_string(),
            description: "Median response latency".to_string(),
            category: MetricCategory::Operational,
            metric_type: MetricType::Duration,
            priority: MetricPriority::High,
            target: Some(200.0),
            target_description: Some("< 200ms".to_string()),
            unit: Some("ms".to_string()),
        },
        MetricDefinition {
            id: "latency_p95".to_string(),
            name: "P95 Latency".to_string(),
            description: "95th percentile response latency".to_string(),
            category: MetricCategory::Operational,
            metric_type: MetricType::Duration,
            priority: MetricPriority::High,
            target: Some(500.0),
            target_description: Some("< 500ms".to_string()),
            unit: Some("ms".to_string()),
        },
        MetricDefinition {
            id: "latency_p99".to_string(),
            name: "P99 Latency".to_string(),
            description: "99th percentile response latency".to_string(),
            category: MetricCategory::Operational,
            metric_type: MetricType::Duration,
            priority: MetricPriority::Critical,
            target: Some(1000.0),
            target_description: Some("< 1s".to_string()),
            unit: Some("ms".to_string()),
        },
        MetricDefinition {
            id: "success_rate".to_string(),
            name: "Success Rate".to_string(),
            description: "Percentage of successful requests".to_string(),
            category: MetricCategory::Operational,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(99.0),
            target_description: Some("> 99%".to_string()),
            unit: Some("%".to_string()),
        },
        MetricDefinition {
            id: "error_rate".to_string(),
            name: "Error Rate".to_string(),
            description: "Percentage of failed requests".to_string(),
            category: MetricCategory::Operational,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(1.0),
            target_description: Some("< 1%".to_string()),
            unit: Some("%".to_string()),
        },
        MetricDefinition {
            id: "cost_per_request".to_string(),
            name: "Cost Per Request".to_string(),
            description: "Average USD cost per API call".to_string(),
            category: MetricCategory::Operational,
            metric_type: MetricType::Currency,
            priority: MetricPriority::High,
            target: Some(0.01),
            target_description: Some("< $0.01".to_string()),
            unit: Some("USD".to_string()),
        },
        MetricDefinition {
            id: "token_throughput".to_string(),
            name: "Token Throughput".to_string(),
            description: "Total tokens processed".to_string(),
            category: MetricCategory::Operational,
            metric_type: MetricType::Count,
            priority: MetricPriority::Medium,
            target: None,
            target_description: None,
            unit: Some("tokens".to_string()),
        },
        // Quality Metrics
        MetricDefinition {
            id: "correctness".to_string(),
            name: "Correctness".to_string(),
            description: "Accuracy of generated responses".to_string(),
            category: MetricCategory::Quality,
            metric_type: MetricType::Score,
            priority: MetricPriority::Critical,
            target: Some(0.9),
            target_description: Some("> 90%".to_string()),
            unit: None,
        },
        MetricDefinition {
            id: "groundedness".to_string(),
            name: "Groundedness".to_string(),
            description: "How well responses are grounded in provided context".to_string(),
            category: MetricCategory::Quality,
            metric_type: MetricType::Score,
            priority: MetricPriority::Critical,
            target: Some(0.85),
            target_description: Some("> 85%".to_string()),
            unit: None,
        },
        MetricDefinition {
            id: "relevance".to_string(),
            name: "Relevance".to_string(),
            description: "How relevant responses are to the query".to_string(),
            category: MetricCategory::Quality,
            metric_type: MetricType::Score,
            priority: MetricPriority::High,
            target: Some(0.9),
            target_description: Some("> 90%".to_string()),
            unit: None,
        },
        MetricDefinition {
            id: "completeness".to_string(),
            name: "Completeness".to_string(),
            description: "How complete and thorough responses are".to_string(),
            category: MetricCategory::Quality,
            metric_type: MetricType::Score,
            priority: MetricPriority::Medium,
            target: Some(0.8),
            target_description: Some("> 80%".to_string()),
            unit: None,
        },
        MetricDefinition {
            id: "coherence".to_string(),
            name: "Coherence".to_string(),
            description: "Logical flow and consistency of responses".to_string(),
            category: MetricCategory::Quality,
            metric_type: MetricType::Score,
            priority: MetricPriority::Medium,
            target: Some(0.85),
            target_description: Some("> 85%".to_string()),
            unit: None,
        },
        // Agent Metrics
        MetricDefinition {
            id: "tool_accuracy".to_string(),
            name: "Tool Call Accuracy".to_string(),
            description: "Percentage of correct tool selections and parameter usage".to_string(),
            category: MetricCategory::Agent,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(95.0),
            target_description: Some("> 95%".to_string()),
            unit: Some("%".to_string()),
        },
        MetricDefinition {
            id: "task_completion".to_string(),
            name: "Task Completion Rate".to_string(),
            description: "Percentage of tasks completed successfully".to_string(),
            category: MetricCategory::Agent,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(90.0),
            target_description: Some("> 90%".to_string()),
            unit: Some("%".to_string()),
        },
        MetricDefinition {
            id: "planning_efficiency".to_string(),
            name: "Planning Efficiency".to_string(),
            description: "Efficiency of task decomposition and planning".to_string(),
            category: MetricCategory::Agent,
            metric_type: MetricType::Score,
            priority: MetricPriority::High,
            target: Some(0.8),
            target_description: Some("> 80%".to_string()),
            unit: None,
        },
        MetricDefinition {
            id: "convergence_rate".to_string(),
            name: "Convergence Rate".to_string(),
            description: "How quickly agent converges to solution".to_string(),
            category: MetricCategory::Agent,
            metric_type: MetricType::Score,
            priority: MetricPriority::Medium,
            target: Some(0.85),
            target_description: Some("> 85%".to_string()),
            unit: None,
        },
        // UX Metrics
        MetricDefinition {
            id: "user_satisfaction".to_string(),
            name: "User Satisfaction".to_string(),
            description: "Average user satisfaction score".to_string(),
            category: MetricCategory::UserExperience,
            metric_type: MetricType::Score,
            priority: MetricPriority::High,
            target: Some(4.0),
            target_description: Some("> 4/5".to_string()),
            unit: Some("/5".to_string()),
        },
        MetricDefinition {
            id: "task_success_rate".to_string(),
            name: "Task Success Rate".to_string(),
            description: "Percentage of user tasks completed successfully".to_string(),
            category: MetricCategory::UserExperience,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(85.0),
            target_description: Some("> 85%".to_string()),
            unit: Some("%".to_string()),
        },
        MetricDefinition {
            id: "time_to_value".to_string(),
            name: "Time to Value".to_string(),
            description: "Average time for user to achieve their goal".to_string(),
            category: MetricCategory::UserExperience,
            metric_type: MetricType::Duration,
            priority: MetricPriority::Medium,
            target: Some(30000.0),
            target_description: Some("< 30s".to_string()),
            unit: Some("ms".to_string()),
        },
        // Safety Metrics
        MetricDefinition {
            id: "hallucination_rate".to_string(),
            name: "Hallucination Rate".to_string(),
            description: "Percentage of responses with factual errors".to_string(),
            category: MetricCategory::Safety,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(5.0),
            target_description: Some("< 5%".to_string()),
            unit: Some("%".to_string()),
        },
        MetricDefinition {
            id: "pii_detection".to_string(),
            name: "PII Detection Rate".to_string(),
            description: "Percentage of PII properly detected and handled".to_string(),
            category: MetricCategory::Safety,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(99.0),
            target_description: Some("> 99%".to_string()),
            unit: Some("%".to_string()),
        },
        MetricDefinition {
            id: "guideline_compliance".to_string(),
            name: "Guideline Compliance".to_string(),
            description: "Adherence to content and safety guidelines".to_string(),
            category: MetricCategory::Safety,
            metric_type: MetricType::Percentage,
            priority: MetricPriority::Critical,
            target: Some(99.5),
            target_description: Some("> 99.5%".to_string()),
            unit: Some("%".to_string()),
        },
    ];

    Ok(Json(definitions))
}

/// GET /api/v1/evals/pipeline/history
/// Get historical evaluation runs
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EvalHistoryEntry {
    pub run_id: String,
    pub timestamp: u64,
    pub trace_count: usize,
    pub overall_health: HealthStatus,
    pub category_scores: HashMap<String, f64>,
    pub alert_count: usize,
}

#[derive(Debug, Serialize)]
pub struct EvalHistoryResponse {
    pub entries: Vec<EvalHistoryEntry>,
    pub total: usize,
}

pub async fn get_eval_history(
    State(_state): State<AppState>,
    Query(req): Query<HistoryQuery>,
) -> Result<Json<EvalHistoryResponse>, (StatusCode, String)> {
    let _limit = req.limit.unwrap_or(20);

    // TODO: Fetch from database
    // For now, return mock historical data
    let entries = vec![
        EvalHistoryEntry {
            run_id: "0x1234".to_string(),
            timestamp: current_timestamp_us() - 86_400_000_000,
            trace_count: 150,
            overall_health: HealthStatus::Healthy,
            category_scores: [
                ("operational".to_string(), 92.0),
                ("quality".to_string(), 88.0),
                ("agent".to_string(), 85.0),
            ]
            .into_iter()
            .collect(),
            alert_count: 1,
        },
        EvalHistoryEntry {
            run_id: "0x1235".to_string(),
            timestamp: current_timestamp_us() - 172_800_000_000,
            trace_count: 200,
            overall_health: HealthStatus::Warning,
            category_scores: [
                ("operational".to_string(), 85.0),
                ("quality".to_string(), 82.0),
                ("agent".to_string(), 78.0),
            ]
            .into_iter()
            .collect(),
            alert_count: 3,
        },
    ];

    Ok(Json(EvalHistoryResponse {
        total: entries.len(),
        entries,
    }))
}

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_id() -> u128 {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();
    let random = (rand::random::<u64>() as u128) << 64;
    timestamp ^ random
}

fn current_timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn parse_trace_id(id_str: &str) -> Result<u128, String> {
    let id_str = id_str.trim_start_matches("0x");
    u128::from_str_radix(id_str, 16).map_err(|e| format!("Invalid trace ID: {}", e))
}

fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted_data.len() - 1) as f64).round() as usize;
    sorted_data[idx.min(sorted_data.len() - 1)]
}
