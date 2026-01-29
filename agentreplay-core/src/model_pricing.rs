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

//! Model Pricing Registry
//!
//! Centralized pricing registry following LiteLLM patterns with:
//! - LiteLLM JSON sync capability
//! - TOML custom overrides
//! - Priority-based resolution (custom > upstream > builtin)
//! - Thread-safe caching

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default URL for LiteLLM pricing data
pub const LITELLM_PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// Model pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Cost per input token in USD
    pub input_cost_per_token: f64,
    /// Cost per output token in USD
    pub output_cost_per_token: f64,
    /// Maximum input tokens
    #[serde(default)]
    pub max_input_tokens: Option<u32>,
    /// Maximum output tokens
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    /// Maximum total tokens
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Context window (max_tokens or max_input_tokens)
    #[serde(default)]
    pub context_window: Option<u32>,
    /// The provider (e.g., "openai", "anthropic")
    #[serde(default)]
    pub provider: Option<String>,
    /// The provider as in LiteLLM (e.g., "openai", "anthropic")
    #[serde(default)]
    pub litellm_provider: Option<String>,
    /// Model mode (chat, completion, embedding, etc.)
    #[serde(default)]
    pub mode: Option<String>,
    /// Whether the model supports function calling
    #[serde(default)]
    pub supports_function_calling: bool,
    /// Whether the model supports vision
    #[serde(default)]
    pub supports_vision: bool,
    /// Whether the model supports streaming
    #[serde(default)]
    pub supports_streaming: Option<bool>,
    /// Cache creation cost per token (if supported)
    #[serde(default)]
    pub cache_creation_input_token_cost: Option<f64>,
    /// Cache read cost per token (if supported)
    #[serde(default)]
    pub cache_read_input_token_cost: Option<f64>,
    /// Source of this pricing data
    #[serde(default)]
    pub source: Option<String>,
    /// Priority (higher = takes precedence)
    #[serde(default)]
    pub priority: PricingPriority,
}

impl ModelPricing {
    /// Calculate the cost for given token counts
    pub fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        (input_tokens as f64 * self.input_cost_per_token)
            + (output_tokens as f64 * self.output_cost_per_token)
    }

    /// Get cost per 1K input tokens (for display)
    pub fn input_cost_per_1k(&self) -> f64 {
        self.input_cost_per_token * 1000.0
    }

    /// Get cost per 1K output tokens (for display)
    pub fn output_cost_per_1k(&self) -> f64 {
        self.output_cost_per_token * 1000.0
    }

    /// Check if this is a free/local model
    pub fn is_free(&self) -> bool {
        self.input_cost_per_token == 0.0 && self.output_cost_per_token == 0.0
    }

    /// Get the effective provider
    pub fn provider(&self) -> &str {
        self.litellm_provider.as_deref().unwrap_or("unknown")
    }
}

impl Default for ModelPricing {
    fn default() -> Self {
        Self {
            input_cost_per_token: 0.0,
            output_cost_per_token: 0.0,
            max_input_tokens: None,
            max_output_tokens: None,
            max_tokens: None,
            context_window: None,
            provider: None,
            litellm_provider: None,
            mode: Some("chat".to_string()),
            supports_function_calling: false,
            supports_vision: false,
            supports_streaming: Some(true),
            cache_creation_input_token_cost: None,
            cache_read_input_token_cost: None,
            source: None,
            priority: PricingPriority::Builtin,
        }
    }
}

/// Priority level for pricing data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum PricingPriority {
    /// Built into the application (lowest priority)
    #[default]
    Builtin = 0,
    /// Synced from LiteLLM upstream
    Upstream = 1,
    /// User-defined custom pricing (highest priority)
    Custom = 2,
}

/// Custom pricing override (TOML format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPricingOverride {
    pub model_id: String,
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub litellm_provider: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Metadata about the pricing registry sync
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PricingRegistryMetadata {
    /// Last sync timestamp (Unix seconds)
    pub last_sync_at: Option<u64>,
    /// Version or ETag from last sync
    pub last_sync_version: Option<String>,
    /// Number of models in upstream data
    pub upstream_model_count: usize,
    /// Number of custom overrides
    pub custom_override_count: usize,
    /// Total models available
    pub total_model_count: usize,
}

/// Thread-safe model pricing registry
#[derive(Clone)]
pub struct ModelPricingRegistry {
    /// Cached pricing data (model_id -> pricing)
    models: Arc<RwLock<HashMap<String, ModelPricing>>>,
    /// Registry metadata
    metadata: Arc<RwLock<PricingRegistryMetadata>>,
    /// Data directory for storing pricing files
    data_dir: PathBuf,
}

