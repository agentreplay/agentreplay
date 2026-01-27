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

//! Native Handler Registry
//!
//! Registry for Rust-native tool handlers that can be called directly
//! without external network calls.

use async_trait::async_trait;
use dashmap::DashMap;
use flowtrace_core::{ExecutionContext, ToolExecutionError};
use std::sync::Arc;

/// Native tool handler trait
#[async_trait]
pub trait NativeHandler: Send + Sync {
    /// Get the handler ID
    fn handler_id(&self) -> &str;

    /// Execute the handler
    async fn execute(
        &self,
        arguments: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError>;

    /// Get handler description
    fn description(&self) -> &str {
        ""
    }
}

/// Registry for native handlers
pub struct NativeHandlerRegistry {
    handlers: DashMap<String, Arc<dyn NativeHandler>>,
}

impl NativeHandlerRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            handlers: DashMap::new(),
        }
    }

    /// Register a native handler
    pub fn register(&self, handler: Arc<dyn NativeHandler>) {
        self.handlers
            .insert(handler.handler_id().to_string(), handler);
    }

    /// Get a handler by ID
    pub fn get(&self, handler_id: &str) -> Option<Arc<dyn NativeHandler>> {
        self.handlers.get(handler_id).map(|h| h.clone())
    }

    /// Execute a handler
    pub async fn execute(
        &self,
        handler_id: &str,
        arguments: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        let handler = self.get(handler_id).ok_or_else(|| ToolExecutionError {
            code: "HANDLER_NOT_FOUND".to_string(),
            message: format!("Native handler not found: {}", handler_id),
            retryable: false,
            details: None,
        })?;

        handler.execute(arguments, context).await
    }

    /// List all registered handlers
    pub fn list(&self) -> Vec<String> {
        self.handlers.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for NativeHandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Built-in handlers

/// Echo handler for testing
#[allow(dead_code)]
pub struct EchoHandler;

#[async_trait]
impl NativeHandler for EchoHandler {
    fn handler_id(&self) -> &str {
        "echo"
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        Ok(arguments)
    }

    fn description(&self) -> &str {
        "Echoes back the input arguments"
    }
}

/// Sleep handler for testing timeouts
#[allow(dead_code)]
pub struct SleepHandler;

#[async_trait]
impl NativeHandler for SleepHandler {
    fn handler_id(&self) -> &str {
        "sleep"
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        _context: &ExecutionContext,
    ) -> Result<serde_json::Value, ToolExecutionError> {
        let ms = arguments.get("ms").and_then(|v| v.as_u64()).unwrap_or(1000);

        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;

        Ok(serde_json::json!({ "slept_ms": ms }))
    }

    fn description(&self) -> &str {
        "Sleeps for the specified number of milliseconds"
    }
}

/// Register built-in handlers
#[allow(dead_code)]
pub fn register_builtin_handlers(registry: &NativeHandlerRegistry) {
    registry.register(Arc::new(EchoHandler));
    registry.register(Arc::new(SleepHandler));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_handler() {
        let registry = NativeHandlerRegistry::new();
        register_builtin_handlers(&registry);

        let result = registry
            .execute(
                "echo",
                serde_json::json!({ "hello": "world" }),
                &ExecutionContext::default(),
            )
            .await
            .unwrap();

        assert_eq!(result, serde_json::json!({ "hello": "world" }));
    }

    #[tokio::test]
    async fn test_handler_not_found() {
        let registry = NativeHandlerRegistry::new();

        let result = registry
            .execute(
                "nonexistent",
                serde_json::json!({}),
                &ExecutionContext::default(),
            )
            .await;

        assert!(result.is_err());
    }
}
