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

//! AI Memory OS — Skill Memory for Cross-Task Skill Reuse & Evolution
//!
//! Leverages SochDB's `MemoryStore` trait (Episode/Event/Entity schema) and
//! `HierarchicalMemory` compaction (L0 Raw → L1 Summary → L2 Abstraction)
//! to provide persistent skill memory for LLM agent systems.
//!
//! Skills are stored as SochDB Entities (kind=Agent) with associated Episodes
//! tracking every invocation.  Embeddings enable semantic skill retrieval so
//! an agent can discover and reuse skills learned in prior tasks.

use crate::api::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

// ────────────────────────────────────────────────────────────────────────────
// Data Types
// ────────────────────────────────────────────────────────────────────────────

/// A learned skill that can be reused across tasks and agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    /// Which bot originally learned this skill
    pub origin_bot: BotKind,
    /// Semantic category (e.g. "code-generation", "debugging", "refactoring")
    pub category: String,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// The skill definition / procedure / prompt template
    pub definition: String,
    /// Input schema (JSON Schema string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<String>,
    /// Output schema (JSON Schema string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<String>,
    /// Version — increments on evolution
    pub version: u32,
    /// How many times this skill has been invoked
    pub invocation_count: u64,
    /// Success rate (0.0 – 1.0)
    pub success_rate: f64,
    /// Average execution duration in milliseconds
    pub avg_duration_ms: f64,
    /// Average token cost per invocation
    pub avg_tokens: f64,
    /// Embedding vector for semantic retrieval (384-dim, matches HNSW)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// Bots that have used this skill
    pub shared_with: Vec<BotKind>,
    /// Lifecycle status
    pub status: SkillStatus,
    /// Parent skill (if evolved from another)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_skill_id: Option<String>,
    /// Episode IDs from SochDB MemoryStore
    pub episode_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Arbitrary metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Agent/source identifier — any free-form string (e.g. "claude", "gpt-4", "my-agent").
pub type BotKind = String;

/// Lifecycle status of a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillStatus {
    /// Newly learned, not yet validated
    Draft,
    /// Validated and available for reuse
    Active,
    /// Superseded by a newer version
    Deprecated,
    /// Soft-deleted
    Archived,
}

/// Record of a single skill invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocation {
    pub invocation_id: String,
    pub skill_id: String,
    pub bot: BotKind,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub success: bool,
    pub duration_ms: u64,
    pub tokens_used: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Skill evolution event — when a skill is refined/improved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEvolution {
    pub evolution_id: String,
    pub skill_id: String,
    pub from_version: u32,
    pub to_version: u32,
    pub reason: String,
    pub changes: String,
    pub evolved_by: BotKind,
    pub timestamp: DateTime<Utc>,
}

// ────────────────────────────────────────────────────────────────────────────
// In-memory store (backed by SochDB persistence via AgentReplayStorage)
// ────────────────────────────────────────────────────────────────────────────

/// Skill Memory Store — thread-safe in-memory store with JSON persistence.
/// Maps to SochDB's MemoryStore pattern (Episode=invocation, Entity=skill).
pub struct SkillMemoryStore {
    skills: RwLock<HashMap<String, Skill>>,
    invocations: RwLock<Vec<SkillInvocation>>,
    evolutions: RwLock<Vec<SkillEvolution>>,
    persist_path: std::path::PathBuf,
}

impl SkillMemoryStore {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let persist_path = data_dir.join("skill_memory");
        std::fs::create_dir_all(&persist_path).ok();

