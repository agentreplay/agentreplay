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

use crate::config::LLMConfig;
use dashmap::DashMap;
use agentreplay_core::{AgentFlowEdge, SpanType};
use agentreplay_query::Agentreplay;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

mod providers;
pub use providers::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub provider: String,               // e.g., "openai", "anthropic"
    pub model: String,                  // Requested model
    pub response_model: Option<String>, // Actual model used (from response)
    pub response_id: Option<String>,    // Provider response ID
    pub tokens_used: Option<u32>,       // Total tokens (legacy)
    pub input_tokens: Option<u32>,      // Prompt tokens
    pub output_tokens: Option<u32>,     // Completion tokens
    pub finish_reason: Option<String>,  // stop/length/tool_calls
    pub duration_ms: u32,
}

pub struct LLMProviderManager {
    providers: DashMap<String, Arc<dyn LLMProvider>>,
    db: Arc<Agentreplay>,
}

#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<ChatResponse>;

    async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<String>>;

    fn list_models(&self) -> Vec<String>;
    fn name(&self) -> &str;
}

impl LLMProviderManager {
    pub async fn new(db: Arc<Agentreplay>, llm_config: &LLMConfig) -> anyhow::Result<Self> {
        let providers = DashMap::new();

        // Initialize OpenAI if key present
        if let Some(key) = &llm_config.openai_api_key {
            let provider = Arc::new(OpenAIProvider::new(key.clone())?);
            providers.insert("openai".to_string(), provider as Arc<dyn LLMProvider>);
            info!("Initialized OpenAI provider");
        } else {
            warn!("OPENAI_API_KEY not set, OpenAI provider disabled");
        }

        // Initialize Anthropic if key present
        if let Some(key) = &llm_config.anthropic_api_key {
            let provider = Arc::new(AnthropicProvider::new(key.clone())?);
            providers.insert("anthropic".to_string(), provider as Arc<dyn LLMProvider>);
            info!("Initialized Anthropic provider");
        } else {
            warn!("ANTHROPIC_API_KEY not set, Anthropic provider disabled");
        }

        // Initialize DeepSeek if key present
        if let Some(key) = &llm_config.deepseek_api_key {
            let provider = Arc::new(DeepSeekProvider::new(key.clone())?);
            providers.insert("deepseek".to_string(), provider as Arc<dyn LLMProvider>);
            info!("Initialized DeepSeek provider");
        } else {
            warn!("DEEPSEEK_API_KEY not set, DeepSeek provider disabled");
        }

        // Initialize Ollama (local, no key needed)
        if let Some(base_url) = &llm_config.ollama_base_url {
            let provider = Arc::new(OllamaProvider::new(base_url.clone())?);
            providers.insert("ollama".to_string(), provider as Arc<dyn LLMProvider>);
            info!("Initialized Ollama provider");
        }

        Ok(Self { providers, db })
    }

    pub async fn chat(
        &self,
        provider_id: &str,
        model: Option<String>,
        messages: Vec<ChatMessage>,
        tenant_id: u64,
        session_id: u64,
    ) -> anyhow::Result<ChatResponse> {
        // Get provider
        let provider = self
            .providers
            .get(provider_id)
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_id))?;

        // Create request edge for tracing
        let request_edge = AgentFlowEdge::new(
            1, // TODO: Get from auth context
            0, // TODO: Get from auth context
            0, // agent_id
            0, // session_id
            SpanType::ToolCall,
            0, // no parent
        );
        let request_id = request_edge.edge_id;
        self.db.insert(request_edge).await?;

        // Call LLM
        let start = Instant::now();
        let response = provider.chat(messages, model).await?;
        let duration_ms = start.elapsed().as_millis() as u32;

        // Log response edge with OpenTelemetry GenAI attributes
        let mut response_edge = AgentFlowEdge::new(
            tenant_id,
            0,
            self.hash_provider(provider_id),
            session_id,
            SpanType::ToolResponse,
            request_id,
        );
        response_edge.duration_us = duration_ms * 1000;
        response_edge.token_count = response.tokens_used.unwrap_or(0);

        // Store OpenTelemetry GenAI attributes in payload
        use std::collections::HashMap;
        let mut attributes = HashMap::new();

        // Provider identification
        attributes.insert("gen_ai.system".to_string(), response.provider.clone());
        attributes.insert("gen_ai.operation.name".to_string(), "chat".to_string());

        // Model information
        attributes.insert("gen_ai.request.model".to_string(), response.model.clone());
        if let Some(rm) = &response.response_model {
            attributes.insert("gen_ai.response.model".to_string(), rm.clone());
        }
        if let Some(rid) = &response.response_id {
            attributes.insert("gen_ai.response.id".to_string(), rid.clone());
        }

        // Token usage (OpenTelemetry standard)
        if let Some(it) = response.input_tokens {
            attributes.insert("gen_ai.usage.input_tokens".to_string(), it.to_string());
        }
        if let Some(ot) = response.output_tokens {
            attributes.insert("gen_ai.usage.output_tokens".to_string(), ot.to_string());
        }
        if let Some(tt) = response.tokens_used {
            attributes.insert("gen_ai.usage.total_tokens".to_string(), tt.to_string());
        }

        // Finish reason
        if let Some(fr) = &response.finish_reason {
            attributes.insert(
                "gen_ai.response.finish_reasons".to_string(),
                serde_json::to_string(&vec![fr]).unwrap_or_default(),
            );
        }

        // Backward compatibility: keep legacy token_count attribute
        if let Some(tc) = response.tokens_used {
            attributes.insert("token_count".to_string(), tc.to_string());
        }

        // Store attributes as payload
        if let Ok(payload_bytes) = serde_json::to_vec(&attributes) {
            let _ = self.db.put_payload(response_edge.edge_id, &payload_bytes);
        }

        self.db.insert(response_edge).await?;

        Ok(response)
    }

    pub async fn stream_chat(
        &self,
        provider_id: &str,
        model: Option<String>,
        messages: Vec<ChatMessage>,
        tenant_id: u64,
        session_id: u64,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<String>> {
        let provider = self
            .providers
            .get(provider_id)
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_id))?;

        // Log request edge
        let request_edge = AgentFlowEdge::new(
            tenant_id,
            0,
            self.hash_provider(provider_id),
            session_id,
            SpanType::ToolCall,
            0,
        );
        let _request_id = request_edge.edge_id;
        self.db.insert(request_edge).await?;

        provider.stream_chat(messages, model).await
    }

    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.providers
            .iter()
            .map(|entry| {
                let (id, provider) = entry.pair();
                ProviderInfo {
                    id: id.clone(),
                    name: provider.name().to_string(),
                    available: true,
                    models: provider.list_models(),
                }
            })
            .collect()
    }

    fn hash_provider(&self, provider_id: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        provider_id.hash(&mut hasher);
        hasher.finish()
    }
}
