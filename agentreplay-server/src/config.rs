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

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// Agentreplay Server Configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub server: HttpServerConfig,
    pub storage: StorageConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub llm: LLMConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpServerConfig {
    /// HTTP API listen address (e.g., "127.0.0.1:47100")
    #[serde(default = "default_http_addr")]
    pub listen_addr: String,

    /// Maximum concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Enable CORS
    #[serde(default = "default_enable_cors")]
    pub enable_cors: bool,

    /// Allowed CORS origins (empty = allow all, use specific origins in production)
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    /// Path to Agentreplay data directory
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    /// Enable compression for stored data
    #[serde(default = "default_enable_compression")]
    pub enable_compression: bool,

    /// Use per-project storage isolation
    #[serde(default)]
    pub use_project_storage: bool,

    /// Enable high-performance WAL mode (Group Commit)
    ///
    /// When enabled, uses batched async fsync for ~10x throughput.
    /// Best for high-volume ingestion (1000+ spans/sec).
    /// Default: true for maximum throughput
    #[serde(default = "default_high_performance")]
    pub high_performance: bool,
}

fn default_high_performance() -> bool {
    true // Default to high-performance mode
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LLMConfig {
    /// OpenAI API key
    pub openai_api_key: Option<String>,

    /// Anthropic API key
    pub anthropic_api_key: Option<String>,

    /// DeepSeek API key
    pub deepseek_api_key: Option<String>,

    /// Ollama base URL (e.g., "http://localhost:11434")
    pub ollama_base_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    /// Enable authentication (default: false for development)
    #[serde(default)]
    pub enabled: bool,

    /// JWT secret for token validation (required if auth enabled)
    pub jwt_secret: Option<String>,

    /// Static API keys (format: "key:tenant_id")
    #[serde(default)]
    pub api_keys: Vec<String>,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting on authentication endpoints
    #[serde(default = "default_rate_limit_enabled")]
    pub enabled: bool,

    /// Maximum requests per window
    #[serde(default = "default_rate_limit_max_requests")]
    pub max_requests: u32,

    /// Time window in seconds
    #[serde(default = "default_rate_limit_window_secs")]
    pub window_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_requests: 100,
            window_secs: 60,
        }
    }
}

// Default values
fn default_http_addr() -> String {
    "127.0.0.1:47100".to_string()
}

fn default_max_connections() -> usize {
    1000
}

fn default_request_timeout() -> u64 {
    30
}

fn default_enable_cors() -> bool {
    true
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./agentreplay-data")
}

fn default_enable_compression() -> bool {
    true
}

fn default_rate_limit_enabled() -> bool {
    true
}

fn default_rate_limit_max_requests() -> u32 {
    100
}

