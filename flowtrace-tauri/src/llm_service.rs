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

//! Unified LLM Service
//!
//! Combines the `llm` crate for provider backends with our ModelPricingRegistry
//! for pricing data. Provides a single source of truth for:
//! - Provider configuration (vendor, base_url, api_key)
//! - Model listing and capabilities
//! - Cost calculation
//! - Secure credential storage
//!
//! Design principles:
//! - OpenAI-compatible API standards (all providers expose similar interface)
//! - Single source of truth - no hardcoded model lists
//! - Secure credential storage via tauri-plugin-store
//! - LiteLLM pricing sync for accurate cost calculation

use anyhow::{anyhow, Result};
use flowtrace_core::model_pricing::{ModelPricing, ModelPricingRegistry};
use llm::builder::LLMBackend;
use llm::chat::{ChatMessage, ChatRole};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Provider configuration following OpenAI-compatible standards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider identifier (openai, anthropic, ollama, deepseek, google, mistral)
    pub vendor: String,
    /// Display name for UI
    pub display_name: String,
    /// API base URL (for custom deployments)
    pub base_url: Option<String>,
    /// API key (stored securely, not serialized)
    #[serde(skip)]
    pub api_key: Option<String>,
    /// Whether this provider is enabled
    pub enabled: bool,
    /// Organization ID (for OpenAI)
    pub org_id: Option<String>,
    /// Custom headers (for proxies)
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            vendor: "ollama".to_string(),
            display_name: "Ollama (Local)".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            api_key: None,
            enabled: true,
            org_id: None,
            headers: HashMap::new(),
        }
    }
}

impl ProviderConfig {
    /// Create OpenAI provider config
    pub fn openai(api_key: String) -> Self {
        Self {
            vendor: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            api_key: Some(api_key),
            enabled: true,
            org_id: None,
            headers: HashMap::new(),
        }
    }

    /// Create Anthropic provider config
    pub fn anthropic(api_key: String) -> Self {
        Self {
            vendor: "anthropic".to_string(),
            display_name: "Anthropic".to_string(),
            base_url: Some("https://api.anthropic.com".to_string()),
            api_key: Some(api_key),
            enabled: true,
            org_id: None,
            headers: HashMap::new(),
        }
    }

    /// Create Ollama provider config (local, no API key)
    pub fn ollama() -> Self {
        Self {
            vendor: "ollama".to_string(),
            display_name: "Ollama (Local)".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            api_key: None,
            enabled: true,
            org_id: None,
            headers: HashMap::new(),
        }
    }

    /// Create DeepSeek provider config
    pub fn deepseek(api_key: String) -> Self {
        Self {
            vendor: "deepseek".to_string(),
            display_name: "DeepSeek".to_string(),
            base_url: Some("https://api.deepseek.com".to_string()),
            api_key: Some(api_key),
            enabled: true,
            org_id: None,
            headers: HashMap::new(),
        }
    }

    /// Create Google (Gemini) provider config
    pub fn google(api_key: String) -> Self {
        Self {
            vendor: "google".to_string(),
            display_name: "Google AI".to_string(),
            base_url: None,
            api_key: Some(api_key),
            enabled: true,
            org_id: None,
            headers: HashMap::new(),
        }
    }

    /// Create Mistral provider config
    pub fn mistral(api_key: String) -> Self {
        Self {
            vendor: "mistral".to_string(),
            display_name: "Mistral AI".to_string(),
            base_url: Some("https://api.mistral.ai".to_string()),
            api_key: Some(api_key),
            enabled: true,
            org_id: None,
            headers: HashMap::new(),
        }
    }

    /// Check if this provider requires an API key
    pub fn requires_api_key(&self) -> bool {
        self.vendor != "ollama"
    }

    /// Check if this provider is properly configured
    pub fn is_configured(&self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.requires_api_key() {
            self.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false)
        } else {
            true // Ollama doesn't need API key
        }
    }
}

