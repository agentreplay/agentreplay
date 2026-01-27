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

//! Dataset Flywheel API
//!
//! Implements the Dataset Flywheel pattern for auto-curating fine-tuning data:
//! - τ_high (≥0.9): Traces with high eval scores → positive examples
//! - τ_low (≤0.3): Traces with low eval scores → negative examples
//! - Export to JSONL format for LLM fine-tuning

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::{build_eval_trace_v1, AppState};
use flowtrace_core::{ContentPartV1, MessageV1};

/// Query parameters for getting flywheel candidates
#[derive(Debug, Deserialize)]
pub struct CandidatesQuery {
    #[serde(default = "default_positive_threshold")]
    pub positive_threshold: f64,
    #[serde(default = "default_negative_threshold")]
    pub negative_threshold: f64,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub project_id: Option<i64>,
}

fn default_positive_threshold() -> f64 {
    0.9
}
fn default_negative_threshold() -> f64 {
    0.3
}
fn default_limit() -> usize {
    100
}

/// A candidate trace for fine-tuning
#[derive(Debug, Serialize)]
pub struct FlywheelCandidate {
    pub trace_id: String,
    pub score: f64,
    pub timestamp_us: u64,
    pub has_payload: bool,
    pub input: Option<String>,
    pub output: Option<String>,
}

/// Response for flywheel candidates
#[derive(Debug, Serialize)]
pub struct CandidatesResponse {
    pub positive_candidates: Vec<FlywheelCandidate>,
    pub negative_candidates: Vec<FlywheelCandidate>,
    pub thresholds: ThresholdInfo,
}

#[derive(Debug, Serialize)]
pub struct ThresholdInfo {
    pub positive: f64,
    pub negative: f64,
}

/// Request for exporting fine-tuning dataset
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    #[serde(default = "default_positive_threshold")]
    pub positive_threshold: f64,
    #[serde(default = "default_negative_threshold")]
    pub negative_threshold: f64,
    #[serde(default = "default_max_examples")]
    pub max_examples: usize,
    #[serde(default = "default_export_format")]
    pub format: String,
    #[serde(default)]
    pub include_scores: bool,
}

fn default_max_examples() -> usize {
    1000
}

fn default_export_format() -> String {
    "flowtrace_evaltrace_v1".to_string()
}

/// Response for export request
#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub jsonl: String,
    pub positive_count: usize,
    pub negative_count: usize,
    pub total_examples: usize,
}

fn merge_message_text(message: &MessageV1) -> String {
    let mut text = String::new();
    for part in &message.content {
        match part {
            ContentPartV1::Text { text: chunk } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(chunk);
            }
            ContentPartV1::ToolUse { name, arguments, .. } => {
                text.push_str(&format!("\n[tool_use:{} {}]", name, arguments.clone().unwrap_or_default()));
            }
            ContentPartV1::ToolResult { tool_call_id, content } => {
                text.push_str(&format!("\n[tool_result:{} {}]", tool_call_id, content.clone().unwrap_or_default()));
            }
            ContentPartV1::Json { value } => {
                text.push_str(&format!("\n{}", value));
            }
        }
    }
    text
}

fn to_chatml(messages: &[MessageV1]) -> serde_json::Value {
    let formatted: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": merge_message_text(m)
            })
        })
        .collect();

    serde_json::json!({ "messages": formatted })
}

fn to_provider_messages(messages: &[MessageV1]) -> serde_json::Value {
    let formatted: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            let content: Vec<serde_json::Value> = m
                .content
                .iter()
                .map(|part| match part {
                    ContentPartV1::Text { text } => serde_json::json!({"type": "text", "text": text}),
                    ContentPartV1::ToolUse { id, name, arguments } => serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": arguments.clone().unwrap_or_default()
                    }),
                    ContentPartV1::ToolResult { tool_call_id, content } => serde_json::json!({
                        "type": "tool_result",
                        "tool_call_id": tool_call_id,
                        "content": content.clone().unwrap_or_default()
                    }),
                    ContentPartV1::Json { value } => serde_json::json!({"type": "json", "value": value}),
                })
                .collect();

            serde_json::json!({"role": m.role, "content": content})
        })
        .collect();

    serde_json::json!({ "messages": formatted })
}

fn to_instruction_tuning(messages: &[MessageV1]) -> serde_json::Value {
    let mut instruction = String::new();
    let mut output = String::new();

    for message in messages {
        if message.role == "user" && instruction.is_empty() {
            instruction = merge_message_text(message);
        }
        if message.role == "assistant" {
            output = merge_message_text(message);
        }
    }

    serde_json::json!({
        "instruction": instruction,
        "input": "",
        "output": output,
    })
}

