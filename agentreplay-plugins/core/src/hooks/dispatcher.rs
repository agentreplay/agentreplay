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

//! Hook dispatcher for executing hooks in response to events.

use super::config::HookConfig;
use super::events::{AgentEvent, EventContext};
use super::handlers::{HookError, HookResult};
use super::registry::HookRegistry;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Semaphore;

/// Result of dispatching hooks for an event.
#[derive(Debug)]
pub struct DispatchResult {
    /// Event that was dispatched.
    pub event_type: String,
    /// Session ID from the event.
    pub session_id: u128,
    /// Results from each hook that executed.
    pub hook_results: Vec<HookExecutionResult>,
    /// Total dispatch time in microseconds.
    pub total_time_us: u64,
    /// Number of hooks that executed successfully.
    pub success_count: usize,
    /// Number of hooks that failed.
    pub failure_count: usize,
}

impl DispatchResult {
    /// Check if all hooks executed successfully.
    pub fn all_successful(&self) -> bool {
        self.failure_count == 0
    }

    /// Get aggregated data from all hook results.
    pub fn aggregated_data(&self) -> serde_json::Value {
        let mut data = serde_json::Map::new();
        for result in &self.hook_results {
            if let Some(hook_data) = &result.result.as_ref().ok().and_then(|r| r.data.clone()) {
                if let serde_json::Value::Object(obj) = hook_data {
                    for (k, v) in obj {
                        data.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        serde_json::Value::Object(data)
    }
}

/// Result of executing a single hook.
#[derive(Debug)]
pub struct HookExecutionResult {
    /// Hook ID.
    pub hook_id: String,
    /// Hook handler name.
    pub handler_name: String,
    /// Result of the hook execution.
    pub result: Result<HookResult, HookError>,
    /// Execution time in microseconds.
    pub execution_time_us: u64,
}

/// Errors that can occur during dispatch.
#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("No hooks registered for event: {0}")]
    NoHooksRegistered(String),

    #[error("Dispatch cancelled due to hook chain stop")]
    ChainStopped,

    #[error("All hooks failed for event: {0}")]
    AllHooksFailed(String),

    #[error("Hook execution error: {0}")]
    HookError(#[from] HookError),

    #[error("Registry error: {0}")]
    RegistryError(String),
}

/// Dispatcher for executing hooks in response to agent events.
///
/// # Concurrency Model
///
/// This dispatcher uses a sharded-lock registry (DashMap) via `HookRegistry`.
/// It provides low contention under typical workloads but is not lock-free.
///
/// The dispatcher handles:
/// - Event routing to registered hooks
/// - Priority-based execution ordering
/// - Timeout management
/// - Concurrent execution limiting
/// - Error handling and chain control
pub struct HookDispatcher {
    registry: Arc<HookRegistry>,
    config: HookConfig,
    concurrency_semaphore: Arc<Semaphore>,
}

impl HookDispatcher {
    /// Create a new hook dispatcher.
    pub fn new(registry: Arc<HookRegistry>, config: HookConfig) -> Self {
        let max_concurrent = config.max_concurrent_hooks;
        Self {
            registry,
            config,
            concurrency_semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Create a dispatcher with default configuration.
    pub fn with_defaults(registry: Arc<HookRegistry>) -> Self {
        Self::new(registry, HookConfig::default())
    }

    /// Dispatch an event to all registered hooks.
    ///
    /// Hooks are executed in priority order (lower priority values first).
    /// If a hook returns `continue_chain: false`, subsequent hooks are skipped.
    pub async fn dispatch(
        &self,
        event: &AgentEvent,
        context: &EventContext,
    ) -> Result<DispatchResult, DispatchError> {
        let start = Instant::now();
        let event_type = event.event_type().to_string();

        let hooks = self.registry.get_hooks_for_event(&event_type);
        if hooks.is_empty() {
            return Ok(DispatchResult {
                event_type,
                session_id: event.session_id(),
                hook_results: Vec::new(),
                total_time_us: start.elapsed().as_micros() as u64,
                success_count: 0,
                failure_count: 0,
            });
        }

        tracing::debug!(
            event_type = %event_type,
            hook_count = hooks.len(),
            session_id = %event.session_id(),
            "Dispatching event to hooks"
        );

        let mut hook_results = Vec::with_capacity(hooks.len());
        let mut success_count = 0;
        let mut failure_count = 0;

        for hook in hooks {
            let hook_start = Instant::now();
            let timeout = self.get_timeout_for_hook(&hook.id);

            // Acquire semaphore permit for concurrency control
            let _permit = self.concurrency_semaphore.acquire().await.unwrap();

            let result = self
                .execute_hook_with_timeout(&hook.handler, event, context, timeout)
                .await;

            let execution_time_us = hook_start.elapsed().as_micros() as u64;

            let should_continue = match &result {
                Ok(hook_result) => {
                    success_count += 1;
                    hook_result.continue_chain
                }
                Err(_) => {
                    failure_count += 1;
                    self.config.continue_on_error
                }
            };

            hook_results.push(HookExecutionResult {
                hook_id: hook.id.clone(),
                handler_name: hook.handler.name().to_string(),
                result,
                execution_time_us,
            });

            if !should_continue {
                tracing::debug!(
                    hook_id = %hook.id,
                    "Hook chain stopped"
                );
                break;
            }
        }

        let total_time_us = start.elapsed().as_micros() as u64;

        tracing::debug!(
            event_type = %event_type,
            total_time_us = total_time_us,
            success_count = success_count,
            failure_count = failure_count,
            "Event dispatch completed"
        );

        Ok(DispatchResult {
            event_type,
            session_id: event.session_id(),
            hook_results,
            total_time_us,
            success_count,
            failure_count,
        })
    }

    /// Execute a hook with timeout.
    async fn execute_hook_with_timeout(
        &self,
        handler: &super::handlers::AsyncHookHandler,
        event: &AgentEvent,
        context: &EventContext,
        timeout: Duration,
    ) -> Result<HookResult, HookError> {
        let handle_future = handler.handle(event, context);

        match tokio::time::timeout(timeout, handle_future).await {
            Ok(result) => result,
            Err(_) => Err(HookError::Timeout {
                timeout_ms: timeout.as_millis() as u64,
            }),
        }
    }

    /// Get the timeout for a specific hook.
    fn get_timeout_for_hook(&self, hook_id: &str) -> Duration {
        // Check if hook has a specific timeout configured
        for hook_def in &self.config.hooks {
            if hook_def.name.as_deref() == Some(hook_id) {
                if let Some(timeout_ms) = hook_def.timeout_ms {
                    return Duration::from_millis(timeout_ms);
                }
            }
        }
        self.config.default_timeout()
    }

    /// Get the registry.
    pub fn registry(&self) -> &Arc<HookRegistry> {
        &self.registry
    }

    /// Get the configuration.
    pub fn config(&self) -> &HookConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: HookConfig) {
        self.concurrency_semaphore = Arc::new(Semaphore::new(config.max_concurrent_hooks));
        self.config = config;
    }
}

/// Builder for creating HookDispatcher with custom settings.
pub struct HookDispatcherBuilder {
    registry: Option<Arc<HookRegistry>>,
    config: HookConfig,
}

impl Default for HookDispatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HookDispatcherBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            registry: None,
            config: HookConfig::default(),
        }
    }

    /// Set the hook registry.
    pub fn with_registry(mut self, registry: Arc<HookRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Set the configuration.
    pub fn with_config(mut self, config: HookConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the default timeout.
    pub fn with_default_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.config.default_timeout_ms = timeout_ms;
        self
    }

    /// Set the maximum concurrent hooks.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.config.max_concurrent_hooks = max;
        self
    }

    /// Set continue on error behavior.
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.config.continue_on_error = continue_on_error;
        self
    }

    /// Build the dispatcher.
    pub fn build(self) -> HookDispatcher {
        let registry = self.registry.unwrap_or_else(|| Arc::new(HookRegistry::new()));
        HookDispatcher::new(registry, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::super::handlers::NoOpHandler;
    use super::super::registry::{HookPriority, RegisteredHook};
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_context() -> EventContext {
        EventContext::new(1, 1, PathBuf::from("/test"))
    }

    #[tokio::test]
    async fn test_dispatch_no_hooks() {
        let registry = Arc::new(HookRegistry::new());
        let dispatcher = HookDispatcher::with_defaults(registry);

        let event = AgentEvent::SessionStart {
            session_id: 1,
            project_id: 1,
            metadata: HashMap::new(),
        };

        let result = dispatcher.dispatch(&event, &test_context()).await.unwrap();
        assert!(result.hook_results.is_empty());
        assert!(result.all_successful());
    }

    #[tokio::test]
    async fn test_dispatch_single_hook() {
        let registry = Arc::new(HookRegistry::new());
        let hook = RegisteredHook::new(
            "test_hook",
            "SessionStart",
            Arc::new(NoOpHandler::new("noop")),
        );
        registry.register_hook(hook).unwrap();

        let dispatcher = HookDispatcher::with_defaults(registry);

        let event = AgentEvent::SessionStart {
            session_id: 1,
            project_id: 1,
            metadata: HashMap::new(),
        };

        let result = dispatcher.dispatch(&event, &test_context()).await.unwrap();
        assert_eq!(result.hook_results.len(), 1);
        assert!(result.all_successful());
    }

    #[tokio::test]
    async fn test_dispatch_priority_order() {
        let execution_order = Arc::new(parking_lot::Mutex::new(Vec::new()));

        let registry = Arc::new(HookRegistry::new());

        // Create hooks that record their execution order
        for (id, priority) in [("hook3", 30), ("hook1", 10), ("hook2", 20)] {
            let order = execution_order.clone();
            let hook = RegisteredHook::new(
                id,
                "SessionStart",
                Arc::new(super::super::handlers::CallbackHandler::new(
                    id,
                    move |_, _| {
                        order.lock().push(id.to_string());
                        Ok(HookResult::success())
                    },
                )),
            )
            .with_priority(HookPriority(priority));
            registry.register_hook(hook).unwrap();
        }

        let dispatcher = HookDispatcher::with_defaults(registry);

        let event = AgentEvent::SessionStart {
            session_id: 1,
            project_id: 1,
            metadata: HashMap::new(),
        };

        dispatcher.dispatch(&event, &test_context()).await.unwrap();

        let order = execution_order.lock();
        assert_eq!(order.as_slice(), &["hook1", "hook2", "hook3"]);
    }

    #[tokio::test]
    async fn test_dispatch_chain_stop() {
        let execution_count = Arc::new(AtomicUsize::new(0));

        let registry = Arc::new(HookRegistry::new());

        // First hook stops the chain
        let count = execution_count.clone();
        let hook1 = RegisteredHook::new(
            "hook1",
            "SessionStart",
            Arc::new(super::super::handlers::CallbackHandler::new(
                "stopper",
                move |_, _| {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(HookResult::stop_chain())
                },
            )),
        )
        .with_priority(HookPriority(10));

        // Second hook should not execute
        let count2 = execution_count.clone();
        let hook2 = RegisteredHook::new(
            "hook2",
            "SessionStart",
            Arc::new(super::super::handlers::CallbackHandler::new(
                "never_runs",
                move |_, _| {
                    count2.fetch_add(1, Ordering::SeqCst);
                    Ok(HookResult::success())
                },
            )),
        )
        .with_priority(HookPriority(20));

        registry.register_hook(hook1).unwrap();
        registry.register_hook(hook2).unwrap();

        let dispatcher = HookDispatcher::with_defaults(registry);

        let event = AgentEvent::SessionStart {
            session_id: 1,
            project_id: 1,
            metadata: HashMap::new(),
        };

        dispatcher.dispatch(&event, &test_context()).await.unwrap();

        assert_eq!(execution_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_builder() {
        let dispatcher = HookDispatcherBuilder::new()
            .with_default_timeout_ms(1000)
            .with_max_concurrent(2)
            .with_continue_on_error(false)
            .build();

        assert_eq!(dispatcher.config().default_timeout_ms, 1000);
        assert_eq!(dispatcher.config().max_concurrent_hooks, 2);
        assert!(!dispatcher.config().continue_on_error);
    }
}
