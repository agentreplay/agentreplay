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

//! OpenClaw OTLP Enrichment
//!
//! Detects openclaw-sourced OTLP spans and enriches agentreplay with:
//!   • Bot detection from service.name / openclaw.* attributes
//!   • Automatic SkillInvocation creation from skill-related spans
//!   • Bot stats updates (tasks completed, tokens used)
//!   • Openclaw-specific metrics aggregation (tokens by type, costs, webhooks,
//!     session states, queue depth, message processing)
//!
//! This module bridges openclaw's `diagnostics-otel` extension output to
//! agentreplay's skill memory and bot registry systems.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

use crate::api::skill_memory::{BotKind, SkillMemoryStore};
use crate::bot_registry::BotRegistry;

// ── Openclaw Metric Types ───────────────────────────────────────────────────

/// Aggregated metrics from openclaw OTLP data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenclawMetrics {
    /// Token usage broken down by type
    pub tokens: TokenBreakdown,
    /// Cost tracking in USD
    pub cost_usd: f64,
    /// Model usage breakdown: model_name → ModelUsage
    pub model_usage: HashMap<String, ModelUsage>,
    /// Channel metrics: channel_name → ChannelMetrics
    pub channel_metrics: HashMap<String, ChannelMetrics>,
    /// Session state counts
    pub session_states: SessionStateMetrics,
    /// Queue metrics
    pub queue_metrics: QueueMetrics,
    /// Webhook metrics
    pub webhook_metrics: WebhookMetrics,
    /// Message processing metrics
    pub message_metrics: MessageMetrics,
    /// Total agent runs
    pub total_runs: u64,
    /// Recent activity events (last 500)
    pub recent_events: Vec<OpenclawEvent>,
    /// Last updated timestamp
    pub last_updated: String,
}

impl Default for OpenclawMetrics {
    fn default() -> Self {
        Self {
            tokens: TokenBreakdown::default(),
            cost_usd: 0.0,
            model_usage: HashMap::new(),
            channel_metrics: HashMap::new(),
            session_states: SessionStateMetrics::default(),
            queue_metrics: QueueMetrics::default(),
            webhook_metrics: WebhookMetrics::default(),
            message_metrics: MessageMetrics::default(),
            total_runs: 0,
            recent_events: Vec::new(),
            last_updated: Utc::now().to_rfc3339(),
        }
    }
}

/// Token usage broken down by direction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenBreakdown {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total: u64,
}

/// Per-model usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelUsage {
    pub provider: String,
    pub model: String,
    pub request_count: u64,
    pub tokens: TokenBreakdown,
    pub cost_usd: f64,
    pub avg_duration_ms: f64,
    pub error_count: u64,
}

/// Per-channel metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelMetrics {
    pub channel: String,
    pub messages_processed: u64,
    pub messages_queued: u64,
    pub webhooks_received: u64,
    pub webhook_errors: u64,
    pub avg_message_duration_ms: f64,
    pub avg_webhook_duration_ms: f64,
}

/// Session state aggregation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStateMetrics {
    pub idle: u64,
    pub processing: u64,
    pub waiting: u64,
    pub stuck: u64,
    pub total_transitions: u64,
}

/// Queue depth and wait metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub current_depth: u64,
    pub total_enqueued: u64,
    pub total_dequeued: u64,
    pub avg_wait_ms: f64,
    pub max_wait_ms: f64,
    pub lanes: HashMap<String, LaneMetrics>,
}

/// Per-lane queue metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaneMetrics {
    pub enqueue_count: u64,
    pub dequeue_count: u64,
    pub current_size: u64,
}

/// Webhook aggregate metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookMetrics {
    pub received: u64,
    pub processed: u64,
    pub errors: u64,
    pub avg_duration_ms: f64,
}

/// Message processing aggregate metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMetrics {
    pub queued: u64,
    pub completed: u64,
    pub skipped: u64,
    pub errors: u64,
    pub avg_duration_ms: f64,
}

/// An enrichment event tracked for the dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenclawEvent {
    pub event_id: String,
    pub event_type: String,
    pub description: String,
    pub metadata: HashMap<String, serde_json::Value>,
    pub timestamp: String,
}

// ── Span Detection ──────────────────────────────────────────────────────────

/// Detect if an OTLP span batch originated from openclaw
pub fn detect_openclaw_source(
    resource_attrs: &HashMap<String, String>,
    span_attrs: &HashMap<String, serde_json::Value>,
) -> Option<DetectedSource> {
    // Check service.name
    let service_name = resource_attrs.get("service.name")
        .or_else(|| resource_attrs.get("service_name"));

    // Check openclaw.plugin resource attribute (custom observability plugin)
    let has_plugin_attr = resource_attrs.get("openclaw.plugin").is_some();

    if let Some(name) = service_name {
        let lower = name.to_lowercase();
        if lower == "openclaw" || lower == "openclaw-gateway" || lower == "clawdbot" || lower == "moltbot" || has_plugin_attr {
            return Some(DetectedSource {
                bot_kind: match lower.as_str() {
                    "moltbot" => "moltbot".to_string(),
                    "clawdbot" => "clawdbot".to_string(),
                    _ => "openclaw".to_string(),
                },
                service_name: name.clone(),
                agent_id: resource_attrs.get("agent.id").cloned(),
                session_key: None,
                is_observability_plugin: has_plugin_attr || lower == "openclaw-gateway",
            });
        }
    }

    // Check openclaw.* span attributes (both diagnostics-otel and custom plugin naming)
    if span_attrs.contains_key("openclaw.channel")
        || span_attrs.contains_key("openclaw.sessionKey")
        || span_attrs.contains_key("openclaw.provider")
        || span_attrs.contains_key("openclaw.session.key")
        || span_attrs.contains_key("openclaw.message.channel")
        || span_attrs.contains_key("openclaw.agent.id")
    {
        let is_plugin = span_attrs.contains_key("openclaw.session.key")
            || span_attrs.contains_key("openclaw.message.channel")
            || span_attrs.contains_key("openclaw.agent.id");
        return Some(DetectedSource {
            bot_kind: "openclaw".to_string(),
            service_name: service_name.cloned().unwrap_or_else(|| "openclaw".to_string()),
            agent_id: span_attrs.get("openclaw.agentId")
                .or_else(|| span_attrs.get("openclaw.agent.id"))
                .and_then(|v| v.as_str().map(String::from)),
            session_key: span_attrs.get("openclaw.sessionKey")
                .or_else(|| span_attrs.get("openclaw.session.key"))
                .and_then(|v| v.as_str().map(String::from)),
            is_observability_plugin: is_plugin,
        });
    }

    None
}