fn export_example(
    format: &str,
    eval_trace: &flowtrace_core::EvalTraceV1,
    label: &str,
    score: f64,
) -> serde_json::Value {
    match format {
        "chatml" => {
            let mut base = to_chatml(&eval_trace.outcome.messages);
            base["label"] = serde_json::json!(label);
            base["score"] = serde_json::json!(score);
            base
        }
        "provider_messages" => {
            let mut base = to_provider_messages(&eval_trace.outcome.messages);
            base["label"] = serde_json::json!(label);
            base["score"] = serde_json::json!(score);
            base
        }
        "instruction_tuning" => {
            let mut base = to_instruction_tuning(&eval_trace.outcome.messages);
            base["label"] = serde_json::json!(label);
            base["score"] = serde_json::json!(score);
            base
        }
        _ => serde_json::json!({
            "schema_version": eval_trace.schema_version,
            "trace": eval_trace,
            "label": label,
            "score": score
        }),
    }
}
/// Get flywheel candidates based on eval scores
pub async fn get_candidates(
    State(state): State<AppState>,
    Query(query): Query<CandidatesQuery>,
) -> Json<CandidatesResponse> {
    let mut positive_candidates = Vec::new();
    let mut negative_candidates = Vec::new();

    // Get all traces with eval metrics
    // In production, this would query stored eval results
    // For now, we scan recent traces and check their eval_metrics

    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    // Scan last 7 days of traces
    let week_ago = now_us.saturating_sub(7 * 24 * 60 * 60 * 1_000_000);

    if let Ok(traces) = state.db.query_temporal_range(week_ago, now_us) {
        for trace in traces.into_iter().take(query.limit * 10) {
            // Check if trace has eval metrics
            if let Ok(metrics) = state.db.get_eval_metrics(trace.edge_id) {
                if !metrics.is_empty() {
                    // Calculate average score from metrics
                    let avg_score: f64 = metrics.iter().map(|m| m.metric_value).sum::<f64>()
                        / metrics.len().max(1) as f64;

                    let candidate = FlywheelCandidate {
                        trace_id: format!("{:032x}", trace.edge_id),
                        score: avg_score,
                        timestamp_us: trace.timestamp_us,
                        has_payload: trace.has_payload != 0,
                        input: None, // Payload stored separately, would need lookup
                        output: None,
                    };

                    if avg_score >= query.positive_threshold
                        && positive_candidates.len() < query.limit
                    {
                        positive_candidates.push(candidate);
                    } else if avg_score <= query.negative_threshold
                        && negative_candidates.len() < query.limit
                    {
                        negative_candidates.push(candidate);
                    }
                }
            }
        }
    }

    Json(CandidatesResponse {
        positive_candidates,
        negative_candidates,
        thresholds: ThresholdInfo {
            positive: query.positive_threshold,
            negative: query.negative_threshold,
        },
    })
}

/// Export fine-tuning dataset in JSONL format
pub async fn export_dataset(
    State(state): State<AppState>,
    Json(request): Json<ExportRequest>,
) -> Json<ExportResponse> {
    let mut jsonl_lines = Vec::new();
    let mut positive_count = 0;
    let mut negative_count = 0;

    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    // Scan last 30 days for more data
    let month_ago = now_us.saturating_sub(30 * 24 * 60 * 60 * 1_000_000);

    if let Ok(traces) = state.db.query_temporal_range(month_ago, now_us) {
        for trace in traces {
            if positive_count + negative_count >= request.max_examples {
                break;
            }

            // Get eval metrics for this trace
            if let Ok(metrics) = state.db.get_eval_metrics(trace.edge_id) {
                if metrics.is_empty() {
                    continue;
                }

                let avg_score: f64 = metrics.iter().map(|m| m.metric_value).sum::<f64>()
                    / metrics.len().max(1) as f64;

                // Determine if positive or negative example
                let label = if avg_score >= request.positive_threshold {
                    positive_count += 1;
                    "positive"
                } else if avg_score <= request.negative_threshold {
                    negative_count += 1;
                    "negative"
                } else {
                    continue; // Skip unlabeled zone
                };

                let eval_trace = build_eval_trace_v1(&state, &trace);
                let mut entry = export_example(&request.format, &eval_trace, label, avg_score);
                entry["trace_id"] = serde_json::json!(format!("{:032x}", trace.edge_id));
                entry["timestamp_us"] = serde_json::json!(trace.timestamp_us);

                // Optionally include eval scores
                if request.include_scores {
                    entry["eval_score"] = serde_json::json!(avg_score);
                    entry["eval_metrics"] = serde_json::json!(metrics
                        .iter()
                        .map(|m| {
                            let name = String::from_utf8_lossy(&m.metric_name)
                                .trim_end_matches('\0')
                                .to_string();
                            serde_json::json!({
                                "name": name,
                                "value": m.metric_value
                            })
                        })
                        .collect::<Vec<_>>());
                }

                if let Ok(line) = serde_json::to_string(&entry) {
                    jsonl_lines.push(line);
                }
            }
        }
    }

    let total_examples = positive_count + negative_count;

    Json(ExportResponse {
        jsonl: jsonl_lines.join("\n"),
        positive_count,
        negative_count,
        total_examples,
    })
}