/// Model information combining provider details with pricing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "gpt-4o", "claude-3-5-sonnet-20241022")
    pub model_id: String,
    /// Display name for UI
    pub display_name: String,
    /// Provider vendor (openai, anthropic, etc.)
    pub vendor: String,
    /// Input cost per 1M tokens (USD)
    pub input_cost_per_1m: f64,
    /// Output cost per 1M tokens (USD)
    pub output_cost_per_1m: f64,
    /// Context window size
    pub context_window: Option<u32>,
    /// Whether the model is available (provider configured)
    pub available: bool,
    /// Supports function/tool calling
    pub supports_function_calling: bool,
    /// Supports vision
    pub supports_vision: bool,
    /// Mode (chat, completion, embedding)
    pub mode: String,
}

impl From<(&str, &ModelPricing, bool)> for ModelInfo {
    fn from((model_id, pricing, available): (&str, &ModelPricing, bool)) -> Self {
        Self {
            model_id: model_id.to_string(),
            display_name: format_model_display_name(model_id),
            vendor: pricing.litellm_provider.clone().unwrap_or_else(|| "unknown".to_string()),
            input_cost_per_1m: pricing.input_cost_per_token * 1_000_000.0,
            output_cost_per_1m: pricing.output_cost_per_token * 1_000_000.0,
            context_window: pricing.context_window.or(pricing.max_input_tokens),
            available,
            supports_function_calling: pricing.supports_function_calling,
            supports_vision: pricing.supports_vision,
            mode: pricing.mode.clone().unwrap_or_else(|| "chat".to_string()),
        }
    }
}

/// Completion request (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> u32 {
    2048
}

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Convert our Message to llm crate's ChatMessage
/// Note: System messages must be handled separately via LLMBuilder::system_prompt()
fn to_chat_message(msg: &Message) -> ChatMessage {
    let role = match msg.role.as_str() {
        "assistant" => ChatRole::Assistant,
        _ => ChatRole::User, // user, system (system should be filtered out)
    };
    ChatMessage {
        role,
        message_type: llm::chat::MessageType::Text,
        content: msg.content.clone(),
    }
}

/// Completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub latency_ms: u64,
    pub cost_usd: f64,
    pub finish_reason: String,
}

/// Unified LLM Service
pub struct LLMService {
    /// Provider configurations
    providers: Arc<RwLock<HashMap<String, ProviderConfig>>>,
    /// Model pricing registry
    pricing_registry: Arc<ModelPricingRegistry>,
    /// HTTP client for raw requests
    http_client: reqwest::Client,
    /// Data directory for caching
    data_dir: PathBuf,
}