impl ModelPricingRegistry {
    /// Create a new registry with the given data directory
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(PricingRegistryMetadata::default())),
            data_dir: data_dir.into(),
        }
    }

    /// Create a registry with built-in defaults (no file system access)
    pub fn with_builtins() -> Self {
        let registry = Self::new(PathBuf::from("."));
        // Load builtin pricing synchronously (it's just in-memory)
        let builtins = Self::builtin_pricing();
        let models = registry.models.clone();
        tokio::spawn(async move {
            let mut guard = models.write().await;
            *guard = builtins;
        });
        registry
    }

    /// Initialize the registry by loading all pricing sources
    pub async fn initialize(&self) -> Result<(), PricingError> {
        // 1. Load builtins first
        self.load_builtins().await;

        // 2. Try to load cached upstream data
        if let Err(e) = self.load_cached_upstream().await {
            tracing::debug!("No cached upstream pricing: {}", e);
        }

        // 3. Load custom overrides
        if let Err(e) = self.load_custom_overrides().await {
            tracing::debug!("No custom overrides: {}", e);
        }

        self.update_metadata().await;
        Ok(())
    }

    /// Get pricing for a model (with fallback resolution)
    pub async fn get_pricing(&self, model_id: &str) -> Option<ModelPricing> {
        let models = self.models.read().await;

        // Try exact match first
        if let Some(pricing) = models.get(model_id) {
            return Some(pricing.clone());
        }

        // Try normalized model name (lowercase, no version)
        let normalized = Self::normalize_model_name(model_id);
        if let Some(pricing) = models.get(&normalized) {
            return Some(pricing.clone());
        }

        // Try prefix matching for versioned models
        let prefix_matches: Vec<_> = models
            .iter()
            .filter(|(k, _)| model_id.starts_with(k.as_str()) || k.starts_with(model_id))
            .collect();

        if let Some((_, pricing)) = prefix_matches.first() {
            return Some((*pricing).clone());
        }

        // Try provider-prefixed matching (e.g., "openai/gpt-4o" -> "gpt-4o")
        if let Some(stripped) = model_id.split('/').next_back() {
            if let Some(pricing) = models.get(stripped) {
                return Some(pricing.clone());
            }
        }

        None
    }

    /// Calculate cost for a model
    pub async fn calculate_cost(
        &self,
        model_id: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> f64 {
        self.get_pricing(model_id)
            .await
            .map(|p| p.calculate_cost(input_tokens, output_tokens))
            .unwrap_or(0.0)
    }

    /// Sync pricing data from LiteLLM upstream
    pub async fn sync_from_upstream(&self) -> Result<SyncResult, PricingError> {
        tracing::info!("Syncing pricing from LiteLLM...");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| PricingError::Network(e.to_string()))?;

        let response = client
            .get(LITELLM_PRICING_URL)
            .send()
            .await
            .map_err(|e| PricingError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(PricingError::Network(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let json: HashMap<String, serde_json::Value> = response
            .json()
            .await
            .map_err(|e| PricingError::Parse(e.to_string()))?;

        let mut added = 0;
        let mut updated = 0;

        let mut models = self.models.write().await;

        for (model_id, value) in json {
            // Skip sample_spec and other non-model entries
            if model_id == "sample_spec" || model_id.starts_with("_") {
                continue;
            }

            // Parse the pricing data
            if let Ok(mut pricing) = Self::parse_litellm_model(&value) {
                pricing.priority = PricingPriority::Upstream;
                pricing.source = Some("LiteLLM".to_string());

                // Only update if we don't have a custom override
                if let Some(existing) = models.get(&model_id) {
                    if existing.priority < PricingPriority::Custom {
                        models.insert(model_id, pricing);
                        updated += 1;
                    }
                } else {
                    models.insert(model_id, pricing);
                    added += 1;
                }
            }
        }

        drop(models);

        // Cache the result
        self.save_cached_upstream().await?;
        self.update_metadata().await;

        let result = SyncResult {
            added,
            updated,
            total: added + updated,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        tracing::info!(
            "Pricing sync complete: {} added, {} updated",
            added,
            updated
        );

        Ok(result)
    }

    /// Sync pricing data from LiteLLM (alias for sync_from_upstream)
    /// Returns the number of models synced
    pub async fn sync_from_litellm(&self) -> Result<usize, PricingError> {
        let result = self.sync_from_upstream().await?;
        Ok(result.total)
    }

    /// Get the timestamp of the last successful sync
    pub async fn last_sync_time(&self) -> Option<u64> {
        let meta = self.metadata.read().await;
        meta.last_sync_at
    }

    /// Add a custom pricing override
    pub async fn add_custom_override(&self, override_data: CustomPricingOverride) {
        let mut models = self.models.write().await;

        let pricing = ModelPricing {
            input_cost_per_token: override_data.input_cost_per_token,
            output_cost_per_token: override_data.output_cost_per_token,
            max_tokens: override_data.max_tokens,
            litellm_provider: override_data.litellm_provider,
            source: override_data.source.or(Some("Custom".to_string())),
            priority: PricingPriority::Custom,
            ..Default::default()
        };

        models.insert(override_data.model_id, pricing);
    }

    /// Remove a custom pricing override
    pub async fn remove_custom_override(&self, model_id: &str) {
        let mut models = self.models.write().await;
        // Only remove if it's a custom entry
        if let Some(pricing) = models.get(model_id) {
            if pricing.source.as_deref() == Some("Custom") || 
               pricing.source.as_deref() == Some("custom") {
                models.remove(model_id);
            }
        }
    }

    /// Save custom pricing overrides to file
    pub async fn save_custom_overrides(&self) -> Result<(), PricingError> {
        let models = self.models.read().await;
        
        // Collect all custom entries
        let custom_entries: Vec<CustomPricingOverride> = models
            .iter()
            .filter(|(_, p)| p.source.as_deref() == Some("Custom") || 
                            p.source.as_deref() == Some("custom"))
            .map(|(model_id, pricing)| CustomPricingOverride {
                model_id: model_id.clone(),
                input_cost_per_token: pricing.input_cost_per_token,
                output_cost_per_token: pricing.output_cost_per_token,
                max_tokens: pricing.max_tokens,
                litellm_provider: pricing.litellm_provider.clone(),
                source: pricing.source.clone(),
            })
            .collect();
        
        drop(models); // Release lock before file I/O
        
        // Save to TOML file
        let custom_file = self.data_dir.join("custom_pricing.toml");
        
        #[derive(serde::Serialize)]
        struct CustomPricingFile {
            overrides: Vec<CustomPricingOverride>,
        }
        
        let content = toml::to_string_pretty(&CustomPricingFile { overrides: custom_entries })
            .map_err(|e| PricingError::Parse(format!("Failed to serialize: {}", e)))?;
        
        std::fs::write(&custom_file, content)
            .map_err(|e| PricingError::Io(format!("Failed to write {}: {}", custom_file.display(), e)))?;
        
        Ok(())
    }

    /// Get all available models
    pub async fn list_models(&self) -> Vec<(String, ModelPricing)> {
        let models = self.models.read().await;
        models.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Get models for a specific provider
    pub async fn list_models_by_provider(&self, provider: &str) -> Vec<(String, ModelPricing)> {
        let models = self.models.read().await;
        models
            .iter()
            .filter(|(_, v)| {
                v.litellm_provider
                    .as_deref()
                    .map(|p| p.eq_ignore_ascii_case(provider))
                    .unwrap_or(false)
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get registry metadata
    pub async fn metadata(&self) -> PricingRegistryMetadata {
        self.metadata.read().await.clone()
    }

    // ============ Private methods ============

    /// Load built-in pricing data
    async fn load_builtins(&self) {
        let builtins = Self::builtin_pricing();
        let mut models = self.models.write().await;
        for (model_id, pricing) in builtins {
            models.entry(model_id).or_insert(pricing);
        }
    }

    /// Built-in pricing data (fallback)
    fn builtin_pricing() -> HashMap<String, ModelPricing> {
        let mut map = HashMap::new();

        // OpenAI models
        map.insert(
            "gpt-4o".to_string(),
            ModelPricing {
                input_cost_per_token: 2.5e-6,
                output_cost_per_token: 10e-6,
                max_input_tokens: Some(128000),
                max_output_tokens: Some(16384),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "gpt-4o-mini".to_string(),
            ModelPricing {
                input_cost_per_token: 0.15e-6,
                output_cost_per_token: 0.6e-6,
                max_input_tokens: Some(128000),
                max_output_tokens: Some(16384),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "gpt-4-turbo".to_string(),
            ModelPricing {
                input_cost_per_token: 10e-6,
                output_cost_per_token: 30e-6,
                max_input_tokens: Some(128000),
                max_output_tokens: Some(4096),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "gpt-4".to_string(),
            ModelPricing {
                input_cost_per_token: 30e-6,
                output_cost_per_token: 60e-6,
                max_input_tokens: Some(8192),
                max_output_tokens: Some(4096),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "gpt-3.5-turbo".to_string(),
            ModelPricing {
                input_cost_per_token: 0.5e-6,
                output_cost_per_token: 1.5e-6,
                max_input_tokens: Some(16385),
                max_output_tokens: Some(4096),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "o1".to_string(),
            ModelPricing {
                input_cost_per_token: 15e-6,
                output_cost_per_token: 60e-6,
                max_input_tokens: Some(200000),
                max_output_tokens: Some(100000),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "o1-mini".to_string(),
            ModelPricing {
                input_cost_per_token: 3e-6,
                output_cost_per_token: 12e-6,
                max_input_tokens: Some(128000),
                max_output_tokens: Some(65536),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "o3-mini".to_string(),
            ModelPricing {
                input_cost_per_token: 1.1e-6,
                output_cost_per_token: 4.4e-6,
                max_input_tokens: Some(200000),
                max_output_tokens: Some(100000),
                litellm_provider: Some("openai".to_string()),
                mode: Some("chat".to_string()),
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );

        // Anthropic Claude models
        map.insert(
            "claude-3-5-sonnet-20241022".to_string(),
            ModelPricing {
                input_cost_per_token: 3e-6,
                output_cost_per_token: 15e-6,
                max_input_tokens: Some(200000),
                max_output_tokens: Some(8192),
                litellm_provider: Some("anthropic".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "claude-3-opus-20240229".to_string(),
            ModelPricing {
                input_cost_per_token: 15e-6,
                output_cost_per_token: 75e-6,
                max_input_tokens: Some(200000),
                max_output_tokens: Some(4096),
                litellm_provider: Some("anthropic".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "claude-3-haiku-20240307".to_string(),
            ModelPricing {
                input_cost_per_token: 0.25e-6,
                output_cost_per_token: 1.25e-6,
                max_input_tokens: Some(200000),
                max_output_tokens: Some(4096),
                litellm_provider: Some("anthropic".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );

        // DeepSeek models
        map.insert(
            "deepseek-chat".to_string(),
            ModelPricing {
                input_cost_per_token: 0.14e-6,
                output_cost_per_token: 0.28e-6,
                max_input_tokens: Some(64000),
                max_output_tokens: Some(8192),
                litellm_provider: Some("deepseek".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "deepseek-coder".to_string(),
            ModelPricing {
                input_cost_per_token: 0.14e-6,
                output_cost_per_token: 0.28e-6,
                max_input_tokens: Some(64000),
                max_output_tokens: Some(8192),
                litellm_provider: Some("deepseek".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );

        // Google Gemini models
        map.insert(
            "gemini-1.5-pro".to_string(),
            ModelPricing {
                input_cost_per_token: 3.5e-6,
                output_cost_per_token: 10.5e-6,
                max_input_tokens: Some(2097152),
                max_output_tokens: Some(8192),
                litellm_provider: Some("vertex_ai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "gemini-1.5-flash".to_string(),
            ModelPricing {
                input_cost_per_token: 0.075e-6,
                output_cost_per_token: 0.3e-6,
                max_input_tokens: Some(1048576),
                max_output_tokens: Some(8192),
                litellm_provider: Some("vertex_ai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "gemini-2.0-flash".to_string(),
            ModelPricing {
                input_cost_per_token: 0.1e-6,
                output_cost_per_token: 0.4e-6,
                max_input_tokens: Some(1048576),
                max_output_tokens: Some(8192),
                litellm_provider: Some("vertex_ai".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                supports_vision: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );

        // Mistral models
        map.insert(
            "mistral-large".to_string(),
            ModelPricing {
                input_cost_per_token: 2e-6,
                output_cost_per_token: 6e-6,
                max_input_tokens: Some(128000),
                max_output_tokens: Some(128000),
                litellm_provider: Some("mistral".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );
        map.insert(
            "mistral-small".to_string(),
            ModelPricing {
                input_cost_per_token: 0.1e-6,
                output_cost_per_token: 0.3e-6,
                max_input_tokens: Some(32000),
                max_output_tokens: Some(8191),
                litellm_provider: Some("mistral".to_string()),
                mode: Some("chat".to_string()),
                supports_function_calling: true,
                priority: PricingPriority::Builtin,
                ..Default::default()
            },
        );

        // Ollama models (free, local)
        for model in [
            "llama3.2",
            "llama3.1",
            "llama2",
            "mistral",
            "mixtral",
            "qwen2",
            "phi3",
            "codellama",
            "gemma",
        ] {
            map.insert(
                model.to_string(),
                ModelPricing {
                    input_cost_per_token: 0.0,
                    output_cost_per_token: 0.0,
                    litellm_provider: Some("ollama".to_string()),
                    mode: Some("chat".to_string()),
                    source: Some("Local".to_string()),
                    priority: PricingPriority::Builtin,
                    ..Default::default()
                },
            );
        }

        map
    }

    /// Parse a LiteLLM model entry
    fn parse_litellm_model(value: &serde_json::Value) -> Result<ModelPricing, PricingError> {
        let obj = value
            .as_object()
            .ok_or(PricingError::Parse("Expected object for model".to_string()))?;

        let input_cost = obj
            .get("input_cost_per_token")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let output_cost = obj
            .get("output_cost_per_token")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let max_tokens = obj
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let max_input_tokens = obj
            .get("max_input_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        // Context window is max_tokens or max_input_tokens
        let context_window = max_tokens.or(max_input_tokens);

        let litellm_provider = obj
            .get("litellm_provider")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ModelPricing {
            input_cost_per_token: input_cost,
            output_cost_per_token: output_cost,
            max_input_tokens,
            max_output_tokens: obj
                .get("max_output_tokens")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            max_tokens,
            context_window,
            provider: litellm_provider.clone(),
            litellm_provider,
            mode: obj
                .get("mode")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            supports_function_calling: obj
                .get("supports_function_calling")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            supports_vision: obj
                .get("supports_vision")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            supports_streaming: None,
            cache_creation_input_token_cost: obj
                .get("cache_creation_input_token_cost")
                .and_then(|v| v.as_f64()),
            cache_read_input_token_cost: obj
                .get("cache_read_input_token_cost")
                .and_then(|v| v.as_f64()),
            source: obj
                .get("source")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            priority: PricingPriority::Upstream,
        })
    }

    /// Normalize a model name for matching
    fn normalize_model_name(name: &str) -> String {
        let normalized = name.to_lowercase();
        // Remove common version suffixes
        let normalized = normalized
            .trim_end_matches(|c: char| c.is_numeric() || c == '-' || c == '.' || c == '@');
        normalized.to_string()
    }

    /// Load cached upstream data from disk
    async fn load_cached_upstream(&self) -> Result<(), PricingError> {
        let cache_path = self.data_dir.join("models/upstream/litellm_models.json");

        if !cache_path.exists() {
            return Err(PricingError::NotFound(
                "Cached upstream data not found".to_string(),
            ));
        }

        let content = tokio::fs::read_to_string(&cache_path)
            .await
            .map_err(|e| PricingError::Io(e.to_string()))?;

        let json: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).map_err(|e| PricingError::Parse(e.to_string()))?;

        let mut models = self.models.write().await;

        for (model_id, value) in json {
            if model_id == "sample_spec" || model_id.starts_with("_") {
                continue;
            }

            if let Ok(mut pricing) = Self::parse_litellm_model(&value) {
                pricing.priority = PricingPriority::Upstream;

                // Only update if we don't have a higher priority entry
                if let Some(existing) = models.get(&model_id) {
                    if existing.priority <= pricing.priority {
                        models.insert(model_id, pricing);
                    }
                } else {
                    models.insert(model_id, pricing);
                }
            }
        }

        Ok(())
    }

    /// Save upstream data to cache
    async fn save_cached_upstream(&self) -> Result<(), PricingError> {
        let cache_dir = self.data_dir.join("models/upstream");
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .map_err(|e| PricingError::Io(e.to_string()))?;

        // We save the full registry state (we'd need to track upstream separately for proper caching)
        // For now, this is just metadata update
        let metadata_path = cache_dir.join("_metadata.toml");
        let metadata = format!(
            "last_sync_at = {}\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );

        tokio::fs::write(&metadata_path, metadata)
            .await
            .map_err(|e| PricingError::Io(e.to_string()))?;

        Ok(())
    }

    /// Load custom TOML overrides
    async fn load_custom_overrides(&self) -> Result<(), PricingError> {
        let custom_dir = self.data_dir.join("models/custom");

        if !custom_dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&custom_dir)
            .await
            .map_err(|e| PricingError::Io(e.to_string()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| PricingError::Io(e.to_string()))?
        {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                if let Err(e) = self.load_custom_override_file(&path).await {
                    tracing::warn!("Failed to load custom override {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Load a single custom override TOML file
    async fn load_custom_override_file(&self, path: &Path) -> Result<(), PricingError> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| PricingError::Io(e.to_string()))?;

        #[derive(Deserialize)]
        struct CustomOverrideFile {
            models: HashMap<String, CustomModelOverride>,
        }

        #[derive(Deserialize)]
        struct CustomModelOverride {
            input_cost_per_token: f64,
            output_cost_per_token: f64,
            #[serde(default)]
            max_tokens: Option<u32>,
            #[serde(default)]
            litellm_provider: Option<String>,
            #[serde(default)]
            source: Option<String>,
        }

        let file: CustomOverrideFile =
            toml::from_str(&content).map_err(|e| PricingError::Parse(e.to_string()))?;

        let mut models = self.models.write().await;

        for (model_id, override_data) in file.models {
            let pricing = ModelPricing {
                input_cost_per_token: override_data.input_cost_per_token,
                output_cost_per_token: override_data.output_cost_per_token,
                max_tokens: override_data.max_tokens,
                litellm_provider: override_data.litellm_provider,
                source: override_data
                    .source
                    .or(Some(format!("Custom: {:?}", path.file_name()))),
                priority: PricingPriority::Custom,
                ..Default::default()
            };
            models.insert(model_id, pricing);
        }

        Ok(())
    }

    /// Update registry metadata
    async fn update_metadata(&self) {
        let models = self.models.read().await;
        let mut metadata = self.metadata.write().await;

        metadata.total_model_count = models.len();
        metadata.upstream_model_count = models
            .values()
            .filter(|p| p.priority == PricingPriority::Upstream)
            .count();
        metadata.custom_override_count = models
            .values()
            .filter(|p| p.priority == PricingPriority::Custom)
            .count();
    }
}

/// Result of a pricing sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// Number of new models added
    pub added: usize,
    /// Number of existing models updated
    pub updated: usize,
    /// Total models affected
    pub total: usize,
    /// Timestamp of the sync
    pub timestamp: u64,
}

/// Errors that can occur in the pricing registry
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum PricingError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid data: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing_calculation() {
        let pricing = ModelPricing {
            input_cost_per_token: 2.5e-6,
            output_cost_per_token: 10e-6,
            ..Default::default()
        };

        // 1000 input + 500 output tokens
        let cost = pricing.calculate_cost(1000, 500);
        assert!((cost - 0.0075).abs() < 1e-10);
    }

    #[test]
    fn test_model_pricing_display() {
        let pricing = ModelPricing {
            input_cost_per_token: 2.5e-6,
            output_cost_per_token: 10e-6,
            ..Default::default()
        };

        assert!((pricing.input_cost_per_1k() - 0.0025).abs() < 1e-10);
        assert!((pricing.output_cost_per_1k() - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_normalize_model_name() {
        assert_eq!(
            ModelPricingRegistry::normalize_model_name("gpt-4o-2024-08-06"),
            "gpt-4o"
        );
        assert_eq!(
            ModelPricingRegistry::normalize_model_name("claude-3-5-sonnet-20241022"),
            "claude-3-5-sonnet"
        );
    }

    #[test]
    fn test_builtin_pricing() {
        let builtins = ModelPricingRegistry::builtin_pricing();

        assert!(builtins.contains_key("gpt-4o"));
        assert!(builtins.contains_key("claude-3-5-sonnet-20241022"));
        assert!(builtins.contains_key("llama3.2"));

        // Verify Ollama models are free
        let llama = builtins.get("llama3.2").unwrap();
        assert!(llama.is_free());
    }

    #[tokio::test]
    async fn test_registry_initialization() {
        let registry = ModelPricingRegistry::new("/tmp/test_pricing");
        registry.initialize().await.unwrap();

        let gpt4o = registry.get_pricing("gpt-4o").await;
        assert!(gpt4o.is_some());

        let cost = registry.calculate_cost("gpt-4o", 1000, 500).await;
        assert!(cost > 0.0);
    }

    #[test]
    fn test_pricing_priority() {
        assert!(PricingPriority::Custom > PricingPriority::Upstream);
        assert!(PricingPriority::Upstream > PricingPriority::Builtin);
    }
}
