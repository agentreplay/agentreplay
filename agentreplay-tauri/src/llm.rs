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

//! LLM integration module for Agentreplay
//!
//! Supports multiple LLM providers with tag-based routing:
//! - Ollama (local, no API key required)
//! - OpenAI (requires API key)
//! - Anthropic (requires API key)
//!
//! ## Tag-Based Routing
//! Providers can have tags like `default`, `eval`, `embedding`, `chat`.
//! The router selects the appropriate provider based on the requested purpose.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Purpose/tag for routing LLM requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LLMPurpose {
    /// Default purpose - general chat/completion
    Default,
    /// Evaluation (G-EVAL, LLM-as-judge)
    Eval,
    /// Embeddings generation
    Embedding,
    /// Analysis (trace analysis, insights)
    Analysis,
    /// Chat (playground, conversations)
    Chat,
}

impl LLMPurpose {
    pub fn as_str(&self) -> &'static str {
        match self {
            LLMPurpose::Default => "default",
            LLMPurpose::Eval => "eval",
            LLMPurpose::Embedding => "embedding",
            LLMPurpose::Analysis => "analysis",
            LLMPurpose::Chat => "chat",
        }
    }
    
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "eval" | "evaluation" => LLMPurpose::Eval,
            "embedding" | "embeddings" => LLMPurpose::Embedding,
            "analysis" | "analyze" => LLMPurpose::Analysis,
            "chat" | "conversation" => LLMPurpose::Chat,
            _ => LLMPurpose::Default,
        }
    }
}

/// Configuration for an LLM provider with tag support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMProviderConfig {
    pub provider: String, // "ollama", "openai", "anthropic", "custom"
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub enabled: bool,
    /// Model name for this provider
    #[serde(default)]
    pub model: Option<String>,
    /// Tags for routing (e.g., ["default", "eval"])
    #[serde(default)]
    pub tags: Vec<String>,
    /// Display name
    #[serde(default)]
    pub name: Option<String>,
}

impl Default for LLMProviderConfig {
    fn default() -> Self {
        Self {
            provider: "ollama".to_string(),
            api_key: None,
            base_url: None,
            enabled: true,
            model: None,
            tags: vec!["default".to_string()],
            name: None,
        }
    }
}

/// Full LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LLMConfig {
    pub providers: Vec<LLMProviderConfig>,
    pub default_model: String,
    pub default_temperature: f32,
    pub default_max_tokens: u32,
}

impl LLMConfig {
    pub fn default_with_ollama() -> Self {
        Self {
            providers: vec![
                LLMProviderConfig {
                    provider: "ollama".to_string(),
                    api_key: None,
                    base_url: Some("http://localhost:11434".to_string()),
                    enabled: true,
                    model: Some("llama3.2".to_string()),
                    tags: vec!["default".to_string()],
                    name: Some("Local Ollama".to_string()),
                },
            ],
            default_model: "llama3.2".to_string(),
            default_temperature: 0.7,
            default_max_tokens: 2048,
        }
    }
    
    /// Find provider by tag, falling back to default
    pub fn find_provider_for_purpose(&self, purpose: LLMPurpose) -> Option<&LLMProviderConfig> {
        let purpose_tag = purpose.as_str();
        
        // First try to find a provider with the specific tag
        let specific = self.providers.iter()
            .find(|p| p.enabled && p.tags.iter().any(|t| t == purpose_tag));
        
        if specific.is_some() {
            tracing::debug!("Found provider with specific tag '{}': {:?}", purpose_tag, specific.as_ref().map(|p| &p.name));
            return specific;
        }
        
        // Fall back to provider with "default" tag
        let default = self.providers.iter()
            .find(|p| p.enabled && p.tags.iter().any(|t| t == "default"));
        
        if default.is_some() {
            tracing::debug!("Using default-tagged provider for purpose '{}': {:?}", purpose_tag, default.as_ref().map(|p| &p.name));
            return default;
        }
        
        // Last resort: use first enabled provider
        let first_enabled = self.providers.iter().find(|p| p.enabled);
        if first_enabled.is_some() {
            tracing::debug!("Using first enabled provider for purpose '{}': {:?}", purpose_tag, first_enabled.as_ref().map(|p| &p.name));
        }
        first_enabled
    }
}

/// Request for LLM completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Chat message format (OpenAI compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
}

/// Response from LLM completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMCompletionResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
    pub finish_reason: String,
    pub latency_ms: u64,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// LLM client for making completions
