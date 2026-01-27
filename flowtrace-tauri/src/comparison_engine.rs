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

//! Model Comparison Engine
//!
//! Executes model comparisons in parallel with independent streaming,
//! error handling, and timeout management.

use crate::llm::{ChatMessage, LLMClient, LLMCompletionRequest, LLMCompletionResponse};
use flowtrace_core::{
    ModelComparisonError, ModelComparisonRequest, ModelComparisonResponse,
    ModelComparisonResult, ModelPricing, ModelPricingRegistry, ModelSelection,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;

/// Default timeout for model completions (120 seconds)
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Model Comparison Engine
///
/// Handles parallel execution of LLM requests for model comparison.
pub struct ModelComparisonEngine {
    /// LLM client for making requests
    llm_client: Arc<RwLock<LLMClient>>,
    /// Pricing registry for cost calculation
    pricing_registry: Arc<ModelPricingRegistry>,
    /// Request timeout
    timeout: Duration,
}

impl ModelComparisonEngine {
    /// Create a new comparison engine
    pub fn new(
        llm_client: Arc<RwLock<LLMClient>>,
        pricing_registry: Arc<ModelPricingRegistry>,
    ) -> Self {
        Self {
            llm_client,
            pricing_registry,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    /// Set the request timeout
    #[allow(dead_code)]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Compare multiple models in parallel
    pub async fn compare(
        &self,
        request: ModelComparisonRequest,
    ) -> Result<ModelComparisonResponse, ModelComparisonError> {
        // Validate the request
        request.validate()?;

        let comparison_id = uuid::Uuid::new_v4().to_string();
        let mut response = ModelComparisonResponse::new(comparison_id.clone(), request.clone());

        // Build futures for all models
        let futures: Vec<_> = request
            .models
            .iter()
            .enumerate()
            .map(|(idx, model)| {
                self.execute_single_model(
                    idx,
                    model.clone(),
                    &request,
                )
            })
            .collect();

        // Execute all models in parallel
        let results = futures::future::join_all(futures).await;

        // Collect results
        for result in results {
            response.add_result(result);
        }

        response.finalize();

        // Check if all models failed
        if response.success_count() == 0 {
            tracing::warn!("All models failed for comparison {}", comparison_id);
        }

        Ok(response)
    }

    /// Execute a single model request
    async fn execute_single_model(
        &self,
        index: usize,
        model: ModelSelection,
        request: &ModelComparisonRequest,
    ) -> ModelComparisonResult {
        let start = Instant::now();

        tracing::debug!(
            "Starting model {} ({}/{}): {}/{} with base_url={:?}",
            index,
            index + 1,
            request.models.len(),
            model.provider,
            model.model_id,
            model.base_url
        );

        // Build the LLM request
        let llm_request = self.build_llm_request(&model, request);

        // Execute with timeout using explicit provider config
        let result = timeout(
            self.timeout,
            self.execute_llm_request_with_provider(llm_request, &model)
        ).await;

        let latency_ms = start.elapsed().as_millis() as u32;

        match result {
            Ok(Ok(llm_response)) => {
                // Calculate cost
                let cost = self
                    .pricing_registry
                    .calculate_cost(
                        &model.model_id,
                        llm_response.usage.prompt_tokens,
                        llm_response.usage.completion_tokens,
                    )
                    .await;

                tracing::debug!(
                    "Model {} completed: {} tokens in {}ms (${:.6})",
                    model.model_id,
                    llm_response.usage.total_tokens,
                    latency_ms,
                    cost
                );

                let mut result = ModelComparisonResult::success(
                    model,
                    llm_response.content,
                    llm_response.usage.prompt_tokens,
                    llm_response.usage.completion_tokens,
                    latency_ms,
                    cost,
                );
                result.finish_reason = Some(llm_response.finish_reason);
                result
            }
            Ok(Err(e)) => {
                tracing::warn!("Model {} failed: {}", model.model_id, e);
                ModelComparisonResult::error(model, e.to_string(), latency_ms)
            }
            Err(_) => {
                tracing::warn!(
                    "Model {} timed out after {}ms",
                    model.model_id,
                    self.timeout.as_millis()
                );
                ModelComparisonResult::timeout(model, latency_ms)
            }
        }
    }

    /// Build an LLM request from a comparison request
    fn build_llm_request(
        &self,
        model: &ModelSelection,
        request: &ModelComparisonRequest,
    ) -> LLMCompletionRequest {
        let mut messages = Vec::new();

        // Add system prompt if present
        if let Some(system) = &request.system_prompt {
            if !system.trim().is_empty() {
                messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: system.clone(),
                });
            }
        }

        // Apply variable substitution to the prompt
        let prompt = self.apply_variables(&request.prompt, &request.variables);

        // Add user message
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt,
        });

        LLMCompletionRequest {
            model: model.model_id.clone(),
            messages,
            temperature: Some(request.temperature),
            max_tokens: Some(request.max_tokens),
            stream: Some(false),
        }
    }

    /// Apply variable substitution to a prompt
    fn apply_variables(
        &self,
        prompt: &str,
        variables: &std::collections::HashMap<String, String>,
    ) -> String {
        let mut result = prompt.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
            result = result.replace(&format!("${{{}}}", key), value);
        }
        result
    }

    /// Execute an LLM request using explicit provider configuration
    async fn execute_llm_request_with_provider(
        &self,
        request: LLMCompletionRequest,
        model: &ModelSelection,
    ) -> anyhow::Result<LLMCompletionResponse> {
        let client = self.llm_client.read().await;
        
        // Use explicit provider config if available
        if let Some(base_url) = &model.base_url {
            tracing::debug!(
                "Using explicit provider config: provider={}, base_url={}, model={}",
                model.provider,
                base_url,
                model.model_id
            );
            client.chat_completion_with_provider(
                request,
                &model.provider,
                base_url,
                model.api_key.as_deref(),
            ).await
        } else {
            // Fall back to auto-detection (legacy behavior)
            tracing::warn!(
                "No base_url provided for model={}, falling back to auto-detection (may route incorrectly!)",
                model.model_id
            );
            client.chat_completion(request).await
        }
    }

    /// Execute an LLM request (legacy - uses auto-detection)
    #[allow(dead_code)]
    async fn execute_llm_request(
        &self,
        request: LLMCompletionRequest,
    ) -> anyhow::Result<LLMCompletionResponse> {
        let client = self.llm_client.read().await;
        client.chat_completion(request).await
    }

    /// Compare models with streaming support (returns events)
    #[allow(dead_code)]
    pub async fn compare_streaming(
        &self,
        request: ModelComparisonRequest,
        event_sender: tokio::sync::mpsc::Sender<ComparisonStreamEvent>,
    ) -> Result<String, ModelComparisonError> {
        request.validate()?;

        let comparison_id = uuid::Uuid::new_v4().to_string();

        // Send start event
        let _ = event_sender
            .send(ComparisonStreamEvent::Started {
                comparison_id: comparison_id.clone(),
                model_count: request.models.len(),
            })
            .await;

        // For now, use non-streaming comparison and send results
        // Full streaming would require changes to the LLM client
        let response = self.compare(request).await?;

        for (idx, result) in response.results.iter().enumerate() {
            let _ = event_sender
                .send(ComparisonStreamEvent::ModelCompleted {
                    comparison_id: comparison_id.clone(),
                    model_index: idx,
                    result: result.clone(),
                })
                .await;
        }

        let _ = event_sender
            .send(ComparisonStreamEvent::Completed {
                comparison_id: comparison_id.clone(),
                response,
            })
            .await;

        Ok(comparison_id)
    }

    /// Get pricing for a model
    #[allow(dead_code)]
    pub async fn get_model_pricing(&self, model_id: &str) -> Option<ModelPricing> {
        self.pricing_registry.get_pricing(model_id).await
    }

    /// List all available models with their pricing
    #[allow(dead_code)]
    pub async fn list_available_models(&self) -> Vec<AvailableModel> {
        let client = self.llm_client.read().await;
        let mut models = Vec::new();

        // Static cloud models
        let cloud_models = [
            ("openai", "gpt-4o", "GPT-4o"),
            ("openai", "gpt-4o-mini", "GPT-4o Mini"),
            ("openai", "gpt-4-turbo", "GPT-4 Turbo"),
            ("openai", "o1", "O1"),
            ("openai", "o1-mini", "O1 Mini"),
            ("anthropic", "claude-3-5-sonnet-20241022", "Claude 3.5 Sonnet"),
            ("anthropic", "claude-3-opus-20240229", "Claude 3 Opus"),
            ("anthropic", "claude-3-haiku-20240307", "Claude 3 Haiku"),
            ("deepseek", "deepseek-chat", "DeepSeek Chat"),
            ("deepseek", "deepseek-coder", "DeepSeek Coder"),
        ];

        for (provider, model_id, display_name) in cloud_models {
            let pricing = self.pricing_registry.get_pricing(model_id).await;
            models.push(AvailableModel {
                selection: ModelSelection::new(provider, model_id).with_display_name(display_name),
                available: self.check_provider_configured(provider),
                pricing,
            });
        }

        // Try to list Ollama models
        if let Ok(ollama_models) = client.list_ollama_models().await {
            for model in ollama_models {
                let pricing = self.pricing_registry.get_pricing(&model.name).await;
                models.push(AvailableModel {
                    selection: ModelSelection::new("ollama", &model.name),
                    available: true,
                    pricing,
                });
            }
        }

        models
    }

    /// Check if a provider is configured
    #[allow(dead_code)]
    fn check_provider_configured(&self, provider: &str) -> bool {
        match provider {
            "ollama" => true, // Always available if Ollama is running
            "openai" => std::env::var("OPENAI_API_KEY").is_ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").is_ok(),
            "deepseek" => std::env::var("DEEPSEEK_API_KEY").is_ok(),
            "google" | "vertex_ai" => std::env::var("GOOGLE_API_KEY").is_ok(),
            "mistral" => std::env::var("MISTRAL_API_KEY").is_ok(),
            _ => false,
        }
    }
}

