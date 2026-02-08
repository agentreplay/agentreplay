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
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

use crate::api::query::{ApiError, AppState};
use crate::otel_genai::{GenAIPayload, ModelPricing};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CostBreakdownQuery {
    pub start_ts: u64,
    pub end_ts: u64,
    #[serde(default)]
    pub group_by: Vec<String>, // ["provider", "model", "route", "user", "project"]
}

#[derive(Debug, Serialize)]
pub struct CostBreakdownResponse {
    pub total_cost: f64,
    pub currency: String,
    pub breakdown: Vec<CostGroup>,
    pub forecast_30d: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct CostGroup {
    pub group_key: String,
    pub cost: f64,
    pub percentage: f64,
    pub token_count: u64,
    pub request_count: u64,
    pub avg_cost_per_request: f64,
}

#[derive(Debug, Serialize)]
pub struct ProviderCostResponse {
    pub providers: Vec<ProviderCost>,
}

#[derive(Debug, Serialize)]
pub struct ProviderCost {
    pub provider: String,
    pub total_cost: f64,
    pub total_tokens: u64,
    pub request_count: u64,
    pub models: Vec<ModelCost>,
    pub avg_latency_ms: f64,
    pub error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct ModelCost {
    pub model: String,
    pub cost: f64,
    pub tokens: u64,
    pub requests: u64,
}

// ============================================================================
// API Handlers
// ============================================================================

/// GET /api/v1/analytics/cost/breakdown
/// Get detailed cost breakdown
pub async fn get_detailed_cost_breakdown(
    State(state): State<AppState>,
    Query(params): Query<CostBreakdownQuery>,
) -> Result<Json<CostBreakdownResponse>, ApiError> {
    debug!(
        "Getting cost breakdown from {} to {}",
        params.start_ts, params.end_ts
    );

    // Query edges in range
    let edges = state
        .db
        .query_temporal_range(params.start_ts, params.end_ts)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut groups: HashMap<String, CostGroup> = HashMap::new();
    let mut total_cost = 0.0;

    for edge in edges {
        // Calculate cost for edge
        // Try to get payload for accurate cost
        let mut cost = 0.0;
        let mut tokens = edge.token_count as u64;
        let mut model = "unknown".to_string();
        let mut provider = "unknown".to_string();

        if let Ok(Some(payload_bytes)) = state.db.get_payload(edge.edge_id) {
            if let Ok(genai) = serde_json::from_slice::<GenAIPayload>(&payload_bytes) {
                let system = genai
                    .system
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                let m = genai
                    .response_model
                    .clone()
                    .or_else(|| genai.request_model.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                let pricing = ModelPricing::for_model(&system, &m);
                cost = genai.calculate_cost(&pricing);
                model = m;
                provider = system;
                tokens =
                    (genai.input_tokens.unwrap_or(0) + genai.output_tokens.unwrap_or(0)) as u64;
            }
        }

        total_cost += cost;

        // Determine group key
        let key = if params.group_by.contains(&"provider".to_string()) {
            provider
        } else if params.group_by.contains(&"model".to_string()) {
            model
        } else if params.group_by.contains(&"project".to_string()) {
            format!("project_{}", edge.project_id)
        } else {
            "all".to_string()
        };

        let entry = groups.entry(key.clone()).or_insert(CostGroup {
            group_key: key,
            cost: 0.0,
            percentage: 0.0,
            token_count: 0,
            request_count: 0,
            avg_cost_per_request: 0.0,
        });

        entry.cost += cost;
        entry.token_count += tokens;
        entry.request_count += 1;
    }

    // Calculate percentages and averages
    let mut breakdown: Vec<CostGroup> = groups.into_values().collect();
    for group in &mut breakdown {
        if total_cost > 0.0 {
            group.percentage = (group.cost / total_cost) * 100.0;
        }
        if group.request_count > 0 {
            group.avg_cost_per_request = group.cost / group.request_count as f64;
        }
    }

    // Sort by cost desc
    breakdown.sort_by(|a, b| {
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Json(CostBreakdownResponse {
        total_cost,
        currency: "USD".to_string(),
        breakdown,
        forecast_30d: Some(total_cost * 30.0), // Naive forecast
    }))
}

/// GET /api/v1/analytics/cost/providers
/// Get cost breakdown by provider
pub async fn get_provider_costs(
    State(state): State<AppState>,
) -> Result<Json<ProviderCostResponse>, ApiError> {
    // Default to last 30 days
    let end_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    let start_ts = end_ts - (30 * 86_400_000_000);

    let edges = state
        .db
        .query_temporal_range(start_ts, end_ts)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut provider_map: HashMap<String, ProviderCost> = HashMap::new();

    for edge in edges {
        let mut cost = 0.0;
        let mut tokens = edge.token_count as u64;
        let mut provider_name = "unknown".to_string();
        let latency = edge.duration_us as f64 / 1000.0;
        let is_error = edge.get_span_type() == agentreplay_core::SpanType::Error;

        if let Ok(Some(payload_bytes)) = state.db.get_payload(edge.edge_id) {
            if let Ok(genai) = serde_json::from_slice::<GenAIPayload>(&payload_bytes) {
                let system = genai
                    .system
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                let m = genai
                    .response_model
                    .clone()
                    .or_else(|| genai.request_model.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                let pricing = ModelPricing::for_model(&system, &m);
                cost = genai.calculate_cost(&pricing);
                provider_name = system;
                tokens =
                    (genai.input_tokens.unwrap_or(0) + genai.output_tokens.unwrap_or(0)) as u64;
            }
        }

        if provider_name == "unknown" {
            continue;
        }

        let entry = provider_map
            .entry(provider_name.clone())
            .or_insert(ProviderCost {
                provider: provider_name.clone(),
                total_cost: 0.0,
                total_tokens: 0,
                request_count: 0,
                models: Vec::new(),
                avg_latency_ms: 0.0,
                error_rate: 0.0,
            });

        entry.total_cost += cost;
        entry.total_tokens += tokens;
        entry.request_count += 1;
        entry.avg_latency_ms += latency; // Accumulate for now
        if is_error {
            entry.error_rate += 1.0; // Accumulate errors
        }

        // Track model within provider (simplified)
        // In real impl, we'd need a nested map
    }

    // Finalize averages
    let mut providers: Vec<ProviderCost> = provider_map.into_values().collect();
    for p in &mut providers {
        if p.request_count > 0 {
            p.avg_latency_ms /= p.request_count as f64;
            p.error_rate = (p.error_rate / p.request_count as f64) * 100.0;
        }
    }

    Ok(Json(ProviderCostResponse { providers }))
}
