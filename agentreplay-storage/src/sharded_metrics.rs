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

//! Sharded Metrics Aggregator - High-Performance Pre-Aggregation
//!
//! A lock-free, sharded implementation of the metrics aggregator with:
//! - DashMap for concurrent access without global locks
//! - Atomic counters for lock-free updates
//! - DDSketch for streaming percentiles
//! - HyperLogLog for cardinality estimation
//! - Automatic time-based rollups
//!
//! ## Performance Characteristics
//! - Write throughput: 100K+ updates/sec per core
//! - Query latency: O(buckets) for time range
//! - Memory: O(log(latency_range)) per bucket via sketches
//!
//! ## Architecture
//! ```text
//! Sharded Aggregator
//! ├── Minute Shards[128] (DashMap per shard)
//! │   └── AtomicMetricsBucket (lock-free counters + sketches)
//! ├── Hour Buckets (rolled up from minutes)
//! └── Day Buckets (rolled up from hours)
//! ```

use crate::sketches::{DDSketch, HyperLogLog};
use dashmap::DashMap;
use agentreplay_core::AgentFlowEdge;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Number of shards for minute buckets (2x typical CPU cores)
const NUM_SHARDS: usize = 128;

/// Maximum recent edge IDs to keep per session (prevents unbounded memory)
const MAX_RECENT_EDGES: usize = 100;

/// Bucket key: (aligned_timestamp_us, project_id)
pub type ShardedBucketKey = (u64, u16);

// ============================================================================
// Bounded Index Summaries (prevent unbounded memory growth)
// ============================================================================

/// Bounded summary for a session - stores count + last N edge IDs
///
/// Memory usage: ~1KB per session (fixed) vs unbounded Vec<u128>
/// For full edge lookup, query the main CausalIndex.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Total edge count for this session
    pub edge_count: u64,
    /// First edge timestamp (for session duration calculation)
    pub first_timestamp_us: u64,
    /// Last edge timestamp
    pub last_timestamp_us: u64,
    /// Last N edge IDs (ring buffer for recent edges only)
    pub recent_edges: Vec<u128>,
}

impl SessionSummary {
    pub fn new() -> Self {
        Self {
            edge_count: 0,
            first_timestamp_us: u64::MAX,
            last_timestamp_us: 0,
            recent_edges: Vec::with_capacity(MAX_RECENT_EDGES),
        }
    }

    /// Add an edge to this session summary
    #[inline]
    pub fn add_edge(&mut self, edge_id: u128, timestamp_us: u64) {
        self.edge_count += 1;
        self.first_timestamp_us = self.first_timestamp_us.min(timestamp_us);
        self.last_timestamp_us = self.last_timestamp_us.max(timestamp_us);

        // Keep only last N edges (ring buffer behavior)
        if self.recent_edges.len() >= MAX_RECENT_EDGES {
            self.recent_edges.remove(0); // O(N) but N is small (100)
        }
        self.recent_edges.push(edge_id);
    }

    /// Get session duration in microseconds
    pub fn duration_us(&self) -> u64 {
        if self.first_timestamp_us == u64::MAX {
            0
        } else {
            self.last_timestamp_us
                .saturating_sub(self.first_timestamp_us)
        }
    }
}

impl Default for SessionSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// Bounded summary for a project - stores count only
///
/// For edge lookup, use the main storage layer with project_id filter.
#[derive(Debug, Clone)]
pub struct ProjectSummary {
    /// Total edge count for this project
    pub edge_count: u64,
    /// Last update timestamp
    pub last_update_us: u64,
}

impl ProjectSummary {
    pub fn new() -> Self {
        Self {
            edge_count: 0,
            last_update_us: 0,
        }
    }

    #[inline]
    pub fn add_edge(&mut self, _edge_id: u128) {
        self.edge_count += 1;
        // Update timestamp lazily when queried to avoid syscall on hot path
    }
}

impl Default for ProjectSummary {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Atomic Metrics Bucket
// ============================================================================

/// Atomic metrics bucket for lock-free updates
///
/// Hot counters use AtomicU64 for lock-free updates.
/// Sketches use fine-grained RwLock (low contention).
#[derive(Debug)]
pub struct AtomicMetricsBucket {
    /// Bucket start timestamp
    pub timestamp_us: u64,
    /// Project ID
    pub project_id: u16,

