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

//! Semantic Search Engine
//!
//! Natural language search across agent traces using vector embeddings.
//!
//! ## Features
//!
//! - Hybrid search: Combines structural filters with semantic similarity
//! - Query caching: Caches query embeddings for repeated searches
//! - Reranking: Two-stage retrieval for accuracy
//! - Filtering: Time, agent, session, span type filters
//!
//! ## Usage
//!
//! ```rust,ignore
//! use agentreplay_query::semantic::{SemanticSearchEngine, SemanticQuery, QueryFilters};
//!
//! let engine = SemanticSearchEngine::new(embedding_pipeline, hnsw_index, storage);
//!
//! // Basic search
//! let results = engine.search(&SemanticQuery {
//!     query_text: "Find traces with authentication errors".to_string(),
//!     limit: 10,
//!     min_similarity: 0.7,
//!     filters: QueryFilters::default(),
//! })?;
//!
//! // Filtered search
//! let results = engine.search(&SemanticQuery {
//!     query_text: "Show tool call failures".to_string(),
//!     limit: 10,
//!     min_similarity: 0.6,
//!     filters: QueryFilters {
//!         time_range: Some(TimeRange::last_hours(24)),
//!         has_error: Some(true),
//!         ..Default::default()
//!     },
//! })?;
//! ```

use agentreplay_core::AgentFlowEdge;
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Errors from semantic search operations
#[derive(Error, Debug)]
pub enum SemanticSearchError {
    /// Embedding failed
    #[error("Embedding failed: {0}")]
    EmbeddingFailed(String),

    /// Index search failed
    #[error("Index search failed: {0}")]
    IndexSearchFailed(String),

    /// Storage access failed
    #[error("Storage failed: {0}")]
    StorageFailed(String),

    /// Invalid query
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    /// Feature not available
    #[error("Feature not available: {0}")]
    NotAvailable(String),
}

/// Semantic search query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQuery {
    /// Natural language query text
    pub query_text: String,

    /// Maximum number of results to return
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Minimum similarity threshold (0.0 - 1.0)
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,

    /// Structural filters to apply before semantic search
    #[serde(default)]
    pub filters: QueryFilters,

    /// Whether to include highlights
    #[serde(default)]
    pub include_highlights: bool,

    /// Whether to rerank with full vectors
    #[serde(default = "default_rerank")]
    pub rerank: bool,
}

fn default_limit() -> usize {
    10
}

fn default_min_similarity() -> f32 {
    0.5
}

fn default_rerank() -> bool {
    true
}

impl Default for SemanticQuery {
    fn default() -> Self {
        Self {
            query_text: String::new(),
            limit: 10,
            min_similarity: 0.5,
            filters: QueryFilters::default(),
            include_highlights: true,
            rerank: true,
        }
    }
}

/// Structural filters for hybrid search
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryFilters {
    /// Time range filter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,

    /// Filter by agent IDs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_ids: Option<Vec<u64>>,

    /// Filter by session IDs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ids: Option<Vec<u64>>,

    /// Filter by span types
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_types: Option<Vec<String>>,

    /// Filter by error state
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_error: Option<bool>,

    /// Filter by project ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<u16>,
}

impl QueryFilters {
    /// Check if any filters are set
    pub fn has_any(&self) -> bool {
        self.time_range.is_some()
            || self.agent_ids.is_some()
            || self.session_ids.is_some()
            || self.span_types.is_some()
            || self.has_error.is_some()
            || self.project_id.is_some()
    }

    /// Check if an edge matches the filters
    pub fn matches(&self, edge: &AgentFlowEdge) -> bool {
        // Check time range
        if let Some(ref range) = self.time_range {
            if edge.timestamp_us < range.start_us || edge.timestamp_us > range.end_us {
                return false;
            }
        }

        // Check agent ID
        if let Some(ref ids) = self.agent_ids {
            if !ids.contains(&edge.agent_id) {
                return false;
            }
        }

        // Check session ID
        if let Some(ref ids) = self.session_ids {
            if !ids.contains(&edge.session_id) {
                return false;
            }
        }

        // Check project ID
        if let Some(project_id) = self.project_id {
            if edge.project_id != project_id {
                return false;
            }
        }

        // Check span type
        if let Some(ref types) = self.span_types {
            let span_type_str = format!("{:?}", edge.span_type);
            if !types.iter().any(|t| span_type_str.contains(t)) {
                return false;
            }
        }

        // Check error state
        if let Some(has_error) = self.has_error {
            let edge_has_error = edge.flags & 0x01 != 0; // Assuming bit 0 is error flag
            if has_error != edge_has_error {
                return false;
            }
        }

        true
    }
}

/// Time range for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start timestamp in microseconds
    pub start_us: u64,
    /// End timestamp in microseconds
    pub end_us: u64,
}

impl TimeRange {
    /// Create a time range for the last N hours
    pub fn last_hours(hours: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let start = now - (hours * 3600 * 1_000_000);
        Self {
            start_us: start,
            end_us: now,
        }
    }

