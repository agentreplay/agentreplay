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

//! Agent Adapter Interface for CIP Live Invocation
//!
//! CIP requires live agent invocation (prospective analysis) rather than
//! post-hoc trace analysis. This module provides adapters for various
//! agent deployment models:
//! - HTTP endpoints
//! - Local functions
//! - OpenAI-compatible APIs
//!
//! Reference: Task 1.5 from CIP Integration document

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors that can occur during agent invocation
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Agent invocation failed: {0}")]
    InvocationFailed(String),

    #[error("Agent timeout after {0:?}")]
    Timeout(Duration),

    #[error("Agent returned invalid response: {0}")]
    InvalidResponse(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Agent rate limited, retry after {0:?}")]
    RateLimited(Option<Duration>),

    #[error("Agent authentication failed")]
    AuthenticationFailed,
}

/// Metadata captured from agent invocation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvocationMetadata {
    /// Time taken for the invocation in milliseconds
    pub latency_ms: u64,
    /// Token count (if available)
    pub token_count: Option<u64>,
    /// Cost in USD (if available)
    pub cost_usd: Option<f64>,
    /// Model used (if available)
    pub model: Option<String>,
    /// Raw response metadata
    pub raw_metadata: Option<serde_json::Value>,
}

/// Result of an agent invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInvocationResult {
    /// The agent's response text
    pub response: String,
    /// Invocation metadata
    pub metadata: InvocationMetadata,
    /// Whether the invocation was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl AgentInvocationResult {
    /// Create a successful result
    pub fn success(response: String, metadata: InvocationMetadata) -> Self {
        Self {
            response,
            metadata,
            success: true,
            error: None,
        }
    }

    /// Create a failed result
    pub fn failure(error: String) -> Self {
        Self {
            response: String::new(),
            metadata: InvocationMetadata::default(),
            success: false,
            error: Some(error),
        }
    }
}

/// Trait for agents that can be evaluated with CIP
///
/// This trait abstracts the agent invocation mechanism, allowing CIP to
/// evaluate agents regardless of their deployment model (local, HTTP, SDK).
#[async_trait]
pub trait CIPAgent: Send + Sync {
    /// Invoke the agent with a query and context
    ///
    /// # Arguments
    /// * `query` - The user's question/request
    /// * `context` - The retrieved context (will be perturbed by CIP)
    ///
    /// # Returns
    /// * `AgentInvocationResult` containing response and metadata
    async fn invoke(&self, query: &str, context: &str)
        -> Result<AgentInvocationResult, AgentError>;

    /// Unique identifier for this agent
    fn agent_id(&self) -> &str;

    /// Optional: Agent description for logging/UI
    fn description(&self) -> Option<&str> {
        None
    }

    /// Optional: Expected cost per invocation (for budgeting)
    fn expected_cost_per_call(&self) -> Option<f64> {
        None
    }

    /// Optional: Timeout for invocations (default: 30s)
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    /// Optional: Maximum retries on transient failures
    fn max_retries(&self) -> u32 {
        2
    }
}

/// HTTP-based agent adapter for remote agents
///
/// Supports any agent exposed via HTTP POST endpoint that accepts:
/// ```json
/// {
///     "query": "user question",
///     "context": "retrieved context"
/// }
/// ```
pub struct HttpAgentAdapter {
    /// Agent endpoint URL
    endpoint: String,
    /// HTTP client with connection pooling
    client: reqwest::Client,
    /// Agent identifier
    id: String,
    /// Optional API key for authentication
    api_key: Option<String>,
    /// Request timeout
    timeout: Duration,
    /// Maximum retries
    max_retries: u32,
}

impl HttpAgentAdapter {
    /// Create a new HTTP agent adapter
    pub fn new(endpoint: &str, id: impl Into<String>) -> Result<Self, AgentError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(|e| AgentError::InvocationFailed(format!("Failed to create client: {}", e)))?;

        Ok(Self {
            endpoint: endpoint.to_string(),
            client,
            id: id.into(),
            api_key: None,
            timeout: Duration::from_secs(30),
            max_retries: 2,
        })
    }

    /// Set API key for authentication
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

#[derive(Serialize)]
struct HttpAgentRequest<'a> {
    query: &'a str,
    context: &'a str,
}

#[derive(Deserialize)]
struct HttpAgentResponse {
    response: String,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[async_trait]
impl CIPAgent for HttpAgentAdapter {
    async fn invoke(
        &self,
        query: &str,
        context: &str,
    ) -> Result<AgentInvocationResult, AgentError> {
        let start = Instant::now();

        let mut request = self
            .client
            .post(&self.endpoint)
            .json(&HttpAgentRequest { query, context })
            .timeout(self.timeout);

        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AgentError::NetworkError(e.to_string()))?;

        // Handle rate limiting
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(Duration::from_secs);
            return Err(AgentError::RateLimited(retry_after));
        }