    // === Atomic counters (lock-free) ===
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    pub total_tokens: AtomicU64,
    pub total_duration_us: AtomicU64,
    pub total_cost_micros: AtomicU64,

    // Min/max use CAS loops (atomic but may retry)
    pub min_duration_us: AtomicU64,
    pub max_duration_us: AtomicU64,

    // === Sketches (fine-grained locks, ~1μs hold time) ===
    /// DDSketch for streaming percentiles (P50, P90, P95, P99)
    pub latency_sketch: RwLock<DDSketch>,
    /// HyperLogLog for unique session counting
    pub session_hll: RwLock<HyperLogLog>,
    /// HyperLogLog for unique agent counting
    pub agent_hll: RwLock<HyperLogLog>,
}

impl AtomicMetricsBucket {
    /// Create a new atomic bucket
    pub fn new(timestamp_us: u64, project_id: u16) -> Self {
        Self {
            timestamp_us,
            project_id,
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            total_duration_us: AtomicU64::new(0),
            total_cost_micros: AtomicU64::new(0),
            min_duration_us: AtomicU64::new(u64::MAX),
            max_duration_us: AtomicU64::new(0),
            latency_sketch: RwLock::new(DDSketch::default_accuracy()),
            session_hll: RwLock::new(HyperLogLog::new(12)), // ~1.6% error, 4KB
            agent_hll: RwLock::new(HyperLogLog::new(10)),   // ~3.2% error, 1KB
        }
    }