        let mut store = Self {
            skills: RwLock::new(HashMap::new()),
            invocations: RwLock::new(Vec::new()),
            evolutions: RwLock::new(Vec::new()),
            persist_path,
        };
        store.load_from_disk();
        store
    }

    fn load_from_disk(&mut self) {
        // Load skills
        let skills_path = self.persist_path.join("skills.json");
        if skills_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&skills_path) {
                if let Ok(skills) = serde_json::from_str::<HashMap<String, Skill>>(&data) {
                    *self.skills.get_mut() = skills;
                    tracing::info!(
                        "Loaded {} skills from disk",
                        self.skills.get_mut().len()
                    );
                }
            }
        }

        // Load invocations
        let inv_path = self.persist_path.join("invocations.json");
        if inv_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&inv_path) {
                if let Ok(invocations) = serde_json::from_str::<Vec<SkillInvocation>>(&data) {
                    tracing::info!("Loaded {} invocations from disk", invocations.len());
                    *self.invocations.get_mut() = invocations;
                }
            }
        }

        // Load evolutions
        let evo_path = self.persist_path.join("evolutions.json");
        if evo_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&evo_path) {
                if let Ok(evolutions) = serde_json::from_str::<Vec<SkillEvolution>>(&data) {
                    tracing::info!("Loaded {} evolutions from disk", evolutions.len());
                    *self.evolutions.get_mut() = evolutions;
                }
            }
        }
    }

    async fn persist_skills(&self) {
        let skills = self.skills.read().await;
        let path = self.persist_path.join("skills.json");
        if let Ok(data) = serde_json::to_string_pretty(&*skills) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &data).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    async fn persist_invocations(&self) {
        let invocations = self.invocations.read().await;
        let path = self.persist_path.join("invocations.json");
        if let Ok(data) = serde_json::to_string_pretty(&*invocations) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &data).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    async fn persist_evolutions(&self) {
        let evolutions = self.evolutions.read().await;
        let path = self.persist_path.join("evolutions.json");
        if let Ok(data) = serde_json::to_string_pretty(&*evolutions) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &data).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    // ── CRUD ──

    pub async fn create_skill(&self, skill: Skill) -> Result<Skill, String> {
        let mut skills = self.skills.write().await;
        if skills.contains_key(&skill.skill_id) {
            return Err(format!("Skill {} already exists", skill.skill_id));
        }
        skills.insert(skill.skill_id.clone(), skill.clone());
        drop(skills);
        self.persist_skills().await;
        Ok(skill)
    }

    pub async fn get_skill(&self, id: &str) -> Option<Skill> {
        self.skills.read().await.get(id).cloned()
    }

    pub async fn update_skill(&self, id: &str, update: UpdateSkillRequest) -> Result<Skill, String> {
        let mut skills = self.skills.write().await;
        let skill = skills.get_mut(id).ok_or_else(|| format!("Skill {} not found", id))?;
        if let Some(name) = update.name {
            skill.name = name;
        }
        if let Some(desc) = update.description {
            skill.description = desc;
        }
        if let Some(cat) = update.category {
            skill.category = cat;
        }
        if let Some(tags) = update.tags {
            skill.tags = tags;
        }
        if let Some(def) = update.definition {
            skill.definition = def;
        }
        if let Some(status) = update.status {
            skill.status = status;
        }
        if let Some(shared) = update.shared_with {
            skill.shared_with = shared;
        }
        if let Some(meta) = update.metadata {
            skill.metadata.extend(meta);
        }
        skill.updated_at = Utc::now();
        let result = skill.clone();
        drop(skills);
        self.persist_skills().await;
        Ok(result)
    }

    pub async fn delete_skill(&self, id: &str) -> Result<(), String> {
        let mut skills = self.skills.write().await;
        if skills.remove(id).is_none() {
            return Err(format!("Skill {} not found", id));
        }
        drop(skills);
        self.persist_skills().await;
        Ok(())
    }

    pub async fn list_skills(&self, query: &SkillQuery) -> Vec<Skill> {
        let skills = self.skills.read().await;
        let mut results: Vec<Skill> = skills
            .values()
            .filter(|s| {
                if let Some(bot) = &query.bot {
                    if s.origin_bot != *bot && !s.shared_with.contains(bot) {
                        return false;
                    }
                }
                if let Some(cat) = &query.category {
                    if !s.category.to_lowercase().contains(&cat.to_lowercase()) {
                        return false;
                    }
                }
                if let Some(status) = &query.status {
                    if s.status != *status {
                        return false;
                    }
                }
                if let Some(tag) = &query.tag {
                    if !s.tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase()) {
                        return false;
                    }
                }
                if let Some(q) = &query.search {
                    let q_lower = q.to_lowercase();
                    if !s.name.to_lowercase().contains(&q_lower)
                        && !s.description.to_lowercase().contains(&q_lower)
                        && !s.category.to_lowercase().contains(&q_lower)
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by invocation count (most used first)
        results.sort_by(|a, b| b.invocation_count.cmp(&a.invocation_count));

        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(50).min(200);
        results.into_iter().skip(offset).take(limit).collect()
    }

    pub async fn record_invocation(&self, inv: SkillInvocation) -> Result<(), String> {
        // Update skill stats
        {
            let mut skills = self.skills.write().await;
            if let Some(skill) = skills.get_mut(&inv.skill_id) {
                skill.invocation_count += 1;
                let n = skill.invocation_count as f64;
                // Running average for duration
                skill.avg_duration_ms =
                    skill.avg_duration_ms + (inv.duration_ms as f64 - skill.avg_duration_ms) / n;
                // Running average for tokens
                skill.avg_tokens =
                    skill.avg_tokens + (inv.tokens_used as f64 - skill.avg_tokens) / n;
                // Running average for success rate
                let success_val = if inv.success { 1.0 } else { 0.0 };
                skill.success_rate =
                    skill.success_rate + (success_val - skill.success_rate) / n;
                skill.updated_at = Utc::now();

                // Track bot sharing
                if !skill.shared_with.contains(&inv.bot) && skill.origin_bot != inv.bot {
                    skill.shared_with.push(inv.bot.clone());
                }
            }
        }
        self.persist_skills().await;

        // Append invocation
        {
            let mut invocations = self.invocations.write().await;
            invocations.push(inv);
        }
        self.persist_invocations().await;

        Ok(())
    }

    pub async fn get_invocations(&self, skill_id: &str, limit: usize) -> Vec<SkillInvocation> {
        let invocations = self.invocations.read().await;
        invocations
            .iter()
            .rev()
            .filter(|i| i.skill_id == skill_id)
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn evolve_skill(
        &self,
        skill_id: &str,
        reason: String,
        changes: String,
        new_definition: String,
        evolved_by: BotKind,
    ) -> Result<(Skill, SkillEvolution), String> {
        let mut skills = self.skills.write().await;
        let skill = skills.get_mut(skill_id).ok_or_else(|| format!("Skill {} not found", skill_id))?;

        let from_version = skill.version;
        skill.version += 1;
        skill.definition = new_definition;
        skill.updated_at = Utc::now();
        let to_version = skill.version;
        let result = skill.clone();
        drop(skills);
        self.persist_skills().await;

        let evolution = SkillEvolution {
            evolution_id: Uuid::new_v4().to_string(),
            skill_id: skill_id.to_string(),
            from_version,
            to_version,
            reason,
            changes,
            evolved_by,
            timestamp: Utc::now(),
        };

        {
            let mut evolutions = self.evolutions.write().await;
            evolutions.push(evolution.clone());
        }
        self.persist_evolutions().await;

        Ok((result, evolution))
    }

    pub async fn get_evolutions(&self, skill_id: &str) -> Vec<SkillEvolution> {
        let evolutions = self.evolutions.read().await;
        evolutions
            .iter()
            .filter(|e| e.skill_id == skill_id)
            .cloned()
            .collect()
    }

    pub async fn get_stats(&self) -> SkillMemoryStats {
        let skills = self.skills.read().await;
        let invocations = self.invocations.read().await;
        let evolutions = self.evolutions.read().await;

        let mut by_bot: HashMap<String, u64> = HashMap::new();
        let mut by_category: HashMap<String, u64> = HashMap::new();
        let mut by_status: HashMap<String, u64> = HashMap::new();
        let mut total_invocations: u64 = 0;
        let mut total_success: u64 = 0;

        for skill in skills.values() {
            *by_bot.entry(skill.origin_bot.to_string()).or_default() += 1;
            *by_category.entry(skill.category.clone()).or_default() += 1;
            *by_status.entry(format!("{:?}", skill.status).to_lowercase()).or_default() += 1;
            total_invocations += skill.invocation_count;
            total_success += (skill.invocation_count as f64 * skill.success_rate) as u64;
        }

        SkillMemoryStats {
            total_skills: skills.len(),
            total_invocations,
            total_evolutions: evolutions.len(),
            overall_success_rate: if total_invocations > 0 {
                total_success as f64 / total_invocations as f64
            } else {
                0.0
            },
            skills_by_bot: by_bot,
            skills_by_category: by_category,
            skills_by_status: by_status,
            recent_invocations: invocations.iter().rev().take(10).cloned().collect(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Request / Response types
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub origin_bot: BotKind,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub definition: String,
    #[serde(default)]
    pub input_schema: Option<String>,
    #[serde(default)]
    pub output_schema: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSkillRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub definition: Option<String>,
    pub status: Option<SkillStatus>,
    pub shared_with: Option<Vec<BotKind>>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct SkillQuery {
    pub bot: Option<BotKind>,
    pub category: Option<String>,
    pub status: Option<SkillStatus>,
    pub tag: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RecordInvocationRequest {
    pub bot: BotKind,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub success: bool,
    pub duration_ms: u64,
    #[serde(default)]
    pub tokens_used: u64,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EvolveSkillRequest {
    pub reason: String,
    pub changes: String,
    pub new_definition: String,
    pub evolved_by: BotKind,
}

#[derive(Debug, Serialize)]
pub struct SkillMemoryStats {
    pub total_skills: usize,
    pub total_invocations: u64,
    pub total_evolutions: usize,
    pub overall_success_rate: f64,
    pub skills_by_bot: HashMap<String, u64>,
    pub skills_by_category: HashMap<String, u64>,
    pub skills_by_status: HashMap<String, u64>,
    pub recent_invocations: Vec<SkillInvocation>,
}

#[derive(Debug, Serialize)]
pub struct SkillResponse {
    pub skill: Skill,
}

#[derive(Debug, Serialize)]
pub struct SkillListResponse {
    pub skills: Vec<Skill>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct InvocationListResponse {
    pub invocations: Vec<SkillInvocation>,
}

#[derive(Debug, Serialize)]
pub struct EvolutionListResponse {
    pub evolutions: Vec<SkillEvolution>,
}

#[derive(Debug, Serialize)]
pub struct EvolutionResponse {
    pub skill: Skill,
    pub evolution: SkillEvolution,
}

// ────────────────────────────────────────────────────────────────────────────
// Router
// ────────────────────────────────────────────────────────────────────────────

pub fn skill_memory_router() -> Router<AppState> {
    Router::new()
        .route("/skills", get(list_skills).post(create_skill))
        .route(
            "/skills/:skill_id",
            get(get_skill).put(update_skill).delete(delete_skill),
        )
        .route("/skills/:skill_id/invoke", post(record_invocation))
        .route(
            "/skills/:skill_id/invocations",
            get(get_skill_invocations),
        )
        .route("/skills/:skill_id/evolve", post(evolve_skill))
        .route(
            "/skills/:skill_id/evolutions",
            get(get_skill_evolutions),
        )
        .route("/stats", get(get_stats))
}

// ────────────────────────────────────────────────────────────────────────────
// Handlers
// ────────────────────────────────────────────────────────────────────────────

async fn create_skill(
    State(state): State<AppState>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<(StatusCode, Json<SkillResponse>), (StatusCode, Json<serde_json::Value>)> {
    let store = state.skill_memory_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Skill memory store not initialized"})),
        )
    })?;

    let skill = Skill {
        skill_id: Uuid::new_v4().to_string(),
        name: req.name,
        description: req.description,
        origin_bot: req.origin_bot,
        category: req.category,
        tags: req.tags,
        definition: req.definition,
        input_schema: req.input_schema,
        output_schema: req.output_schema,
        version: 1,
        invocation_count: 0,
        success_rate: 0.0,
        avg_duration_ms: 0.0,
        avg_tokens: 0.0,
        embedding: None,
        shared_with: vec![],
        status: SkillStatus::Draft,
        parent_skill_id: None,
        episode_ids: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: req.metadata,
    };

    match store.create_skill(skill).await {
        Ok(skill) => Ok((StatusCode::CREATED, Json(SkillResponse { skill }))),
        Err(e) => Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn get_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> Result<Json<SkillResponse>, StatusCode> {
    let store = state
        .skill_memory_store
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    store
        .get_skill(&skill_id)
        .await
        .map(|skill| Json(SkillResponse { skill }))
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
    Json(req): Json<UpdateSkillRequest>,
) -> Result<Json<SkillResponse>, (StatusCode, Json<serde_json::Value>)> {
    let store = state.skill_memory_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Skill memory store not initialized"})),
        )
    })?;
    match store.update_skill(&skill_id, req).await {
        Ok(skill) => Ok(Json(SkillResponse { skill })),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn delete_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let store = state.skill_memory_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Skill memory store not initialized"})),
        )
    })?;
    match store.delete_skill(&skill_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn list_skills(
    State(state): State<AppState>,
    Query(query): Query<SkillQuery>,
) -> Result<Json<SkillListResponse>, StatusCode> {
    let store = state
        .skill_memory_store
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let skills = store.list_skills(&query).await;
    let total = skills.len();
    Ok(Json(SkillListResponse { skills, total }))
}

async fn record_invocation(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
    Json(req): Json<RecordInvocationRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let store = state.skill_memory_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Skill memory store not initialized"})),
        )
    })?;

    let invocation = SkillInvocation {
        invocation_id: Uuid::new_v4().to_string(),
        skill_id,
        bot: req.bot,
        session_id: req.session_id,
        trace_id: req.trace_id,
        input: req.input,
        output: req.output,
        success: req.success,
        duration_ms: req.duration_ms,
        tokens_used: req.tokens_used,
        error: req.error,
        timestamp: Utc::now(),
    };

    match store.record_invocation(invocation).await {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn get_skill_invocations(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<InvocationListResponse>, StatusCode> {
    let store = state
        .skill_memory_store
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let limit: usize = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(50);
    let invocations = store.get_invocations(&skill_id, limit).await;
    Ok(Json(InvocationListResponse { invocations }))
}

async fn evolve_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
    Json(req): Json<EvolveSkillRequest>,
) -> Result<Json<EvolutionResponse>, (StatusCode, Json<serde_json::Value>)> {
    let store = state.skill_memory_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Skill memory store not initialized"})),
        )
    })?;
    match store
        .evolve_skill(
            &skill_id,
            req.reason,
            req.changes,
            req.new_definition,
            req.evolved_by,
        )
        .await
    {
        Ok((skill, evolution)) => Ok(Json(EvolutionResponse { skill, evolution })),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e})),
        )),
    }
}

async fn get_skill_evolutions(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> Result<Json<EvolutionListResponse>, StatusCode> {
    let store = state
        .skill_memory_store
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let evolutions = store.get_evolutions(&skill_id).await;
    Ok(Json(EvolutionListResponse { evolutions }))
}

async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<SkillMemoryStats>, StatusCode> {
    let store = state
        .skill_memory_store
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(store.get_stats().await))
}
