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

//! Hook registry for managing registered hooks and handlers.

use super::handlers::{AsyncHookHandler, HookHandler};
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;

/// Priority level for hook execution.
/// Lower values execute first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HookPriority(pub i32);

impl Default for HookPriority {
    fn default() -> Self {
        HookPriority(100)
    }
}

impl HookPriority {
    /// Highest priority (executes first).
    pub const HIGHEST: HookPriority = HookPriority(0);
    /// High priority.
    pub const HIGH: HookPriority = HookPriority(25);
    /// Normal priority.
    pub const NORMAL: HookPriority = HookPriority(50);
    /// Low priority.
    pub const LOW: HookPriority = HookPriority(75);
    /// Lowest priority (executes last).
    pub const LOWEST: HookPriority = HookPriority(100);
}

/// A registered hook with its handler and metadata.
#[derive(Clone)]
pub struct RegisteredHook {
    /// Unique identifier for this hook registration.
    pub id: String,
    /// Event type this hook listens to.
    pub event_type: String,
    /// The handler to execute.
    pub handler: AsyncHookHandler,
    /// Priority for execution order.
    pub priority: HookPriority,
    /// Whether this hook is enabled.
    pub enabled: bool,
    /// Optional description.
    pub description: Option<String>,
}

impl RegisteredHook {
    /// Create a new registered hook.
    pub fn new(
        id: impl Into<String>,
        event_type: impl Into<String>,
        handler: AsyncHookHandler,
    ) -> Self {
        Self {
            id: id.into(),
            event_type: event_type.into(),
            handler,
            priority: HookPriority::default(),
            enabled: true,
            description: None,
        }
    }

