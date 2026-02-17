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

//! Bot Registry — managed registry for moltbot, clawdbot, and openclaw.
//!
//! Each bot is a named agent system with its own configuration, memory
//! namespace, and skill pool.  The registry tracks bot instances, their
//! active sessions, and cross-bot skill sharing rules.
//!
//! This integrates with SochDB's Entity (kind=Agent) and AgentContext
//! for per-bot session state, tool registry, and permission scoping.

use crate::api::skill_memory::BotKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;
use uuid::Uuid;

// ────────────────────────────────────────────────────────────────────────────
// Core types
// ────────────────────────────────────────────────────────────────────────────

/// A registered bot instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInstance {
    pub bot_id: String,
    pub kind: BotKind,
    pub name: String,
    pub description: String,
    /// Version of the bot deployment
    pub version: String,
    /// The LLM model this bot uses
    pub model: String,
    /// Current operational status
    pub status: BotStatus,
    /// Configuration for the bot
    pub config: BotConfig,
    /// Skill IDs this bot has access to
    pub skill_ids: Vec<String>,
    /// Active session count
    pub active_sessions: u64,
    /// Total tasks completed
    pub tasks_completed: u64,
    /// Total tokens consumed
    pub total_tokens: u64,
    /// Average task success rate
    pub success_rate: f64,
    /// Memory namespace in SochDB (for entity/episode isolation)
    pub memory_namespace: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Bot operational status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BotStatus {
    /// Bot is available and ready
    Online,
    /// Bot is currently processing tasks
    Busy,
    /// Bot is offline / not deployed
    Offline,
    /// Bot experienced an error
    Error,
    /// Bot is in maintenance mode
    Maintenance,
}

/// Bot-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Max concurrent sessions
    #[serde(default = "default_max_sessions")]
    pub max_concurrent_sessions: u32,
    /// Token budget per task
    #[serde(default = "default_token_budget")]
    pub token_budget: u64,
    /// Timeout per task in seconds
    #[serde(default = "default_timeout")]
    pub task_timeout_secs: u64,
    /// Whether this bot can share skills with other bots
    #[serde(default = "default_true")]
    pub skill_sharing_enabled: bool,
    /// Which bot kinds this bot accepts skills from
    #[serde(default)]
    pub accept_skills_from: Vec<BotKind>,
    /// System prompt override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Temperature for LLM calls
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    /// Custom tool definitions for this bot
    #[serde(default)]
    pub tools: Vec<BotTool>,
}

fn default_max_sessions() -> u32 {
    5
}
fn default_token_budget() -> u64 {
    50000
}
fn default_timeout() -> u64 {
    300
}
fn default_true() -> bool {
    true
}
fn default_temperature() -> f64 {
    0.3
}

/// A tool registered to a specific bot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotTool {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Bot activity event for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotActivityEvent {
    pub event_id: String,
    pub bot_id: String,
    pub event_type: BotEventType,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotEventType {
    Started,
    Stopped,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    SkillLearned,
    SkillShared,
    SkillInvoked,
    ConfigUpdated,
    Error,
}

// ────────────────────────────────────────────────────────────────────────────
// Bot Registry
// ────────────────────────────────────────────────────────────────────────────

pub struct BotRegistry {
    bots: RwLock<HashMap<String, BotInstance>>,
    events: RwLock<Vec<BotActivityEvent>>,
    persist_path: PathBuf,
}

impl BotRegistry {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let persist_path = data_dir.join("bot_registry");
        std::fs::create_dir_all(&persist_path).ok();

        let mut registry = Self {
            bots: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            persist_path,
        };
        registry.load_from_disk();

        // Register default bot instances if empty
        let bots = registry.bots.get_mut();
        if bots.is_empty() {
            let defaults = vec![
                Self::default_bot("moltbot".to_string()),
                Self::default_bot("clawdbot".to_string()),
                Self::default_bot("openclaw".to_string()),
            ];
            for bot in defaults {
                bots.insert(bot.bot_id.clone(), bot);
            }
            tracing::info!("Registered 3 default bot instances (moltbot, clawdbot, openclaw)");
        }