impl LLMService {
    /// Create a new LLM service
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let pricing_registry = Arc::new(ModelPricingRegistry::new(data_dir.clone()));

        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            pricing_registry,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            data_dir,
        }
    }

    /// Initialize the service (load pricing, check providers)
    pub async fn initialize(&self) -> Result<()> {
        // Initialize pricing registry
        self.pricing_registry.initialize().await?;

        // Add default Ollama provider (always available locally)
        self.register_provider(ProviderConfig::ollama()).await;

        // Check Ollama availability
        if self.check_ollama_available().await {
            tracing::info!("Ollama is available at localhost:11434");
        }

        Ok(())
    }

    /// Register a provider configuration
    pub async fn register_provider(&self, config: ProviderConfig) {
        let mut providers = self.providers.write().await;
        providers.insert(config.vendor.clone(), config);
    }

    /// Register a provider with API key from secure store
    pub async fn register_provider_with_key(
        &self,
        vendor: &str,
        api_key: String,
        base_url: Option<String>,
    ) -> Result<()> {
        let config = match vendor {
            "openai" => {
                let mut c = ProviderConfig::openai(api_key);
                if let Some(url) = base_url {
                    c.base_url = Some(url);
                }
                c
            }
            "anthropic" => {
                let mut c = ProviderConfig::anthropic(api_key);
                if let Some(url) = base_url {
                    c.base_url = Some(url);
                }
                c
            }
            "deepseek" => {
                let mut c = ProviderConfig::deepseek(api_key);
                if let Some(url) = base_url {
                    c.base_url = Some(url);
                }
                c
            }
            "google" => {
                let mut c = ProviderConfig::google(api_key);
                if let Some(url) = base_url {
                    c.base_url = Some(url);
                }
                c
            }
            "mistral" => {
                let mut c = ProviderConfig::mistral(api_key);
                if let Some(url) = base_url {
                    c.base_url = Some(url);
                }
                c
            }
            "ollama" => ProviderConfig::ollama(),
            _ => return Err(anyhow!("Unknown provider vendor: {}", vendor)),
        };

        self.register_provider(config).await;
        Ok(())
    }

    /// Get configured providers
    pub async fn get_configured_providers(&self) -> Vec<ProviderConfig> {
        let providers = self.providers.read().await;
        providers
            .values()
            .filter(|p| p.is_configured())
            .cloned()
            .collect()
    }

    /// List all available models from the pricing registry
    /// Only returns models for configured providers
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        let configured_vendors: Vec<String> = {
            let providers = self.providers.read().await;
            providers
                .values()
                .filter(|p| p.is_configured())
                .map(|p| p.vendor.clone())
                .collect()
        };

        let all_models = self.pricing_registry.list_models().await;

        all_models
            .into_iter()
            .map(|(model_id, pricing)| {
                let vendor = pricing.litellm_provider.clone().unwrap_or_default();
                let available = configured_vendors.iter().any(|v| v.eq_ignore_ascii_case(&vendor));
                ModelInfo::from((model_id.as_str(), &pricing, available))
            })
            .filter(|m| m.available) // Only return models for configured providers
            .collect()
    }

    /// List models for a specific provider
    pub async fn list_models_by_provider(&self, vendor: &str) -> Vec<ModelInfo> {
        let provider_configured = {
            let providers = self.providers.read().await;
            providers
                .get(vendor)
                .map(|p| p.is_configured())
                .unwrap_or(false)
        };

        if !provider_configured {
            return vec![];
        }

        let models = self.pricing_registry.list_models_by_provider(vendor).await;

        models
            .into_iter()
            .map(|(model_id, pricing)| ModelInfo::from((model_id.as_str(), &pricing, true)))
            .collect()
    }

    /// Get model info with pricing
    pub async fn get_model_info(&self, model_id: &str) -> Option<ModelInfo> {
        let pricing = self.pricing_registry.get_pricing(model_id).await?;
        let vendor = pricing.litellm_provider.clone().unwrap_or_default();

        let available = {
            let providers = self.providers.read().await;
            providers
                .get(&vendor)
                .map(|p| p.is_configured())
                .unwrap_or(false)
        };

        Some(ModelInfo::from((model_id, &pricing, available)))
    }

    /// Calculate cost for a completion
    pub async fn calculate_cost(&self, model_id: &str, input_tokens: u32, output_tokens: u32) -> f64 {
        self.pricing_registry
            .calculate_cost(model_id, input_tokens, output_tokens)
            .await
    }

    /// Run a completion request
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let vendor = self.detect_vendor(&request.model);
        
        let provider_config = {
            let providers = self.providers.read().await;
            providers.get(&vendor).cloned()
        };

        let config = provider_config
            .ok_or_else(|| anyhow!("Provider '{}' not configured", vendor))?;

        if !config.is_configured() {
            return Err(anyhow!(
                "Provider '{}' requires API key. Configure it in Settings.",
                vendor
            ));
        }

        let start = std::time::Instant::now();

        // Use the llm crate for the actual API call
        let response = self.call_provider(&config, &request).await?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Calculate cost from pricing registry
        let cost_usd = self
            .calculate_cost(&request.model, response.input_tokens, response.output_tokens)
            .await;

        Ok(CompletionResponse {
            content: response.content,
            model: request.model,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
            latency_ms,
            cost_usd,
            finish_reason: response.finish_reason,
        })
    }

    /// Sync pricing from LiteLLM
    pub async fn sync_pricing(&self) -> Result<usize> {
        self.pricing_registry
            .sync_from_litellm()
            .await
            .map_err(|e| anyhow!("Failed to sync pricing: {}", e))
    }

    /// Check if Ollama is available
    pub async fn check_ollama_available(&self) -> bool {
        let response = self
            .http_client
            .get("http://localhost:11434/api/tags")
            .timeout(Duration::from_secs(3))
            .send()
            .await;

        response.is_ok()
    }

    /// List Ollama models (if available)
    pub async fn list_ollama_models(&self) -> Vec<String> {
        #[derive(Deserialize)]
        struct OllamaModelList {
            models: Vec<OllamaModel>,
        }

        #[derive(Deserialize)]
        struct OllamaModel {
            name: String,
        }

        let response = self
            .http_client
            .get("http://localhost:11434/api/tags")
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(list) = resp.json::<OllamaModelList>().await {
                    return list.models.into_iter().map(|m| m.name).collect();
                }
            }
            _ => {}
        }

        vec![]
    }

    // ============ Private methods ============

    /// Detect vendor from model name
    fn detect_vendor(&self, model: &str) -> String {
        if model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3") {
            "openai".to_string()
        } else if model.starts_with("claude") {
            "anthropic".to_string()
        } else if model.starts_with("deepseek") {
            "deepseek".to_string()
        } else if model.starts_with("gemini") {
            "google".to_string()
        } else if model.starts_with("mistral") || model.starts_with("mixtral") {
            "mistral".to_string()
        } else {
            // Default to Ollama for unknown models (likely local)
            "ollama".to_string()
        }
    }

    /// Call the provider using the llm crate
    async fn call_provider(
        &self,
        config: &ProviderConfig,
        request: &CompletionRequest,
    ) -> Result<ProviderResponse> {
        match config.vendor.as_str() {
            "openai" => self.call_openai(config, request).await,
            "anthropic" => self.call_anthropic(config, request).await,
            "ollama" => self.call_ollama(config, request).await,
            "deepseek" => self.call_deepseek(config, request).await,
            "google" => self.call_google(config, request).await,
            "mistral" => self.call_mistral(config, request).await,
            _ => Err(anyhow!("Unsupported provider: {}", config.vendor)),
        }
    }

    /// Extract system prompt from messages
    fn extract_system_prompt(messages: &[Message]) -> Option<String> {
        messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
    }

    /// Filter out system messages and convert to ChatMessage
    fn prepare_messages(messages: &[Message]) -> Vec<ChatMessage> {
        messages
            .iter()
            .filter(|m| m.role != "system")
            .map(to_chat_message)
            .collect()
    }

    /// Call OpenAI API
    async fn call_openai(
        &self,
        config: &ProviderConfig,
        request: &CompletionRequest,
    ) -> Result<ProviderResponse> {
        let api_key = config.api_key.as_ref()
            .ok_or_else(|| anyhow!("OpenAI API key not configured"))?;

        // Build the LLM provider using the llm crate
        let mut builder = llm::builder::LLMBuilder::new()
            .backend(LLMBackend::OpenAI)
            .api_key(api_key.clone())
            .model(request.model.clone())
            .temperature(request.temperature)
            .max_tokens(request.max_tokens);

        // Add system prompt if present
        if let Some(system) = Self::extract_system_prompt(&request.messages) {
            builder = builder.system(system);
        }

        let llm = builder.build()?;

        // Convert messages (excluding system)
        let messages = Self::prepare_messages(&request.messages);

        // Call the API
        let response = llm.chat(&messages).await?;
        let usage = response.usage();

        Ok(ProviderResponse {
            content: response.text().unwrap_or_default(),
            input_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            output_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            finish_reason: "stop".to_string(),
        })
    }

    /// Call Anthropic API
    async fn call_anthropic(
        &self,
        config: &ProviderConfig,
        request: &CompletionRequest,
    ) -> Result<ProviderResponse> {
        let api_key = config.api_key.as_ref()
            .ok_or_else(|| anyhow!("Anthropic API key not configured"))?;

        let mut builder = llm::builder::LLMBuilder::new()
            .backend(LLMBackend::Anthropic)
            .api_key(api_key.clone())
            .model(request.model.clone())
            .temperature(request.temperature)
            .max_tokens(request.max_tokens);

        if let Some(system) = Self::extract_system_prompt(&request.messages) {
            builder = builder.system(system);
        }

        let llm = builder.build()?;
        let messages = Self::prepare_messages(&request.messages);
        let response = llm.chat(&messages).await?;
        let usage = response.usage();

        Ok(ProviderResponse {
            content: response.text().unwrap_or_default(),
            input_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            output_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            finish_reason: "stop".to_string(),
        })
    }

    /// Call Ollama API (local)
    async fn call_ollama(
        &self,
        config: &ProviderConfig,
        request: &CompletionRequest,
    ) -> Result<ProviderResponse> {
        let base_url = config.base_url.as_deref().unwrap_or("http://localhost:11434");

        let mut builder = llm::builder::LLMBuilder::new()
            .backend(LLMBackend::Ollama)
            .base_url(base_url)
            .model(request.model.clone())
            .temperature(request.temperature)
            .max_tokens(request.max_tokens);

        if let Some(system) = Self::extract_system_prompt(&request.messages) {
            builder = builder.system(system);
        }

        let llm = builder.build()?;
        let messages = Self::prepare_messages(&request.messages);
        let response = llm.chat(&messages).await?;
        let usage = response.usage();

        Ok(ProviderResponse {
            content: response.text().unwrap_or_default(),
            input_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            output_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            finish_reason: "stop".to_string(),
        })
    }

    /// Call DeepSeek API
    async fn call_deepseek(
        &self,
        config: &ProviderConfig,
        request: &CompletionRequest,
    ) -> Result<ProviderResponse> {
        let api_key = config.api_key.as_ref()
            .ok_or_else(|| anyhow!("DeepSeek API key not configured"))?;

        let mut builder = llm::builder::LLMBuilder::new()
            .backend(LLMBackend::DeepSeek)
            .api_key(api_key.clone())
            .model(request.model.clone())
            .temperature(request.temperature)
            .max_tokens(request.max_tokens);

        if let Some(system) = Self::extract_system_prompt(&request.messages) {
            builder = builder.system(system);
        }

        let llm = builder.build()?;
        let messages = Self::prepare_messages(&request.messages);
        let response = llm.chat(&messages).await?;
        let usage = response.usage();

        Ok(ProviderResponse {
            content: response.text().unwrap_or_default(),
            input_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            output_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            finish_reason: "stop".to_string(),
        })
    }

    /// Call Google (Gemini) API
    async fn call_google(
        &self,
        config: &ProviderConfig,
        request: &CompletionRequest,
    ) -> Result<ProviderResponse> {
        let api_key = config.api_key.as_ref()
            .ok_or_else(|| anyhow!("Google API key not configured"))?;

        let mut builder = llm::builder::LLMBuilder::new()
            .backend(LLMBackend::Google)
            .api_key(api_key.clone())
            .model(request.model.clone())
            .temperature(request.temperature)
            .max_tokens(request.max_tokens);

        if let Some(system) = Self::extract_system_prompt(&request.messages) {
            builder = builder.system(system);
        }

        let llm = builder.build()?;
        let messages = Self::prepare_messages(&request.messages);
        let response = llm.chat(&messages).await?;
        let usage = response.usage();

        Ok(ProviderResponse {
            content: response.text().unwrap_or_default(),
            input_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            output_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            finish_reason: "stop".to_string(),
        })
    }

    /// Call Mistral API
    async fn call_mistral(
        &self,
        config: &ProviderConfig,
        request: &CompletionRequest,
    ) -> Result<ProviderResponse> {
        let api_key = config.api_key.as_ref()
            .ok_or_else(|| anyhow!("Mistral API key not configured"))?;

        let mut builder = llm::builder::LLMBuilder::new()
            .backend(LLMBackend::Mistral)
            .api_key(api_key.clone())
            .model(request.model.clone())
            .temperature(request.temperature)
            .max_tokens(request.max_tokens);

        if let Some(system) = Self::extract_system_prompt(&request.messages) {
            builder = builder.system(system);
        }

        let llm = builder.build()?;
        let messages = Self::prepare_messages(&request.messages);
        let response = llm.chat(&messages).await?;
        let usage = response.usage();

        Ok(ProviderResponse {
            content: response.text().unwrap_or_default(),
            input_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            output_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            finish_reason: "stop".to_string(),
        })
    }
}