pub struct LLMClient {
    http_client: reqwest::Client,
    config: LLMConfig,
}

impl LLMClient {
    pub fn new(config: LLMConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http_client,
            config,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(LLMConfig::default_with_ollama())
    }

    /// Check if any LLM provider is configured and potentially available
    pub fn is_configured(&self) -> bool {
        // Check if we have any enabled providers
        for provider in &self.config.providers {
            if provider.enabled {
                // Ollama doesn't need API key, just needs to be running
                if provider.provider == "ollama" {
                    return true;
                }
                // Other providers need API key
                if provider.api_key.is_some() {
                    return true;
                }
            }
        }
        // Default to true if using default config (Ollama)
        !self.config.providers.is_empty()
    }
    
    /// Check if a specific purpose has a configured provider
    pub fn is_configured_for(&self, purpose: LLMPurpose) -> bool {
        self.config.find_provider_for_purpose(purpose).is_some()
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: LLMConfig) {
        self.config = config;
    }
    
    /// Get the default model from configuration
    pub fn get_default_model(&self) -> &str {
        &self.config.default_model
    }
    
    /// Get the model for a specific purpose
    pub fn get_model_for(&self, purpose: LLMPurpose) -> Option<String> {
        self.config.find_provider_for_purpose(purpose)
            .and_then(|p| p.model.clone())
            .or_else(|| Some(self.config.default_model.clone()))
    }
    
    /// Get the full configuration (read-only)
    pub fn get_config(&self) -> &LLMConfig {
        &self.config
    }
    
    /// Get cost per token (input, output) for the default model
    /// Returns approximate costs in USD
    pub fn cost_per_token(&self) -> (f64, f64) {
        self.cost_per_token_for_model(&self.config.default_model)
    }
    
    /// Get cost per token for a specific model
    pub fn cost_per_token_for_model(&self, model: &str) -> (f64, f64) {
        // Approximate costs per token in USD
        match model {
            m if m.starts_with("gpt-4o") => (0.0000025, 0.00001),      // $2.50/$10 per 1M
            m if m.starts_with("gpt-4-turbo") => (0.00001, 0.00003),   // $10/$30 per 1M
            m if m.starts_with("gpt-4") => (0.00003, 0.00006),         // $30/$60 per 1M
            m if m.starts_with("gpt-3.5") => (0.0000005, 0.0000015),   // $0.50/$1.50 per 1M
            m if m.starts_with("claude-3-opus") => (0.000015, 0.000075), // $15/$75 per 1M
            m if m.starts_with("claude-3-sonnet") => (0.000003, 0.000015), // $3/$15 per 1M
            m if m.starts_with("claude-3-haiku") => (0.00000025, 0.00000125), // $0.25/$1.25 per 1M
            _ => (0.0, 0.0), // Ollama/local models are free
        }
    }
    