        registry
    }

    fn default_bot(kind: BotKind) -> BotInstance {
        let (name, description, model, version) = match kind.as_str() {
            "moltbot" => (
                "Moltbot".to_string(),
                "Multi-model orchestration bot — routes tasks to optimal LLM providers".to_string(),
                "multi-model".to_string(),
                "1.0.0".to_string(),
            ),
            "clawdbot" => (
                "Clawdbot".to_string(),
                "Claude-based coding assistant — deep code understanding and generation".to_string(),
                "claude-sonnet-4-20250514".to_string(),
                "1.0.0".to_string(),
            ),
            _ => (
                kind.clone(),
                format!("{} agent", kind),
                "open-source".to_string(),
                "1.0.0".to_string(),
            ),
        };

        BotInstance {
            bot_id: Uuid::new_v4().to_string(),
            kind: kind.clone(),
            name,
            description,
            version,
            model,
            status: BotStatus::Online,
            config: BotConfig {
                max_concurrent_sessions: default_max_sessions(),
                token_budget: default_token_budget(),
                task_timeout_secs: default_timeout(),
                skill_sharing_enabled: true,
                accept_skills_from: vec!["moltbot".to_string(), "clawdbot".to_string(), "openclaw".to_string()],
                system_prompt: None,
                temperature: default_temperature(),
                tools: vec![],
            },
            skill_ids: vec![],
            active_sessions: 0,
            tasks_completed: 0,
            total_tokens: 0,
            success_rate: 0.0,
            memory_namespace: format!("bot/{}", kind),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_active_at: None,
            metadata: HashMap::new(),
        }
    }

    fn load_from_disk(&mut self) {
        let bots_path = self.persist_path.join("bots.json");
        if bots_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&bots_path) {
                if let Ok(bots) = serde_json::from_str::<HashMap<String, BotInstance>>(&data) {
                    tracing::info!("Loaded {} bots from disk", bots.len());
                    *self.bots.get_mut() = bots;
                }
            }
        }

        let events_path = self.persist_path.join("events.json");
        if events_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&events_path) {
                if let Ok(events) = serde_json::from_str::<Vec<BotActivityEvent>>(&data) {
                    tracing::info!("Loaded {} bot events from disk", events.len());
                    *self.events.get_mut() = events;
                }
            }
        }
    }

    async fn persist_bots(&self) {
        let bots = self.bots.read().await;
        let path = self.persist_path.join("bots.json");
        if let Ok(data) = serde_json::to_string_pretty(&*bots) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &data).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    async fn persist_events(&self) {
        let events = self.events.read().await;
        let path = self.persist_path.join("events.json");
        if let Ok(data) = serde_json::to_string_pretty(&*events) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &data).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    async fn record_event(&self, event: BotActivityEvent) {
        let mut events = self.events.write().await;
        // Keep last 10k events
        if events.len() > 10_000 {
            let drain_to = events.len() - 9_000;
            events.drain(..drain_to);
        }
        events.push(event);
        drop(events);
        self.persist_events().await;
    }

    // ── Public API ──

    pub async fn register_bot(&self, bot: BotInstance) -> Result<BotInstance, String> {
        let mut bots = self.bots.write().await;
        if bots.values().any(|b| b.kind == bot.kind && b.name == bot.name) {
            return Err(format!("Bot '{}' of kind {:?} already registered", bot.name, bot.kind));
        }
        bots.insert(bot.bot_id.clone(), bot.clone());
        drop(bots);
        self.persist_bots().await;

        self.record_event(BotActivityEvent {
            event_id: Uuid::new_v4().to_string(),
            bot_id: bot.bot_id.clone(),
            event_type: BotEventType::Started,
            description: format!("Bot '{}' registered", bot.name),
            session_id: None,
            skill_id: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }).await;

        Ok(bot)
    }

    pub async fn get_bot(&self, bot_id: &str) -> Option<BotInstance> {
        self.bots.read().await.get(bot_id).cloned()
    }

    pub async fn get_bot_by_kind(&self, kind: BotKind) -> Vec<BotInstance> {
        self.bots
            .read()
            .await
            .values()
            .filter(|b| b.kind == kind)
            .cloned()
            .collect()
    }

    pub async fn list_bots(&self) -> Vec<BotInstance> {
        self.bots.read().await.values().cloned().collect()
    }

    pub async fn update_bot(&self, bot_id: &str, update: UpdateBotRequest) -> Result<BotInstance, String> {
        let mut bots = self.bots.write().await;
        let bot = bots.get_mut(bot_id).ok_or_else(|| format!("Bot {} not found", bot_id))?;

        if let Some(name) = update.name {
            bot.name = name;
        }
        if let Some(desc) = update.description {
            bot.description = desc;
        }
        if let Some(version) = update.version {
            bot.version = version;
        }
        if let Some(model) = update.model {
            bot.model = model;
        }
        if let Some(status) = update.status {
            bot.status = status;
        }
        if let Some(config) = update.config {
            bot.config = config;
        }
        if let Some(meta) = update.metadata {
            bot.metadata.extend(meta);
        }
        bot.updated_at = Utc::now();
        let result = bot.clone();
        drop(bots);
        self.persist_bots().await;

        self.record_event(BotActivityEvent {
            event_id: Uuid::new_v4().to_string(),
            bot_id: bot_id.to_string(),
            event_type: BotEventType::ConfigUpdated,
            description: format!("Bot '{}' updated", result.name),
            session_id: None,
            skill_id: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }).await;

        Ok(result)
    }

    pub async fn delete_bot(&self, bot_id: &str) -> Result<(), String> {
        let mut bots = self.bots.write().await;
        let bot = bots.remove(bot_id).ok_or_else(|| format!("Bot {} not found", bot_id))?;
        drop(bots);
        self.persist_bots().await;

        self.record_event(BotActivityEvent {
            event_id: Uuid::new_v4().to_string(),
            bot_id: bot_id.to_string(),
            event_type: BotEventType::Stopped,
            description: format!("Bot '{}' deleted", bot.name),
            session_id: None,
            skill_id: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }).await;

        Ok(())
    }

    pub async fn update_bot_stats(
        &self,
        bot_id: &str,
        success: bool,
        tokens: u64,
    ) -> Result<(), String> {
        let mut bots = self.bots.write().await;
        let bot = bots.get_mut(bot_id).ok_or_else(|| format!("Bot {} not found", bot_id))?;
        bot.tasks_completed += 1;
        bot.total_tokens += tokens;
        let n = bot.tasks_completed as f64;
        let sv = if success { 1.0 } else { 0.0 };
        bot.success_rate = bot.success_rate + (sv - bot.success_rate) / n;
        bot.last_active_at = Some(Utc::now());
        bot.updated_at = Utc::now();
        drop(bots);
        self.persist_bots().await;
        Ok(())
    }

    pub async fn get_events(&self, bot_id: Option<&str>, limit: usize) -> Vec<BotActivityEvent> {
        let events = self.events.read().await;
        events
            .iter()
            .rev()
            .filter(|e| bot_id.map_or(true, |id| e.bot_id == id))
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn get_bot_stats(&self) -> BotRegistryStats {
        let bots = self.bots.read().await;
        let events = self.events.read().await;

        let mut by_kind: HashMap<String, usize> = HashMap::new();
        let mut by_status: HashMap<String, usize> = HashMap::new();
        let mut total_tasks: u64 = 0;
        let mut total_tokens: u64 = 0;

        for bot in bots.values() {
            *by_kind.entry(bot.kind.to_string()).or_default() += 1;
            *by_status
                .entry(format!("{:?}", bot.status).to_lowercase())
                .or_default() += 1;
            total_tasks += bot.tasks_completed;
            total_tokens += bot.total_tokens;
        }

        BotRegistryStats {
            total_bots: bots.len(),
            bots_by_kind: by_kind,
            bots_by_status: by_status,
            total_tasks_completed: total_tasks,
            total_tokens_consumed: total_tokens,
            total_events: events.len(),
        }
    }

    pub fn count(&self) -> usize {
        // Synchronous count for startup logging
        // Uses try_read to avoid blocking
        self.bots
            .try_read()
            .map(|b| b.len())
            .unwrap_or(0)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Request types (used by api/bots.rs)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterBotRequest {
    pub kind: BotKind,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub config: Option<BotConfig>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBotRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub model: Option<String>,
    pub status: Option<BotStatus>,
    pub config: Option<BotConfig>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct BotRegistryStats {
    pub total_bots: usize,
    pub bots_by_kind: HashMap<String, usize>,
    pub bots_by_status: HashMap<String, usize>,
    pub total_tasks_completed: u64,
    pub total_tokens_consumed: u64,
    pub total_events: usize,
}
