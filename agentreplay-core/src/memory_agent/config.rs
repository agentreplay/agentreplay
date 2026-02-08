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

//! Memory agent configuration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the memory agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAgentConfig {
    /// Model to use for the memory agent (e.g., "claude-3-haiku-20240307").
    pub model: String,

    /// Maximum tokens for observation generation.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// Temperature for LLM generation (lower = more deterministic).
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Session timeout in seconds.
    #[serde(default = "default_session_timeout")]
    pub session_timeout_secs: u64,

    /// Maximum conversation history length before summarization.
    #[serde(default = "default_max_history")]
    pub max_history_messages: usize,

    /// Token budget for sliding window summarization.
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,

    /// Maximum retry attempts for failed messages.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Request timeout in seconds.
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Enable tracing for debugging.
    #[serde(default)]
    pub enable_tracing: bool,

    /// API provider configuration.
    #[serde(default)]
    pub provider: ProviderConfig,
}

fn default_max_tokens() -> u32 {
    2000
}

fn default_temperature() -> f32 {
    0.3
}

fn default_session_timeout() -> u64 {
    1800 // 30 minutes
}

fn default_max_history() -> usize {
    20
}

fn default_token_budget() -> usize {
    8000
}

fn default_max_retries() -> u32 {
    3
}

fn default_request_timeout() -> u64 {
    30
}

impl Default for MemoryAgentConfig {
    fn default() -> Self {
        Self {
            model: "claude-3-haiku-20240307".to_string(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            session_timeout_secs: default_session_timeout(),
            max_history_messages: default_max_history(),
            token_budget: default_token_budget(),
            max_retries: default_max_retries(),
            request_timeout_secs: default_request_timeout(),
            enable_tracing: false,
            provider: ProviderConfig::default(),
        }
    }
}

impl MemoryAgentConfig {
    /// Create a new configuration with the specified model.
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Set the maximum tokens.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the temperature.
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set the session timeout.
    pub fn session_timeout(mut self, timeout: Duration) -> Self {
        self.session_timeout_secs = timeout.as_secs();
        self
    }

    /// Get the session timeout as a Duration.
    pub fn session_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.session_timeout_secs)
    }

    /// Get the request timeout as a Duration.
    pub fn request_timeout_duration(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }
}

/// API provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API base URL override.
    pub base_url: Option<String>,

    /// API key environment variable name.
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,

    /// Additional headers for API requests.
    #[serde(default)]
    pub additional_headers: std::collections::HashMap<String, String>,
}

fn default_api_key_env() -> String {
    "ANTHROPIC_API_KEY".to_string()
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key_env: default_api_key_env(),
            additional_headers: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MemoryAgentConfig::default();
        assert_eq!(config.model, "claude-3-haiku-20240307");
        assert_eq!(config.max_tokens, 2000);
        assert!((config.temperature - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_builder_pattern() {
        let config = MemoryAgentConfig::with_model("gpt-4")
            .max_tokens(1000)
            .temperature(0.5)
            .session_timeout(Duration::from_secs(600));

        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.max_tokens, 1000);
        assert_eq!(config.session_timeout_secs, 600);
    }
}
