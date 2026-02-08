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

//! Evaluation Result Caching Layer
//!
//! Provides LRU caching for evaluation results to prevent redundant LLM API calls.
//! Expected 80-95% cost reduction for repeated evaluations on same traces.
//!
//! ## Cache Key Generation
//!
//! ```text
//! key = SHA256(trace_id || evaluator || sort(criteria))
//! ```
//!
//! ## Configuration
//!
//! - Max entries: 10,000
//! - TTL: 24 hours (evaluations don't change for stable traces)
//!
//! ## Expected Cache Hit Rate
//!
//! - Development: ~80% (repeated evaluations during iteration)
//! - Production: ~20-40% (similar traces common)
//!
//! ## Cost Savings
//!
//! For 1000 evaluations/day at $0.005 each:
//! - Without cache: $5.00/day
//! - With 80% hit rate: $1.00/day
//! - Annual savings: ~$1,460

use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for the evaluation cache
#[derive(Debug, Clone)]
pub struct EvalCacheConfig {
    /// Maximum number of cached entries
    pub max_entries: u64,
    /// Time-to-live for cache entries
    pub ttl: Duration,
    /// Whether to enable cache statistics tracking
    pub track_stats: bool,
}

impl Default for EvalCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            ttl: Duration::from_secs(86400), // 24 hours
            track_stats: true,
        }
    }
}

/// Cached evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEvalResult {
    /// Per-criterion scores
    pub scores: HashMap<String, f64>,
    /// Overall explanation
    pub explanation: String,
    /// Confidence score
    pub confidence: f64,
    /// Model used for evaluation
    pub model: String,
    /// Timestamp when cached
    pub cached_at: u64,
    /// Original evaluation time in milliseconds
    pub eval_time_ms: u64,
}

/// Cache key for evaluation results
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EvalCacheKey {
    hash: [u8; 32],
}

impl EvalCacheKey {
    /// Create a cache key from trace ID, evaluator type, and criteria
    pub fn new(trace_id: u128, evaluator: &str, criteria: &[String]) -> Self {
        let mut hasher = Sha256::new();

        // Add trace_id
        hasher.update(trace_id.to_le_bytes());

        // Add evaluator type
        hasher.update(evaluator.as_bytes());
        hasher.update(b"|");

        // Add sorted criteria for deterministic key
        let mut sorted_criteria: Vec<_> = criteria.iter().collect();
        sorted_criteria.sort();
        for criterion in sorted_criteria {
            hasher.update(criterion.as_bytes());
            hasher.update(b",");
        }

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);

        Self { hash }
    }

    /// Create a key from input/output/context hash (for direct evaluations)
    pub fn from_content(
        evaluator: &str,
        criteria: &[String],
        input: &str,
        output: &str,
        context: &str,
    ) -> Self {
        let mut hasher = Sha256::new();

        // Hash content
        hasher.update(input.as_bytes());
        hasher.update(b"|");
        hasher.update(output.as_bytes());
        hasher.update(b"|");
        hasher.update(context.as_bytes());
        hasher.update(b"|");

        // Add evaluator type
        hasher.update(evaluator.as_bytes());
        hasher.update(b"|");

        // Add sorted criteria
        let mut sorted_criteria: Vec<_> = criteria.iter().collect();
        sorted_criteria.sort();
        for criterion in sorted_criteria {
            hasher.update(criterion.as_bytes());
            hasher.update(b",");
        }

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);

        Self { hash }
    }

    /// Get the hash as hex string (for debugging)
    pub fn to_hex(&self) -> String {
        hex::encode(self.hash)
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct EvalCacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Cache hit rate (0.0 - 1.0)
    pub hit_rate: f64,
    /// Number of entries currently in cache
    pub entry_count: u64,
    /// Estimated cost savings (USD)
    pub estimated_savings_usd: f64,
    /// Total requests processed
    pub total_requests: u64,
}

/// Evaluation result cache
pub struct EvalCache {
    cache: Cache<EvalCacheKey, CachedEvalResult>,
    config: EvalCacheConfig,
    hits: AtomicU64,
    misses: AtomicU64,
    /// Cost per evaluation (for savings estimation)
    cost_per_eval: f64,
}

impl EvalCache {
    /// Create a new evaluation cache with the given configuration
    pub fn new(config: EvalCacheConfig) -> Self {
        let cache = Cache::builder()
            .max_capacity(config.max_entries)
            .time_to_live(config.ttl)
            .build();

        Self {
            cache,
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            cost_per_eval: 0.005, // Default: $0.005 per evaluation
        }
    }

    /// Create with default configuration
    pub fn default_cache() -> Self {
        Self::new(EvalCacheConfig::default())
    }

    /// Set the estimated cost per evaluation (for savings tracking)
    pub fn with_cost_per_eval(mut self, cost: f64) -> Self {
        self.cost_per_eval = cost;
        self
    }

    /// Get a cached result
    pub fn get(&self, key: &EvalCacheKey) -> Option<CachedEvalResult> {
        match self.cache.get(key) {
            Some(result) => {
                if self.config.track_stats {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                }
                Some(result)
            }
            None => {
                if self.config.track_stats {
                    self.misses.fetch_add(1, Ordering::Relaxed);
                }
                None
            }
        }
    }

