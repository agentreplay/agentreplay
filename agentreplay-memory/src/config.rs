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

//! Memory system configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the memory engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Data directory for persistent storage
    pub data_dir: PathBuf,

    /// Maximum observations to keep per workspace (0 = unlimited)
    pub max_observations_per_workspace: usize,

    /// Maximum session summaries to keep (0 = unlimited)
    pub max_session_summaries: usize,

    /// Enable automatic session summarization
    pub auto_summarize_sessions: bool,

    /// Token budget for context packing
    pub context_token_budget: usize,

    /// Enable semantic search (requires embedding model)
    pub enable_semantic_search: bool,

    /// Number of results to return from semantic search
    pub semantic_search_k: usize,

    /// Embedding model to use (if semantic search enabled)
    pub embedding_model: EmbeddingModel,

    /// Enable compression for stored observations
    pub enable_compression: bool,

    /// Retention policy for old observations
    pub retention_policy: RetentionPolicy,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agentreplay")
            .join("memory");

        Self {
            data_dir,
            max_observations_per_workspace: 10_000,
            max_session_summaries: 1_000,
            auto_summarize_sessions: true,
            context_token_budget: 4_000,
            enable_semantic_search: true,
            semantic_search_k: 10,
            embedding_model: EmbeddingModel::default(),
            enable_compression: true,
            retention_policy: RetentionPolicy::default(),
        }
    }
}

/// Embedding model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    /// Model provider: "local", "openai", "anthropic"
    pub provider: String,
    /// Model name
    pub model: String,
    /// Embedding dimensions
    pub dimensions: usize,
}

impl Default for EmbeddingModel {
    fn default() -> Self {
        Self {
            provider: "local".to_string(),
            model: "all-MiniLM-L6-v2".to_string(),
            dimensions: 384,
        }
    }
}

/// Retention policy for memory cleanup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Days to keep observations (0 = forever)
    pub observation_retention_days: u32,
    /// Days to keep session summaries (0 = forever)
    pub summary_retention_days: u32,
    /// Run cleanup on startup
    pub cleanup_on_startup: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            observation_retention_days: 365,
            summary_retention_days: 0, // Keep forever
            cleanup_on_startup: true,
        }
    }
}