/// Internal response from provider
struct ProviderResponse {
    content: String,
    input_tokens: u32,
    output_tokens: u32,
    finish_reason: String,
}

/// Format a model ID into a display name
fn format_model_display_name(model_id: &str) -> String {
    // Handle known model patterns
    if model_id.starts_with("gpt-4o-mini") {
        return "GPT-4o Mini".to_string();
    }
    if model_id.starts_with("gpt-4o") {
        return "GPT-4o".to_string();
    }
    if model_id.starts_with("gpt-4-turbo") {
        return "GPT-4 Turbo".to_string();
    }
    if model_id.starts_with("gpt-4") {
        return "GPT-4".to_string();
    }
    if model_id.starts_with("gpt-3.5") {
        return "GPT-3.5 Turbo".to_string();
    }
    if model_id.starts_with("o1-mini") {
        return "o1 Mini".to_string();
    }
    if model_id.starts_with("o1") {
        return "o1 Preview".to_string();
    }
    if model_id.starts_with("o3-mini") {
        return "o3 Mini".to_string();
    }
    if model_id.contains("claude-3-5-sonnet") {
        return "Claude 3.5 Sonnet".to_string();
    }
    if model_id.contains("claude-3-5-haiku") {
        return "Claude 3.5 Haiku".to_string();
    }
    if model_id.contains("claude-3-opus") {
        return "Claude 3 Opus".to_string();
    }
    if model_id.contains("claude-3-haiku") {
        return "Claude 3 Haiku".to_string();
    }
    if model_id.starts_with("deepseek-chat") {
        return "DeepSeek Chat".to_string();
    }
    if model_id.starts_with("deepseek-coder") {
        return "DeepSeek Coder".to_string();
    }
    if model_id.starts_with("gemini-2.0") {
        return "Gemini 2.0".to_string();
    }
    if model_id.starts_with("gemini-1.5-pro") {
        return "Gemini 1.5 Pro".to_string();
    }
    if model_id.starts_with("gemini-1.5-flash") {
        return "Gemini 1.5 Flash".to_string();
    }
    if model_id.starts_with("mistral-large") {
        return "Mistral Large".to_string();
    }
    if model_id.starts_with("mistral-small") {
        return "Mistral Small".to_string();
    }

    // For Ollama/local models, capitalize first letter
    let mut chars = model_id.chars();
    match chars.next() {
        None => model_id.to_string(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_requires_api_key() {
        assert!(ProviderConfig::openai("key".to_string()).requires_api_key());
        assert!(ProviderConfig::anthropic("key".to_string()).requires_api_key());
        assert!(!ProviderConfig::ollama().requires_api_key());
    }

    #[test]
    fn test_provider_config_is_configured() {
        let openai = ProviderConfig::openai("sk-xxx".to_string());
        assert!(openai.is_configured());

        let openai_empty = ProviderConfig {
            vendor: "openai".to_string(),
            api_key: Some("".to_string()),
            ..Default::default()
        };
        assert!(!openai_empty.is_configured());

        let ollama = ProviderConfig::ollama();
        assert!(ollama.is_configured());
    }

    #[test]
    fn test_format_model_display_name() {
        assert_eq!(format_model_display_name("gpt-4o"), "GPT-4o");
        assert_eq!(format_model_display_name("gpt-4o-mini"), "GPT-4o Mini");
        assert_eq!(
            format_model_display_name("claude-3-5-sonnet-20241022"),
            "Claude 3.5 Sonnet"
        );
        assert_eq!(format_model_display_name("llama3.2"), "Llama3.2");
    }
}
