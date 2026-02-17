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

//! Bot Management REST API
//!
//! CRUD and activity endpoints for moltbot, clawdbot, and openclaw.

use crate::api::AppState;
use crate::bot_registry::{
    BotActivityEvent, BotInstance, BotRegistryStats, RegisterBotRequest, UpdateBotRequest,
};
use crate::api::skill_memory::BotKind;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ────────────────────────────────────────────────────────────────────────────
// Response types
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BotResponse {
    pub bot: BotInstance,
}

#[derive(Debug, Serialize)]
pub struct BotListResponse {
    pub bots: Vec<BotInstance>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct BotEventsResponse {
    pub events: Vec<BotActivityEvent>,
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    #[serde(default = "default_event_limit")]
    pub limit: usize,
}

fn default_event_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
pub struct BotListQuery {
    pub kind: Option<BotKind>,
}

// ────────────────────────────────────────────────────────────────────────────
// Router
// ────────────────────────────────────────────────────────────────────────────

pub fn bots_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_bots).post(register_bot))
        .route("/:bot_id", get(get_bot).put(update_bot).delete(delete_bot))
        .route("/:bot_id/events", get(get_bot_events))
        .route("/events", get(get_all_events))
        .route("/stats", get(get_bot_stats))
}

// ────────────────────────────────────────────────────────────────────────────
// Handlers
// ────────────────────────────────────────────────────────────────────────────

async fn list_bots(
    State(state): State<AppState>,
    Query(query): Query<BotListQuery>,
) -> Result<Json<BotListResponse>, StatusCode> {
    let registry = state
        .bot_registry_v2
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let bots: Vec<BotInstance> = if let Some(kind) = query.kind {
        registry.get_bot_by_kind(kind).await
    } else {
        registry.list_bots().await
    };
    let total = bots.len();
    Ok(Json(BotListResponse { bots, total }))
}

async fn register_bot(
    State(state): State<AppState>,
    Json(req): Json<RegisterBotRequest>,
) -> Result<(StatusCode, Json<BotResponse>), (StatusCode, Json<serde_json::Value>)> {
    let registry = state.bot_registry_v2.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Bot registry not initialized"})),
        )
    })?;

    let bot = BotInstance {
        bot_id: Uuid::new_v4().to_string(),
        kind: req.kind.clone(),
        name: req.name.clone(),
        description: req.description.unwrap_or_default(),
        version: req.version.unwrap_or_else(|| "1.0.0".to_string()),
        model: req.model.unwrap_or_else(|| match req.kind.as_str() {
            "moltbot" => "multi-model".to_string(),
            "clawdbot" => "claude-sonnet-4-20250514".to_string(),
            _ => "open-source".to_string(),
        }),
        status: crate::bot_registry::BotStatus::Online,
        config: req.config.unwrap_or_else(|| crate::bot_registry::BotConfig {
            max_concurrent_sessions: 5,
            token_budget: 50000,
            task_timeout_secs: 300,
            skill_sharing_enabled: true,
            accept_skills_from: vec!["moltbot".to_string(), "clawdbot".to_string(), "openclaw".to_string()],
            system_prompt: None,
            temperature: 0.3,
            tools: vec![],
        }),
        skill_ids: vec![],
        active_sessions: 0,
        tasks_completed: 0,
        total_tokens: 0,
        success_rate: 0.0,
        memory_namespace: format!("bot/{}", req.kind),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_active_at: None,
        metadata: req.metadata,
    };

    match registry.register_bot(bot).await {
        Ok(bot) => Ok((StatusCode::CREATED, Json(BotResponse { bot }))),
        Err(e) => Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn get_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<Json<BotResponse>, StatusCode> {
    let registry = state
        .bot_registry_v2
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    registry
        .get_bot(&bot_id)
        .await
        .map(|bot| Json(BotResponse { bot }))
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
    Json(req): Json<UpdateBotRequest>,
) -> Result<Json<BotResponse>, (StatusCode, Json<serde_json::Value>)> {
    let registry = state.bot_registry_v2.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Bot registry not initialized"})),
        )
    })?;
    match registry.update_bot(&bot_id, req).await {
        Ok(bot) => Ok(Json(BotResponse { bot })),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn delete_bot(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let registry = state.bot_registry_v2.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Bot registry not initialized"})),
        )
    })?;
    match registry.delete_bot(&bot_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn get_bot_events(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Result<Json<BotEventsResponse>, StatusCode> {
    let registry = state
        .bot_registry_v2
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let events = registry.get_events(Some(&bot_id), query.limit).await;
    Ok(Json(BotEventsResponse { events }))
}

async fn get_all_events(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> Result<Json<BotEventsResponse>, StatusCode> {
    let registry = state
        .bot_registry_v2
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let events = registry.get_events(None, query.limit).await;
    Ok(Json(BotEventsResponse { events }))
}

async fn get_bot_stats(
    State(state): State<AppState>,
) -> Result<Json<BotRegistryStats>, StatusCode> {
    let registry = state
        .bot_registry_v2
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(registry.get_bot_stats().await))
}
