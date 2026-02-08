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

//! Caching layer for evaluation results

use crate::{EvalResult, TraceContext};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Cache for evaluation results
pub struct EvalCache {
    cache: Cache<CacheKey, HashMap<String, EvalResult>>,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl EvalCache {
    /// Create a new cache with specified TTL in seconds
    pub fn new(ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(ttl_secs))
            .build();

        Self {
            cache,
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Compute cache key for a trace and evaluator set
    pub fn compute_key(&self, trace: &TraceContext, evaluator_ids: &[String]) -> CacheKey {
        CacheKey::new(trace, evaluator_ids)
    }

    /// Get cached result
    pub async fn get(&self, key: &CacheKey) -> Option<HashMap<String, EvalResult>> {
        match self.cache.get(key).await {
            Some(result) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(result)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Set cache entry
    pub async fn set(&self, key: CacheKey, value: HashMap<String, EvalResult>) {
        self.cache.insert(key, value).await;
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: &CacheKey) {
        self.cache.invalidate(key).await;
    }

    /// Clear entire cache
    pub async fn clear(&self) {
        self.cache.invalidate_all();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        CacheStats {
            hits,
            misses,
            hit_rate: if total > 0 {
                hits as f64 / total as f64
            } else {
                0.0
            },
            entry_count: self.cache.entry_count(),
        }
    }
}

/// Cache key based on trace content and evaluator IDs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    trace_hash: u64,
    evaluator_ids_hash: u64,
}

impl CacheKey {
    pub fn new(trace: &TraceContext, evaluator_ids: &[String]) -> Self {
        let trace_hash = Self::hash_trace(trace);
        let evaluator_ids_hash = Self::hash_evaluator_ids(evaluator_ids);

        Self {
            trace_hash,
            evaluator_ids_hash,
        }
    }

    fn hash_trace(trace: &TraceContext) -> u64 {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();

        // Hash trace ID
        trace.trace_id.hash(&mut hasher);

        // Hash input/output
        if let Some(input) = &trace.input {
            input.hash(&mut hasher);
        }
        if let Some(output) = &trace.output {
            output.hash(&mut hasher);
        }

        // Hash context
        if let Some(context) = &trace.context {
            for ctx in context {
                ctx.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    fn hash_evaluator_ids(ids: &[String]) -> u64 {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        let mut sorted_ids = ids.to_vec();
        sorted_ids.sort();

        for id in sorted_ids {
            id.hash(&mut hasher);
        }

        hasher.finish()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub entry_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_set_get() {
        let cache = EvalCache::new(3600);
        let trace = TraceContext {
            trace_id: 1,
            edges: vec![],
            input: Some("test".to_string()),
            output: Some("output".to_string()),
            context: None,
            metadata: HashMap::new(),
            timestamp_us: 0,
            eval_trace: None,
        };

        let key = cache.compute_key(&trace, &["test_v1".to_string()]);
        let results = HashMap::new();

        cache.set(key.clone(), results.clone()).await;
        let cached = cache.get(&key).await;

        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = EvalCache::new(3600);
        let trace = TraceContext {
            trace_id: 1,
            edges: vec![],
            input: Some("test".to_string()),
            output: Some("output".to_string()),
            context: None,
            metadata: HashMap::new(),
            timestamp_us: 0,
            eval_trace: None,
        };

        let key = cache.compute_key(&trace, &["test_v1".to_string()]);

        // Miss
        cache.get(&key).await;

        // Hit
        cache.set(key.clone(), HashMap::new()).await;
        cache.get(&key).await;

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.5);
    }
}
