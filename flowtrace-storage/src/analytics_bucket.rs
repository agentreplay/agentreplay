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

//! Analytics Bucket with Temporal Bloom Filters
//!
//! Extends AtomicMetricsBucket with Bloom filters for fast range query negation.
//! When querying with filters like "WHERE project_id = X AND agent_id = Y",
//! the Bloom filter enables O(1) bucket rejection instead of O(bucket_size) scans.
//!
//! ## False Positive Rates
//! - 1% FPR with optimal sizing (~1KB per bucket for 1000 items)
//! - On average, scan 1% extra buckets (acceptable trade-off)
//!
//! ## Query Pattern
//! ```text
//! for bucket in time_range_buckets:
//!     if bloom.may_contain(project_id, agent_id, ...):
//!         scan_bucket(bucket)  # Only if Bloom says "maybe"
//!     else:
//!         skip_bucket(bucket)  # Definite negative - O(1) rejection
//! ```

use crate::bloom::BloomFilter;
use crate::sharded_metrics::{AtomicMetricsBucket, MergeableMetricsBucket, MetricsBucketSnapshot};
use flowtrace_core::AgentFlowEdge;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Query filters for analytics queries
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    pub project_id: Option<u16>,
    pub agent_id: Option<u64>,
    pub session_id: Option<u64>,
    pub model_id: Option<u64>,
    pub has_error: Option<bool>,
    pub min_duration_us: Option<u64>,
    pub max_duration_us: Option<u64>,
}

impl QueryFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_project(mut self, id: u16) -> Self {
        self.project_id = Some(id);
        self
    }

    pub fn with_agent(mut self, id: u64) -> Self {
        self.agent_id = Some(id);
        self
    }

    pub fn with_session(mut self, id: u64) -> Self {
        self.session_id = Some(id);
        self
    }

    pub fn with_error_filter(mut self, has_error: bool) -> Self {
        self.has_error = Some(has_error);
        self
    }

    pub fn with_duration_range(mut self, min_us: u64, max_us: u64) -> Self {
        self.min_duration_us = Some(min_us);
        self.max_duration_us = Some(max_us);
        self
    }

    /// Check if any filters are set
    pub fn is_empty(&self) -> bool {
        self.project_id.is_none()
            && self.agent_id.is_none()
            && self.session_id.is_none()
            && self.model_id.is_none()
            && self.has_error.is_none()
            && self.min_duration_us.is_none()
            && self.max_duration_us.is_none()
    }
}

/// Bloom filter key types for different queryable dimensions
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum BloomKey {
    Project(u16),
    Agent(u64),
    Session(u64),
    Model(u64),
    HasError,
}

/// Analytics bucket with Bloom filter for fast query rejection
pub struct AnalyticsBucket {
    /// Core metrics with atomic counters and sketches
    pub metrics: AtomicMetricsBucket,

    /// Bloom filter for queryable dimensions
    /// Expected ~1000 unique combinations per minute bucket
    /// 1% FPR = ~1.2KB per bucket
    filter: RwLock<BloomFilter>,

    /// Count of distinct items inserted (for filter saturation detection)
    distinct_count: AtomicU64,

    /// Maximum items before filter needs rebuild
    max_items: usize,
}

impl AnalyticsBucket {
    /// Create new analytics bucket
    ///
    /// # Arguments
    /// * `timestamp_us` - Bucket start timestamp
    /// * `project_id` - Project identifier
    /// * `expected_items` - Expected unique filter combinations (default: 1000)
    /// * `fpr` - False positive rate (default: 0.01 = 1%)
    pub fn new(timestamp_us: u64, project_id: u16) -> Self {
        Self::with_capacity(timestamp_us, project_id, 1000, 0.01)
    }

    /// Create with custom capacity
    pub fn with_capacity(
        timestamp_us: u64,
        project_id: u16,
        expected_items: usize,
        fpr: f64,
    ) -> Self {
        Self {
            metrics: AtomicMetricsBucket::new(timestamp_us, project_id),
            filter: RwLock::new(BloomFilter::new(expected_items, fpr)),
            distinct_count: AtomicU64::new(0),
            max_items: expected_items * 2, // Rebuild at 2x expected
        }
    }

