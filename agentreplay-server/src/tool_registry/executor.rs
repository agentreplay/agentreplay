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

//! Tool Executor - Unified execution engine
//!
//! Handles execution of tools across all kinds (MCP, REST, Native)
//! with rate limiting, retries, and timeout handling.

use crate::tool_registry::ToolRegistry;
use agentreplay_core::{
    ExecutionContext, RateLimit, ToolExecutionError, ToolExecutionResult, ToolKind,
    UnifiedToolDefinition,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Executor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutorConfig {
    /// Default timeout for tool execution (ms)
    pub default_timeout_ms: u64,
    /// Maximum concurrent tool executions
    pub max_concurrent_executions: usize,
    /// Global rate limit (requests per second)
    pub global_rate_limit: Option<RateLimit>,
    /// Enable execution tracing
    pub enable_tracing: bool,
}

impl Default for ToolExecutorConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 30_000,
            max_concurrent_executions: 100,
            global_rate_limit: None,
            enable_tracing: true,
        }
    }
}

/// Token bucket state for rate limiting
struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(limit: &RateLimit) -> Self {
        Self {
            tokens: limit.capacity as f64,
            capacity: limit.capacity as f64,
            refill_rate: limit.refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }

    fn time_until_available(&self) -> Duration {
        if self.tokens >= 1.0 {
            Duration::ZERO
        } else {
            let needed = 1.0 - self.tokens;
            Duration::from_secs_f64(needed / self.refill_rate)
        }
    }
}

/// Unified Tool Executor
pub struct ToolExecutor {
    /// Reference to tool registry
    registry: Arc<ToolRegistry>,
    /// Configuration
    #[allow(dead_code)]
    config: ToolExecutorConfig,
    /// Concurrency limiter
    semaphore: Arc<Semaphore>,
    /// Global rate limit bucket
    global_bucket: Option<Mutex<TokenBucket>>,
    /// Per-kind rate limit buckets
    kind_buckets: Mutex<HashMap<String, TokenBucket>>,
    /// Per-tool rate limit buckets
    tool_buckets: Mutex<HashMap<String, TokenBucket>>,
    /// Native handler registry
    native_handlers: Arc<super::NativeHandlerRegistry>,
}

