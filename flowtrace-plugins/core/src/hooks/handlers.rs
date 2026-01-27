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

//! Hook handler traits and implementations.

use super::events::{AgentEvent, EventContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Result returned by hook handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    /// Whether the hook execution was successful.
    pub success: bool,

    /// Optional data to pass to subsequent hooks or the agent.
    pub data: Option<serde_json::Value>,

    /// Optional message describing the result.
    pub message: Option<String>,

    /// Execution time in microseconds.
    pub execution_time_us: u64,

    /// Whether to continue processing subsequent hooks.
    #[serde(default = "default_continue")]
    pub continue_chain: bool,
}

fn default_continue() -> bool {
    true
}

impl HookResult {
    /// Create a successful hook result.
    pub fn success() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
            execution_time_us: 0,
            continue_chain: true,
        }
    }

    /// Create a successful hook result with data.
    pub fn success_with_data(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            execution_time_us: 0,
            continue_chain: true,
        }
    }

    /// Create a failed hook result.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message.into()),
            execution_time_us: 0,
            continue_chain: true,
        }
    }

    /// Create a result that stops the hook chain.
    pub fn stop_chain() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
            execution_time_us: 0,
            continue_chain: false,
        }
    }

    /// Set the execution time.
    pub fn with_execution_time(mut self, time_us: u64) -> Self {
        self.execution_time_us = time_us;
        self
    }

    /// Set whether to continue the chain.
    pub fn with_continue_chain(mut self, continue_chain: bool) -> Self {
        self.continue_chain = continue_chain;
        self
    }
}

/// Errors that can occur during hook execution.
#[derive(Debug, Error)]
pub enum HookError {
    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Hook timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Hook handler not found: {0}")]
    HandlerNotFound(String),

    #[error("Hook configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("External command failed: {command} - {message}")]
    ExternalCommandFailed { command: String, message: String },

    #[error("Webhook request failed: {url} - {message}")]
    WebhookFailed { url: String, message: String },
}

/// Trait for asynchronous hook handlers.
#[async_trait]
pub trait HookHandler: Send + Sync {
    /// Handle an agent event.
    async fn handle(
        &self,
        event: &AgentEvent,
        context: &EventContext,
    ) -> Result<HookResult, HookError>;

    /// Get the handler name.
    fn name(&self) -> &str;

    /// Get supported event types (empty means all events).
    fn supported_events(&self) -> Vec<&'static str> {
        vec![]
    }
}

/// Type alias for a boxed async hook handler.
pub type AsyncHookHandler = Arc<dyn HookHandler>;

/// Synchronous hook handler that runs in a blocking context.
pub trait SyncHookHandler: Send + Sync {
    /// Handle an agent event synchronously.
    fn handle_sync(
        &self,
        event: &AgentEvent,
        context: &EventContext,
    ) -> Result<HookResult, HookError>;

    /// Get the handler name.
    fn name(&self) -> &str;
}

/// Wrapper to convert a sync handler to async.
pub struct SyncToAsyncWrapper<H: SyncHookHandler> {
    inner: H,
}

impl<H: SyncHookHandler> SyncToAsyncWrapper<H> {
    pub fn new(handler: H) -> Self {
        Self { inner: handler }
    }
}

#[async_trait]
impl<H: SyncHookHandler + 'static> HookHandler for SyncToAsyncWrapper<H> {
    async fn handle(
        &self,
        event: &AgentEvent,
        context: &EventContext,
    ) -> Result<HookResult, HookError> {
        // Run sync handler in a blocking task
        let event = event.clone();
        let context = context.clone();
        let handler = &self.inner;

        // For truly sync operations, we can just call directly
        // For blocking I/O, consider using tokio::task::spawn_blocking
        handler.handle_sync(&event, &context)
    }

    fn name(&self) -> &str {
        self.inner.name()
    }
}

/// A no-op handler for testing.
pub struct NoOpHandler {
    name: String,
}

impl NoOpHandler {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl HookHandler for NoOpHandler {
    async fn handle(
        &self,
        _event: &AgentEvent,
        _context: &EventContext,
    ) -> Result<HookResult, HookError> {
        Ok(HookResult::success())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Handler that logs events (for debugging).
pub struct LoggingHandler {
    name: String,
}

impl LoggingHandler {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl HookHandler for LoggingHandler {
    async fn handle(
        &self,
        event: &AgentEvent,
        context: &EventContext,
    ) -> Result<HookResult, HookError> {
        tracing::info!(
            handler = %self.name,
            event_type = %event.event_type(),
            session_id = %event.session_id(),
            project_id = %context.project_id,
            "Hook event received"
        );
        Ok(HookResult::success())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Handler that invokes a callback function.
pub struct CallbackHandler<F>
where
    F: Fn(&AgentEvent, &EventContext) -> Result<HookResult, HookError> + Send + Sync,
{
    name: String,
    callback: F,
}

impl<F> CallbackHandler<F>
where
    F: Fn(&AgentEvent, &EventContext) -> Result<HookResult, HookError> + Send + Sync,
{
    pub fn new(name: impl Into<String>, callback: F) -> Self {
        Self {
            name: name.into(),
            callback,
        }
    }
}

#[async_trait]
impl<F> HookHandler for CallbackHandler<F>
where
    F: Fn(&AgentEvent, &EventContext) -> Result<HookResult, HookError> + Send + Sync + 'static,
{
    async fn handle(
        &self,
        event: &AgentEvent,
        context: &EventContext,
    ) -> Result<HookResult, HookError> {
        (self.callback)(event, context)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_context() -> EventContext {
        EventContext::new(1, 1, PathBuf::from("/test"))
    }

    #[tokio::test]
    async fn test_noop_handler() {
        let handler = NoOpHandler::new("test");
        let event = AgentEvent::SessionStart {
            session_id: 1,
            project_id: 1,
            metadata: HashMap::new(),
        };

        let result = handler.handle(&event, &test_context()).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_callback_handler() {
        let handler = CallbackHandler::new("callback", |event, _ctx| {
            Ok(HookResult::success_with_data(serde_json::json!({
                "event_type": event.event_type()
            })))
        });

        let event = AgentEvent::Stop {
            session_id: 1,
            reason: super::super::events::StopReason::TaskComplete,
            final_message: None,
        };

        let result = handler.handle(&event, &test_context()).await.unwrap();
        assert!(result.success);
        assert!(result.data.is_some());
    }

    #[test]
    fn test_hook_result_builder() {
        let result = HookResult::success()
            .with_execution_time(1000)
            .with_continue_chain(false);

        assert!(result.success);
        assert_eq!(result.execution_time_us, 1000);
        assert!(!result.continue_chain);
    }
}
