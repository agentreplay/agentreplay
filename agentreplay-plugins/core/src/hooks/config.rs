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

//! Hook configuration and definition structures.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Configuration for the hook system.
///
/// # Example JSON Configuration
///
/// ```json
/// {
///     "hooks": [
///         {"event": "SessionStart", "command": "init_memory", "priority": 10},
///         {"event": "UserPromptSubmit", "command": "capture_prompt"},
///         {"event": "PostToolUse", "command": "compress_observation"},
///         {"event": "Stop", "command": "summarize_session", "timeout_ms": 30000}
///     ],
///     "default_timeout_ms": 5000,
///     "max_concurrent_hooks": 4
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// List of hook definitions.
    pub hooks: Vec<HookDefinition>,

    /// Default timeout for hook execution in milliseconds.
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: u64,

    /// Maximum number of concurrent hook executions.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_hooks: usize,

    /// Whether to continue dispatching if a hook fails.
    #[serde(default = "default_continue_on_error")]
    pub continue_on_error: bool,

    /// Enable hook execution tracing.
    #[serde(default)]
    pub enable_tracing: bool,
}

fn default_timeout() -> u64 {
    5000
}

fn default_max_concurrent() -> usize {
    4
}

fn default_continue_on_error() -> bool {
    true
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            hooks: Vec::new(),
            default_timeout_ms: default_timeout(),
            max_concurrent_hooks: default_max_concurrent(),
            continue_on_error: default_continue_on_error(),
            enable_tracing: false,
        }
    }
}

impl HookConfig {
    /// Create a new hook configuration from JSON string.
    pub fn from_json(json: &str) -> Result<Self, HookConfigError> {
        serde_json::from_str(json).map_err(|e| HookConfigError::ParseError(e.to_string()))
    }

    /// Create a new hook configuration from TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, HookConfigError> {
        toml::from_str(toml_str).map_err(|e| HookConfigError::ParseError(e.to_string()))
    }

    /// Get the default timeout as a Duration.
    pub fn default_timeout(&self) -> Duration {
        Duration::from_millis(self.default_timeout_ms)
    }

    /// Find all hooks for a specific event type.
    pub fn hooks_for_event(&self, event_type: &str) -> Vec<&HookDefinition> {
        let mut hooks: Vec<_> = self
            .hooks
            .iter()
            .filter(|h| h.event == event_type)
            .collect();
        // Sort by priority (lower executes first)
        hooks.sort_by_key(|h| h.priority);
        hooks
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), HookConfigError> {
        for (i, hook) in self.hooks.iter().enumerate() {
            hook.validate().map_err(|e| {
                HookConfigError::InvalidHook {
                    index: i,
                    reason: e.to_string(),
                }
            })?;
        }
        Ok(())
    }
}

/// Definition of a single hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinition {
    /// Event type to hook into.
    /// Valid values: "SessionStart", "UserPromptSubmit", "PreToolUse",
    /// "PostToolUse", "AssistantResponse", "Stop", "SessionTimeout", "Error", "Custom"
    pub event: String,

    /// Command to execute when the hook is triggered.
    pub command: HookCommand,

    /// Priority for execution order (lower values execute first).
    #[serde(default = "default_priority")]
    pub priority: i32,

    /// Timeout override for this specific hook (in milliseconds).
    pub timeout_ms: Option<u64>,

    /// Whether this hook is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Optional filter condition (JSON path expression).
    pub filter: Option<String>,

    /// Optional hook name for identification.
    pub name: Option<String>,

    /// Optional description.
    pub description: Option<String>,
}

fn default_priority() -> i32 {
    100
}

fn default_enabled() -> bool {
    true
}

impl HookDefinition {
    /// Create a new hook definition.
    pub fn new(event: impl Into<String>, command: HookCommand) -> Self {
        Self {
            event: event.into(),
            command,
            priority: default_priority(),
            timeout_ms: None,
            enabled: default_enabled(),
            filter: None,
            name: None,
            description: None,
        }
    }

    /// Set the priority for this hook.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the timeout for this hook.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set the name for this hook.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Get the timeout duration for this hook.
    pub fn timeout(&self, default: Duration) -> Duration {
        self.timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(default)
    }

    /// Validate this hook definition.
    pub fn validate(&self) -> Result<(), HookConfigError> {
        // Validate event type
        let valid_events = [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "AssistantResponse",
            "Stop",
            "SessionTimeout",
            "Error",
            "Custom",
        ];

        if !valid_events.contains(&self.event.as_str()) {
            return Err(HookConfigError::InvalidEventType(self.event.clone()));
        }

        // Validate command
        self.command.validate()?;

        Ok(())
    }
}