    /// Add an edge to this bucket (lock-free for hot counters)
    ///
    /// Performance notes:
    /// - Atomic counters: Zero contention, ~2ns per update
    /// - CAS min/max: Rare retries, ~5ns average
    /// - Sketches: try_lock to avoid blocking under high load
    #[inline]
    pub fn add_edge(&self, edge: &AgentFlowEdge, cost_micros: u64, is_error: bool) {
        // === HOT PATH: Lock-free atomic updates (guaranteed ~10ns) ===
        self.request_count.fetch_add(1, Ordering::Relaxed);
        if is_error {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
        self.total_tokens
            .fetch_add(edge.token_count as u64, Ordering::Relaxed);

        let duration = edge.duration_us as u64;
        self.total_duration_us
            .fetch_add(duration, Ordering::Relaxed);
        self.total_cost_micros
            .fetch_add(cost_micros, Ordering::Relaxed);

        // CAS loop for min (only updates if new value is smaller)
        self.update_min(duration);

        // CAS loop for max (only updates if new value is larger)
        self.update_max(duration);

        // === WARM PATH: Sketch updates with try_lock ===
        // Use try_lock to avoid blocking under high contention.
        // If lock is held, we skip this sample. With 100k+ samples/sec,
        // missing a few doesn't affect P99 accuracy significantly.
        // DDSketch accuracy comes from bucket distribution, not sample count.
        if let Some(mut sketch) = self.latency_sketch.try_write() {
            sketch.add(duration as f64);
        }

        // HyperLogLog is probabilistic - missing samples has minimal impact
        if let Some(mut hll) = self.session_hll.try_write() {
            hll.add(&edge.session_id);
        }
        if let Some(mut hll) = self.agent_hll.try_write() {
            hll.add(&edge.agent_id);
        }
    }

    /// Atomic min update using CAS
    #[inline]
    fn update_min(&self, value: u64) {
        let mut current = self.min_duration_us.load(Ordering::Relaxed);
        while value < current {
            match self.min_duration_us.compare_exchange_weak(
                current,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Atomic max update using CAS
    #[inline]
    fn update_max(&self, value: u64) {
        let mut current = self.max_duration_us.load(Ordering::Relaxed);
        while value > current {
            match self.max_duration_us.compare_exchange_weak(
                current,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Convert to a snapshot for queries/rollups
    pub fn to_snapshot(&self) -> MetricsBucketSnapshot {
        let latency_sketch = self.latency_sketch.read();
        let percentiles = latency_sketch.percentiles();

        MetricsBucketSnapshot {
            timestamp_us: self.timestamp_us,
            project_id: self.project_id,
            request_count: self.request_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            total_tokens: self.total_tokens.load(Ordering::Relaxed),
            total_duration_us: self.total_duration_us.load(Ordering::Relaxed),
            total_cost_micros: self.total_cost_micros.load(Ordering::Relaxed),
            min_duration_us: self.min_duration_us.load(Ordering::Relaxed),
            max_duration_us: self.max_duration_us.load(Ordering::Relaxed),
            p50_duration_us: percentiles.p50 as u64,
            p90_duration_us: percentiles.p90 as u64,
            p95_duration_us: percentiles.p95 as u64,
            p99_duration_us: percentiles.p99 as u64,
            unique_sessions: self.session_hll.read().cardinality() as u32,
            unique_agents: self.agent_hll.read().cardinality() as u32,
        }
    }

    /// Create a mergeable version for rollups
    pub fn to_mergeable(&self) -> MergeableMetricsBucket {
        MergeableMetricsBucket {
            timestamp_us: self.timestamp_us,
            project_id: self.project_id,
            request_count: self.request_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            total_tokens: self.total_tokens.load(Ordering::Relaxed),
            total_duration_us: self.total_duration_us.load(Ordering::Relaxed),
            total_cost_micros: self.total_cost_micros.load(Ordering::Relaxed),
            min_duration_us: self.min_duration_us.load(Ordering::Relaxed),
            max_duration_us: self.max_duration_us.load(Ordering::Relaxed),
            latency_sketch: self.latency_sketch.read().clone(),
            session_hll: self.session_hll.read().clone(),
            agent_hll: self.agent_hll.read().clone(),
        }
    }
}

/// Snapshot of bucket metrics for queries
#[derive(Debug, Clone, Default)]
pub struct MetricsBucketSnapshot {
    pub timestamp_us: u64,
    pub project_id: u16,
    pub request_count: u64,
    pub error_count: u64,
    pub total_tokens: u64,
    pub total_duration_us: u64,
    pub total_cost_micros: u64,
    pub min_duration_us: u64,
    pub max_duration_us: u64,
    pub p50_duration_us: u64,
    pub p90_duration_us: u64,
    pub p95_duration_us: u64,
    pub p99_duration_us: u64,
    pub unique_sessions: u32,
    pub unique_agents: u32,
}

impl MetricsBucketSnapshot {
    /// Get average duration in milliseconds
    pub fn avg_duration_ms(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            (self.total_duration_us as f64 / self.request_count as f64) / 1000.0
        }
    }

    /// Get error rate as percentage
    pub fn error_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            (self.error_count as f64 / self.request_count as f64) * 100.0
        }
    }

    /// Get total cost in dollars
    pub fn total_cost(&self) -> f64 {
        self.total_cost_micros as f64 / 1_000_000.0
    }
}

/// Mergeable bucket with full sketch data for rollups
#[derive(Debug, Clone)]
pub struct MergeableMetricsBucket {
    pub timestamp_us: u64,
    pub project_id: u16,
    pub request_count: u64,
    pub error_count: u64,
    pub total_tokens: u64,
    pub total_duration_us: u64,
    pub total_cost_micros: u64,
    pub min_duration_us: u64,
    pub max_duration_us: u64,
    pub latency_sketch: DDSketch,
    pub session_hll: HyperLogLog,
    pub agent_hll: HyperLogLog,
}

impl MergeableMetricsBucket {
    /// Create new empty bucket
    pub fn new(timestamp_us: u64, project_id: u16) -> Self {
        Self {
            timestamp_us,
            project_id,
            request_count: 0,
            error_count: 0,
            total_tokens: 0,
            total_duration_us: 0,
            total_cost_micros: 0,
            min_duration_us: u64::MAX,
            max_duration_us: 0,
            latency_sketch: DDSketch::default_accuracy(),
            session_hll: HyperLogLog::new(12),
            agent_hll: HyperLogLog::new(10),
        }
    }

    /// Merge another bucket into this one
    pub fn merge(&mut self, other: &MergeableMetricsBucket) {
        self.request_count += other.request_count;
        self.error_count += other.error_count;
        self.total_tokens += other.total_tokens;
        self.total_duration_us += other.total_duration_us;
        self.total_cost_micros += other.total_cost_micros;
        self.min_duration_us = self.min_duration_us.min(other.min_duration_us);
        self.max_duration_us = self.max_duration_us.max(other.max_duration_us);
        self.latency_sketch.merge(&other.latency_sketch);
        self.session_hll.merge(&other.session_hll);
        self.agent_hll.merge(&other.agent_hll);
    }

    /// Merge multiple buckets
    pub fn merge_all(buckets: &[MergeableMetricsBucket]) -> Self {
        if buckets.is_empty() {
            return Self::new(0, 0);
        }

        let mut result = buckets[0].clone();
        for bucket in &buckets[1..] {
            result.merge(bucket);
        }
        result
    }

    /// Convert to snapshot
    pub fn to_snapshot(&self) -> MetricsBucketSnapshot {
        let percentiles = self.latency_sketch.percentiles();

        MetricsBucketSnapshot {
            timestamp_us: self.timestamp_us,
            project_id: self.project_id,
            request_count: self.request_count,
            error_count: self.error_count,
            total_tokens: self.total_tokens,
            total_duration_us: self.total_duration_us,
            total_cost_micros: self.total_cost_micros,
            min_duration_us: self.min_duration_us,
            max_duration_us: self.max_duration_us,
            p50_duration_us: percentiles.p50 as u64,
            p90_duration_us: percentiles.p90 as u64,
            p95_duration_us: percentiles.p95 as u64,
            p99_duration_us: percentiles.p99 as u64,
            unique_sessions: self.session_hll.cardinality() as u32,
            unique_agents: self.agent_hll.cardinality() as u32,
        }
    }
}

/// Sharded metrics aggregator with lock-free updates
pub struct ShardedMetricsAggregator {
    /// Sharded minute buckets using DashMap
    minute_shards: Arc<[DashMap<ShardedBucketKey, AtomicMetricsBucket>; NUM_SHARDS]>,

    /// Rolled-up hour buckets (less write contention, can use RwLock)
    hour_buckets: Arc<RwLock<BTreeMap<ShardedBucketKey, MergeableMetricsBucket>>>,

    /// Rolled-up day buckets
    day_buckets: Arc<RwLock<BTreeMap<ShardedBucketKey, MergeableMetricsBucket>>>,

    /// Secondary index: session_id -> bounded summary (NOT full edge list)
    /// Stores count + last N edge IDs to prevent unbounded memory growth.
    /// For full edge lookup, use the main CausalIndex.
    session_index: Arc<DashMap<u64, SessionSummary>>,

    /// Secondary index: project_id -> bounded summary
    project_index: Arc<DashMap<u16, ProjectSummary>>,
}

impl ShardedMetricsAggregator {
    /// Create a new sharded aggregator
    pub fn new() -> Self {
        // Initialize shards
        let shards: [DashMap<ShardedBucketKey, AtomicMetricsBucket>; NUM_SHARDS] =
            std::array::from_fn(|_| DashMap::new());

        Self {
            minute_shards: Arc::new(shards),
            hour_buckets: Arc::new(RwLock::new(BTreeMap::new())),
            day_buckets: Arc::new(RwLock::new(BTreeMap::new())),
            session_index: Arc::new(DashMap::with_capacity(10_000)),
            project_index: Arc::new(DashMap::with_capacity(1_000)),
        }
    }

    /// Compute shard index from bucket key
    #[inline]
    fn shard_index(key: &ShardedBucketKey) -> usize {
        // Use ahash for fast, high-quality hashing
        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % NUM_SHARDS
    }

    /// Align timestamp to bucket boundary
    #[inline]
    fn align_to_bucket(timestamp_us: u64, bucket_size_us: u64) -> u64 {
        (timestamp_us / bucket_size_us) * bucket_size_us
    }

    /// Record a new edge (lock-free for minute buckets)
    pub fn record_edge(&self, edge: &AgentFlowEdge, cost_micros: u64, is_error: bool) {
        let minute_size = 60 * 1_000_000u64;
        let minute_ts = Self::align_to_bucket(edge.timestamp_us, minute_size);
        let key = (minute_ts, edge.project_id);

        // Get shard and update atomically
        let shard_idx = Self::shard_index(&key);
        let shard = &self.minute_shards[shard_idx];

        // DashMap entry API - gets or inserts, then updates
        shard
            .entry(key)
            .or_insert_with(|| AtomicMetricsBucket::new(minute_ts, edge.project_id))
            .add_edge(edge, cost_micros, is_error);

        // Update secondary indexes with BOUNDED summaries
        // These track count + last N edges, not unbounded vectors
        self.session_index
            .entry(edge.session_id)
            .or_insert_with(SessionSummary::new)
            .add_edge(edge.edge_id, edge.timestamp_us);

        self.project_index
            .entry(edge.project_id)
            .or_insert_with(ProjectSummary::new)
            .add_edge(edge.edge_id);
    }

    /// Simple record method
    pub fn record(&self, edge: &AgentFlowEdge) {
        self.record_edge(edge, 0, false);
    }

    /// Query metrics for a time range
    pub fn query_metrics(
        &self,
        start_us: u64,
        end_us: u64,
        project_id: Option<u16>,
    ) -> Vec<MetricsBucketSnapshot> {
        let range_us = end_us.saturating_sub(start_us);
        let minute_size = 60 * 1_000_000u64;
        let hour_size = 60 * minute_size;
        let day_size = 24 * hour_size;

        // Choose granularity based on range
        if range_us <= 6 * hour_size {
            // Up to 6 hours: use minute buckets
            self.query_minute_buckets(start_us, end_us, project_id)
        } else if range_us <= 7 * day_size {
            // Up to 7 days: use hour buckets
            self.query_hour_buckets(start_us, end_us, project_id)
        } else {
            // Longer: use day buckets
            self.query_day_buckets(start_us, end_us, project_id)
        }
    }

    /// Query minute buckets from all shards
    fn query_minute_buckets(
        &self,
        start_us: u64,
        end_us: u64,
        project_id: Option<u16>,
    ) -> Vec<MetricsBucketSnapshot> {
        let minute_size = 60 * 1_000_000u64;
        let start_aligned = Self::align_to_bucket(start_us, minute_size);
        let end_aligned = Self::align_to_bucket(end_us, minute_size) + minute_size;

        let mut results = Vec::new();

        // Query all shards
        for shard in self.minute_shards.iter() {
            for entry in shard.iter() {
                let (key, bucket) = entry.pair();
                if key.0 >= start_aligned && key.0 < end_aligned {
                    if project_id.map_or(true, |p| key.1 == p) {
                        results.push(bucket.to_snapshot());
                    }
                }
            }
        }

        // Sort by timestamp
        results.sort_by_key(|b| b.timestamp_us);
        results
    }

    /// Query hour buckets
    fn query_hour_buckets(
        &self,
        start_us: u64,
        end_us: u64,
        project_id: Option<u16>,
    ) -> Vec<MetricsBucketSnapshot> {
        let hour_size = 60 * 60 * 1_000_000u64;
        let start_aligned = Self::align_to_bucket(start_us, hour_size);
        let end_aligned = Self::align_to_bucket(end_us, hour_size) + hour_size;

        self.hour_buckets
            .read()
            .range((start_aligned, 0)..=(end_aligned, u16::MAX))
            .filter(|((ts, pid), _)| {
                *ts >= start_aligned && *ts < end_aligned && project_id.map_or(true, |p| *pid == p)
            })
            .map(|(_, bucket)| bucket.to_snapshot())
            .collect()
    }

    /// Query day buckets
    fn query_day_buckets(
        &self,
        start_us: u64,
        end_us: u64,
        project_id: Option<u16>,
    ) -> Vec<MetricsBucketSnapshot> {
        let day_size = 24 * 60 * 60 * 1_000_000u64;
        let start_aligned = Self::align_to_bucket(start_us, day_size);
        let end_aligned = Self::align_to_bucket(end_us, day_size) + day_size;

        self.day_buckets
            .read()
            .range((start_aligned, 0)..=(end_aligned, u16::MAX))
            .filter(|((ts, pid), _)| {
                *ts >= start_aligned && *ts < end_aligned && project_id.map_or(true, |p| *pid == p)
            })
            .map(|(_, bucket)| bucket.to_snapshot())
            .collect()
    }

    /// Get summary statistics for a time range
    pub fn get_summary(
        &self,
        start_us: u64,
        end_us: u64,
        project_id: Option<u16>,
    ) -> ShardedMetricsSummary {
        let buckets = self.query_metrics(start_us, end_us, project_id);

        let mut summary = ShardedMetricsSummary::default();
        for bucket in buckets {
            summary.total_requests += bucket.request_count;
            summary.total_errors += bucket.error_count;
            summary.total_tokens += bucket.total_tokens;
            summary.total_duration_us += bucket.total_duration_us;
            summary.total_cost_micros += bucket.total_cost_micros;
        }

        if summary.total_requests > 0 {
            summary.avg_duration_ms =
                (summary.total_duration_us as f64 / summary.total_requests as f64) / 1000.0;
            summary.error_rate =
                (summary.total_errors as f64 / summary.total_requests as f64) * 100.0;
        }

        summary
    }

    /// Query method for compatibility with LSMTree
    pub fn query(&self, project_id: u64, start_ts: u64, end_ts: u64) -> ShardedMetricsSummary {
        let pid = (project_id & 0xFFFF) as u16;
        self.get_summary(start_ts, end_ts, Some(pid))
    }

    /// Query timeseries buckets for rendering charts
    pub fn query_timeseries(
        &self,
        project_id: u64,
        start_ts: u64,
        end_ts: u64,
    ) -> Vec<((u64, u16), MetricsBucketSnapshot)> {
        let pid = (project_id & 0xFFFF) as u16;
        let buckets = self.query_metrics(start_ts, end_ts, Some(pid));
        buckets
            .into_iter()
            .map(|b| ((b.timestamp_us, b.project_id), b))
            .collect()
    }

    /// Get session summary (count + recent edges)
    ///
    /// Returns bounded summary instead of full edge list.
    /// For complete edge lookup, use CausalIndex with session_id filter.
    pub fn get_session_summary(&self, session_id: u64) -> Option<SessionSummary> {
        self.session_index.get(&session_id).map(|v| v.clone())
    }

    /// Get recent edge IDs for a session (last 100)
    ///
    /// DEPRECATED: Use get_session_summary() for full info.
    pub fn get_session_edges(&self, session_id: u64) -> Vec<u128> {
        self.session_index
            .get(&session_id)
            .map(|v| v.recent_edges.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get project summary (count only - edges not stored)
    pub fn get_project_summary(&self, project_id: u16) -> Option<ProjectSummary> {
        self.project_index.get(&project_id).map(|v| v.clone())
    }

    /// Get count of edges for a project (O(1))
    pub fn get_project_edge_count(&self, project_id: u16) -> usize {
        self.project_index
            .get(&project_id)
            .map(|v| v.edge_count as usize)
            .unwrap_or(0)
    }

    /// Roll up minute buckets to hour buckets
    ///
    /// Call this periodically (e.g., every hour) to aggregate old minute data.
    pub fn rollup_minutes_to_hours(&self, max_age_us: u64) {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let cutoff = now_us.saturating_sub(max_age_us);
        let hour_size = 60 * 60 * 1_000_000u64;

        // Collect minute buckets to roll up
        let mut to_rollup: std::collections::HashMap<
            ShardedBucketKey,
            Vec<MergeableMetricsBucket>,
        > = std::collections::HashMap::new();

        for shard in self.minute_shards.iter() {
            // Use retain to atomically remove old buckets
            shard.retain(|key, bucket| {
                if key.0 < cutoff {
                    let hour_ts = Self::align_to_bucket(key.0, hour_size);
                    let hour_key = (hour_ts, key.1);
                    to_rollup
                        .entry(hour_key)
                        .or_default()
                        .push(bucket.to_mergeable());
                    false // Remove from minute buckets
                } else {
                    true // Keep
                }
            });
        }

        // Merge into hour buckets
        if !to_rollup.is_empty() {
            let mut hour_buckets = self.hour_buckets.write();
            for (hour_key, minute_buckets) in to_rollup {
                let merged = MergeableMetricsBucket::merge_all(&minute_buckets);
                hour_buckets
                    .entry(hour_key)
                    .and_modify(|existing| existing.merge(&merged))
                    .or_insert(merged);
            }
        }
    }

    /// Roll up hour buckets to day buckets
    pub fn rollup_hours_to_days(&self, max_age_us: u64) {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let cutoff = now_us.saturating_sub(max_age_us);
        let day_size = 24 * 60 * 60 * 1_000_000u64;

        let mut to_rollup: std::collections::HashMap<
            ShardedBucketKey,
            Vec<MergeableMetricsBucket>,
        > = std::collections::HashMap::new();

        {
            let mut hour_buckets = self.hour_buckets.write();
            hour_buckets.retain(|key, bucket| {
                if key.0 < cutoff {
                    let day_ts = Self::align_to_bucket(key.0, day_size);
                    let day_key = (day_ts, key.1);
                    to_rollup.entry(day_key).or_default().push(bucket.clone());
                    false
                } else {
                    true
                }
            });
        }

        if !to_rollup.is_empty() {
            let mut day_buckets = self.day_buckets.write();
            for (day_key, hour_buckets) in to_rollup {
                let merged = MergeableMetricsBucket::merge_all(&hour_buckets);
                day_buckets
                    .entry(day_key)
                    .and_modify(|existing| existing.merge(&merged))
                    .or_insert(merged);
            }
        }
    }

    /// Prune old buckets (convenience method)
    pub fn prune_old_buckets(&self, max_minute_age_us: u64, max_hour_age_us: u64) {
        self.rollup_minutes_to_hours(max_minute_age_us);
        self.rollup_hours_to_days(max_hour_age_us);
    }

    /// Get bucket counts for monitoring
    pub fn bucket_counts(&self) -> (usize, usize, usize) {
        let minute_count: usize = self.minute_shards.iter().map(|s| s.len()).sum();
        (
            minute_count,
            self.hour_buckets.read().len(),
            self.day_buckets.read().len(),
        )
    }

    /// Get memory usage estimate
    pub fn memory_usage(&self) -> usize {
        let minute_count: usize = self.minute_shards.iter().map(|s| s.len()).sum();
        let hour_count = self.hour_buckets.read().len();
        let day_count = self.day_buckets.read().len();

        // Rough estimate per bucket
        let per_minute_bucket = std::mem::size_of::<AtomicMetricsBucket>() + 500; // sketches
        let per_rollup_bucket = std::mem::size_of::<MergeableMetricsBucket>() + 500;

        minute_count * per_minute_bucket
            + hour_count * per_rollup_bucket
            + day_count * per_rollup_bucket
    }
}

impl Default for ShardedMetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics
#[derive(Debug, Default, Clone)]
pub struct ShardedMetricsSummary {
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_tokens: u64,
    pub total_duration_us: u64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
    pub total_cost_micros: u64,
}

impl ShardedMetricsSummary {
    pub fn total_cost(&self) -> f64 {
        self.total_cost_micros as f64 / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sharded_record_and_query() {
        let agg = ShardedMetricsAggregator::new();

        let edge = AgentFlowEdge {
            edge_id: 1,
            timestamp_us: 100 * 1_000_000, // 100 seconds
            duration_us: 500_000,          // 500ms
            token_count: 100,
            project_id: 1,
            session_id: 42,
            ..Default::default()
        };

        agg.record_edge(&edge, 1000, false);

        let buckets = agg.query_metrics(0, 200 * 1_000_000, Some(1));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].request_count, 1);
        assert_eq!(buckets[0].total_tokens, 100);

        // Test session index
        let session_edges = agg.get_session_edges(42);
        assert_eq!(session_edges, vec![1]);
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let agg = Arc::new(ShardedMetricsAggregator::new());
        let mut handles = vec![];

        // Spawn multiple threads
        for t in 0..4 {
            let agg_clone = Arc::clone(&agg);
            handles.push(thread::spawn(move || {
                for i in 0..1000 {
                    let edge = AgentFlowEdge {
                        edge_id: (t * 1000 + i) as u128,
                        timestamp_us: 100 * 1_000_000,
                        duration_us: 500_000,
                        token_count: 100,
                        project_id: 1,
                        session_id: t as u64,
                        ..Default::default()
                    };
                    agg_clone.record_edge(&edge, 0, false);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let buckets = agg.query_metrics(0, 200 * 1_000_000, Some(1));
        let total: u64 = buckets.iter().map(|b| b.request_count).sum();
        assert_eq!(total, 4000);
    }

    #[test]
    fn test_percentiles() {
        let agg = ShardedMetricsAggregator::new();

        // Add edges with varying latencies
        for i in 0..1000 {
            let edge = AgentFlowEdge {
                edge_id: i as u128,
                timestamp_us: 100 * 1_000_000,
                duration_us: (i * 1000) as u32, // 0ms to 999ms
                token_count: 100,
                project_id: 1,
                session_id: 1,
                ..Default::default()
            };
            agg.record_edge(&edge, 0, false);
        }

        let buckets = agg.query_metrics(0, 200 * 1_000_000, Some(1));
        assert_eq!(buckets.len(), 1);

        let bucket = &buckets[0];

        // P50 should be around 500ms
        assert!(
            bucket.p50_duration_us > 400_000 && bucket.p50_duration_us < 600_000,
            "P50 was {} us",
            bucket.p50_duration_us
        );

        // P99 should be around 990ms
        assert!(
            bucket.p99_duration_us > 900_000,
            "P99 was {} us",
            bucket.p99_duration_us
        );
    }
}
