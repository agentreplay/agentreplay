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

//! Event Store - Storage for OpenTelemetry span events
//!
//! Stores events separately from edges for efficient querying by event type,
//! role, and content.
//!
//! CRITICAL FIX: Now uses bounded LRU cache to prevent unbounded memory growth.
//! Default capacity is 100,000 entries with automatic LRU eviction.

use agentreplay_core::{Result, SpanEvent};
use moka::sync::Cache;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Default maximum number of entries in the event cache
const DEFAULT_MAX_CAPACITY: u64 = 100_000;

/// Default TTL for cache entries (24 hours)
const DEFAULT_TTL_SECS: u64 = 24 * 60 * 60;

/// Event store for managing span events
///
/// CRITICAL FIX: Uses bounded moka cache with:
/// - Maximum capacity (default 100K entries)
/// - LRU eviction when capacity exceeded
/// - TTL-based expiration (default 24h)
/// - Thread-safe concurrent access
pub struct EventStore {
    cache: Arc<Cache<u128, Vec<SpanEvent>>>,
}

impl EventStore {
    /// Create a new event store with default capacity
    pub fn new(_path: impl AsRef<Path>) -> Result<Self> {
        Self::with_capacity(DEFAULT_MAX_CAPACITY, DEFAULT_TTL_SECS)
    }

    /// Create a new event store with custom capacity and TTL
    pub fn with_capacity(max_capacity: u64, ttl_secs: u64) -> Result<Self> {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(Duration::from_secs(ttl_secs))
            .build();

        Ok(Self {
            cache: Arc::new(cache),
        })
    }

    /// Store events for an edge
    pub fn put_events(&self, edge_id: u128, events: &[SpanEvent]) -> Result<()> {
        self.cache.insert(edge_id, events.to_vec());
        Ok(())
    }

    /// Get all events for an edge
    pub fn get_events(&self, edge_id: u128) -> Result<Vec<SpanEvent>> {
        Ok(self.cache.get(&edge_id).unwrap_or_default())
    }

    /// Delete events for an edge
    pub fn delete_events(&self, edge_id: u128) -> Result<()> {
        self.cache.invalidate(&edge_id);
        Ok(())
    }

    /// Get current number of entries in the cache
    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }

    /// Get weighted size of entries (if weight function configured)
    pub fn weighted_size(&self) -> u64 {
        self.cache.weighted_size()
    }

    /// Force eviction of expired entries
    pub fn run_pending_tasks(&self) {
        self.cache.run_pending_tasks();
    }
}
