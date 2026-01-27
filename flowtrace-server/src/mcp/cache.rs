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

//! MCP resource cache with invalidation.

use crate::mcp::resource::ContextRequest;
use blake3::Hasher;
use moka::sync::Cache;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Cache key for context resources.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CacheKey {
    pub project_id: u128,
    pub filter_hash: u64,
}

/// Invalidation event emitted on observation updates.
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    pub project_id: u128,
}

/// Cached context document.
#[derive(Debug, Clone)]
pub struct ContextDocument {
    pub content: String,
}

/// Resource cache with TTL and invalidation.
pub struct ResourceCache {
    cache: Cache<CacheKey, Arc<ContextDocument>>,
    invalidation_rx: broadcast::Receiver<InvalidationEvent>,
}

impl ResourceCache {
    /// Create a new cache with TTL and capacity.
    pub fn new(
        ttl: Duration,
        capacity: u64,
        invalidation_rx: broadcast::Receiver<InvalidationEvent>,
    ) -> Self {
        let cache = Cache::builder()
            .time_to_live(ttl)
            .max_capacity(capacity)
            .build();
        Self { cache, invalidation_rx }
    }

    /// Compute a cache key for a request.
    pub fn key_for_request(request: &ContextRequest) -> CacheKey {
        let filter_hash = hash_request(request);
        CacheKey {
            project_id: request.project_id,
            filter_hash,
        }
    }

    /// Get cached context or build and store.
    pub fn get_or_build<F>(&self, key: CacheKey, builder: F) -> Arc<ContextDocument>
    where
        F: FnOnce() -> ContextDocument,
    {
        self.cache.get_with(key, || Arc::new(builder()))
    }

    /// Invalidation loop to purge entries for updated projects.
    pub async fn invalidation_loop(&mut self) {
        while let Ok(event) = self.invalidation_rx.recv().await {
            let project_id = event.project_id;
            let _ = self
                .cache
                .invalidate_entries_if(move |k, _| k.project_id == project_id);
        }
    }
}

fn hash_request(request: &ContextRequest) -> u64 {
    #[derive(Serialize)]
    struct HashableRequest<'a> {
        project_id: u128,
        session_id: Option<u128>,
        max_observations: Option<usize>,
        max_tokens: Option<usize>,
        concepts: Option<&'a Vec<String>>,
        since: Option<u64>,
        query: Option<&'a String>,
    }

    let data = HashableRequest {
        project_id: request.project_id,
        session_id: request.session_id,
        max_observations: request.max_observations,
        max_tokens: request.max_tokens,
        concepts: request.concepts.as_ref(),
        since: request.since,
        query: request.query.as_ref(),
    };

    let bytes = serde_json::to_vec(&data).unwrap_or_default();
    let mut hasher = Hasher::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    u64::from_le_bytes(hash.as_bytes()[0..8].try_into().unwrap())
}