    /// Route a completion request to the appropriate provider based on purpose
    /// This is the main entry point for all LLM calls
    pub async fn complete_for_purpose(
        &self,
        purpose: LLMPurpose,
        messages: Vec<ChatMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<LLMCompletionResponse> {
        let provider = self.config.find_provider_for_purpose(purpose)
            .ok_or_else(|| anyhow!("No provider configured for purpose: {:?}", purpose))?;
        
        let model = provider.model.clone()
            .unwrap_or_else(|| self.config.default_model.clone());
        
        let base_url = provider.base_url.clone()
            .unwrap_or_else(|| self.get_default_base_url(&provider.provider));
        
        tracing::info!(
            "LLM Router: purpose={:?}, provider={}, model={}, base_url={}",
            purpose, provider.provider, model, base_url
        );
        
        let request = LLMCompletionRequest {
            model: model.clone(),
            messages,
            temperature: temperature.or(Some(self.config.default_temperature)),
            max_tokens: max_tokens.or(Some(self.config.default_max_tokens)),
            stream: Some(false),
        };
        
        self.chat_completion_with_provider(
            request,
            &provider.provider,
            &base_url,
            provider.api_key.as_deref(),
        ).await
    }
    
    fn get_default_base_url(&self, provider: &str) -> String {
        match provider {
            "ollama" => "http://localhost:11434".to_string(),
            "openai" => "https://api.openai.com/v1".to_string(),
            "anthropic" => "https://api.anthropic.com/v1".to_string(),
            _ => "http://localhost:11434".to_string(),
        }
    }

    /// Complete a chat request using explicit provider configuration
    /// This is the preferred method when you have user-configured providers
    pub async fn chat_completion_with_provider(
        &self,
        request: LLMCompletionRequest,
        provider_type: &str,
        base_url: &str,
        api_key: Option<&str>,
    ) -> Result<LLMCompletionResponse> {
        tracing::info!(
            "chat_completion_with_provider: provider={}, base_url={}, model={}, has_api_key={}",
            provider_type, base_url, request.model, api_key.is_some()
        );
        
        match provider_type {
            "ollama" => self.ollama_completion_with_url(request, base_url).await,
            "openai" | "custom" => {
                // OpenAI-compatible endpoint (works for OpenAI, Llama API, Together, Groq, etc.)
                self.openai_compatible_completion(request, base_url, api_key).await
            }
            "anthropic" => self.anthropic_completion_with_config(request, base_url, api_key).await,
            _ => {
                // Default to OpenAI-compatible for unknown providers
                tracing::info!("Unknown provider '{}', using OpenAI-compatible endpoint", provider_type);
                self.openai_compatible_completion(request, base_url, api_key).await
            }
        }
    }

    /// Complete a chat request (auto-detects provider from model name)
    /// Use chat_completion_with_provider when you have explicit provider config
    pub async fn chat_completion(&self, request: LLMCompletionRequest) -> Result<LLMCompletionResponse> {
        // Determine provider based on model name
        let provider = self.detect_provider(&request.model);

        match provider.as_str() {
            "ollama" => self.ollama_completion(request).await,
            "openai" => self.openai_completion(request).await,
            "anthropic" => self.anthropic_completion(request).await,
            _ => Err(anyhow!("Unknown provider for model: {}", request.model)),
        }
    }

    /// Detect provider from model name
    fn detect_provider(&self, model: &str) -> String {
        if model.starts_with("gpt-") || model.starts_with("o1") {
            "openai".to_string()
        } else if model.starts_with("claude") {
            "anthropic".to_string()
        } else {
            // Default to Ollama for local models
            "ollama".to_string()
        }
    }

    /// Get the base URL for a provider
    fn get_base_url(&self, provider: &str) -> String {
        // Check if there's a custom URL in config
        for p in &self.config.providers {
            if p.provider == provider {
                if let Some(url) = &p.base_url {
                    return url.clone();
                }
            }
        }

        // Default URLs
        match provider {
            "ollama" => "http://localhost:11434".to_string(),
            "openai" => "https://api.openai.com/v1".to_string(),
            "anthropic" => "https://api.anthropic.com/v1".to_string(),
            _ => "http://localhost:11434".to_string(),
        }
    }

    /// Get API key for a provider
    fn get_api_key(&self, provider: &str) -> Option<String> {
        for p in &self.config.providers {
            if p.provider == provider {
                return p.api_key.clone();
            }
        }
        // Check environment variables as fallback
        match provider {
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
            _ => None,
        }
    }

    /// Ollama chat completion with explicit URL
    async fn ollama_completion_with_url(&self, request: LLMCompletionRequest, base_url: &str) -> Result<LLMCompletionResponse> {
        let start = std::time::Instant::now();

        // Convert to Ollama format
        let ollama_request = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "stream": false,
            "options": {
                "temperature": request.temperature.unwrap_or(0.7),
                "num_predict": request.max_tokens.unwrap_or(2048)
            }
        });

        // Ollama uses /api/chat not /v1/chat/completions
        let url = if base_url.ends_with("/v1") {
            format!("{}", base_url.trim_end_matches("/v1"))
        } else {
            base_url.to_string()
        };