impl ToolExecutor {
    /// Create a new executor
    pub fn new(
        registry: Arc<ToolRegistry>,
        native_handlers: Arc<super::NativeHandlerRegistry>,
        config: ToolExecutorConfig,
    ) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_executions));

        let global_bucket = config
            .global_rate_limit
            .as_ref()
            .map(|limit| Mutex::new(TokenBucket::new(limit)));

        Self {
            registry,
            config,
            semaphore,
            global_bucket,
            kind_buckets: Mutex::new(HashMap::new()),
            tool_buckets: Mutex::new(HashMap::new()),
            native_handlers,
        }
    }

    /// Execute a tool by name with optional version constraint
    pub async fn execute(
        &self,
        namespace: Option<&str>,
        name: &str,
        version_constraint: Option<&str>,
        arguments: serde_json::Value,
        context: ExecutionContext,
    ) -> Result<ToolExecutionResult, ToolExecutionError> {
        let start = Instant::now();

        // Look up the tool
        let lookup_result = self
            .registry
            .lookup(namespace, name, version_constraint)
            .map_err(|e| ToolExecutionError::not_found(&e.to_string()))?;

        let tool = lookup_result.tool;

        // Check if tool is enabled
        if !tool.enabled {
            return Err(ToolExecutionError {
                code: "TOOL_DISABLED".to_string(),
                message: format!("Tool {} is disabled", tool.tool_id),
                retryable: false,
                details: None,
            });
        }

        // Acquire rate limit token
        self.acquire_rate_limit(&tool).await?;

        // Acquire concurrency permit
        let _permit = self.semaphore.acquire().await.map_err(|_| {
            ToolExecutionError::execution_failed("Failed to acquire execution permit")
        })?;

        // Execute with retries
        let result = self
            .execute_with_retries(&tool, arguments.clone(), &context)
            .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok((data, retries)) => Ok(ToolExecutionResult {
                success: true,
                data: Some(data),
                error: None,
                latency_ms,
                retries,
                tool_version: tool.version.clone(),
            }),
            Err((error, retries)) => Ok(ToolExecutionResult {
                success: false,
                data: None,
                error: Some(error),
                latency_ms,
                retries,
                tool_version: tool.version.clone(),
            }),
        }
    }

    /// Execute a tool directly (bypassing lookup)
    pub async fn execute_direct(
        &self,
        tool: &UnifiedToolDefinition,
        arguments: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        match &tool.kind {
            ToolKind::Native { handler_id } => {
                self.execute_native(handler_id, arguments, context).await
            }
            ToolKind::MCP {
                server_uri,
                transport,
            } => {
                self.execute_mcp(server_uri, transport, &tool.name, arguments, context)
                    .await
            }
            ToolKind::REST {
                endpoint,
                method,
                headers,
            } => {
                self.execute_rest(endpoint, method, headers, arguments, context)
                    .await
            }
            ToolKind::Mock { responses } => self.execute_mock(responses, arguments).await,
        }
    }

    /// Execute with retry policy
    async fn execute_with_retries(
        &self,
        tool: &UnifiedToolDefinition,
        arguments: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<(serde_json::Value, u32), (ToolExecutionError, u32)> {
        let retry = &tool.execution.retry;
        let timeout = Duration::from_millis(tool.execution.timeout_ms);
        let mut retries = 0u32;
        let mut delay = Duration::from_millis(retry.initial_delay_ms);

        loop {
            let result = tokio::time::timeout(
                timeout,
                self.execute_direct(tool, arguments.clone(), context),
            )
            .await;

            match result {
                Ok(Ok(data)) => return Ok((data, retries)),
                Ok(Err(error)) => {
                    if !error.retryable || retries >= retry.max_retries {
                        return Err((error, retries));
                    }
                }
                Err(_) => {
                    let error = ToolExecutionError::timeout(format!(
                        "Tool {} timed out after {}ms",
                        tool.name, tool.execution.timeout_ms
                    ));
                    if retries >= retry.max_retries {
                        return Err((error, retries));
                    }
                }
            }

            retries += 1;

            // Apply backoff with optional jitter
            let mut sleep_duration = delay;
            if retry.jitter {
                use rand::Rng;
                let jitter = rand::thread_rng().gen_range(0.8..1.2);
                sleep_duration = Duration::from_secs_f64(delay.as_secs_f64() * jitter);
            }
            tokio::time::sleep(sleep_duration).await;

            // Exponential backoff
            delay = Duration::from_secs_f64(
                (delay.as_secs_f64() * retry.backoff_multiplier)
                    .min(retry.max_delay_ms as f64 / 1000.0),
            );
        }
    }

    /// Execute native handler
    async fn execute_native(
        &self,
        handler_id: &str,
        arguments: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        self.native_handlers
            .execute(handler_id, arguments, context)
            .await
    }

    /// Execute MCP tool (placeholder - integrates with MCP adapter)
    async fn execute_mcp(
        &self,
        _server_uri: &str,
        _transport: &agentreplay_core::MCPTransport,
        _tool_name: &str,
        _arguments: serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        // TODO: Delegate to McpToolAdapter
        Err(ToolExecutionError::execution_failed(
            "MCP execution not yet implemented via adapter",
        ))
    }

    /// Execute REST tool
    async fn execute_rest(
        &self,
        endpoint: &str,
        method: &agentreplay_core::HttpMethod,
        headers: &HashMap<String, String>,
        arguments: serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        let client = reqwest::Client::new();

        let mut request = match method {
            agentreplay_core::HttpMethod::GET => client.get(endpoint),
            agentreplay_core::HttpMethod::POST => client.post(endpoint),
            agentreplay_core::HttpMethod::PUT => client.put(endpoint),
            agentreplay_core::HttpMethod::DELETE => client.delete(endpoint),
            agentreplay_core::HttpMethod::PATCH => client.patch(endpoint),
        };

        // Add headers
        for (key, value) in headers {
            request = request.header(key, value);
        }

        // Add body for non-GET methods
        if !matches!(method, agentreplay_core::HttpMethod::GET) {
            request = request.json(&arguments);
        }

        let response = request.send().await.map_err(|e| ToolExecutionError {
            code: "REQUEST_FAILED".to_string(),
            message: e.to_string(),
            retryable: e.is_timeout() || e.is_connect(),
            details: None,
        })?;

        if response.status().is_success() {
            response.json().await.map_err(|e| ToolExecutionError {
                code: "PARSE_ERROR".to_string(),
                message: e.to_string(),
                retryable: false,
                details: None,
            })
        } else {
            Err(ToolExecutionError {
                code: format!("HTTP_{}", response.status().as_u16()),
                message: format!("HTTP error: {}", response.status()),
                retryable: response.status().is_server_error(),
                details: None,
            })
        }
    }

    /// Execute mock tool
    async fn execute_mock(
        &self,
        responses: &[agentreplay_core::MockResponse],
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        // Find matching response
        for mock in responses {
            let matches = match &mock.match_pattern {
                Some(pattern) => {
                    // Simple JSON path matching
                    let args_str = serde_json::to_string(&arguments).unwrap_or_default();
                    args_str.contains(pattern)
                }
                None => true, // No pattern = default response
            };

            if matches {
                // Apply latency
                if let Some(latency) = mock.latency_ms {
                    tokio::time::sleep(Duration::from_millis(latency)).await;
                }

                if mock.is_error {
                    return Err(ToolExecutionError {
                        code: "MOCK_ERROR".to_string(),
                        message: mock.response.to_string(),
                        retryable: false,
                        details: Some(mock.response.clone()),
                    });
                }

                return Ok(mock.response.clone());
            }
        }

        // No matching response
        Ok(serde_json::json!({ "mock": true, "message": "No matching mock response" }))
    }

    /// Acquire rate limit tokens (hierarchical)
    async fn acquire_rate_limit(
        &self,
        tool: &UnifiedToolDefinition,
    ) -> Result<(), ToolExecutionError> {
        // Check global rate limit
        if let Some(ref global_bucket) = self.global_bucket {
            let mut bucket = global_bucket.lock();
            if !bucket.try_acquire() {
                let wait = bucket.time_until_available();
                return Err(ToolExecutionError::rate_limited(format!(
                    "Global rate limit exceeded, retry after {}ms",
                    wait.as_millis()
                )));
            }
        }

        // Check per-kind rate limit
        let kind_key = match &tool.kind {
            ToolKind::Native { .. } => "native",
            ToolKind::MCP { .. } => "mcp",
            ToolKind::REST { .. } => "rest",
            ToolKind::Mock { .. } => "mock",
        };

        if let Some(limit) = tool.execution.rate_limits.per_kind.get(kind_key) {
            let mut buckets = self.kind_buckets.lock();
            let bucket = buckets
                .entry(kind_key.to_string())
                .or_insert_with(|| TokenBucket::new(limit));

            if !bucket.try_acquire() {
                let wait = bucket.time_until_available();
                return Err(ToolExecutionError::rate_limited(format!(
                    "Per-kind rate limit exceeded for {}, retry after {}ms",
                    kind_key,
                    wait.as_millis()
                )));
            }
        }

        // Check per-tool rate limit
        if let Some(ref limit) = tool.execution.rate_limits.per_tool {
            let mut buckets = self.tool_buckets.lock();
            let bucket = buckets
                .entry(tool.tool_id.clone())
                .or_insert_with(|| TokenBucket::new(limit));

            if !bucket.try_acquire() {
                let wait = bucket.time_until_available();
                return Err(ToolExecutionError::rate_limited(format!(
                    "Per-tool rate limit exceeded for {}, retry after {}ms",
                    tool.tool_id,
                    wait.as_millis()
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let limit = RateLimit::per_second(10);
        let mut bucket = TokenBucket::new(&limit);

        // Should be able to acquire initially
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());

        // Drain the bucket
        for _ in 0..8 {
            bucket.try_acquire();
        }

        // Should fail now
        assert!(!bucket.try_acquire());
    }
}