    /// Get cached result for a trace evaluation
    pub fn get_for_trace(
        &self,
        trace_id: u128,
        evaluator: &str,
        criteria: &[String],
    ) -> Option<CachedEvalResult> {
        let key = EvalCacheKey::new(trace_id, evaluator, criteria);
        self.get(&key)
    }

    /// Get cached result for direct content evaluation
    pub fn get_for_content(
        &self,
        evaluator: &str,
        criteria: &[String],
        input: &str,
        output: &str,
        context: &str,
    ) -> Option<CachedEvalResult> {
        let key = EvalCacheKey::from_content(evaluator, criteria, input, output, context);
        self.get(&key)
    }

    /// Cache an evaluation result
    pub fn insert(&self, key: EvalCacheKey, result: CachedEvalResult) {
        self.cache.insert(key, result);
    }

    /// Cache result for a trace evaluation
    pub fn insert_for_trace(
        &self,
        trace_id: u128,
        evaluator: &str,
        criteria: &[String],
        result: CachedEvalResult,
    ) {
        let key = EvalCacheKey::new(trace_id, evaluator, criteria);
        self.insert(key, result);
    }

    /// Cache result for direct content evaluation
    pub fn insert_for_content(
        &self,
        evaluator: &str,
        criteria: &[String],
        input: &str,
        output: &str,
        context: &str,
        result: CachedEvalResult,
    ) {
        let key = EvalCacheKey::from_content(evaluator, criteria, input, output, context);
        self.insert(key, result);
    }

    /// Invalidate a cached result
    pub fn invalidate(&self, key: &EvalCacheKey) {
        self.cache.invalidate(key);
    }

    /// Clear all cached results
    pub fn clear(&self) {
        self.cache.invalidate_all();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Get cache statistics
    pub fn stats(&self) -> EvalCacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        EvalCacheStats {
            hits,
            misses,
            hit_rate,
            entry_count: self.cache.entry_count(),
            estimated_savings_usd: hits as f64 * self.cost_per_eval,
            total_requests: total,
        }
    }

    /// Get the number of entries in the cache
    pub fn len(&self) -> u64 {
        self.cache.entry_count()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.entry_count() == 0
    }
}

impl Default for EvalCache {
    fn default() -> Self {
        Self::default_cache()
    }
}

/// Thread-safe wrapper for evaluation cache
pub type SharedEvalCache = Arc<EvalCache>;

/// Create a shared evaluation cache
pub fn create_shared_cache(config: EvalCacheConfig) -> SharedEvalCache {
    Arc::new(EvalCache::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_deterministic() {
        let key1 = EvalCacheKey::new(123, "g-eval", &["coherence".into(), "fluency".into()]);
        let key2 = EvalCacheKey::new(123, "g-eval", &["fluency".into(), "coherence".into()]);

        // Keys should be equal regardless of criteria order
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_different_traces() {
        let key1 = EvalCacheKey::new(123, "g-eval", &["coherence".into()]);
        let key2 = EvalCacheKey::new(456, "g-eval", &["coherence".into()]);

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = EvalCache::default_cache();

        let result = CachedEvalResult {
            scores: [("coherence".into(), 0.85)].into_iter().collect(),
            explanation: "Good coherence".into(),
            confidence: 0.9,
            model: "gpt-4".into(),
            cached_at: 12345,
            eval_time_ms: 500,
        };

        cache.insert_for_trace(123, "g-eval", &["coherence".into()], result.clone());

        let retrieved = cache.get_for_trace(123, "g-eval", &["coherence".into()]);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().confidence, 0.9);
    }

    #[test]
    fn test_cache_miss() {
        let cache = EvalCache::default_cache();

        let result = cache.get_for_trace(999, "g-eval", &["coherence".into()]);
        assert!(result.is_none());

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = EvalCache::default_cache();

        let result = CachedEvalResult {
            scores: HashMap::new(),
            explanation: "".into(),
            confidence: 0.9,
            model: "gpt-4".into(),
            cached_at: 0,
            eval_time_ms: 0,
        };

        cache.insert_for_trace(1, "g-eval", &["test".into()], result.clone());

        // Hit
        cache.get_for_trace(1, "g-eval", &["test".into()]);
        // Miss
        cache.get_for_trace(2, "g-eval", &["test".into()]);
        // Hit
        cache.get_for_trace(1, "g-eval", &["test".into()]);

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_content_based_key() {
        let cache = EvalCache::default_cache();

        let result = CachedEvalResult {
            scores: HashMap::new(),
            explanation: "test".into(),
            confidence: 0.8,
            model: "gpt-4".into(),
            cached_at: 0,
            eval_time_ms: 100,
        };

        cache.insert_for_content(
            "g-eval",
            &["coherence".into()],
            "What is AI?",
            "AI is artificial intelligence",
            "context here",
            result,
        );

        // Same content should hit
        let retrieved = cache.get_for_content(
            "g-eval",
            &["coherence".into()],
            "What is AI?",
            "AI is artificial intelligence",
            "context here",
        );
        assert!(retrieved.is_some());

        // Different content should miss
        let missed = cache.get_for_content(
            "g-eval",
            &["coherence".into()],
            "What is ML?", // Different input
            "AI is artificial intelligence",
            "context here",
        );
        assert!(missed.is_none());
    }
}