        let response = self.http_client
            .post(format!("{}/api/chat", url))
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| anyhow!("Ollama request failed: {}. Is Ollama running at {}?", e, url))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama error ({}): {}", status, text));
        }

        let ollama_response: OllamaResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Ollama response: {}", e))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Combine thinking (if present) with content for thinking models
        let content = match &ollama_response.message.thinking {
            Some(thinking) if !thinking.is_empty() => {
                format!("<thinking>\n{}\n</thinking>\n\n{}", thinking, ollama_response.message.content)
            }
            _ => ollama_response.message.content.clone(),
        };

        Ok(LLMCompletionResponse {
            content,
            model: ollama_response.model,
            usage: TokenUsage {
                prompt_tokens: ollama_response.prompt_eval_count.unwrap_or(0),
                completion_tokens: ollama_response.eval_count.unwrap_or(0),
                total_tokens: ollama_response.prompt_eval_count.unwrap_or(0) + ollama_response.eval_count.unwrap_or(0),
            },
            finish_reason: if ollama_response.done { "stop" } else { "length" }.to_string(),
            latency_ms,
        })
    }

    /// OpenAI-compatible chat completion (works for OpenAI, Llama API, Together, Groq, OpenRouter, etc.)
    async fn openai_compatible_completion(
        &self,
        request: LLMCompletionRequest,
        base_url: &str,
        api_key: Option<&str>,
    ) -> Result<LLMCompletionResponse> {
        let start = std::time::Instant::now();

        let openai_request = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "stream": false
        });

        // Clean up base_url: remove trailing slashes and ensure proper path
        let clean_url = base_url.trim_end_matches('/');
        // Handle different URL patterns:
        // - "http://localhost:8084/compat/v1" -> append /chat/completions
        // - "http://localhost:8084/v1" -> append /chat/completions  
        // - "https://api.openai.com/v1" -> append /chat/completions
        // - "http://localhost:11434" (ollama) -> should use /api/chat instead
        let url = format!("{}/chat/completions", clean_url);
        tracing::info!("Making OpenAI-compatible request to: {}", url);

        let mut req = self.http_client
            .post(&url)
            .header("Content-Type", "application/json");

        if let Some(key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        let response = req
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| anyhow!("Request to {} failed: {}", base_url, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error ({}): {}", status, text));
        }

        let openai_response: OpenAIResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        let choice = openai_response.choices.first()
            .ok_or_else(|| anyhow!("No choices in response"))?;

        let content = choice.message.content.clone()
            .unwrap_or_else(|| "[No content in response]".to_string());

        Ok(LLMCompletionResponse {
            content,
            model: openai_response.model,
            usage: TokenUsage {
                prompt_tokens: openai_response.usage.prompt_tokens,
                completion_tokens: openai_response.usage.completion_tokens,
                total_tokens: openai_response.usage.total_tokens,
            },
            finish_reason: choice.finish_reason.clone(),
            latency_ms,
        })
    }

    /// Anthropic chat completion with explicit config
    async fn anthropic_completion_with_config(
        &self,
        request: LLMCompletionRequest,
        base_url: &str,
        api_key: Option<&str>,
    ) -> Result<LLMCompletionResponse> {
        let start = std::time::Instant::now();
        let api_key = api_key
            .ok_or_else(|| anyhow!("Anthropic API key not configured"))?;

        // Convert messages to Anthropic format (separate system message)
        let mut system_message = String::new();
        let mut messages: Vec<serde_json::Value> = Vec::new();

        for msg in &request.messages {
            if msg.role == "system" {
                system_message = msg.content.clone();
            } else {
                messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                }));
            }
        }

        let mut anthropic_request = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "temperature": request.temperature.unwrap_or(0.7)
        });

        if !system_message.is_empty() {
            anthropic_request["system"] = serde_json::json!(system_message);
        }

        let response = self.http_client
            .post(format!("{}/messages", base_url.trim_end_matches('/')))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| anyhow!("Anthropic request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic error ({}): {}", status, text));
        }

        let anthropic_response: AnthropicResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Anthropic response: {}", e))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        let content = anthropic_response.content.first()
            .map(|c| c.text.clone())
            .unwrap_or_default();

        Ok(LLMCompletionResponse {
            content,
            model: anthropic_response.model,
            usage: TokenUsage {
                prompt_tokens: anthropic_response.usage.input_tokens,
                completion_tokens: anthropic_response.usage.output_tokens,
                total_tokens: anthropic_response.usage.input_tokens + anthropic_response.usage.output_tokens,
            },
            finish_reason: anthropic_response.stop_reason.unwrap_or_else(|| "stop".to_string()),
            latency_ms,
        })
    }

    /// Ollama chat completion
    async fn ollama_completion(&self, request: LLMCompletionRequest) -> Result<LLMCompletionResponse> {
        let start = std::time::Instant::now();
        let base_url = self.get_base_url("ollama");

        // Convert to Ollama format
        let ollama_request = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "stream": false,
            "options": {
                "temperature": request.temperature.unwrap_or(0.7),
                "num_predict": request.max_tokens.unwrap_or(2048)
            }
        });

        let response = self.http_client
            .post(format!("{}/api/chat", base_url))
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| anyhow!("Ollama request failed: {}. Is Ollama running?", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama error ({}): {}", status, text));
        }

        let ollama_response: OllamaResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Ollama response: {}", e))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Combine thinking (if present) with content for thinking models
        let content = match &ollama_response.message.thinking {
            Some(thinking) if !thinking.is_empty() => {
                format!("<thinking>\n{}\n</thinking>\n\n{}", thinking, ollama_response.message.content)
            }
            _ => ollama_response.message.content.clone(),
        };

        Ok(LLMCompletionResponse {
            content,
            model: ollama_response.model,
            usage: TokenUsage {
                prompt_tokens: ollama_response.prompt_eval_count.unwrap_or(0),
                completion_tokens: ollama_response.eval_count.unwrap_or(0),
                total_tokens: ollama_response.prompt_eval_count.unwrap_or(0) + ollama_response.eval_count.unwrap_or(0),
            },
            finish_reason: if ollama_response.done { "stop" } else { "length" }.to_string(),
            latency_ms,
        })
    }

    /// OpenAI chat completion
    async fn openai_completion(&self, request: LLMCompletionRequest) -> Result<LLMCompletionResponse> {
        let start = std::time::Instant::now();
        let base_url = self.get_base_url("openai");
        let api_key = self.get_api_key("openai")
            .ok_or_else(|| anyhow!("OpenAI API key not configured. Set OPENAI_API_KEY or add to settings."))?;

        let openai_request = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "stream": false
        });

        let response = self.http_client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| anyhow!("OpenAI request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI error ({}): {}", status, text));
        }

        let openai_response: OpenAIResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse OpenAI response: {}", e))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        let choice = openai_response.choices.first()
            .ok_or_else(|| anyhow!("No choices in OpenAI response"))?;

        let content = choice.message.content.clone()
            .unwrap_or_else(|| "[No content in response]".to_string());

        Ok(LLMCompletionResponse {
            content,
            model: openai_response.model,
            usage: TokenUsage {
                prompt_tokens: openai_response.usage.prompt_tokens,
                completion_tokens: openai_response.usage.completion_tokens,
                total_tokens: openai_response.usage.total_tokens,
            },
            finish_reason: choice.finish_reason.clone(),
            latency_ms,
        })
    }

    /// Anthropic chat completion
    async fn anthropic_completion(&self, request: LLMCompletionRequest) -> Result<LLMCompletionResponse> {
        let start = std::time::Instant::now();
        let base_url = self.get_base_url("anthropic");
        let api_key = self.get_api_key("anthropic")
            .ok_or_else(|| anyhow!("Anthropic API key not configured. Set ANTHROPIC_API_KEY or add to settings."))?;

        // Convert messages to Anthropic format (separate system message)
        let mut system_message = String::new();
        let mut messages: Vec<serde_json::Value> = Vec::new();

        for msg in &request.messages {
            if msg.role == "system" {
                system_message = msg.content.clone();
            } else {
                messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                }));
            }
        }

        let mut anthropic_request = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "temperature": request.temperature.unwrap_or(0.7)
        });

        if !system_message.is_empty() {
            anthropic_request["system"] = serde_json::json!(system_message);
        }

        let response = self.http_client
            .post(format!("{}/messages", base_url))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| anyhow!("Anthropic request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic error ({}): {}", status, text));
        }

        let anthropic_response: AnthropicResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Anthropic response: {}", e))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        let content = anthropic_response.content.first()
            .map(|c| c.text.clone())
            .unwrap_or_default();

        Ok(LLMCompletionResponse {
            content,
            model: anthropic_response.model,
            usage: TokenUsage {
                prompt_tokens: anthropic_response.usage.input_tokens,
                completion_tokens: anthropic_response.usage.output_tokens,
                total_tokens: anthropic_response.usage.input_tokens + anthropic_response.usage.output_tokens,
            },
            finish_reason: anthropic_response.stop_reason.unwrap_or_else(|| "stop".to_string()),
            latency_ms,
        })
    }

    /// List available models from Ollama
    pub async fn list_ollama_models(&self) -> Result<Vec<OllamaModel>> {
        let base_url = self.get_base_url("ollama");

        let response = self.http_client
            .get(format!("{}/api/tags", base_url))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to list Ollama models: {}. Is Ollama running?", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to list Ollama models"));
        }

        let list: OllamaModelList = response.json().await
            .map_err(|e| anyhow!("Failed to parse Ollama model list: {}", e))?;

        Ok(list.models)
    }

    /// Check if Ollama is available
    pub async fn check_ollama(&self) -> Result<bool> {
        let base_url = self.get_base_url("ollama");

        let response = self.http_client
            .get(format!("{}/api/tags", base_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        Ok(response.is_ok())
    }
}

// ============ Provider-specific response types ============

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    message: OllamaMessage,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
    /// Thinking/reasoning content from thinking models (e.g., lfm2.5-thinking)
    #[serde(default)]
    thinking: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub modified_at: Option<String>,
    pub size: Option<u64>,
    pub digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelList {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// ============================================================================
// agentreplay_evals::LLMClient adapter (Gap #10 fix)
// ============================================================================

/// Adapter to use LLMClient with agentreplay-evals GEval
pub struct LLMClientAdapter {
    inner: std::sync::Arc<tokio::sync::RwLock<LLMClient>>,
    model: String,
}

impl LLMClientAdapter {
    pub fn new(client: std::sync::Arc<tokio::sync::RwLock<LLMClient>>, model: String) -> Self {
        Self {
            inner: client,
            model,
        }
    }
}

#[async_trait::async_trait]
impl agentreplay_evals::llm_client::LLMClient for LLMClientAdapter {
    async fn evaluate(&self, prompt: String) -> Result<agentreplay_evals::llm_client::LLMResponse, agentreplay_evals::llm_client::LLMError> {
        let client = self.inner.read().await;
        
        let request = LLMCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            temperature: Some(0.1), // Low temperature for evaluation
            max_tokens: Some(1024),
            stream: Some(false),
        };
        
        let response = client.chat_completion(request).await
            .map_err(|e| agentreplay_evals::llm_client::LLMError::ApiError(e.to_string()))?;
        
        Ok(agentreplay_evals::llm_client::LLMResponse {
            content: response.content,
            usage: agentreplay_evals::llm_client::TokenUsage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
            },
            model: response.model.clone(),
        })
    }
    
    fn model_name(&self) -> &str {
        &self.model
    }
    
    fn cost_per_token(&self) -> (f64, f64) {
        // Default costs - could be made configurable
        (0.00001, 0.00003) // $0.01 / 1K input, $0.03 / 1K output
    }
}