    /// Add an edge to this bucket
    ///
    /// Updates both the metrics and the Bloom filter
    #[inline]
    pub fn add_edge(&self, edge: &AgentFlowEdge, cost_micros: u64, is_error: bool) {
        // Update core metrics (lock-free for hot counters)
        self.metrics.add_edge(edge, cost_micros, is_error);

        // Update Bloom filter with queryable dimensions
        // This is a write lock but very fast (~1μs)
        let mut filter = self.filter.write();

        // Insert dimension keys
        filter.insert(&BloomKey::Project(edge.project_id));
        filter.insert(&BloomKey::Agent(edge.agent_id));
        filter.insert(&BloomKey::Session(edge.session_id));

        if is_error {
            filter.insert(&BloomKey::HasError);
        }

        // Track distinct count for saturation monitoring
        self.distinct_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Fast check if bucket may match the query filters
    ///
    /// Returns `false` if the bucket definitely doesn't contain matching data (O(1) rejection)
    /// Returns `true` if the bucket might contain matching data (needs full scan)
    #[inline]
    pub fn may_match(&self, filters: &QueryFilters) -> bool {
        // No filters = match everything
        if filters.is_empty() {
            return true;
        }

        let filter = self.filter.read();

        // Check each filter dimension against Bloom filter
        if let Some(pid) = filters.project_id {
            if !filter.contains(&BloomKey::Project(pid)) {
                return false; // Definitely not in bucket
            }
        }

        if let Some(aid) = filters.agent_id {
            if !filter.contains(&BloomKey::Agent(aid)) {
                return false;
            }
        }

        if let Some(sid) = filters.session_id {
            if !filter.contains(&BloomKey::Session(sid)) {
                return false;
            }
        }

        if let Some(mid) = filters.model_id {
            if !filter.contains(&BloomKey::Model(mid)) {
                return false;
            }
        }

        if let Some(true) = filters.has_error {
            if !filter.contains(&BloomKey::HasError) {
                return false;
            }
        }

        // Duration range filters can't use Bloom (continuous values)
        // These require actual data scan

        true // May match - needs full scan
    }

    /// Get metrics snapshot
    pub fn to_snapshot(&self) -> MetricsBucketSnapshot {
        self.metrics.to_snapshot()
    }

    /// Get mergeable version for rollups
    pub fn to_mergeable(&self) -> MergeableMetricsBucket {
        self.metrics.to_mergeable()
    }

    /// Check if Bloom filter is saturated (too many items)
    pub fn is_filter_saturated(&self) -> bool {
        self.distinct_count.load(Ordering::Relaxed) as usize > self.max_items
    }

    /// Get filter statistics
    pub fn filter_stats(&self) -> BloomFilterStats {
        let filter = self.filter.read();
        BloomFilterStats {
            distinct_items: self.distinct_count.load(Ordering::Relaxed),
            max_items: self.max_items as u64,
            is_saturated: self.is_filter_saturated(),
            memory_bytes: filter.memory_size(),
        }
    }
}

/// Bloom filter statistics
#[derive(Debug, Clone)]
pub struct BloomFilterStats {
    pub distinct_items: u64,
    pub max_items: u64,
    pub is_saturated: bool,
    pub memory_bytes: usize,
}

/// Query execution result with statistics
#[derive(Debug, Clone, Default)]
pub struct QueryExecutionStats {
    pub buckets_scanned: usize,
    pub buckets_skipped: usize,
    pub bloom_rejections: usize,
    pub duration_filter_rejections: usize,
    pub total_buckets: usize,
}

impl QueryExecutionStats {
    /// Get skip rate as percentage
    pub fn skip_rate(&self) -> f64 {
        if self.total_buckets == 0 {
            return 0.0;
        }
        (self.buckets_skipped as f64 / self.total_buckets as f64) * 100.0
    }

    /// Get Bloom filter efficiency
    pub fn bloom_efficiency(&self) -> f64 {
        if self.total_buckets == 0 {
            return 0.0;
        }
        (self.bloom_rejections as f64 / self.total_buckets as f64) * 100.0
    }
}

/// Execute analytics query with Bloom filter optimization
pub fn execute_analytics_query(
    buckets: &[AnalyticsBucket],
    filters: &QueryFilters,
) -> (Vec<MetricsBucketSnapshot>, QueryExecutionStats) {
    let mut results = Vec::new();
    let mut stats = QueryExecutionStats {
        total_buckets: buckets.len(),
        ..Default::default()
    };

    for bucket in buckets {
        // O(1) Bloom filter check
        if bucket.may_match(filters) {
            // Need to scan this bucket
            let snapshot = bucket.to_snapshot();

            // Apply duration filters (can't use Bloom)
            let duration_match = match (filters.min_duration_us, filters.max_duration_us) {
                (Some(min), Some(max)) => {
                    snapshot.max_duration_us >= min && snapshot.min_duration_us <= max
                }
                (Some(min), None) => snapshot.max_duration_us >= min,
                (None, Some(max)) => snapshot.min_duration_us <= max,
                (None, None) => true,
            };

            if duration_match {
                results.push(snapshot);
                stats.buckets_scanned += 1;
            } else {
                stats.duration_filter_rejections += 1;
                stats.buckets_skipped += 1;
            }
        } else {
            // Bloom filter rejection - O(1) skip
            stats.bloom_rejections += 1;
            stats.buckets_skipped += 1;
        }
    }

    (results, stats)
}

/// Aggregate query results into summary
pub fn aggregate_results(results: &[MetricsBucketSnapshot]) -> AggregatedMetrics {
    let mut agg = AggregatedMetrics::default();

    for bucket in results {
        agg.total_requests += bucket.request_count;
        agg.total_errors += bucket.error_count;
        agg.total_tokens += bucket.total_tokens;
        agg.total_duration_us += bucket.total_duration_us;
        agg.total_cost_micros += bucket.total_cost_micros;
        agg.min_duration_us = agg.min_duration_us.min(bucket.min_duration_us);
        agg.max_duration_us = agg.max_duration_us.max(bucket.max_duration_us);
        agg.unique_sessions += bucket.unique_sessions as u64;
        agg.unique_agents += bucket.unique_agents as u64;
    }

    if agg.total_requests > 0 {
        agg.avg_duration_us = agg.total_duration_us / agg.total_requests;
        agg.error_rate = (agg.total_errors as f64 / agg.total_requests as f64) * 100.0;
    }

    agg
}

/// Aggregated metrics from query
#[derive(Debug, Clone, Default)]
pub struct AggregatedMetrics {
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_tokens: u64,
    pub total_duration_us: u64,
    pub avg_duration_us: u64,
    pub min_duration_us: u64,
    pub max_duration_us: u64,
    pub total_cost_micros: u64,
    pub error_rate: f64,
    pub unique_sessions: u64,
    pub unique_agents: u64,
}

impl AggregatedMetrics {
    pub fn avg_duration_ms(&self) -> f64 {
        self.avg_duration_us as f64 / 1000.0
    }