/// What was detected from the span
#[derive(Debug, Clone)]
pub struct DetectedSource {
    pub bot_kind: BotKind,
    pub service_name: String,
    pub agent_id: Option<String>,
    pub session_key: Option<String>,
    /// True when spans come from the openclaw-observability-plugin
    /// (connected traces with gen_ai.* attributes) vs the built-in
    /// diagnostics-otel extension (flat metric spans).
    pub is_observability_plugin: bool,
}

/// Classify an openclaw span by its name
#[derive(Debug, Clone, PartialEq)]
pub enum OpenclawSpanKind {
    ModelUsage,
    WebhookProcessed,
    WebhookError,
    MessageProcessed,
    SessionStuck,
    SkillInvocation,
    ToolCall,
    AgentLifecycle,
    /// Root request span from the observability plugin (openclaw.request)
    Request,
    /// Agent turn span from the observability plugin (openclaw.agent.turn)
    AgentTurn,
    /// Command span from the observability plugin (openclaw.command.*)
    Command,
    /// Gateway lifecycle span from the observability plugin
    GatewayLifecycle,
    /// Security event detected in a span
    SecurityEvent,
    Unknown,
}

pub fn classify_openclaw_span(span_name: &str) -> OpenclawSpanKind {
    match span_name {
        // Built-in diagnostics-otel spans
        "openclaw.model.usage" => OpenclawSpanKind::ModelUsage,
        "openclaw.webhook.processed" => OpenclawSpanKind::WebhookProcessed,
        "openclaw.webhook.error" => OpenclawSpanKind::WebhookError,
        "openclaw.message.processed" => OpenclawSpanKind::MessageProcessed,
        "openclaw.session.stuck" => OpenclawSpanKind::SessionStuck,

        // Custom observability plugin spans
        "openclaw.request" => OpenclawSpanKind::Request,
        "openclaw.agent.turn" => OpenclawSpanKind::AgentTurn,
        "openclaw.gateway.startup" => OpenclawSpanKind::GatewayLifecycle,

        // Skill invocations
        name if name.starts_with("openclaw.skill.") => OpenclawSpanKind::SkillInvocation,

        // Tool calls — both `openclaw.tool.*` (diagnostics) and `tool.*` (plugin)
        name if name.starts_with("openclaw.tool.") => OpenclawSpanKind::ToolCall,
        name if name.starts_with("tool.") => OpenclawSpanKind::ToolCall,

        // Command spans (openclaw.command.new, openclaw.command.reset, etc.)
        name if name.starts_with("openclaw.command.") => OpenclawSpanKind::Command,

        // Agent lifecycle (openclaw.agent.*, openclaw.run.*)
        name if name.starts_with("openclaw.agent.") || name.starts_with("openclaw.run.") => {
            OpenclawSpanKind::AgentLifecycle
        }

        // Gateway lifecycle
        name if name.starts_with("openclaw.gateway.") => OpenclawSpanKind::GatewayLifecycle,

        _ => OpenclawSpanKind::Unknown,
    }
}

// ── Enrichment Engine ───────────────────────────────────────────────────────

/// The main openclaw enrichment engine
pub struct OpenclawEnricher {
    metrics: RwLock<OpenclawMetrics>,
    skill_memory: Option<Arc<SkillMemoryStore>>,
    bot_registry: Option<Arc<BotRegistry>>,
    data_dir: std::path::PathBuf,
}

