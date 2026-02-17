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

//! OpenClaw Observability API endpoints
//!
//! Provides REST endpoints for:
//!   • Aggregated openclaw metrics (tokens, costs, models, channels)
//!   • Session state monitoring
//!   • Queue depth & wait time metrics
//!   • Webhook & message processing stats
//!   • SKILL.md import
//!   • Activity event feed

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::api::AppState;
use crate::bot_registry::{
    BotActivityEvent, BotInstance, BotRegistryStats, UpdateBotRequest,
};
use crate::api::skill_memory::{BotKind, SkillMemoryStats};
use crate::openclaw_enrichment::{
    ChannelMetrics, ModelUsage, OpenclawEvent, OpenclawMetrics, QueueMetrics,
    SessionStateMetrics, WebhookMetrics, MessageMetrics,
};

/// Build the openclaw API router
pub fn openclaw_router() -> Router<AppState> {
    Router::new()
        // Observability metrics
        .route("/metrics", get(get_metrics))
        .route("/model-usage", get(get_model_usage))
        .route("/channels", get(get_channel_metrics))
        .route("/sessions", get(get_session_states))
        .route("/queue", get(get_queue_metrics))
        .route("/webhooks", get(get_webhook_metrics))
        .route("/messages", get(get_message_metrics))
        .route("/events", get(get_recent_events))
        // SKILL.md import
        .route("/skills/import", post(import_skill_md))
        .route("/skills/import-batch", post(import_skill_batch))
        // Agent management (proxy to bot_registry)
        .route("/agents", get(list_agents))
        .route("/agents/stats", get(get_agents_stats))
        .route("/agents/events", get(get_agents_events))
        .route("/agents/:bot_id", get(get_agent).put(update_agent))
        // Memory integration (proxy to skill_memory)
        .route("/memory/stats", get(get_memory_stats))
}

// ── Response Types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MetricsResponse {
    success: bool,
    data: OpenclawMetrics,
}

#[derive(Serialize)]
struct ModelUsageResponse {
    success: bool,
    models: Vec<ModelUsage>,
    total_models: usize,
}

#[derive(Serialize)]
struct ChannelResponse {
    success: bool,
    channels: Vec<ChannelMetrics>,
    total_channels: usize,
}

#[derive(Serialize)]
struct SessionResponse {
    success: bool,
    sessions: SessionStateMetrics,
}

#[derive(Serialize)]
struct QueueResponse {
    success: bool,
    queue: QueueMetrics,
}

#[derive(Serialize)]
struct WebhookResponse {
    success: bool,
    webhooks: WebhookMetrics,
}

#[derive(Serialize)]
struct MessageResponse {
    success: bool,
    messages: MessageMetrics,
}

#[derive(Serialize)]
struct EventsResponse {
    success: bool,
    events: Vec<OpenclawEvent>,
    total: usize,
}

#[derive(Deserialize)]
struct ImportSkillRequest {
    content: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct ImportBatchRequest {
    skills: Vec<ImportSkillRequest>,
}

#[derive(Serialize)]
struct ImportSkillResponse {
    success: bool,
    skill_id: Option<String>,
    skill_name: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ImportBatchResponse {
    success: bool,
    imported: usize,
    failed: usize,
    results: Vec<ImportSkillResponse>,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/v1/openclaw/metrics — Full aggregated metrics
async fn get_metrics(
    State(state): State<AppState>,
) -> (StatusCode, Json<MetricsResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(MetricsResponse {
                    success: false,
                    data: OpenclawMetrics::default(),
                }),
            );
        }
    };

    let data = enricher.get_metrics().await;
    (StatusCode::OK, Json(MetricsResponse { success: true, data }))
}

/// GET /api/v1/openclaw/model-usage — Per-model token & cost breakdown
async fn get_model_usage(
    State(state): State<AppState>,
) -> (StatusCode, Json<ModelUsageResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ModelUsageResponse {
                    success: false,
                    models: vec![],
                    total_models: 0,
                }),
            );
        }
    };

    let models = enricher.get_model_usage().await;
    let total_models = models.len();
    (
        StatusCode::OK,
        Json(ModelUsageResponse {
            success: true,
            models,
            total_models,
        }),
    )
}

/// GET /api/v1/openclaw/channels — Per-channel metrics
async fn get_channel_metrics(
    State(state): State<AppState>,
) -> (StatusCode, Json<ChannelResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ChannelResponse {
                    success: false,
                    channels: vec![],
                    total_channels: 0,
                }),
            );
        }
    };

    let channels = enricher.get_channel_metrics().await;
    let total_channels = channels.len();
    (
        StatusCode::OK,
        Json(ChannelResponse {
            success: true,
            channels,
            total_channels,
        }),
    )
}

