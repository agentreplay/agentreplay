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

use super::{ChatMessage, ChatResponse, LLMProvider};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client as OpenAIClient,
};
use serde_json::json;
use std::time::Instant;
use tokio::sync::mpsc;

// OpenAI Provider
pub struct OpenAIProvider {
    client: OpenAIClient<OpenAIConfig>,
    models: Vec<String>,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> anyhow::Result<Self> {
        let config = OpenAIConfig::new().with_api_key(api_key);
        let client = OpenAIClient::with_config(config);

        Ok(Self {
            client,
            models: vec![
                "gpt-4-turbo".to_string(),
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
            ],
        })
    }

    fn convert_messages(&self, messages: Vec<ChatMessage>) -> Vec<ChatCompletionRequestMessage> {
        messages
            .into_iter()
            .filter_map(|msg| match msg.role.as_str() {
                "system" => ChatCompletionRequestSystemMessageArgs::default()
                    .content(msg.content)
                    .build()
                    .ok()
                    .map(ChatCompletionRequestMessage::System),
                "user" => ChatCompletionRequestUserMessageArgs::default()
                    .content(msg.content)
                    .build()
                    .ok()
                    .map(ChatCompletionRequestMessage::User),
                "assistant" => ChatCompletionRequestAssistantMessageArgs::default()
                    .content(msg.content)
                    .build()
                    .ok()
                    .map(ChatCompletionRequestMessage::Assistant),
                _ => None,
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<ChatResponse> {
        let start = Instant::now();
        let model_name = model.unwrap_or_else(|| "gpt-4-turbo".to_string());

        let request = CreateChatCompletionRequestArgs::default()
            .model(&model_name)
            .messages(self.convert_messages(messages))
            .build()?;

        let response = self.client.chat().create(request).await?;

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_default();

        // Extract token usage with input/output split (OpenTelemetry standard)
        let (tokens_used, input_tokens, output_tokens) = if let Some(usage) = &response.usage {
            (
                Some(usage.total_tokens),
                Some(usage.prompt_tokens),
                Some(usage.completion_tokens),
            )
        } else {
            (None, None, None)
        };

        // Extract finish reason
        let finish_reason = response.choices.first().and_then(|choice| {
            choice
                .finish_reason
                .as_ref()
                .map(|r| format!("{:?}", r).to_lowercase())
        });

        Ok(ChatResponse {
            content,
            provider: "openai".to_string(),
            model: model_name.clone(),
            response_model: Some(response.model.clone()),
            response_id: Some(response.id),
            tokens_used,
            input_tokens,
            output_tokens,
            finish_reason,
            duration_ms: start.elapsed().as_millis() as u32,
        })
    }

    async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);
        let model_name = model.unwrap_or_else(|| "gpt-4-turbo".to_string());

        let request = CreateChatCompletionRequestArgs::default()
            .model(&model_name)
            .messages(self.convert_messages(messages))
            .build()?;

        let mut stream = self.client.chat().create_stream(request).await?;

        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        if let Some(choice) = response.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                if tx.send(content.clone()).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(rx)
    }

    fn list_models(&self) -> Vec<String> {
        self.models.clone()
    }

    fn name(&self) -> &str {
        "OpenAI"
    }
}

// Anthropic Provider
pub struct AnthropicProvider {
    api_key: String,
    models: Vec<String>,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> anyhow::Result<Self> {
        Ok(Self {
            api_key,
            models: vec![
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-opus-20240229".to_string(),
                "claude-3-sonnet-20240229".to_string(),
                "claude-3-haiku-20240307".to_string(),
            ],
        })
    }
}

#[async_trait::async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<ChatResponse> {
        let start = Instant::now();
        let model_name = model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

        let client = reqwest::Client::new();

        let formatted_messages: Vec<_> = messages
            .iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let body = json!({
            "model": model_name,
            "messages": formatted_messages,
            "max_tokens": 4096,
        });

        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;