    /// Create a time range for the last N minutes
    pub fn last_minutes(minutes: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let start = now - (minutes * 60 * 1_000_000);
        Self {
            start_us: start,
            end_us: now,
        }
    }

    /// Create a time range for a specific interval
    pub fn new(start_us: u64, end_us: u64) -> Self {
        Self { start_us, end_us }
    }
}

/// Search result with similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    /// Edge ID
    pub edge_id: u128,

    /// Cosine similarity score (0.0 - 1.0)
    pub similarity: f32,

    /// The matched edge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge: Option<AgentFlowEdge>,

    /// Highlighted text snippets
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlights: Option<Vec<String>>,

    /// Rank in results (1-based)
    pub rank: usize,
}

/// Query cache entry
#[allow(dead_code)]
struct CachedQuery {
    embedding: Vec<f32>,
    timestamp: std::time::Instant,
}

/// Semantic search engine configuration
#[derive(Debug, Clone)]
pub struct SemanticSearchConfig {
    /// Maximum candidates to fetch from HNSW (before filtering/reranking)
    pub max_candidates: usize,

    /// Query cache capacity
    pub query_cache_size: u64,

    /// Query cache TTL
    pub query_cache_ttl: Duration,

    /// Minimum query length
    pub min_query_length: usize,

    /// Maximum query length
    pub max_query_length: usize,
}

impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            max_candidates: 100,
            query_cache_size: 1000,
            query_cache_ttl: Duration::from_secs(300), // 5 minutes
            min_query_length: 3,
            max_query_length: 1000,
        }
    }
}

/// Semantic search engine
///
/// Provides natural language search over embedded traces.
pub struct SemanticSearchEngine<P, I, S>
where
    P: EmbeddingProviderTrait,
    I: VectorIndexTrait,
    S: StorageTrait,
{
    /// Embedding provider
    provider: Arc<P>,

    /// Vector index (HNSW or Vamana)
    index: Arc<I>,

    /// Storage for full vectors (for reranking)
    storage: Arc<S>,

    /// Query embedding cache
    query_cache: Cache<String, Vec<f32>>,

    /// Configuration
    config: SemanticSearchConfig,
}

/// Trait for embedding providers (abstraction over agentreplay-index)
pub trait EmbeddingProviderTrait: Send + Sync {
    /// Embed a single text
    fn embed(&self, text: &str) -> Result<Vec<f32>, SemanticSearchError>;

    /// Get embedding dimension
    fn dimension(&self) -> usize;
}

/// Trait for vector index (abstraction over HNSW/Vamana)
pub trait VectorIndexTrait: Send + Sync {
    /// Search for nearest neighbors
    fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u128, f32)>, SemanticSearchError>;

    /// Search with candidate filter
    fn search_filtered(
        &self,
        query: &[f32],
        k: usize,
        candidates: &HashSet<u128>,
    ) -> Result<Vec<(u128, f32)>, SemanticSearchError>;

    /// Check if an ID is in the index
    fn contains(&self, id: u128) -> bool;
}

/// Trait for storage (abstraction over agentreplay-storage)
pub trait StorageTrait: Send + Sync {
    /// Get an edge by ID
    fn get_edge(&self, id: u128) -> Result<Option<AgentFlowEdge>, SemanticSearchError>;

    /// Get full vector for reranking
    fn get_vector(&self, id: u128) -> Result<Option<Vec<f32>>, SemanticSearchError>;

    /// Get edges matching filters
    fn get_filtered_ids(
        &self,
        filters: &QueryFilters,
    ) -> Result<HashSet<u128>, SemanticSearchError>;
}