#[async_trait::async_trait]
impl agentreplay_evals::llm_client::EmbeddingClient for LLMClientAdapter {
    async fn embed(&self, text: &str) -> Result<Vec<f64>, agentreplay_evals::llm_client::EmbedError> {
        let embeddings = self.embed_batch(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| agentreplay_evals::llm_client::EmbedError::ApiError("No embedding returned".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f64>>, agentreplay_evals::llm_client::EmbedError> {
        let client = self.inner.read().await;
        
        // Use OpenAI-compatible embedding endpoint
        let request_body = serde_json::json!({
            "model": "text-embedding-3-small",
            "input": texts,
        });
        
        // Get the endpoint URL and API key from the config
        let config = client.get_config();
        let openai_provider = config.providers.iter()
            .find(|p| p.provider == "openai" && p.enabled);
        
        let base_url = openai_provider
            .and_then(|c| c.base_url.clone())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        
        let api_key = openai_provider
            .and_then(|c| c.api_key.clone())
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .unwrap_or_default();
        
        let url = format!("{}/embeddings", base_url);
        
        // Make the request
        let http_client = reqwest::Client::new();
        let response = http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| agentreplay_evals::llm_client::EmbedError::Http(e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(agentreplay_evals::llm_client::EmbedError::ApiError(
                format!("Embedding API error: {}", error_text)
            ));
        }
        
        let response_json: serde_json::Value = response.json().await
            .map_err(|e| agentreplay_evals::llm_client::EmbedError::ApiError(e.to_string()))?;
        
        // Extract embeddings from response
        let data = response_json.get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| agentreplay_evals::llm_client::EmbedError::ApiError(
                "Invalid embedding response format".to_string()
            ))?;
        
        let embeddings: Result<Vec<Vec<f64>>, _> = data
            .iter()
            .map(|item| {
                item.get("embedding")
                    .and_then(|e| e.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_f64())
                            .collect()
                    })
                    .ok_or_else(|| agentreplay_evals::llm_client::EmbedError::ApiError(
                        "Invalid embedding data".to_string()
                    ))
            })
            .collect();
        
        embeddings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_provider() {
        let client = LLMClient::with_default_config();

        assert_eq!(client.detect_provider("gpt-4"), "openai");
        assert_eq!(client.detect_provider("gpt-3.5-turbo"), "openai");
        assert_eq!(client.detect_provider("claude-3-opus-20240229"), "anthropic");
        assert_eq!(client.detect_provider("llama3.2"), "ollama");
        assert_eq!(client.detect_provider("mistral"), "ollama");
    }
}
