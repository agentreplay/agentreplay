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

// flowtrace-server/src/api/budget_alerts.rs
//
// Budget alerts and cost monitoring API endpoints

use super::query::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// Data Models - Use core types with API enums
// ============================================================================

use flowtrace_core::enterprise::{
    AlertAction as CoreAlertAction, AlertFilters as CoreAlertFilters,
    BudgetAlert as CoreBudgetAlert,
};

// API representation with typed enums
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub id: u128,
    pub name: String,
    pub description: String,
    pub threshold_type: ThresholdType,
    pub threshold_value: f64,
    pub period: Period,
    pub filters: AlertFilters,
    pub actions: Vec<AlertAction>,
    pub status: AlertStatus,
    pub triggered_count: u32,
    pub last_triggered: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ThresholdType {
    Total,    // Total cost
    Daily,    // Daily cost
    Hourly,   // Hourly cost
    PerTrace, // Average per trace
    PerToken, // Cost per token
}

impl ThresholdType {
    pub fn as_str(&self) -> &str {
        match self {
            ThresholdType::Total => "total",
            ThresholdType::Daily => "daily",
            ThresholdType::Hourly => "hourly",
            ThresholdType::PerTrace => "per_trace",
            ThresholdType::PerToken => "per_token",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "total" => ThresholdType::Total,
            "daily" => ThresholdType::Daily,
            "hourly" => ThresholdType::Hourly,
            "per_trace" => ThresholdType::PerTrace,
            "per_token" => ThresholdType::PerToken,
            _ => ThresholdType::Total,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Hour,
    Day,
    Week,
    Month,
    AllTime,
}

impl Period {
    pub fn as_str(&self) -> &str {
        match self {
            Period::Hour => "hour",
            Period::Day => "day",
            Period::Week => "week",
            Period::Month => "month",
            Period::AllTime => "all_time",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "hour" => Period::Hour,
            "day" => Period::Day,
            "week" => Period::Week,
            "month" => Period::Month,
            "all_time" | "alltime" => Period::AllTime,
            _ => Period::Day,
        }
    }

    pub fn to_microseconds(&self) -> Option<u64> {
        match self {
            Period::Hour => Some(3_600_000_000),
            Period::Day => Some(86_400_000_000),
            Period::Week => Some(604_800_000_000),
            Period::Month => Some(2_592_000_000_000), // 30 days
            Period::AllTime => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertFilters {
    #[serde(default)]
    pub project_ids: Vec<u16>,
    #[serde(default)]
    pub agent_ids: Vec<u64>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub environments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertAction {
    pub action_type: ActionType,
    pub config: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    Email,
    Webhook,
    Slack,
    Log,
    PauseTracing,
}

impl ActionType {
    pub fn as_str(&self) -> &str {
        match self {
            ActionType::Email => "email",
            ActionType::Webhook => "webhook",
            ActionType::Slack => "slack",
            ActionType::Log => "log",
            ActionType::PauseTracing => "pause_tracing",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "email" => ActionType::Email,
            "webhook" => ActionType::Webhook,
            "slack" => ActionType::Slack,
            "log" => ActionType::Log,
            "pause_tracing" => ActionType::PauseTracing,
            _ => ActionType::Log,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlertStatus {
    Active,
    Paused,
    Disabled,
}

impl AlertStatus {
    pub fn as_str(&self) -> &str {
        match self {
            AlertStatus::Active => "active",
            AlertStatus::Paused => "paused",
            AlertStatus::Disabled => "disabled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" => AlertStatus::Active,
            "paused" => AlertStatus::Paused,
            "disabled" => AlertStatus::Disabled,
            _ => AlertStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub alert_id: u128,
    pub triggered_at: u64,
    pub actual_value: f64,
    pub threshold_value: f64,
    pub message: String,
}

// Conversion from API type to Core type
impl From<BudgetAlert> for CoreBudgetAlert {
    fn from(alert: BudgetAlert) -> Self {
        CoreBudgetAlert {
            id: alert.id,
            name: alert.name,
            description: alert.description,
            threshold_type: alert.threshold_type.as_str().to_string(),
            threshold_value: alert.threshold_value,
            period: alert.period.as_str().to_string(),
            filters: CoreAlertFilters {
                project_ids: alert.filters.project_ids,
                agent_ids: alert.filters.agent_ids,
                models: alert.filters.models,
                environments: alert.filters.environments,
            },
            actions: alert
                .actions
                .into_iter()
                .map(|a| CoreAlertAction {
                    action_type: a.action_type.as_str().to_string(),
                    config: a.config,
                })
                .collect(),
            status: alert.status.as_str().to_string(),
            triggered_count: alert.triggered_count,
            last_triggered: alert.last_triggered,
            created_at: alert.created_at,
            updated_at: alert.updated_at,
        }
    }
}

// Conversion from Core type to API type
impl From<CoreBudgetAlert> for BudgetAlert {
    fn from(core: CoreBudgetAlert) -> Self {
        BudgetAlert {
            id: core.id,
            name: core.name,
            description: core.description,
            threshold_type: ThresholdType::parse(&core.threshold_type),
            threshold_value: core.threshold_value,
            period: Period::parse(&core.period),
            filters: AlertFilters {
                project_ids: core.filters.project_ids,
                agent_ids: core.filters.agent_ids,
                models: core.filters.models,
                environments: core.filters.environments,
            },
            actions: core
                .actions
                .into_iter()
                .map(|a| AlertAction {
                    action_type: ActionType::parse(&a.action_type),
                    config: a.config,
                })
                .collect(),
            status: AlertStatus::parse(&core.status),
            triggered_count: core.triggered_count,
            last_triggered: core.last_triggered,
            created_at: core.created_at,
            updated_at: core.updated_at,
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateAlertRequest {
    pub name: String,
    pub description: String,
    pub threshold_type: ThresholdType,
    pub threshold_value: f64,
    pub period: Period,
    #[serde(default)]
    pub filters: AlertFilters,
    #[serde(default)]
    pub actions: Vec<AlertAction>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlertRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub threshold_value: Option<f64>,
    pub status: Option<AlertStatus>,
}

#[derive(Debug, Deserialize)]
pub struct ListAlertsQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AlertResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub threshold_type: String,
    pub threshold_value: f64,
    pub period: String,
    pub filters: AlertFilters,
    pub actions: Vec<AlertAction>,
    pub status: String,
    pub triggered_count: u32,
    pub last_triggered: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize)]
pub struct AlertListResponse {
    pub alerts: Vec<AlertResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct AlertEventResponse {
    pub alert_id: String,
    pub alert_name: String,
    pub triggered_at: u64,
    pub actual_value: f64,
    pub threshold_value: f64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AlertEventsResponse {
    pub events: Vec<AlertEventResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct BudgetStatusResponse {
    pub period: String,
    pub total_cost: f64,
    pub trace_count: usize,
    pub average_cost_per_trace: f64,
    pub token_count: u64,
    pub alerts_triggered: usize,
    pub active_alerts: usize,
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

fn alert_to_response(alert: &BudgetAlert) -> AlertResponse {
    AlertResponse {
        id: format!("0x{:x}", alert.id),
        name: alert.name.clone(),
        description: alert.description.clone(),
        threshold_type: alert.threshold_type.as_str().to_string(),
        threshold_value: alert.threshold_value,
        period: alert.period.as_str().to_string(),
        filters: alert.filters.clone(),
        actions: alert.actions.clone(),
        status: alert.status.as_str().to_string(),
        triggered_count: alert.triggered_count,
        last_triggered: alert.last_triggered,
        created_at: alert.created_at,
        updated_at: alert.updated_at,
    }
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/v1/budget/alerts
/// Create a new budget alert
pub async fn create_alert(
    State(state): State<AppState>,
    Json(req): Json<CreateAlertRequest>,
) -> Result<(StatusCode, Json<AlertResponse>), (StatusCode, String)> {
    let alert_id = generate_id();
    let timestamp = current_timestamp_us();

    let alert = BudgetAlert {
        id: alert_id,
        name: req.name,
        description: req.description,
        threshold_type: req.threshold_type,
        threshold_value: req.threshold_value,
        period: req.period,
        filters: req.filters,
        actions: req.actions,
        status: AlertStatus::Active,
        triggered_count: 0,
        last_triggered: None,
        created_at: timestamp,
        updated_at: timestamp,
    };

    // Convert API type to Core type for storage
    let core_alert: CoreBudgetAlert = alert.clone().into();
    state
        .db
        .store_budget_alert(core_alert)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(alert_to_response(&alert))))
}

/// GET /api/v1/budget/alerts
/// List all budget alerts
pub async fn list_alerts(
    State(state): State<AppState>,
    Query(params): Query<ListAlertsQuery>,
) -> Result<Json<AlertListResponse>, (StatusCode, String)> {
    let core_alerts = state
        .db
        .list_budget_alerts()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Convert from Core to API types
    let mut alerts: Vec<BudgetAlert> = core_alerts.into_iter().map(|a| a.into()).collect();

    // Filter by status if provided
    if let Some(status_str) = params.status {
        alerts.retain(|a| a.status.as_str() == status_str.to_lowercase());
    }

    let total = alerts.len();
    let alert_responses: Vec<AlertResponse> = alerts.iter().map(alert_to_response).collect();

    Ok(Json(AlertListResponse {
        alerts: alert_responses,
        total,
    }))
}

/// GET /api/v1/budget/alerts/:id
/// Get a specific budget alert
pub async fn get_alert(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AlertResponse>, (StatusCode, String)> {
    let alert_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let core_alert = state
        .db
        .get_budget_alert(alert_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Alert not found".to_string()))?;

    let alert: BudgetAlert = core_alert.into();
    Ok(Json(alert_to_response(&alert)))
}

/// PUT /api/v1/budget/alerts/:id
/// Update a budget alert
pub async fn update_alert(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateAlertRequest>,
) -> Result<Json<AlertResponse>, (StatusCode, String)> {
    let alert_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let timestamp = current_timestamp_us();

    state
        .db
        .update_budget_alert(alert_id, |alert| {
            if let Some(name) = req.name {
                alert.name = name;
            }
            if let Some(description) = req.description {
                alert.description = description;
            }
            if let Some(threshold_value) = req.threshold_value {
                alert.threshold_value = threshold_value;
            }
            if let Some(status) = req.status {
                alert.status = status.as_str().to_string();
            }
            alert.updated_at = timestamp;
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch updated alert
    let core_alert = state
        .db
        .get_budget_alert(alert_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Alert not found".to_string()))?;

    let alert: BudgetAlert = core_alert.into();
    Ok(Json(alert_to_response(&alert)))
}

/// GET /api/v1/budget/alerts/:id/events
/// Get alert events history
pub async fn get_alert_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AlertEventsResponse>, (StatusCode, String)> {
    let alert_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let core_alert = state
        .db
        .get_budget_alert(alert_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Alert not found".to_string()))?;

    let alert: BudgetAlert = core_alert.into();

    let events = state
        .db
        .get_alert_events(alert_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = events.len();
    let event_responses: Vec<AlertEventResponse> = events
        .into_iter()
        .map(|e| AlertEventResponse {
            alert_id: format!("0x{:x}", e.alert_id),
            alert_name: alert.name.clone(),
            triggered_at: e.triggered_at,
            actual_value: e.actual_value,
            threshold_value: e.threshold_value,
            message: e.message,
        })
        .collect();

    Ok(Json(AlertEventsResponse {
        events: event_responses,
        total,
    }))
}

/// GET /api/v1/budget/status
/// Get current budget status
pub async fn get_budget_status(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<BudgetStatusResponse>, (StatusCode, String)> {
    let period_str = params.get("period").map(|s| s.as_str()).unwrap_or("day");
    let period = match period_str {
        "hour" => Period::Hour,
        "day" => Period::Day,
        "week" => Period::Week,
        "month" => Period::Month,
        _ => Period::Day,
    };

    let current_time = current_timestamp_us();
    let _start_time = if let Some(period_us) = period.to_microseconds() {
        current_time.saturating_sub(period_us)
    } else {
        0
    };

    // Get real-time cost stats from CostTracker (Task 10)
    let cost_summary = state.cost_tracker.get_summary().await;

    // Filter costs for the requested period
    let total_cost = if period == Period::AllTime {
        cost_summary.total_cost
    } else {
        // TODO: Filter by time period using hourly_costs or daily_costs
        cost_summary.total_cost
    };

    let core_alerts = state
        .db
        .list_budget_alerts()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let alerts: Vec<BudgetAlert> = core_alerts.into_iter().map(|a| a.into()).collect();

    let active_alerts = alerts
        .iter()
        .filter(|a| a.status == AlertStatus::Active)
        .count();
    let alerts_triggered = alerts.iter().filter(|a| a.triggered_count > 0).count();

    // Convert Decimal to f64 for JSON serialization
    let total_cost_f64 = total_cost.to_string().parse::<f64>().unwrap_or(0.0);

    Ok(Json(BudgetStatusResponse {
        period: period.as_str().to_string(),
        total_cost: total_cost_f64,
        trace_count: cost_summary.trace_count as usize,
        average_cost_per_trace: if cost_summary.trace_count > 0 {
            total_cost_f64 / cost_summary.trace_count as f64
        } else {
            0.0
        },
        token_count: cost_summary.total_tokens,
        alerts_triggered,
        active_alerts,
    }))
}

/// DELETE /api/v1/budget/alerts/:id
/// Delete a budget alert
pub async fn delete_alert(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, String)> {
    let alert_id = parse_id(&id).map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let deleted = state
        .db
        .delete_budget_alert(alert_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(DeleteResponse {
            success: true,
            message: "Alert deleted successfully".to_string(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Alert not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_period_to_microseconds() {
        assert_eq!(Period::Hour.to_microseconds(), Some(3_600_000_000));
        assert_eq!(Period::Day.to_microseconds(), Some(86_400_000_000));
        assert_eq!(Period::AllTime.to_microseconds(), None);
    }

    #[test]
    fn test_threshold_type_as_str() {
        assert_eq!(ThresholdType::Total.as_str(), "total");
        assert_eq!(ThresholdType::PerTrace.as_str(), "per_trace");
    }
}