    /// Set the priority for this hook.
    pub fn with_priority(mut self, priority: HookPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the description for this hook.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Enable or disable this hook.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Registry for managing hook handlers.
///
/// The registry maintains a mapping of handler names to their implementations,
/// as well as event-to-handler mappings for efficient dispatch.
pub struct HookRegistry {
    /// Handlers indexed by their unique name/id.
    handlers: DashMap<String, AsyncHookHandler>,
    /// Registered hooks indexed by event type.
    hooks_by_event: DashMap<String, Vec<RegisteredHook>>,
    /// All registered hooks by ID.
    hooks_by_id: DashMap<String, RegisteredHook>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create a new empty hook registry.
    pub fn new() -> Self {
        Self {
            handlers: DashMap::new(),
            hooks_by_event: DashMap::new(),
            hooks_by_id: DashMap::new(),
        }
    }

    /// Register a named handler.
    ///
    /// Named handlers can be referenced by hooks using HookCommand::Simple.
    pub fn register_handler(
        &self,
        name: impl Into<String>,
        handler: impl HookHandler + 'static,
    ) -> Result<(), RegistryError> {
        let name = name.into();
        if self.handlers.contains_key(&name) {
            return Err(RegistryError::HandlerAlreadyExists(name));
        }
        self.handlers.insert(name, Arc::new(handler));
        Ok(())
    }

    /// Get a handler by name.
    pub fn get_handler(&self, name: &str) -> Option<AsyncHookHandler> {
        self.handlers.get(name).map(|h| h.clone())
    }

    /// Register a hook for an event.
    pub fn register_hook(&self, hook: RegisteredHook) -> Result<(), RegistryError> {
        let id = hook.id.clone();
        let event_type = hook.event_type.clone();

        if self.hooks_by_id.contains_key(&id) {
            return Err(RegistryError::HookAlreadyExists(id));
        }

        // Add to hooks by ID
        self.hooks_by_id.insert(id.clone(), hook.clone());

        // Add to hooks by event type
        let mut hooks = self.hooks_by_event.entry(event_type).or_insert_with(Vec::new);
        hooks.push(hook);
        // Keep sorted by priority
        hooks.sort_by_key(|h| h.priority);

        Ok(())
    }

    /// Unregister a hook by ID.
    pub fn unregister_hook(&self, id: &str) -> Result<RegisteredHook, RegistryError> {
        let (_, hook) = self
            .hooks_by_id
            .remove(id)
            .ok_or_else(|| RegistryError::HookNotFound(id.to_string()))?;

        // Remove from event hooks
        if let Some(mut hooks) = self.hooks_by_event.get_mut(&hook.event_type) {
            hooks.retain(|h| h.id != id);
        }

        Ok(hook)
    }

    /// Get all hooks for an event type, sorted by priority.
    pub fn get_hooks_for_event(&self, event_type: &str) -> Vec<RegisteredHook> {
        self.hooks_by_event
            .get(event_type)
            .map(|hooks| hooks.iter().filter(|h| h.enabled).cloned().collect())
            .unwrap_or_default()
    }

    /// Enable a hook by ID.
    pub fn enable_hook(&self, id: &str) -> Result<(), RegistryError> {
        self.set_hook_enabled(id, true)
    }

    /// Disable a hook by ID.
    pub fn disable_hook(&self, id: &str) -> Result<(), RegistryError> {
        self.set_hook_enabled(id, false)
    }

    fn set_hook_enabled(&self, id: &str, enabled: bool) -> Result<(), RegistryError> {
        let mut hook = self
            .hooks_by_id
            .get_mut(id)
            .ok_or_else(|| RegistryError::HookNotFound(id.to_string()))?;
        hook.enabled = enabled;

        // Update in event hooks
        if let Some(mut hooks) = self.hooks_by_event.get_mut(&hook.event_type) {
            if let Some(h) = hooks.iter_mut().find(|h| h.id == id) {
                h.enabled = enabled;
            }
        }

        Ok(())
    }

    /// List all registered hooks.
    pub fn list_hooks(&self) -> Vec<RegisteredHook> {
        self.hooks_by_id.iter().map(|r| r.value().clone()).collect()
    }

    /// List all registered handlers.
    pub fn list_handlers(&self) -> Vec<String> {
        self.handlers.iter().map(|r| r.key().clone()).collect()
    }

    /// Get the number of registered hooks.
    pub fn hook_count(&self) -> usize {
        self.hooks_by_id.len()
    }

    /// Get the number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Clear all hooks and handlers.
    pub fn clear(&self) {
        self.handlers.clear();
        self.hooks_by_event.clear();
        self.hooks_by_id.clear();
    }
}

/// Errors that can occur during registry operations.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("Handler already exists: {0}")]
    HandlerAlreadyExists(String),

    #[error("Handler not found: {0}")]
    HandlerNotFound(String),

    #[error("Hook already exists: {0}")]
    HookAlreadyExists(String),

    #[error("Hook not found: {0}")]
    HookNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::super::handlers::NoOpHandler;
    use super::*;

    #[test]
    fn test_register_handler() {
        let registry = HookRegistry::new();
        let handler = NoOpHandler::new("test_handler");

        registry.register_handler("test", handler).unwrap();
        assert!(registry.get_handler("test").is_some());
    }

    #[test]
    fn test_duplicate_handler() {
        let registry = HookRegistry::new();

        registry
            .register_handler("test", NoOpHandler::new("1"))
            .unwrap();
        assert!(registry
            .register_handler("test", NoOpHandler::new("2"))
            .is_err());
    }

    #[test]
    fn test_register_hook() {
        let registry = HookRegistry::new();
        let handler = Arc::new(NoOpHandler::new("handler"));

        let hook = RegisteredHook::new("hook1", "SessionStart", handler);
        registry.register_hook(hook).unwrap();

        let hooks = registry.get_hooks_for_event("SessionStart");
        assert_eq!(hooks.len(), 1);
    }

    #[test]
    fn test_hook_priority_ordering() {
        let registry = HookRegistry::new();

        let hook1 = RegisteredHook::new(
            "hook1",
            "SessionStart",
            Arc::new(NoOpHandler::new("1")),
        )
        .with_priority(HookPriority::LOW);

        let hook2 = RegisteredHook::new(
            "hook2",
            "SessionStart",
            Arc::new(NoOpHandler::new("2")),
        )
        .with_priority(HookPriority::HIGH);

        registry.register_hook(hook1).unwrap();
        registry.register_hook(hook2).unwrap();

        let hooks = registry.get_hooks_for_event("SessionStart");
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].id, "hook2"); // HIGH priority first
        assert_eq!(hooks[1].id, "hook1"); // LOW priority second
    }

    #[test]
    fn test_disable_hook() {
        let registry = HookRegistry::new();
        let hook = RegisteredHook::new(
            "hook1",
            "SessionStart",
            Arc::new(NoOpHandler::new("1")),
        );

        registry.register_hook(hook).unwrap();
        registry.disable_hook("hook1").unwrap();

        let hooks = registry.get_hooks_for_event("SessionStart");
        assert!(hooks.is_empty()); // Disabled hooks are filtered out
    }

    #[test]
    fn test_unregister_hook() {
        let registry = HookRegistry::new();
        let hook = RegisteredHook::new(
            "hook1",
            "SessionStart",
            Arc::new(NoOpHandler::new("1")),
        );

        registry.register_hook(hook).unwrap();
        assert_eq!(registry.hook_count(), 1);

        registry.unregister_hook("hook1").unwrap();
        assert_eq!(registry.hook_count(), 0);
    }
}