fn default_rate_limit_window_secs() -> u64 {
    60
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: HttpServerConfig {
                listen_addr: default_http_addr(),
                max_connections: default_max_connections(),
                request_timeout_secs: default_request_timeout(),
                enable_cors: default_enable_cors(),
                cors_origins: vec![], // Empty = allow all (development mode)
            },
            storage: StorageConfig {
                data_dir: default_data_dir(),
                enable_compression: default_enable_compression(),
                use_project_storage: false,
                high_performance: default_high_performance(),
            },
            auth: AuthConfig {
                enabled: false,
                jwt_secret: None,
                api_keys: vec![],
                rate_limit: RateLimitConfig::default(),
            },
            llm: LLMConfig::default(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from environment variables
    ///
    /// Supported environment variables:
    /// - AGENTREPLAY_HTTP_ADDR: HTTP listen address (default: 127.0.0.1:47100)
    /// - AGENTREPLAY_DATA_DIR: Data directory path (default: ./agentreplay-data)
    /// - AGENTREPLAY_AUTH_ENABLED: Enable authentication (default: false)
    /// - AGENTREPLAY_JWT_SECRET: JWT secret for token validation
    /// - AGENTREPLAY_API_KEYS: Comma-separated API keys (format: key:tenant_id)
    /// - AGENTREPLAY_MAX_CONNECTIONS: Max concurrent connections (default: 1000)
    /// - AGENTREPLAY_REQUEST_TIMEOUT: Request timeout in seconds (default: 30)
    /// - AGENTREPLAY_ENABLE_CORS: Enable CORS (default: true)
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Server configuration
        if let Ok(addr) = std::env::var("AGENTREPLAY_HTTP_ADDR") {
            config.server.listen_addr = addr;
        }

        if let Ok(max_conn) = std::env::var("AGENTREPLAY_MAX_CONNECTIONS") {
            if let Ok(val) = max_conn.parse() {
                config.server.max_connections = val;
            }
        }

        if let Ok(timeout) = std::env::var("AGENTREPLAY_REQUEST_TIMEOUT") {
            if let Ok(val) = timeout.parse() {
                config.server.request_timeout_secs = val;
            }
        }

        if let Ok(cors) = std::env::var("AGENTREPLAY_ENABLE_CORS") {
            config.server.enable_cors = cors.parse().unwrap_or(true);
        }

        // Storage configuration
        if let Ok(data_dir) = std::env::var("AGENTREPLAY_DATA_DIR") {
            config.storage.data_dir = PathBuf::from(data_dir);
        }

        if let Ok(compress) = std::env::var("AGENTREPLAY_ENABLE_COMPRESSION") {
            config.storage.enable_compression = compress.parse().unwrap_or(true);
        }

        if let Ok(use_projects) = std::env::var("AGENTREPLAY_USE_PROJECT_STORAGE") {
            config.storage.use_project_storage = use_projects.parse().unwrap_or(false);
        }

        // Auth configuration
        if let Ok(enabled) = std::env::var("AGENTREPLAY_AUTH_ENABLED") {
            config.auth.enabled = enabled.parse().unwrap_or(false);
        }

        if let Ok(secret) = std::env::var("AGENTREPLAY_JWT_SECRET") {
            config.auth.jwt_secret = Some(secret);
        }

        if let Ok(keys) = std::env::var("AGENTREPLAY_API_KEYS") {
            config.auth.api_keys = keys.split(',').map(String::from).collect();
        }

        // LLM configuration
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            config.llm.openai_api_key = Some(key);
        }

        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            config.llm.anthropic_api_key = Some(key);
        }

        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            config.llm.deepseek_api_key = Some(key);
        }

        if let Ok(base_url) = std::env::var("OLLAMA_BASE_URL") {
            config.llm.ollama_base_url = Some(base_url);
        }

        config
    }

    /// Load configuration with priority: file > env > defaults
    pub fn load(config_file: Option<PathBuf>) -> Result<Self> {
        let mut config = if let Some(path) = config_file {
            if path.exists() {
                tracing::info!("Loading configuration from file: {:?}", path);
                Self::from_file(&path)?
            } else {
                tracing::warn!("Config file not found: {:?}, using defaults", path);
                Self::default()
            }
        } else {
            Self::default()
        };

        // Override with environment variables
        config = Self::merge_with_env(config);

        Ok(config)
    }

    /// Merge config with environment variables (env takes priority)
    fn merge_with_env(mut config: Self) -> Self {
        let env_config = Self::from_env();

        // Only override if env var was explicitly set
        if std::env::var("AGENTREPLAY_HTTP_ADDR").is_ok() {
            config.server.listen_addr = env_config.server.listen_addr;
        }
        if std::env::var("AGENTREPLAY_DATA_DIR").is_ok() {
            config.storage.data_dir = env_config.storage.data_dir;
        }
        if std::env::var("AGENTREPLAY_USE_PROJECT_STORAGE").is_ok() {
            config.storage.use_project_storage = env_config.storage.use_project_storage;
        }
        if std::env::var("AGENTREPLAY_AUTH_ENABLED").is_ok() {
            config.auth.enabled = env_config.auth.enabled;
        }
        if std::env::var("AGENTREPLAY_JWT_SECRET").is_ok() {
            config.auth.jwt_secret = env_config.auth.jwt_secret;
        }
        if std::env::var("AGENTREPLAY_API_KEYS").is_ok() {
            config.auth.api_keys = env_config.auth.api_keys;
        }

        config
    }

    /// Parse listen address as SocketAddr
    pub fn socket_addr(&self) -> Result<SocketAddr> {
        Ok(self.server.listen_addr.parse()?)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate socket address
        self.socket_addr()?;

        // Validate auth configuration
        if self.auth.enabled && self.auth.jwt_secret.is_none() && self.auth.api_keys.is_empty() {
            anyhow::bail!("Authentication enabled but no JWT secret or API keys configured");
        }

        // Validate data directory is writable
        if !self.storage.data_dir.exists() {
            std::fs::create_dir_all(&self.storage.data_dir)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.server.listen_addr, "127.0.0.1:47100");
        assert!(!config.auth.enabled);
    }

    #[test]
    fn test_from_env() {
        std::env::set_var("AGENTREPLAY_HTTP_ADDR", "0.0.0.0:8080");
        std::env::set_var("AGENTREPLAY_AUTH_ENABLED", "true");

        let config = ServerConfig::from_env();
        assert_eq!(config.server.listen_addr, "0.0.0.0:8080");
        assert!(config.auth.enabled);

        std::env::remove_var("AGENTREPLAY_HTTP_ADDR");
        std::env::remove_var("AGENTREPLAY_AUTH_ENABLED");
    }
}