        let content = json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Extract token usage with input/output split (OpenTelemetry standard)
        let input_tokens = json["usage"]["input_tokens"].as_u64().map(|t| t as u32);
        let output_tokens = json["usage"]["output_tokens"].as_u64().map(|t| t as u32);
        let tokens_used = match (input_tokens, output_tokens) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        };

        // Extract response metadata
        let response_id = json["id"].as_str().map(|s| s.to_string());
        let response_model = json["model"].as_str().map(|s| s.to_string());
        let finish_reason = json["stop_reason"].as_str().map(|s| s.to_string());

        Ok(ChatResponse {
            content,
            provider: "anthropic".to_string(),
            model: model_name,
            response_model,
            response_id,
            tokens_used,
            input_tokens,
            output_tokens,
            finish_reason,
            duration_ms: start.elapsed().as_millis() as u32,
        })
    }

    async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);
        let model_name = model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());
        let api_key = self.api_key.clone();

        tokio::spawn(async move {
            let client = reqwest::Client::new();

            let formatted_messages: Vec<_> = messages
                .iter()
                .map(|m| json!({"role": m.role, "content": m.content}))
                .collect();

            let body = json!({
                "model": model_name,
                "messages": formatted_messages,
                "max_tokens": 4096,
                "stream": true,
            });

            if let Ok(response) = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                use futures::StreamExt;
                let mut stream = response.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    if let Ok(bytes) = chunk {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            if tx.send(text).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    fn list_models(&self) -> Vec<String> {
        self.models.clone()
    }

    fn name(&self) -> &str {
        "Anthropic"
    }
}

// DeepSeek Provider
pub struct DeepSeekProvider {
    api_key: String,
    models: Vec<String>,
}

impl DeepSeekProvider {
    pub fn new(api_key: String) -> anyhow::Result<Self> {
        Ok(Self {
            api_key,
            models: vec!["deepseek-chat".to_string(), "deepseek-coder".to_string()],
        })
    }
}

#[async_trait::async_trait]
impl LLMProvider for DeepSeekProvider {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<ChatResponse> {
        let start = Instant::now();
        let model_name = model.unwrap_or_else(|| "deepseek-chat".to_string());

        let client = reqwest::Client::new();

        let formatted_messages: Vec<_> = messages
            .iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let body = json!({
            "model": model_name,
            "messages": formatted_messages,
        });

        let response = client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Extract token usage with input/output split (OpenTelemetry standard)
        let input_tokens = json["usage"]["prompt_tokens"].as_u64().map(|t| t as u32);
        let output_tokens = json["usage"]["completion_tokens"]
            .as_u64()
            .map(|t| t as u32);
        let tokens_used = json["usage"]["total_tokens"].as_u64().map(|t| t as u32);

        // Extract response metadata
        let response_id = json["id"].as_str().map(|s| s.to_string());
        let response_model = json["model"].as_str().map(|s| s.to_string());
        let finish_reason = json["choices"][0]["finish_reason"]
            .as_str()
            .map(|s| s.to_string());

        Ok(ChatResponse {
            content,
            provider: "deepseek".to_string(),
            model: model_name,
            response_model,
            response_id,
            tokens_used,
            input_tokens,
            output_tokens,
            finish_reason,
            duration_ms: start.elapsed().as_millis() as u32,
        })
    }

    async fn stream_chat(
        &self,
        _messages: Vec<ChatMessage>,
        _model: Option<String>,
    ) -> anyhow::Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);
        // DeepSeek streaming implementation would go here
        // For now, send a placeholder and close the channel
        drop(tx);
        Ok(rx)
    }

    fn list_models(&self) -> Vec<String> {
        self.models.clone()
    }

    fn name(&self) -> &str {
        "DeepSeek"
    }
}

// Ollama Provider (Local)
pub struct OllamaProvider {
    base_url: String,
    models: Vec<String>,
}

impl OllamaProvider {
    pub fn new(base_url: String) -> anyhow::Result<Self> {
        Ok(Self {
            base_url,
            models: vec![
                "llama2".to_string(),
                "mistral".to_string(),
                "codellama".to_string(),
            ],
        })
    }
}

#[async_trait::async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        model: Option<String>,
    ) -> anyhow::Result<ChatResponse> {
        let start = Instant::now();
        let model_name = model.unwrap_or_else(|| "llama2".to_string());

        let client = reqwest::Client::new();

        let formatted_messages: Vec<_> = messages
            .iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let body = json!({
            "model": model_name,
            "messages": formatted_messages,
        });

        let response = client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;

        let content = json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Ollama doesn't always provide token counts, so these may be None
        let response_model = json["model"].as_str().map(|s| s.to_string());
        let finish_reason =
            json["done"].as_bool().and_then(
                |done| {
                    if done {
                        Some("stop".to_string())
                    } else {
                        None
                    }
                },
            );

        Ok(ChatResponse {
            content,
            provider: "ollama".to_string(),
            model: model_name,
            response_model,
            response_id: None, // Ollama doesn't provide response IDs
            tokens_used: None,
            input_tokens: None,
            output_tokens: None,
            finish_reason,
            duration_ms: start.elapsed().as_millis() as u32,
        })
    }

    async fn stream_chat(
        &self,
        _messages: Vec<ChatMessage>,
        _model: Option<String>,
    ) -> anyhow::Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);
        // Ollama streaming implementation would go here
        // For now, send a placeholder and close the channel
        drop(tx);
        Ok(rx)
    }

    fn list_models(&self) -> Vec<String> {
        self.models.clone()
    }

    fn name(&self) -> &str {
        "Ollama"
    }
}