/// GET /api/v1/openclaw/sessions — Session state overview
async fn get_session_states(
    State(state): State<AppState>,
) -> (StatusCode, Json<SessionResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(SessionResponse {
                    success: false,
                    sessions: SessionStateMetrics::default(),
                }),
            );
        }
    };

    let sessions = enricher.get_session_states().await;
    (
        StatusCode::OK,
        Json(SessionResponse {
            success: true,
            sessions,
        }),
    )
}

/// GET /api/v1/openclaw/queue — Queue depth & wait metrics
async fn get_queue_metrics(
    State(state): State<AppState>,
) -> (StatusCode, Json<QueueResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(QueueResponse {
                    success: false,
                    queue: QueueMetrics::default(),
                }),
            );
        }
    };

    let metrics = enricher.get_metrics().await;
    (
        StatusCode::OK,
        Json(QueueResponse {
            success: true,
            queue: metrics.queue_metrics,
        }),
    )
}

/// GET /api/v1/openclaw/webhooks — Webhook stats
async fn get_webhook_metrics(
    State(state): State<AppState>,
) -> (StatusCode, Json<WebhookResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(WebhookResponse {
                    success: false,
                    webhooks: WebhookMetrics::default(),
                }),
            );
        }
    };

    let metrics = enricher.get_metrics().await;
    (
        StatusCode::OK,
        Json(WebhookResponse {
            success: true,
            webhooks: metrics.webhook_metrics,
        }),
    )
}

/// GET /api/v1/openclaw/messages — Message processing stats
async fn get_message_metrics(
    State(state): State<AppState>,
) -> (StatusCode, Json<MessageResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(MessageResponse {
                    success: false,
                    messages: MessageMetrics::default(),
                }),
            );
        }
    };

    let metrics = enricher.get_metrics().await;
    (
        StatusCode::OK,
        Json(MessageResponse {
            success: true,
            messages: metrics.message_metrics,
        }),
    )
}

/// GET /api/v1/openclaw/events — Recent activity events
async fn get_recent_events(
    State(state): State<AppState>,
) -> (StatusCode, Json<EventsResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(EventsResponse {
                    success: false,
                    events: vec![],
                    total: 0,
                }),
            );
        }
    };

    let events = enricher.get_recent_events(100).await;
    let total = events.len();
    (
        StatusCode::OK,
        Json(EventsResponse {
            success: true,
            events,
            total,
        }),
    )
}

/// POST /api/v1/openclaw/skills/import — Import a single SKILL.md
async fn import_skill_md(
    State(state): State<AppState>,
    Json(req): Json<ImportSkillRequest>,
) -> (StatusCode, Json<ImportSkillResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ImportSkillResponse {
                    success: false,
                    skill_id: None,
                    skill_name: None,
                    error: Some("OpenClaw enricher not available".to_string()),
                }),
            );
        }
    };

    match enricher
        .import_skill_md(&req.content, req.name.as_deref())
        .await
    {
        Ok(skill) => (
            StatusCode::CREATED,
            Json(ImportSkillResponse {
                success: true,
                skill_id: Some(skill.skill_id),
                skill_name: Some(skill.name),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ImportSkillResponse {
                success: false,
                skill_id: None,
                skill_name: None,
                error: Some(e),
            }),
        ),
    }
}

/// POST /api/v1/openclaw/skills/import-batch — Import multiple SKILL.md files
async fn import_skill_batch(
    State(state): State<AppState>,
    Json(req): Json<ImportBatchRequest>,
) -> (StatusCode, Json<ImportBatchResponse>) {
    let enricher = match &state.openclaw_enricher {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ImportBatchResponse {
                    success: false,
                    imported: 0,
                    failed: req.skills.len(),
                    results: vec![],
                }),
            );
        }
    };

    let mut results = Vec::new();
    let mut imported = 0;
    let mut failed = 0;

    for skill_req in &req.skills {
        match enricher
            .import_skill_md(&skill_req.content, skill_req.name.as_deref())
            .await
        {
            Ok(skill) => {
                results.push(ImportSkillResponse {
                    success: true,
                    skill_id: Some(skill.skill_id),
                    skill_name: Some(skill.name),
                    error: None,
                });
                imported += 1;
            }
            Err(e) => {
                results.push(ImportSkillResponse {
                    success: false,
                    skill_id: None,
                    skill_name: None,
                    error: Some(e),
                });
                failed += 1;
            }
        }
    }

    (
        StatusCode::OK,
        Json(ImportBatchResponse {
            success: failed == 0,
            imported,
            failed,
            results,
        }),
    )
}