impl<P, I, S> SemanticSearchEngine<P, I, S>
where
    P: EmbeddingProviderTrait,
    I: VectorIndexTrait,
    S: StorageTrait,
{
    /// Create a new semantic search engine
    pub fn new(provider: Arc<P>, index: Arc<I>, storage: Arc<S>) -> Self {
        let config = SemanticSearchConfig::default();
        Self::with_config(provider, index, storage, config)
    }

    /// Create with custom configuration
    pub fn with_config(
        provider: Arc<P>,
        index: Arc<I>,
        storage: Arc<S>,
        config: SemanticSearchConfig,
    ) -> Self {
        let query_cache = Cache::builder()
            .max_capacity(config.query_cache_size)
            .time_to_live(config.query_cache_ttl)
            .build();

        Self {
            provider,
            index,
            storage,
            query_cache,
            config,
        }
    }

    /// Execute a semantic search
    pub fn search(
        &self,
        query: &SemanticQuery,
    ) -> Result<Vec<SemanticSearchResult>, SemanticSearchError> {
        // Validate query
        if query.query_text.len() < self.config.min_query_length {
            return Err(SemanticSearchError::InvalidQuery(format!(
                "Query too short (min {} chars)",
                self.config.min_query_length
            )));
        }

        if query.query_text.len() > self.config.max_query_length {
            return Err(SemanticSearchError::InvalidQuery(format!(
                "Query too long (max {} chars)",
                self.config.max_query_length
            )));
        }

        // Get or compute query embedding
        let query_embedding = self.get_or_embed_query(&query.query_text)?;

        // Determine candidate set
        let k = (query.limit * 10).min(self.config.max_candidates);

        let raw_results = if query.filters.has_any() {
            // Filtered search
            let candidates = self.storage.get_filtered_ids(&query.filters)?;
            if candidates.is_empty() {
                return Ok(Vec::new());
            }
            self.index
                .search_filtered(&query_embedding, k, &candidates)?
        } else {
            // Full index search
            self.index.search(&query_embedding, k)?
        };

        // Rerank with full vectors if enabled
        let ranked_results = if query.rerank {
            self.rerank(&query_embedding, &raw_results)?
        } else {
            raw_results
        };

        // Filter by similarity and build results
        let mut results: Vec<SemanticSearchResult> = Vec::new();

        for (rank, (edge_id, similarity)) in ranked_results.iter().enumerate() {
            if *similarity < query.min_similarity {
                continue;
            }

            if results.len() >= query.limit {
                break;
            }

            let edge = if query.include_highlights {
                self.storage.get_edge(*edge_id)?
            } else {
                None
            };

            let highlights = if query.include_highlights {
                self.generate_highlights(&query.query_text, edge.as_ref())
            } else {
                None
            };

            results.push(SemanticSearchResult {
                edge_id: *edge_id,
                similarity: *similarity,
                edge,
                highlights,
                rank: rank + 1,
            });
        }

        Ok(results)
    }

    /// Get cached embedding or compute new one
    fn get_or_embed_query(&self, query_text: &str) -> Result<Vec<f32>, SemanticSearchError> {
        // Normalize query for cache key
        let cache_key = query_text.to_lowercase().trim().to_string();

        // Check cache
        if let Some(embedding) = self.query_cache.get(&cache_key) {
            return Ok(embedding);
        }

        // Compute embedding
        let embedding = self.provider.embed(query_text)?;

        // Cache it
        self.query_cache.insert(cache_key, embedding.clone());

        Ok(embedding)
    }

    /// Rerank results using full vectors
    fn rerank(
        &self,
        query: &[f32],
        candidates: &[(u128, f32)],
    ) -> Result<Vec<(u128, f32)>, SemanticSearchError> {
        let mut reranked: Vec<(u128, f32)> = Vec::with_capacity(candidates.len());

        for (id, _approx_score) in candidates {
            // Get full vector
            if let Some(full_vector) = self.storage.get_vector(*id)? {
                // Compute exact cosine similarity
                let similarity = cosine_similarity(query, &full_vector);
                reranked.push((*id, similarity));
            }
        }

        // Sort by similarity descending
        reranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(reranked)
    }

    /// Generate highlight snippets
    fn generate_highlights(
        &self,
        query: &str,
        edge: Option<&AgentFlowEdge>,
    ) -> Option<Vec<String>> {
        let _edge = edge?;

        // Simple keyword-based highlighting
        let query_words: Vec<&str> = query.split_whitespace().collect();
        if query_words.is_empty() {
            return None;
        }

        // In a real implementation, this would:
        // 1. Get the payload text
        // 2. Find sentences containing query words
        // 3. Return highlighted snippets

        Some(vec![format!("Matched query: {}", query)])
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (u64, u64) {
        (self.query_cache.entry_count(), self.config.query_cache_size)
    }

    /// Clear the query cache
    pub fn clear_cache(&self) {
        self.query_cache.invalidate_all();
    }

    /// Get embedding dimension
    pub fn dimension(&self) -> usize {
        self.provider.dimension()
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-8 || norm_b < 1e-8 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range() {
        let range = TimeRange::last_hours(24);
        assert!(range.start_us < range.end_us);
        assert_eq!(range.end_us - range.start_us, 24 * 3600 * 1_000_000);
    }

    #[test]
    fn test_query_filters_empty() {
        let filters = QueryFilters::default();
        assert!(!filters.has_any());
    }

    #[test]
    fn test_query_filters_with_time() {
        let filters = QueryFilters {
            time_range: Some(TimeRange::last_hours(1)),
            ..Default::default()
        };
        assert!(filters.has_any());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 1e-5);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_semantic_query_default() {
        let query = SemanticQuery::default();
        assert_eq!(query.limit, 10);
        assert_eq!(query.min_similarity, 0.5);
        assert!(query.rerank);
    }

    #[test]
    fn test_semantic_query_serialization() {
        let query = SemanticQuery {
            query_text: "Find errors in tool calls".to_string(),
            limit: 20,
            min_similarity: 0.7,
            filters: QueryFilters {
                has_error: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };

        let json = serde_json::to_string(&query).unwrap();
        let parsed: SemanticQuery = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.query_text, query.query_text);
        assert_eq!(parsed.limit, query.limit);
        assert_eq!(parsed.min_similarity, query.min_similarity);
    }
}