impl OpenclawEnricher {
    pub fn new(
        data_dir: &std::path::Path,
        skill_memory: Option<Arc<SkillMemoryStore>>,
        bot_registry: Option<Arc<BotRegistry>>,
    ) -> Self {
        let metrics_path = data_dir.join("openclaw_metrics.json");
        let metrics = if metrics_path.exists() {
            match std::fs::read_to_string(&metrics_path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
                Err(_) => OpenclawMetrics::default(),
            }
        } else {
            OpenclawMetrics::default()
        };

        Self {
            metrics: RwLock::new(metrics),
            skill_memory,
            bot_registry,
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// Process a batch of OTLP spans that have been identified as coming from openclaw.
    /// Handles both the built-in diagnostics-otel extension spans AND the
    /// openclaw-observability-plugin's connected trace spans.
    pub async fn enrich_spans(
        &self,
        source: &DetectedSource,
        spans: &[(String, HashMap<String, serde_json::Value>)], // (span_name, attributes)
    ) {
        let mut metrics = self.metrics.write().await;

        for (span_name, attrs) in spans {
            let kind = classify_openclaw_span(span_name);

            // Check for security events embedded in any span
            if attrs.contains_key("security.event.detected") {
                self.process_security_event(&mut metrics, span_name, attrs);
            }

            match kind {
                // ── Built-in diagnostics-otel spans ──
                OpenclawSpanKind::ModelUsage => {
                    self.process_model_usage(&mut metrics, source, attrs);
                }
                OpenclawSpanKind::WebhookProcessed => {
                    self.process_webhook_processed(&mut metrics, attrs);
                }
                OpenclawSpanKind::WebhookError => {
                    self.process_webhook_error(&mut metrics, attrs);
                }
                OpenclawSpanKind::MessageProcessed => {
                    self.process_message_processed(&mut metrics, attrs);
                }
                OpenclawSpanKind::SessionStuck => {
                    self.process_session_stuck(&mut metrics, attrs);
                }

                // ── Observability plugin: root request span ──
                OpenclawSpanKind::Request => {
                    self.process_request_span(&mut metrics, source, attrs);
                }

                // ── Observability plugin: agent turn span (carries gen_ai.* attrs) ──
                OpenclawSpanKind::AgentTurn => {
                    self.process_agent_turn(&mut metrics, source, attrs);
                }

                // ── Tool calls from either source ──
                OpenclawSpanKind::ToolCall => {
                    self.process_tool_call(&mut metrics, source, span_name, attrs).await;
                }

                // ── Observability plugin: command spans ──
                OpenclawSpanKind::Command => {
                    self.process_command_span(&mut metrics, span_name, attrs);
                }

                // ── Gateway lifecycle ──
                OpenclawSpanKind::GatewayLifecycle => {
                    self.record_event(
                        &mut metrics,
                        "gateway.lifecycle",
                        &format!("Gateway event: {}", span_name),
                        attrs,
                    );
                }

                // ── General agent lifecycle / skill invocations ──
                OpenclawSpanKind::AgentLifecycle => {
                    metrics.total_runs += 1;
                    self.record_event(
                        &mut metrics,
                        "agent.run",
                        &format!("Agent run on {} via {}", source.service_name, source.bot_kind),
                        attrs,
                    );
                }
                OpenclawSpanKind::SkillInvocation => {
                    let tool_result = self.check_skill_match(source, attrs).await;
                    if let Some((_skill_name, event_desc)) = tool_result {
                        self.record_event(
                            &mut metrics,
                            "skill.invocation",
                            &event_desc,
                            attrs,
                        );
                    }
                }

                OpenclawSpanKind::SecurityEvent | OpenclawSpanKind::Unknown => {
                    // SecurityEvent already handled above; Unknown spans are ignored.
                }
            }
        }

        metrics.last_updated = Utc::now().to_rfc3339();
        drop(metrics);
        self.persist_metrics().await;

        // Update bot stats asynchronously
        if let Some(ref registry) = self.bot_registry {
            self.update_bot_stats(registry, source).await;
        }
    }

    // ── Observability Plugin Span Processors ────────────────────────────────

    /// Process `openclaw.request` root span — records inbound message metrics
    fn process_request_span(
        &self,
        metrics: &mut OpenclawMetrics,
        source: &DetectedSource,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        let channel = attr_str(attrs, "openclaw.message.channel").unwrap_or_default();
        let session_key = attr_str(attrs, "openclaw.session.key").unwrap_or_default();
        let duration_ms = attr_f64(attrs, "openclaw.request.duration_ms");

        // Update channel metrics
        if !channel.is_empty() {
            let ch = metrics.channel_metrics.entry(channel.clone()).or_insert_with(|| {
                ChannelMetrics { channel: channel.clone(), ..Default::default() }
            });
            ch.messages_processed += 1;
            if duration_ms > 0.0 {
                let mn = ch.messages_processed as f64;
                ch.avg_message_duration_ms =
                    ch.avg_message_duration_ms * (mn - 1.0) / mn + duration_ms / mn;
            }
        }

        metrics.message_metrics.completed += 1;
        if duration_ms > 0.0 {
            let total = (metrics.message_metrics.completed + metrics.message_metrics.errors) as f64;
            metrics.message_metrics.avg_duration_ms =
                metrics.message_metrics.avg_duration_ms * (total - 1.0) / total + duration_ms / total;
        }

        self.record_event(
            metrics,
            "request",
            &format!(
                "Request from {} on channel={} session={}",
                source.service_name, channel, session_key
            ),
            attrs,
        );

        debug!(
            "OpenClaw enrichment: request span channel={} session={}",
            channel, session_key
        );
    }

    /// Process `openclaw.agent.turn` span — extracts gen_ai.* semantic convention
    /// attributes (token usage, model, cost) that the observability plugin attaches
    fn process_agent_turn(
        &self,
        metrics: &mut OpenclawMetrics,
        source: &DetectedSource,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        metrics.total_runs += 1;

        // Extract GenAI semantic convention attributes
        let input_tokens = attr_u64(attrs, "gen_ai.usage.input_tokens");
        let output_tokens = attr_u64(attrs, "gen_ai.usage.output_tokens");
        let cache_read = attr_u64(attrs, "gen_ai.usage.cache_read_tokens");
        let cache_write = attr_u64(attrs, "gen_ai.usage.cache_write_tokens");
        let total_tokens = attr_u64(attrs, "gen_ai.usage.total_tokens");
        let cost = attr_f64(attrs, "openclaw.llm.cost_usd");
        let duration_ms = attr_f64(attrs, "openclaw.agent.duration_ms");
        let model = attr_str(attrs, "gen_ai.response.model")
            .or_else(|| attr_str(attrs, "openclaw.agent.model"))
            .unwrap_or_default();
        let provider = attr_str(attrs, "gen_ai.system").unwrap_or_default();
        let agent_id = attr_str(attrs, "openclaw.agent.id").unwrap_or_default();
        let success = attrs.get("openclaw.agent.success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Update global token breakdown
        metrics.tokens.input += input_tokens;
        metrics.tokens.output += output_tokens;
        metrics.tokens.cache_read += cache_read;
        metrics.tokens.cache_write += cache_write;
        metrics.tokens.total += total_tokens;
        metrics.cost_usd += cost;

        // Update per-model metrics
        if !model.is_empty() {
            let key = format!("{}:{}", provider, model);
            let entry = metrics.model_usage.entry(key).or_insert_with(|| ModelUsage {
                provider: provider.clone(),
                model: model.clone(),
                ..Default::default()
            });
            entry.request_count += 1;
            entry.tokens.input += input_tokens;
            entry.tokens.output += output_tokens;
            entry.tokens.cache_read += cache_read;
            entry.tokens.cache_write += cache_write;
            entry.tokens.total += total_tokens;
            entry.cost_usd += cost;
            if duration_ms > 0.0 {
                let n = entry.request_count as f64;
                entry.avg_duration_ms = entry.avg_duration_ms * (n - 1.0) / n + duration_ms / n;
            }
            if !success {
                entry.error_count += 1;
            }
        }

        self.record_event(
            metrics,
            "agent.turn",
            &format!(
                "Agent turn agent={} model={}/{} tokens={} cost=${:.4} {}",
                agent_id, provider, model, total_tokens, cost,
                if success { "OK" } else { "FAILED" }
            ),
            attrs,
        );

        debug!(
            "OpenClaw enrichment: agent.turn agent={} model={}/{} tokens={} cost={:.4}",
            agent_id, provider, model, total_tokens, cost
        );
    }

    /// Process tool call spans — handles both `tool.*` (plugin) and `openclaw.tool.*` (diagnostics)
    async fn process_tool_call(
        &self,
        metrics: &mut OpenclawMetrics,
        source: &DetectedSource,
        span_name: &str,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        // Extract tool name from span name or attributes
        let tool_name = if let Some(name) = attr_str(attrs, "openclaw.tool.name") {
            name
        } else if span_name.starts_with("tool.") {
            span_name.strip_prefix("tool.").unwrap_or(span_name).to_string()
        } else if span_name.starts_with("openclaw.tool.") {
            span_name.strip_prefix("openclaw.tool.").unwrap_or(span_name).to_string()
        } else {
            span_name.to_string()
        };

        self.record_event(
            metrics,
            "tool.call",
            &format!("Tool '{}' invoked via {}", tool_name, source.service_name),
            attrs,
        );

        // Try to match to a known skill
        let tool_result = self.check_skill_match(source, attrs).await;
        if let Some((_skill_name, event_desc)) = tool_result {
            self.record_event(
                metrics,
                "skill.invocation",
                &event_desc,
                attrs,
            );
        }
    }

    /// Process `openclaw.command.*` spans (command:new, command:reset, command:stop)
    fn process_command_span(
        &self,
        metrics: &mut OpenclawMetrics,
        span_name: &str,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        let action = attr_str(attrs, "openclaw.command.action")
            .unwrap_or_else(|| {
                span_name.strip_prefix("openclaw.command.").unwrap_or("unknown").to_string()
            });

        if action == "new" || action == "reset" {
            // Session reset — track in session states
            metrics.session_states.total_transitions += 1;
        }

        self.record_event(
            metrics,
            "command",
            &format!("Command: {} (session={})",
                action,
                attr_str(attrs, "openclaw.command.session_key").unwrap_or_default(),
            ),
            attrs,
        );

        debug!("OpenClaw enrichment: command action={}", action);
    }

    /// Process security events embedded in span attributes
    fn process_security_event(
        &self,
        metrics: &mut OpenclawMetrics,
        span_name: &str,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        let detection = attr_str(attrs, "security.event.detection").unwrap_or_default();
        let severity = attr_str(attrs, "security.event.severity").unwrap_or_else(|| "info".to_string());
        let description = attr_str(attrs, "security.event.description").unwrap_or_default();

        self.record_event(
            metrics,
            &format!("security.{}", detection),
            &format!(
                "[{}] {} in span '{}': {}",
                severity.to_uppercase(), detection, span_name, description
            ),
            attrs,
        );

        info!(
            "OpenClaw SECURITY event: detection={} severity={} span={} desc={}",
            detection, severity, span_name, description
        );
    }

    fn process_model_usage(
        &self,
        metrics: &mut OpenclawMetrics,
        _source: &DetectedSource,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        // Extract token counts
        let input = attr_u64(attrs, "openclaw.tokens.input");
        let output = attr_u64(attrs, "openclaw.tokens.output");
        let cache_read = attr_u64(attrs, "openclaw.tokens.cache_read");
        let cache_write = attr_u64(attrs, "openclaw.tokens.cache_write");
        let total = attr_u64(attrs, "openclaw.tokens.total");
        let cost = attr_f64(attrs, "openclaw.cost.usd");
        let duration_ms = attr_f64(attrs, "openclaw.run.duration_ms");

        // Update global token breakdown
        metrics.tokens.input += input;
        metrics.tokens.output += output;
        metrics.tokens.cache_read += cache_read;
        metrics.tokens.cache_write += cache_write;
        metrics.tokens.total += total;
        metrics.cost_usd += cost;

        // Update per-model metrics
        let provider = attr_str(attrs, "openclaw.provider").unwrap_or_default();
        let model = attr_str(attrs, "openclaw.model").unwrap_or_default();
        if !model.is_empty() {
            let key = format!("{}:{}", provider, model);
            let entry = metrics.model_usage.entry(key).or_insert_with(|| ModelUsage {
                provider: provider.clone(),
                model: model.clone(),
                ..Default::default()
            });
            entry.request_count += 1;
            entry.tokens.input += input;
            entry.tokens.output += output;
            entry.tokens.cache_read += cache_read;
            entry.tokens.cache_write += cache_write;
            entry.tokens.total += total;
            entry.cost_usd += cost;
            // Running average for duration
            let n = entry.request_count as f64;
            entry.avg_duration_ms = entry.avg_duration_ms * (n - 1.0) / n + duration_ms / n;
        }

        // Update channel metrics
        if let Some(channel) = attr_str(attrs, "openclaw.channel") {
            let ch = metrics.channel_metrics.entry(channel.clone()).or_insert_with(|| {
                ChannelMetrics { channel, ..Default::default() }
            });
            ch.messages_processed += 1;
        }

        self.record_event(
            metrics,
            "model.usage",
            &format!("{}/{} — {} tokens, ${:.4}", provider, model, total, cost),
            attrs,
        );

        debug!(
            "OpenClaw enrichment: model.usage provider={} model={} tokens={} cost={}",
            provider, model, total, cost
        );
    }

    fn process_webhook_processed(
        &self,
        metrics: &mut OpenclawMetrics,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        metrics.webhook_metrics.received += 1;
        metrics.webhook_metrics.processed += 1;
        let duration = attr_f64(attrs, "openclaw.webhook.duration_ms");
        let n = metrics.webhook_metrics.processed as f64;
        metrics.webhook_metrics.avg_duration_ms =
            metrics.webhook_metrics.avg_duration_ms * (n - 1.0) / n + duration / n;

        if let Some(channel) = attr_str(attrs, "openclaw.channel") {
            let ch = metrics.channel_metrics.entry(channel.clone()).or_insert_with(|| {
                ChannelMetrics { channel, ..Default::default() }
            });
            ch.webhooks_received += 1;
            let wn = ch.webhooks_received as f64;
            ch.avg_webhook_duration_ms =
                ch.avg_webhook_duration_ms * (wn - 1.0) / wn + duration / wn;
        }
    }

    fn process_webhook_error(
        &self,
        metrics: &mut OpenclawMetrics,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        metrics.webhook_metrics.errors += 1;

        if let Some(channel) = attr_str(attrs, "openclaw.channel") {
            let ch = metrics.channel_metrics.entry(channel.clone()).or_insert_with(|| {
                ChannelMetrics { channel, ..Default::default() }
            });
            ch.webhook_errors += 1;
        }

        let error = attr_str(attrs, "openclaw.error").unwrap_or_default();
        self.record_event(
            metrics,
            "webhook.error",
            &format!("Webhook error: {}", error),
            attrs,
        );
    }

    fn process_message_processed(
        &self,
        metrics: &mut OpenclawMetrics,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        let outcome = attr_str(attrs, "openclaw.outcome").unwrap_or_default();
        let duration = attr_f64(attrs, "openclaw.message.duration_ms");

        match outcome.as_str() {
            "completed" => metrics.message_metrics.completed += 1,
            "skipped" => metrics.message_metrics.skipped += 1,
            "error" => metrics.message_metrics.errors += 1,
            _ => metrics.message_metrics.completed += 1,
        }

        let total = metrics.message_metrics.completed + metrics.message_metrics.errors;
        if total > 0 {
            let n = total as f64;
            metrics.message_metrics.avg_duration_ms =
                metrics.message_metrics.avg_duration_ms * (n - 1.0) / n + duration / n;
        }

        if let Some(channel) = attr_str(attrs, "openclaw.channel") {
            let ch = metrics.channel_metrics.entry(channel.clone()).or_insert_with(|| {
                ChannelMetrics { channel, ..Default::default() }
            });
            ch.messages_processed += 1;
            let mn = ch.messages_processed as f64;
            ch.avg_message_duration_ms =
                ch.avg_message_duration_ms * (mn - 1.0) / mn + duration / mn;
        }
    }

    fn process_session_stuck(
        &self,
        metrics: &mut OpenclawMetrics,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        metrics.session_states.stuck += 1;
        let age_ms = attr_f64(attrs, "openclaw.ageMs");
        let state = attr_str(attrs, "openclaw.state").unwrap_or_default();

        self.record_event(
            metrics,
            "session.stuck",
            &format!("Session stuck in {} state for {:.0}ms", state, age_ms),
            attrs,
        );
    }

    /// Check if a tool call matches a known skill and auto-record invocation.
    /// Returns (skill_name, event_description) if a match was found.
    async fn check_skill_match(
        &self,
        source: &DetectedSource,
        attrs: &HashMap<String, serde_json::Value>,
    ) -> Option<(String, String)> {
        let tool_name = attr_str(attrs, "openclaw.tool.name")
            .or_else(|| attr_str(attrs, "gen_ai.tool.name"))
            .unwrap_or_default();

        let duration = attr_f64(attrs, "openclaw.tool.duration_ms");
        let success = attrs.get("error.type").is_none();
        let tokens = attr_u64(attrs, "openclaw.tokens.total");

        let skill_store = self.skill_memory.as_ref()?;

        let query = crate::api::skill_memory::SkillQuery {
            bot: None,
            category: None,
            status: None,
            tag: None,
            search: None,
            limit: None,
            offset: None,
        };
        let skills = skill_store.list_skills(&query).await;
        let matched_skill = skills.iter().find(|s| {
            s.name.eq_ignore_ascii_case(&tool_name)
                || s.tags.iter().any(|t| t.eq_ignore_ascii_case(&tool_name))
        }).cloned()?;

        let invocation = crate::api::skill_memory::SkillInvocation {
            invocation_id: Uuid::new_v4().to_string(),
            skill_id: matched_skill.skill_id.clone(),
            bot: source.bot_kind.clone(),
            session_id: source.session_key.clone(),
            trace_id: None,
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            success,
            duration_ms: duration as u64,
            tokens_used: tokens,
            error: attr_str(attrs, "error.type"),
            timestamp: Utc::now(),
        };

        let _ = skill_store.record_invocation(invocation).await;

        let event_desc = format!(
            "Skill '{}' invoked by {} — {} in {:.0}ms",
            matched_skill.name,
            source.bot_kind,
            if success { "success" } else { "failed" },
            duration
        );

        Some((matched_skill.name, event_desc))
    }

    fn record_event(
        &self,
        metrics: &mut OpenclawMetrics,
        event_type: &str,
        description: &str,
        attrs: &HashMap<String, serde_json::Value>,
    ) {
        let event = OpenclawEvent {
            event_id: Uuid::new_v4().to_string(),
            event_type: event_type.to_string(),
            description: description.to_string(),
            metadata: attrs
                .iter()
                .filter(|(k, _)| k.starts_with("openclaw."))
                .take(20)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            timestamp: Utc::now().to_rfc3339(),
        };

        metrics.recent_events.push(event);
        // Keep last 500 events
        if metrics.recent_events.len() > 500 {
            let drain_to = metrics.recent_events.len() - 400;
            metrics.recent_events.drain(..drain_to);
        }
    }

    async fn update_bot_stats(&self, registry: &BotRegistry, source: &DetectedSource) {
        // Find the bot by kind and update stats
        let bots = registry.list_bots().await;
        if let Some(bot) = bots.iter().find(|b| b.kind == source.bot_kind) {
            let _ = registry.update_bot_stats(
                &bot.bot_id,
                true,
                0,
            ).await;
        }
    }

    async fn persist_metrics(&self) {
        let metrics = self.metrics.read().await;
        let path = self.data_dir.join("openclaw_metrics.json");
        if let Ok(data) = serde_json::to_string_pretty(&*metrics) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &data).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    // ── Public API ──

    pub async fn get_metrics(&self) -> OpenclawMetrics {
        self.metrics.read().await.clone()
    }

    pub async fn get_model_usage(&self) -> Vec<ModelUsage> {
        let metrics = self.metrics.read().await;
        metrics.model_usage.values().cloned().collect()
    }

    pub async fn get_channel_metrics(&self) -> Vec<ChannelMetrics> {
        let metrics = self.metrics.read().await;
        metrics.channel_metrics.values().cloned().collect()
    }

    pub async fn get_recent_events(&self, limit: usize) -> Vec<OpenclawEvent> {
        let metrics = self.metrics.read().await;
        let len = metrics.recent_events.len();
        let start = if len > limit { len - limit } else { 0 };
        metrics.recent_events[start..].to_vec()
    }

    pub async fn get_session_states(&self) -> SessionStateMetrics {
        self.metrics.read().await.session_states.clone()
    }

    /// Update session state from an OTLP diagnostic event
    pub async fn update_session_state(&self, state: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.session_states.total_transitions += 1;
        match state {
            "idle" => metrics.session_states.idle += 1,
            "processing" => metrics.session_states.processing += 1,
            "waiting" => metrics.session_states.waiting += 1,
            _ => {}
        }
    }

    /// Update queue metrics from an OTLP diagnostic event
    pub async fn update_queue_metrics(&self, lane: &str, enqueue: bool, queue_size: u64, wait_ms: Option<f64>) {
        let mut metrics = self.metrics.write().await;
        if enqueue {
            metrics.queue_metrics.total_enqueued += 1;
            let lane_metrics = metrics.queue_metrics.lanes.entry(lane.to_string()).or_default();
            lane_metrics.enqueue_count += 1;
            lane_metrics.current_size = queue_size;
        } else {
            metrics.queue_metrics.total_dequeued += 1;
            let total_dequeued = metrics.queue_metrics.total_dequeued;
            if let Some(w) = wait_ms {
                let n = total_dequeued as f64;
                metrics.queue_metrics.avg_wait_ms =
                    metrics.queue_metrics.avg_wait_ms * (n - 1.0) / n + w / n;
                if w > metrics.queue_metrics.max_wait_ms {
                    metrics.queue_metrics.max_wait_ms = w;
                }
            }
            let lane_metrics = metrics.queue_metrics.lanes.entry(lane.to_string()).or_default();
            lane_metrics.dequeue_count += 1;
            lane_metrics.current_size = queue_size;
        }
        let depth: u64 = metrics.queue_metrics.lanes
            .values()
            .map(|l| l.current_size)
            .sum();
        metrics.queue_metrics.current_depth = depth;
    }

    /// Import an openclaw SKILL.md into the skill memory store
    pub async fn import_skill_md(
        &self,
        skill_md_content: &str,
        skill_name: Option<&str>,
    ) -> Result<crate::api::skill_memory::Skill, String> {
        let skill_store = self.skill_memory.as_ref()
            .ok_or_else(|| "Skill memory store not available".to_string())?;

        // Parse SKILL.md frontmatter
        let parsed = parse_skill_md(skill_md_content)?;

        let name = skill_name.map(|s| s.to_string())
            .or(parsed.name.clone())
            .ok_or_else(|| "Skill name not found in SKILL.md".to_string())?;

        let description = parsed.description.clone()
            .unwrap_or_else(|| format!("Imported openclaw skill: {}", name));

        // Extract tags from metadata
        let mut tags = Vec::new();
        if let Some(ref emoji) = parsed.emoji {
            tags.push(format!("emoji:{}", emoji));
        }
        if let Some(ref bins) = parsed.required_bins {
            for bin in bins {
                tags.push(format!("requires:{}", bin));
            }
        }
        tags.push("openclaw".to_string());
        tags.push("imported".to_string());

        // Build definition from SKILL.md body
        let mut definition = serde_json::Map::new();
        definition.insert("type".to_string(), serde_json::json!("openclaw_skill_md"));
        definition.insert("body".to_string(), serde_json::json!(parsed.body));
        if let Some(ref homepage) = parsed.homepage {
            definition.insert("homepage".to_string(), serde_json::json!(homepage));
        }
        if let Some(ref os) = parsed.os {
            definition.insert("os".to_string(), serde_json::json!(os));
        }
        if let Some(ref install) = parsed.install_specs {
            definition.insert("install".to_string(), serde_json::json!(install));
        }

        let skill = crate::api::skill_memory::Skill {
            skill_id: Uuid::new_v4().to_string(),
            name,
            description,
            origin_bot: "openclaw".to_string(),
            category: parsed.category.unwrap_or_else(|| "imported".to_string()),
            tags,
            definition: serde_json::to_string(&serde_json::Value::Object(definition)).unwrap_or_default(),
            input_schema: None,
            output_schema: None,
            version: 1,
            invocation_count: 0,
            success_rate: 0.0,
            avg_duration_ms: 0.0,
            avg_tokens: 0.0,
            embedding: None,
            shared_with: vec!["moltbot".to_string(), "clawdbot".to_string(), "openclaw".to_string()],
            status: crate::api::skill_memory::SkillStatus::Active,
            parent_skill_id: None,
            episode_ids: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: HashMap::new(),
        };

        let created = skill_store.create_skill(skill).await?;
        info!("Imported openclaw skill: {}", created.name);
        Ok(created)
    }
}

// ── SKILL.md Parser ─────────────────────────────────────────────────────────

/// Parsed data from a SKILL.md file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedSkillMd {
    pub name: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub emoji: Option<String>,
    pub os: Option<Vec<String>>,
    pub required_bins: Option<Vec<String>>,
    pub any_bins: Option<Vec<String>>,
    pub required_env: Option<Vec<String>>,
    pub required_config: Option<Vec<String>>,
    pub install_specs: Option<Vec<serde_json::Value>>,
    pub category: Option<String>,
    pub body: String,
    pub always: bool,
}

/// Parse SKILL.md content with ```skill frontmatter block
pub fn parse_skill_md(content: &str) -> Result<ParsedSkillMd, String> {
    let mut result = ParsedSkillMd::default();
    result.body = content.to_string();

    // Look for ```skill ... ``` block
    let skill_block_start = content.find("```skill");
    let skill_block_end = if skill_block_start.is_some() {
        // Find the closing ``` after the opening ```skill
        let after_open = skill_block_start.unwrap() + "```skill".len();
        content[after_open..].find("```").map(|pos| after_open + pos)
    } else {
        None
    };

    if let (Some(start), Some(end)) = (skill_block_start, skill_block_end) {
        let block_content = &content[start + "```skill".len()..end].trim();

        // Extract YAML frontmatter between --- markers
        let lines: Vec<&str> = block_content.lines().collect();
        let mut in_frontmatter = false;
        let mut frontmatter_lines = Vec::new();

        for line in &lines {
            let trimmed = line.trim();
            if trimmed == "---" {
                if in_frontmatter {
                    break; // End of frontmatter
                } else {
                    in_frontmatter = true;
                    continue;
                }
            }
            if in_frontmatter {
                frontmatter_lines.push(*line);
            }
        }

        let frontmatter_str = frontmatter_lines.join("\n");

        // Parse frontmatter as YAML-ish key-value pairs
        for line in frontmatter_lines {
            let trimmed = line.trim();
            if let Some(pos) = trimmed.find(':') {
                let key = trimmed[..pos].trim();
                let value = trimmed[pos + 1..].trim();

                match key {
                    "name" => result.name = Some(value.to_string()),
                    "description" => result.description = Some(value.to_string()),
                    "homepage" => result.homepage = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        // Try to parse metadata JSON
        if let Some(meta_start) = frontmatter_str.find("metadata:") {
            let meta_str = &frontmatter_str[meta_start + "metadata:".len()..].trim();
            // Try to parse the rest as JSON
            if let Ok(meta_val) = serde_json::from_str::<serde_json::Value>(meta_str) {
                if let Some(oc) = meta_val.get("openclaw") {
                    result.emoji = oc.get("emoji")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    result.always = oc.get("always")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    result.os = oc.get("os")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect());

                    if let Some(requires) = oc.get("requires") {
                        result.required_bins = requires.get("bins")
                            .and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                        result.any_bins = requires.get("anyBins")
                            .and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                        result.required_env = requires.get("env")
                            .and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                        result.required_config = requires.get("config")
                            .and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                    }

                    result.install_specs = oc.get("install")
                        .and_then(|v| v.as_array())
                        .map(|a| a.to_vec());
                }
            }
        }

        // Body is everything after the ```skill block
        if end + 3 < content.len() {
            result.body = content[end + 3..].trim().to_string();
        }
    }

    Ok(result)
}

// ── Attribute Helpers ───────────────────────────────────────────────────────

fn attr_str(attrs: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
    attrs.get(key).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    })
}

fn attr_u64(attrs: &HashMap<String, serde_json::Value>, key: &str) -> u64 {
    attrs.get(key).and_then(|v| match v {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }).unwrap_or(0)
}

fn attr_f64(attrs: &HashMap<String, serde_json::Value>, key: &str) -> f64 {
    attrs.get(key).and_then(|v| match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_openclaw_by_service_name() {
        let mut resource = HashMap::new();
        resource.insert("service.name".to_string(), "openclaw".to_string());
        let attrs = HashMap::new();

        let source = detect_openclaw_source(&resource, &attrs);
        assert!(source.is_some());
        let s = source.unwrap();
        assert_eq!(s.bot_kind, "openclaw");
        assert!(!s.is_observability_plugin);
    }

    #[test]
    fn test_detect_openclaw_gateway_service_name() {
        let mut resource = HashMap::new();
        resource.insert("service.name".to_string(), "openclaw-gateway".to_string());
        let attrs = HashMap::new();

        let source = detect_openclaw_source(&resource, &attrs);
        assert!(source.is_some());
        let s = source.unwrap();
        assert_eq!(s.bot_kind, "openclaw");
        assert!(s.is_observability_plugin);
    }

    #[test]
    fn test_detect_by_openclaw_plugin_attr() {
        let mut resource = HashMap::new();
        resource.insert("service.name".to_string(), "openclaw-gateway".to_string());
        resource.insert("openclaw.plugin".to_string(), "otel-observability".to_string());
        let attrs = HashMap::new();

        let source = detect_openclaw_source(&resource, &attrs);
        assert!(source.is_some());
        assert!(source.unwrap().is_observability_plugin);
    }

    #[test]
    fn test_detect_moltbot_by_service_name() {
        let mut resource = HashMap::new();
        resource.insert("service.name".to_string(), "moltbot".to_string());
        let attrs = HashMap::new();

        let source = detect_openclaw_source(&resource, &attrs);
        assert!(source.is_some());
        assert_eq!(source.unwrap().bot_kind, "moltbot");
    }

    #[test]
    fn test_detect_by_openclaw_attributes() {
        let resource = HashMap::new();
        let mut attrs = HashMap::new();
        attrs.insert("openclaw.channel".to_string(), serde_json::json!("telegram"));

        let source = detect_openclaw_source(&resource, &attrs);
        assert!(source.is_some());
        assert_eq!(source.unwrap().bot_kind, "openclaw");
    }

    #[test]
    fn test_detect_by_plugin_span_attributes() {
        let resource = HashMap::new();
        let mut attrs = HashMap::new();
        attrs.insert("openclaw.session.key".to_string(), serde_json::json!("abc123"));
        attrs.insert("openclaw.agent.id".to_string(), serde_json::json!("agent-1"));

        let source = detect_openclaw_source(&resource, &attrs);
        assert!(source.is_some());
        let s = source.unwrap();
        assert_eq!(s.bot_kind, "openclaw");
        assert!(s.is_observability_plugin);
        assert_eq!(s.agent_id, Some("agent-1".to_string()));
        assert_eq!(s.session_key, Some("abc123".to_string()));
    }

    #[test]
    fn test_classify_openclaw_spans() {
        // Built-in diagnostics-otel spans
        assert_eq!(classify_openclaw_span("openclaw.model.usage"), OpenclawSpanKind::ModelUsage);
        assert_eq!(classify_openclaw_span("openclaw.webhook.processed"), OpenclawSpanKind::WebhookProcessed);
        assert_eq!(classify_openclaw_span("openclaw.webhook.error"), OpenclawSpanKind::WebhookError);
        assert_eq!(classify_openclaw_span("openclaw.message.processed"), OpenclawSpanKind::MessageProcessed);
        assert_eq!(classify_openclaw_span("openclaw.session.stuck"), OpenclawSpanKind::SessionStuck);
        assert_eq!(classify_openclaw_span("openclaw.skill.invoke"), OpenclawSpanKind::SkillInvocation);
        assert_eq!(classify_openclaw_span("openclaw.tool.bash"), OpenclawSpanKind::ToolCall);
        assert_eq!(classify_openclaw_span("openclaw.run.attempt"), OpenclawSpanKind::AgentLifecycle);

        // Observability plugin spans
        assert_eq!(classify_openclaw_span("openclaw.request"), OpenclawSpanKind::Request);
        assert_eq!(classify_openclaw_span("openclaw.agent.turn"), OpenclawSpanKind::AgentTurn);
        assert_eq!(classify_openclaw_span("tool.Read"), OpenclawSpanKind::ToolCall);
        assert_eq!(classify_openclaw_span("tool.exec"), OpenclawSpanKind::ToolCall);
        assert_eq!(classify_openclaw_span("openclaw.command.new"), OpenclawSpanKind::Command);
        assert_eq!(classify_openclaw_span("openclaw.command.reset"), OpenclawSpanKind::Command);
        assert_eq!(classify_openclaw_span("openclaw.command.stop"), OpenclawSpanKind::Command);
        assert_eq!(classify_openclaw_span("openclaw.gateway.startup"), OpenclawSpanKind::GatewayLifecycle);

        assert_eq!(classify_openclaw_span("something.else"), OpenclawSpanKind::Unknown);
    }

    #[test]
    fn test_parse_skill_md() {
        let content = r#"```skill
---
name: weather
description: Get current weather and forecasts
metadata: { "openclaw": { "emoji": "🌤️", "requires": { "bins": ["curl"] } } }
---
```

# Weather Skill

Use curl to fetch weather data from wttr.in.
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.name, Some("weather".to_string()));
        assert_eq!(parsed.description, Some("Get current weather and forecasts".to_string()));
        assert_eq!(parsed.emoji, Some("🌤️".to_string()));
        assert_eq!(parsed.required_bins, Some(vec!["curl".to_string()]));
        assert!(parsed.body.contains("Weather Skill"));
    }
}