/// Command to execute when a hook is triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookCommand {
    /// Simple command name (resolved from registry).
    Simple(String),

    /// Inline handler configuration.
    Inline {
        /// Handler type.
        handler: String,
        /// Handler-specific configuration.
        #[serde(default)]
        config: serde_json::Value,
    },

    /// External command execution.
    External {
        /// Command to execute.
        command: String,
        /// Command arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables.
        #[serde(default)]
        env: std::collections::HashMap<String, String>,
        /// Working directory.
        cwd: Option<String>,
    },

    /// HTTP webhook.
    Webhook {
        /// URL to call.
        url: String,
        /// HTTP method (default: POST).
        #[serde(default = "default_http_method")]
        method: String,
        /// Additional headers.
        #[serde(default)]
        headers: std::collections::HashMap<String, String>,
    },
}

fn default_http_method() -> String {
    "POST".to_string()
}

impl HookCommand {
    /// Validate the command configuration.
    pub fn validate(&self) -> Result<(), HookConfigError> {
        match self {
            HookCommand::Simple(name) => {
                if name.is_empty() {
                    return Err(HookConfigError::EmptyCommandName);
                }
            }
            HookCommand::Inline { handler, .. } => {
                if handler.is_empty() {
                    return Err(HookConfigError::EmptyHandlerType);
                }
            }
            HookCommand::External { command, .. } => {
                if command.is_empty() {
                    return Err(HookConfigError::EmptyCommandName);
                }
            }
            HookCommand::Webhook { url, method, .. } => {
                if url.is_empty() {
                    return Err(HookConfigError::InvalidWebhookUrl(
                        "URL cannot be empty".to_string(),
                    ));
                }
                if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
                    return Err(HookConfigError::InvalidHttpMethod(method.clone()));
                }
            }
        }
        Ok(())
    }

    /// Get a descriptive name for this command.
    pub fn name(&self) -> String {
        match self {
            HookCommand::Simple(name) => name.clone(),
            HookCommand::Inline { handler, .. } => format!("inline:{}", handler),
            HookCommand::External { command, .. } => format!("exec:{}", command),
            HookCommand::Webhook { url, .. } => format!("webhook:{}", url),
        }
    }
}

/// Errors that can occur during hook configuration.
#[derive(Debug, Error)]
pub enum HookConfigError {
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Invalid event type: {0}")]
    InvalidEventType(String),

    #[error("Invalid hook at index {index}: {reason}")]
    InvalidHook { index: usize, reason: String },

    #[error("Command name cannot be empty")]
    EmptyCommandName,

    #[error("Handler type cannot be empty")]
    EmptyHandlerType,

    #[error("Invalid webhook URL: {0}")]
    InvalidWebhookUrl(String),

    #[error("Invalid HTTP method: {0}")]
    InvalidHttpMethod(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_config() {
        let json = r#"{
            "hooks": [
                {"event": "SessionStart", "command": "init_memory"},
                {"event": "Stop", "command": "summarize", "priority": 10}
            ],
            "default_timeout_ms": 10000
        }"#;

        let config = HookConfig::from_json(json).unwrap();
        assert_eq!(config.hooks.len(), 2);
        assert_eq!(config.default_timeout_ms, 10000);
    }

    #[test]
    fn test_hooks_for_event() {
        let config = HookConfig {
            hooks: vec![
                HookDefinition::new("SessionStart", HookCommand::Simple("a".to_string()))
                    .with_priority(20),
                HookDefinition::new("SessionStart", HookCommand::Simple("b".to_string()))
                    .with_priority(10),
                HookDefinition::new("Stop", HookCommand::Simple("c".to_string())),
            ],
            ..Default::default()
        };

        let start_hooks = config.hooks_for_event("SessionStart");
        assert_eq!(start_hooks.len(), 2);
        assert_eq!(start_hooks[0].priority, 10); // Lower priority first
    }

    #[test]
    fn test_validate_invalid_event() {
        let hook = HookDefinition::new("InvalidEvent", HookCommand::Simple("test".to_string()));
        assert!(hook.validate().is_err());
    }

    #[test]
    fn test_webhook_command() {
        let cmd = HookCommand::Webhook {
            url: "https://example.com/hook".to_string(),
            method: "POST".to_string(),
            headers: std::collections::HashMap::new(),
        };
        assert!(cmd.validate().is_ok());

        let invalid_cmd = HookCommand::Webhook {
            url: "".to_string(),
            method: "POST".to_string(),
            headers: std::collections::HashMap::new(),
        };
        assert!(invalid_cmd.validate().is_err());
    }
}