        // Handle auth errors
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AgentError::AuthenticationFailed);
        }

        // Handle other errors
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AgentError::InvocationFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let latency_ms = start.elapsed().as_millis() as u64;
        let agent_response: HttpAgentResponse = response
            .json()
            .await
            .map_err(|e| AgentError::InvalidResponse(format!("Failed to parse response: {}", e)))?;

        // Extract metadata if available
        let (token_count, cost_usd, model) = if let Some(meta) = &agent_response.metadata {
            (
                meta.get("token_count").and_then(|v| v.as_u64()),
                meta.get("cost").and_then(|v| v.as_f64()),
                meta.get("model").and_then(|v| v.as_str()).map(String::from),
            )
        } else {
            (None, None, None)
        };

        Ok(AgentInvocationResult {
            response: agent_response.response,
            metadata: InvocationMetadata {
                latency_ms,
                token_count,
                cost_usd,
                model,
                raw_metadata: agent_response.metadata,
            },
            success: true,
            error: None,
        })
    }

    fn agent_id(&self) -> &str {
        &self.id
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Adapter for local function-based agents
///
/// Wraps an async function as a CIPAgent.
pub struct FunctionAgentAdapter<F>
where
    F: Fn(
            String,
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
{
    id: String,
    func: F,
    description: Option<String>,
}

impl<F> FunctionAgentAdapter<F>
where
    F: Fn(
            String,
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
{
    /// Create a new function agent adapter
    pub fn new(id: impl Into<String>, func: F) -> Self {
        Self {
            id: id.into(),
            func,
            description: None,
        }
    }

    /// Set agent description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl<F> CIPAgent for FunctionAgentAdapter<F>
where
    F: Fn(
            String,
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
{
    async fn invoke(
        &self,
        query: &str,
        context: &str,
    ) -> Result<AgentInvocationResult, AgentError> {
        let start = Instant::now();

        let result = (self.func)(query.to_string(), context.to_string()).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(response) => Ok(AgentInvocationResult {
                response,
                metadata: InvocationMetadata {
                    latency_ms,
                    token_count: None,
                    cost_usd: None,
                    model: None,
                    raw_metadata: None,
                },
                success: true,
                error: None,
            }),
            Err(e) => Err(AgentError::InvocationFailed(e)),
        }
    }

    fn agent_id(&self) -> &str {
        &self.id
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Adapter for OpenAI-compatible agents
///
/// Works with any OpenAI API-compatible endpoint (OpenAI, Azure, local vLLM, etc.)
pub struct OpenAIAgentAdapter {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: String,
    system_prompt: Option<String>,
    id: String,
    temperature: f64,
}

impl OpenAIAgentAdapter {
    /// Create a new OpenAI agent adapter
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        id: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: api_key.into(),
            model: model.into(),
            system_prompt: None,
            id: id.into(),
            temperature: 0.0, // Deterministic for evaluation
        }
    }

    /// Set custom endpoint (for Azure, vLLM, etc.)
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = temperature;
        self
    }

    /// Estimate cost based on model and tokens
    fn estimate_cost(&self, tokens: u64) -> f64 {
        match self.model.as_str() {
            m if m.contains("gpt-4o-mini") => tokens as f64 * 0.00000015,
            m if m.contains("gpt-4o") => tokens as f64 * 0.0000025,
            m if m.contains("gpt-4-turbo") => tokens as f64 * 0.00001,
            m if m.contains("gpt-4") => tokens as f64 * 0.00003,
            m if m.contains("gpt-3.5") => tokens as f64 * 0.0000005,
            _ => tokens as f64 * 0.000001, // Default estimate
        }
    }
}

#[async_trait]
impl CIPAgent for OpenAIAgentAdapter {
    async fn invoke(
        &self,
        query: &str,
        context: &str,
    ) -> Result<AgentInvocationResult, AgentError> {
        let start = Instant::now();

        let mut messages = vec![];

        if let Some(system) = &self.system_prompt {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system
            }));
        }

        // Combine context and query as user message
        messages.push(serde_json::json!({
            "role": "user",
            "content": format!("Context:\n{}\n\nQuestion: {}", context, query)
        }));

        let request_body = serde_json::json!({
            "model": &self.model,
            "messages": messages,
            "temperature": self.temperature
        });

        let response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AgentError::NetworkError(e.to_string()))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AgentError::InvocationFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AgentError::InvalidResponse(e.to_string()))?;

        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidResponse("Missing content in response".to_string()))?
            .to_string();

        let token_count = response_json["usage"]["total_tokens"].as_u64();
        let cost_usd = token_count.map(|t| self.estimate_cost(t));

        Ok(AgentInvocationResult {
            response: content,
            metadata: InvocationMetadata {
                latency_ms,
                token_count,
                cost_usd,
                model: Some(self.model.clone()),
                raw_metadata: Some(response_json),
            },
            success: true,
            error: None,
        })
    }

    fn agent_id(&self) -> &str {
        &self.id
    }
}