/// Available model with pricing info
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct AvailableModel {
    pub selection: ModelSelection,
    pub available: bool,
    pub pricing: Option<ModelPricing>,
}

/// Events emitted during streaming comparison
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ComparisonStreamEvent {
    /// Comparison has started
    Started {
        comparison_id: String,
        model_count: usize,
    },
    /// A model started generating
    ModelStarted {
        comparison_id: String,
        model_index: usize,
        model: ModelSelection,
    },
    /// A model generated a token chunk
    ModelChunk {
        comparison_id: String,
        model_index: usize,
        delta: String,
        token_count: u32,
    },
    /// A model completed
    ModelCompleted {
        comparison_id: String,
        model_index: usize,
        result: ModelComparisonResult,
    },
    /// A model failed
    ModelError {
        comparison_id: String,
        model_index: usize,
        error: String,
    },
    /// All models completed
    Completed {
        comparison_id: String,
        response: ModelComparisonResponse,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_substitution() {
        let engine = create_test_engine();
        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "World".to_string());
        vars.insert("topic".to_string(), "Rust".to_string());

        let result = engine.apply_variables("Hello {{name}}! Tell me about ${topic}.", &vars);
        assert_eq!(result, "Hello World! Tell me about Rust.");
    }

    fn create_test_engine() -> ModelComparisonEngine {
        use crate::llm::LLMConfig;
        
        let client = Arc::new(RwLock::new(LLMClient::new(LLMConfig::default_with_ollama())));
        let registry = Arc::new(ModelPricingRegistry::with_builtins());
        ModelComparisonEngine::new(client, registry)
    }
}