// ── Agent Management Handlers (proxy to bot_registry) ───────────────────────

#[derive(Debug, Serialize)]
struct AgentListResponse {
    success: bool,
    agents: Vec<BotInstance>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct AgentResponse {
    success: bool,
    agent: BotInstance,
}

#[derive(Debug, Serialize)]
struct AgentStatsResponse {
    success: bool,
    stats: BotRegistryStats,
}

#[derive(Debug, Serialize)]
struct AgentEventsResponse {
    success: bool,
    events: Vec<BotActivityEvent>,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct AgentEventsQuery {
    #[serde(default = "default_agent_event_limit")]
    limit: usize,
}

fn default_agent_event_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
struct AgentListQuery {
    kind: Option<BotKind>,
}

/// GET /api/v1/openclaw/agents — List all registered agents
async fn list_agents(
    State(state): State<AppState>,
    Query(query): Query<AgentListQuery>,
) -> (StatusCode, Json<AgentListResponse>) {
    let registry = match &state.bot_registry_v2 {
        Some(r) => r,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AgentListResponse {
                    success: false,
                    agents: vec![],
                    total: 0,
                }),
            );
        }
    };

    let agents = if let Some(kind) = query.kind {
        registry.get_bot_by_kind(kind).await
    } else {
        registry.list_bots().await
    };
    let total = agents.len();
    (
        StatusCode::OK,
        Json(AgentListResponse {
            success: true,
            agents,
            total,
        }),
    )
}

/// GET /api/v1/openclaw/agents/:bot_id — Get a single agent
async fn get_agent(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<Json<AgentResponse>, StatusCode> {
    let registry = state
        .bot_registry_v2
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let agent = registry
        .get_bot(&bot_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(AgentResponse {
        success: true,
        agent,
    }))
}

/// PUT /api/v1/openclaw/agents/:bot_id — Update an agent (status, config)
async fn update_agent(
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
    Json(req): Json<UpdateBotRequest>,
) -> Result<Json<AgentResponse>, (StatusCode, Json<serde_json::Value>)> {
    let registry = state.bot_registry_v2.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Bot registry not initialized"})),
        )
    })?;
    match registry.update_bot(&bot_id, req).await {
        Ok(agent) => Ok(Json(AgentResponse {
            success: true,
            agent,
        })),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

/// GET /api/v1/openclaw/agents/stats — Agent registry stats
async fn get_agents_stats(
    State(state): State<AppState>,
) -> (StatusCode, Json<AgentStatsResponse>) {
    let registry = match &state.bot_registry_v2 {
        Some(r) => r,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AgentStatsResponse {
                    success: false,
                    stats: BotRegistryStats {
                        total_bots: 0,
                        bots_by_kind: Default::default(),
                        bots_by_status: Default::default(),
                        total_tasks_completed: 0,
                        total_tokens_consumed: 0,
                        total_events: 0,
                    },
                }),
            );
        }
    };

    let stats = registry.get_bot_stats().await;
    (
        StatusCode::OK,
        Json(AgentStatsResponse {
            success: true,
            stats,
        }),
    )
}

/// GET /api/v1/openclaw/agents/events — Recent agent activity events
async fn get_agents_events(
    State(state): State<AppState>,
    Query(query): Query<AgentEventsQuery>,
) -> (StatusCode, Json<AgentEventsResponse>) {
    let registry = match &state.bot_registry_v2 {
        Some(r) => r,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AgentEventsResponse {
                    success: false,
                    events: vec![],
                    total: 0,
                }),
            );
        }
    };

    let events = registry.get_events(None, query.limit).await;
    let total = events.len();
    (
        StatusCode::OK,
        Json(AgentEventsResponse {
            success: true,
            events,
            total,
        }),
    )
}

// ── Memory Integration Handler ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct MemoryStatsResponse {
    success: bool,
    stats: Option<SkillMemoryStats>,
}

/// GET /api/v1/openclaw/memory/stats — Skill memory stats for the OpenClaw page
async fn get_memory_stats(
    State(state): State<AppState>,
) -> (StatusCode, Json<MemoryStatsResponse>) {
    let store = match &state.skill_memory_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(MemoryStatsResponse {
                    success: false,
                    stats: None,
                }),
            );
        }
    };

    let stats = store.get_stats().await;
    (
        StatusCode::OK,
        Json(MemoryStatsResponse {
            success: true,
            stats: Some(stats),
        }),
    )
}