    pub fn total_cost(&self) -> f64 {
        self.total_cost_micros as f64 / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_rejection() {
        let bucket = AnalyticsBucket::new(1000000, 1);

        // Add some edges
        let edge = AgentFlowEdge {
            edge_id: 1,
            timestamp_us: 1000000,
            duration_us: 5000,
            token_count: 100,
            project_id: 1,
            session_id: 42,
            agent_id: 100,
            ..Default::default()
        };

        bucket.add_edge(&edge, 0, false);

        // Query for matching project - should NOT be rejected
        let filters = QueryFilters::new().with_project(1);
        assert!(bucket.may_match(&filters));

        // Query for non-matching project - SHOULD be rejected
        let filters = QueryFilters::new().with_project(999);
        assert!(!bucket.may_match(&filters));

        // Query for matching session
        let filters = QueryFilters::new().with_session(42);
        assert!(bucket.may_match(&filters));

        // Query for non-matching session
        let filters = QueryFilters::new().with_session(999);
        assert!(!bucket.may_match(&filters));
    }

    #[test]
    fn test_error_filter() {
        let bucket = AnalyticsBucket::new(1000000, 1);

        // Add normal edge
        let edge = AgentFlowEdge {
            edge_id: 1,
            project_id: 1,
            session_id: 1,
            agent_id: 1,
            ..Default::default()
        };
        bucket.add_edge(&edge, 0, false);

        // Query for errors - should be rejected (no errors added)
        let filters = QueryFilters::new().with_error_filter(true);
        assert!(!bucket.may_match(&filters));

        // Add error edge
        bucket.add_edge(&edge, 0, true);

        // Now should match
        assert!(bucket.may_match(&filters));
    }

    #[test]
    fn test_query_execution() {
        let mut buckets = Vec::new();

        // Create buckets with different projects
        for project_id in 1..=5 {
            let bucket = AnalyticsBucket::new(project_id as u64 * 1000000, project_id);
            let edge = AgentFlowEdge {
                edge_id: project_id as u128,
                timestamp_us: project_id as u64 * 1000000,
                duration_us: 5000,
                token_count: 100,
                project_id,
                session_id: 1,
                agent_id: 1,
                ..Default::default()
            };
            bucket.add_edge(&edge, 1000, false);
            buckets.push(bucket);
        }

        // Query for project 3
        let filters = QueryFilters::new().with_project(3);
        let (results, stats) = execute_analytics_query(&buckets, &filters);

        // Should have 1 result, 4 skipped
        assert_eq!(results.len(), 1);
        assert_eq!(stats.buckets_scanned, 1);
        assert_eq!(stats.bloom_rejections, 4);
        assert_eq!(stats.buckets_skipped, 4);

        // Verify efficiency
        assert!(stats.bloom_efficiency() >= 80.0);
    }

    #[test]
    fn test_empty_filters_match_all() {
        let bucket = AnalyticsBucket::new(1000000, 1);
        let edge = AgentFlowEdge {
            edge_id: 1,
            project_id: 1,
            ..Default::default()
        };
        bucket.add_edge(&edge, 0, false);

        // Empty filters should match everything
        let filters = QueryFilters::new();
        assert!(bucket.may_match(&filters));
    }
}