/// Wrapper that adds retry logic to any CIPAgent
pub struct RetryingAgent<A: CIPAgent> {
    inner: A,
    max_retries: u32,
    base_delay: Duration,
}

impl<A: CIPAgent> RetryingAgent<A> {
    /// Wrap an agent with retry logic
    pub fn new(inner: A) -> Self {
        let max_retries = inner.max_retries();
        Self {
            inner,
            max_retries,
            base_delay: Duration::from_millis(100),
        }
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set base delay for exponential backoff
    pub fn with_base_delay(mut self, base_delay: Duration) -> Self {
        self.base_delay = base_delay;
        self
    }
}

#[async_trait]
impl<A: CIPAgent> CIPAgent for RetryingAgent<A> {
    async fn invoke(
        &self,
        query: &str,
        context: &str,
    ) -> Result<AgentInvocationResult, AgentError> {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            match self.inner.invoke(query, context).await {
                Ok(result) => return Ok(result),
                Err(AgentError::RateLimited(retry_after)) => {
                    let delay = retry_after.unwrap_or(self.base_delay * 2u32.pow(attempt));
                    tokio::time::sleep(delay).await;
                    last_error = Some(AgentError::RateLimited(retry_after));
                }
                Err(AgentError::NetworkError(e)) if attempt < self.max_retries => {
                    let delay = self.base_delay * 2u32.pow(attempt);
                    tokio::time::sleep(delay).await;
                    last_error = Some(AgentError::NetworkError(e));
                }
                Err(e) => return Err(e), // Non-retryable error
            }
        }

        Err(last_error.unwrap_or(AgentError::InvocationFailed(
            "Max retries exceeded".to_string(),
        )))
    }

    fn agent_id(&self) -> &str {
        self.inner.agent_id()
    }

    fn timeout(&self) -> Duration {
        self.inner.timeout()
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Mock agent for testing - always returns the same response
pub struct MockAgent {
    id: String,
    response: String,
    latency_ms: u64,
}

impl MockAgent {
    /// Create a mock agent that returns a fixed response
    pub fn new(id: &str, response: &str) -> Self {
        Self {
            id: id.to_string(),
            response: response.to_string(),
            latency_ms: 10,
        }
    }

    /// Set simulated latency
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }
}

#[async_trait]
impl CIPAgent for MockAgent {
    async fn invoke(
        &self,
        _query: &str,
        _context: &str,
    ) -> Result<AgentInvocationResult, AgentError> {
        tokio::time::sleep(Duration::from_millis(self.latency_ms)).await;

        Ok(AgentInvocationResult::success(
            self.response.clone(),
            InvocationMetadata {
                latency_ms: self.latency_ms,
                ..Default::default()
            },
        ))
    }

    fn agent_id(&self) -> &str {
        &self.id
    }
}

/// Mock agent that uses context - changes response based on context
pub struct ContextAwareAgent {
    id: String,
}

impl ContextAwareAgent {
    /// Create a context-aware mock agent
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

#[async_trait]
impl CIPAgent for ContextAwareAgent {
    async fn invoke(
        &self,
        query: &str,
        context: &str,
    ) -> Result<AgentInvocationResult, AgentError> {
        // Simple simulation: use first 50 chars of context in response
        let context_snippet = if context.len() > 50 {
            &context[..50]
        } else {
            context
        };

        let response = format!(
            "Based on the context '{}...', the answer to '{}' is derived from the provided information.",
            context_snippet, query
        );

        Ok(AgentInvocationResult::success(
            response,
            InvocationMetadata::default(),
        ))
    }

    fn agent_id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_agent() {
        let agent = MockAgent::new("test-agent", "Test response");

        let result = agent.invoke("test query", "test context").await.unwrap();

        assert!(result.success);
        assert_eq!(result.response, "Test response");
        assert_eq!(agent.agent_id(), "test-agent");
    }

    #[tokio::test]
    async fn test_context_aware_agent() {
        let agent = ContextAwareAgent::new("context-agent");

        let result = agent
            .invoke("What is X?", "X is the answer to everything")
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.response.contains("X is the answer"));
    }

    #[test]
    fn test_http_adapter_construction() {
        let adapter = HttpAgentAdapter::new("http://localhost:8080/agent", "test-agent")
            .unwrap()
            .with_api_key("test-key")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(adapter.agent_id(), "test-agent");
        assert_eq!(adapter.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_openai_adapter_construction() {
        let adapter = OpenAIAgentAdapter::new("sk-test", "gpt-4o-mini", "openai-agent")
            .with_system_prompt("You are a helpful assistant")
            .with_temperature(0.5);

        assert_eq!(adapter.agent_id(), "openai-agent");
    }

    #[test]
    fn test_invocation_result_success() {
        let result = AgentInvocationResult::success(
            "test response".to_string(),
            InvocationMetadata::default(),
        );

        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_invocation_result_failure() {
        let result = AgentInvocationResult::failure("test error".to_string());

        assert!(!result.success);
        assert_eq!(result.error, Some("test error".to_string()));
    }
}
