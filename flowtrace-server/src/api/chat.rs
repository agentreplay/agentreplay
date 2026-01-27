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

use crate::api::query::{ApiError, AppState};
use crate::auth::AuthContext;
use crate::llm::ChatMessage;
use axum::{
    extract::{Extension, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub provider: String,
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
pub struct ChatResponseWrapper {
    pub content: String,
    pub provider: String,
    pub model: String,
    pub tokens_used: Option<u32>,
    pub duration_ms: u32,
}

#[derive(Serialize)]
pub struct ModelsResponse {
    pub providers: Vec<crate::llm::ProviderInfo>,
}

pub async fn chat_completion(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponseWrapper>, ApiError> {
    let llm_manager = state
        .llm_manager
        .as_ref()
        .ok_or_else(|| ApiError::Internal("LLM features are not enabled".to_string()))?;

    // Generate session ID from timestamp
    let session_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let response = llm_manager
        .chat(
            &req.provider,
            req.model,
            req.messages,
            auth.tenant_id,
            session_id,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("LLM request failed: {}", e)))?;

    Ok(Json(ChatResponseWrapper {
        content: response.content,
        provider: response.provider,
        model: response.model,
        tokens_used: response.tokens_used,
        duration_ms: response.duration_ms,
    }))
}

pub async fn stream_completion(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let llm_manager = state
        .llm_manager
        .as_ref()
        .ok_or_else(|| ApiError::Internal("LLM features are not enabled".to_string()))?;

    let session_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Clone for cost calculation
    let model_name = req.model.clone().unwrap_or_else(|| req.provider.clone());
    let messages_for_counting = req.messages.clone();

    let rx = llm_manager
        .stream_chat(
            &req.provider,
            req.model,
            req.messages,
            auth.tenant_id,
            session_id,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("LLM stream failed: {}", e)))?;

    // Get cost per 1K tokens based on model (simplified pricing)
    let (input_cost_per_1k, output_cost_per_1k) = get_model_pricing(&model_name);

    // Estimate input tokens (rough approximation: 4 chars ≈ 1 token)
    let input_tokens: u32 = messages_for_counting
        .iter()
        .map(|msg| (msg.content.len() as u32) / 4)
        .sum();

    // Use scan() on the stream to track cumulative tokens and cost
    use std::sync::{Arc, Mutex};
    let state = Arc::new(Mutex::new((0u32, 0f64))); // (output_tokens, total_cost)

    let stream = ReceiverStream::new(rx).map(move |chunk| {
        let mut state_guard = state.lock().unwrap();
        let (ref mut output_tokens, ref mut total_cost) = *state_guard;

        // Estimate tokens in chunk
        let chunk_tokens = (chunk.len() as u32) / 4;
        *output_tokens += chunk_tokens;

        // Calculate cost: input_cost + output_cost
        let input_cost = (input_tokens as f64 / 1000.0) * input_cost_per_1k;
        let output_cost = (*output_tokens as f64 / 1000.0) * output_cost_per_1k;
        *total_cost = input_cost + output_cost;

        // Create SSE event with cost metadata in comment
        let event = Event::default().data(chunk).comment(format!(
            "tokens:{{\"input\":{},\"output\":{},\"cost\":{:.6}}}",
            input_tokens, *output_tokens, *total_cost
        ));

        Ok(event)
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Get model pricing per 1K tokens (input, output)
/// Based on 2024-2025 pricing - should be kept up-to-date
fn get_model_pricing(model: &str) -> (f64, f64) {
    let model_lower = model.to_lowercase();

    match () {
        // OpenAI models
        _ if model_lower.contains("gpt-4o-mini") => (0.00015, 0.0006),
        _ if model_lower.contains("gpt-4o") => (0.0025, 0.01),
        _ if model_lower.contains("gpt-4-turbo") => (0.01, 0.03),
        _ if model_lower.contains("gpt-4") => (0.03, 0.06),
        _ if model_lower.contains("gpt-3.5") => (0.0005, 0.0015),

        // Anthropic models
        _ if model_lower.contains("claude-opus") => (0.015, 0.075),
        _ if model_lower.contains("claude-sonnet") || model_lower.contains("claude-3.5") => {
            (0.003, 0.015)
        }
        _ if model_lower.contains("claude-haiku") => (0.00025, 0.00125),
        _ if model_lower.contains("claude-2") => (0.008, 0.024),

        // Google Gemini
        _ if model_lower.contains("gemini-1.5-pro") || model_lower.contains("gemini-pro-1.5") => {
            (0.0035, 0.0105)
        }
        _ if model_lower.contains("gemini-1.5-flash") || model_lower.contains("gemini-flash") => {
            (0.000075, 0.0003)
        }
        _ if model_lower.contains("gemini-pro") => (0.0005, 0.0015),

        // Cohere
        _ if model_lower.contains("command-r-plus") || model_lower.contains("command-r+") => {
            (0.003, 0.015)
        }
        _ if model_lower.contains("command-r") => (0.0005, 0.0015),

        // Default fallback
        _ => {
            tracing::warn!(model = %model, "Unknown model for cost calculation, using default $0.001/$0.003 per 1K");
            (0.001, 0.003) // Generic fallback pricing
        }
    }
}

pub async fn list_models(State(state): State<AppState>) -> Result<Json<ModelsResponse>, ApiError> {
    let llm_manager = state
        .llm_manager
        .as_ref()
        .ok_or_else(|| ApiError::Internal("LLM features are not enabled".to_string()))?;

    let providers = llm_manager.list_providers();
    Ok(Json(ModelsResponse { providers }))
}
