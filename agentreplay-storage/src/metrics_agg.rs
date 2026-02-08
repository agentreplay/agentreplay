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

//! Pre-aggregated metrics storage for O(1) analytics queries
//!
//! Instead of scanning all edges for every analytics query, we maintain
//! pre-aggregated buckets that are updated incrementally on each insert.
//!
//! **Performance Impact:**
//! - Before: O(N) full scan for each analytics query (100k traces = ~10 seconds)
//! - After: O(buckets) where buckets = time_range / bucket_size (typically < 1000)

use agentreplay_core::AgentFlowEdge;
use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Time bucket size in microseconds (1 minute default)
pub const DEFAULT_BUCKET_SIZE_US: u64 = 60 * 1_000_000;

/// Type alias for bucket key (timestamp, project_id)
pub type BucketKey = (u64, u16);

/// Type alias for summary statistics (used by LSMTree query interface)
pub type BucketStats = MetricsSummary;

/// Pre-aggregated metrics for a time bucket
#[derive(Debug, Clone, Default)]
pub struct MetricsBucket {
    /// Bucket start timestamp (aligned to bucket_size)
    pub timestamp_us: u64,
    /// Project ID for this bucket
    pub project_id: u16,

    // Counters
    pub request_count: u64,
    pub error_count: u64,
    pub total_tokens: u64,

    // Latency stats (for computing avg, we track sum and count)
    pub total_duration_us: u64,
    pub min_duration_us: u64,
    pub max_duration_us: u64,

    // Cost tracking (in microdollars for precision)
    pub total_cost_micros: u64,

    // For session/agent tracking
    pub unique_sessions: u32,
    pub unique_agents: u32,
}

impl MetricsBucket {
    pub fn new(timestamp_us: u64, project_id: u16) -> Self {
        Self {
            timestamp_us,
            project_id,
            min_duration_us: u64::MAX,
            ..Default::default()
        }
    }

    /// Update bucket with a new edge
    pub fn add_edge(&mut self, edge: &AgentFlowEdge, cost_micros: u64, is_error: bool) {
        self.request_count += 1;
        if is_error {
            self.error_count += 1;
        }
        self.total_tokens += edge.token_count as u64;
        self.total_duration_us += edge.duration_us as u64;
        self.min_duration_us = self.min_duration_us.min(edge.duration_us as u64);
        self.max_duration_us = self.max_duration_us.max(edge.duration_us as u64);
        self.total_cost_micros += cost_micros;
    }

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

/// Pre-aggregated metrics store with multiple granularities
pub struct MetricsAggregator {
    /// 1-minute buckets (for last 24 hours of fine-grained data)
    minute_buckets: Arc<RwLock<BTreeMap<BucketKey, MetricsBucket>>>,

    /// 1-hour buckets (for last 30 days)
    hour_buckets: Arc<RwLock<BTreeMap<BucketKey, MetricsBucket>>>,

    /// 1-day buckets (for long-term trends)
    day_buckets: Arc<RwLock<BTreeMap<BucketKey, MetricsBucket>>>,

    /// Secondary index: session_id -> list of edge_ids
    session_index: Arc<RwLock<BTreeMap<u64, Vec<u128>>>>,

    /// Secondary index: project_id -> list of edge_ids (for efficient project filtering)
    project_index: Arc<RwLock<BTreeMap<u16, Vec<u128>>>>,
}

impl MetricsAggregator {
    pub fn new() -> Self {
        Self {
            minute_buckets: Arc::new(RwLock::new(BTreeMap::new())),
            hour_buckets: Arc::new(RwLock::new(BTreeMap::new())),
            day_buckets: Arc::new(RwLock::new(BTreeMap::new())),
            session_index: Arc::new(RwLock::new(BTreeMap::new())),
            project_index: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Load metrics from disk or create empty aggregator
    pub fn load<P: AsRef<std::path::Path>>(_data_dir: P) -> agentreplay_core::Result<Self> {
        // TODO: Implement persistent loading from metrics file
        // For now, start fresh - metrics will be rebuilt from edges if needed
        Ok(Self::new())
    }

    /// Persist metrics to disk
    pub fn flush<P: AsRef<std::path::Path>>(&self, _data_dir: P) -> agentreplay_core::Result<()> {
        // TODO: Implement persistence
        // For now, metrics are in-memory only
        Ok(())
    }

    /// Simple record method that extracts cost/error from edge attributes
    pub fn record(&mut self, edge: &AgentFlowEdge) {
        // Extract cost from edge - for now use 0 until we properly track costs
        let cost_micros = 0u64;

        // For now, assume no errors - error tracking will need status field
        // TODO: Add error detection based on span status attribute
        let is_error = false;

        self.record_edge(edge, cost_micros, is_error);
    }

    /// Query method for compatibility with LSMTree
    pub fn query(&self, project_id: u64, start_ts: u64, end_ts: u64) -> BucketStats {
        // Convert to internal project_id type
        let pid = (project_id & 0xFFFF) as u16;
        self.get_summary(start_ts, end_ts, Some(pid))
    }

    /// Query timeseries buckets for rendering charts
    pub fn query_timeseries(
        &self,
        project_id: u64,
        start_ts: u64,
        end_ts: u64,
    ) -> Vec<((u64, u16), MetricsBucket)> {
        let pid = (project_id & 0xFFFF) as u16;
        let buckets = self.query_metrics(start_ts, end_ts, Some(pid));
        buckets
            .into_iter()
            .map(|b| ((b.timestamp_us, b.project_id), b))
            .collect()
    }

    /// Align timestamp to bucket boundary
    fn align_to_bucket(timestamp_us: u64, bucket_size_us: u64) -> u64 {
        (timestamp_us / bucket_size_us) * bucket_size_us
    }

    /// Record a new edge in all aggregation levels
    pub fn record_edge(&self, edge: &AgentFlowEdge, cost_micros: u64, is_error: bool) {
        let minute_size = 60 * 1_000_000u64;
        let hour_size = 60 * minute_size;
        let day_size = 24 * hour_size;

        let minute_ts = Self::align_to_bucket(edge.timestamp_us, minute_size);
        let hour_ts = Self::align_to_bucket(edge.timestamp_us, hour_size);
        let day_ts = Self::align_to_bucket(edge.timestamp_us, day_size);

        // Update minute bucket
        {
            let mut buckets = self.minute_buckets.write();
            let bucket = buckets
                .entry((minute_ts, edge.project_id))
                .or_insert_with(|| MetricsBucket::new(minute_ts, edge.project_id));
            bucket.add_edge(edge, cost_micros, is_error);
        }

        // Update hour bucket
        {
            let mut buckets = self.hour_buckets.write();
            let bucket = buckets
                .entry((hour_ts, edge.project_id))
                .or_insert_with(|| MetricsBucket::new(hour_ts, edge.project_id));
            bucket.add_edge(edge, cost_micros, is_error);
        }

        // Update day bucket
        {
            let mut buckets = self.day_buckets.write();
            let bucket = buckets
                .entry((day_ts, edge.project_id))
                .or_insert_with(|| MetricsBucket::new(day_ts, edge.project_id));
            bucket.add_edge(edge, cost_micros, is_error);
        }

        // Update session index
        {
            let mut index = self.session_index.write();
            index
                .entry(edge.session_id)
                .or_insert_with(Vec::new)
                .push(edge.edge_id);
        }

        // Update project index
        {
            let mut index = self.project_index.write();
            index
                .entry(edge.project_id)
                .or_insert_with(Vec::new)
                .push(edge.edge_id);
        }
    }

    /// Query aggregated metrics for a time range and project
    /// Returns buckets at the appropriate granularity based on range size
    pub fn query_metrics(
        &self,
        start_us: u64,
        end_us: u64,
        project_id: Option<u16>,
    ) -> Vec<MetricsBucket> {
        let range_us = end_us.saturating_sub(start_us);
        let minute_size = 60 * 1_000_000u64;
        let hour_size = 60 * minute_size;
        let day_size = 24 * hour_size;

        // Choose granularity based on range
        let (buckets, bucket_size) = if range_us <= 6 * hour_size {
            // Up to 6 hours: use minute buckets
            (self.minute_buckets.read(), minute_size)
        } else if range_us <= 7 * day_size {
            // Up to 7 days: use hour buckets
            (self.hour_buckets.read(), hour_size)
        } else {
            // Longer: use day buckets
            (self.day_buckets.read(), day_size)
        };

        let start_aligned = Self::align_to_bucket(start_us, bucket_size);
        let end_aligned = Self::align_to_bucket(end_us, bucket_size) + bucket_size;

        buckets
            .range((start_aligned, 0)..=(end_aligned, u16::MAX))
            .filter(|((ts, pid), _)| {
                *ts >= start_aligned && *ts < end_aligned && project_id.map_or(true, |p| *pid == p)
            })
            .map(|(_, bucket)| bucket.clone())
            .collect()
    }

    /// Get summary statistics for a time range
    pub fn get_summary(
        &self,
        start_us: u64,
        end_us: u64,
        project_id: Option<u16>,
    ) -> MetricsSummary {
        let buckets = self.query_metrics(start_us, end_us, project_id);

        let mut summary = MetricsSummary::default();
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

    /// Get edge IDs for a session (O(1) lookup instead of O(N) scan)
    pub fn get_session_edges(&self, session_id: u64) -> Vec<u128> {
        self.session_index
            .read()
            .get(&session_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get edge IDs for a project (O(1) lookup instead of O(N) scan)
    pub fn get_project_edges(&self, project_id: u16) -> Vec<u128> {
        self.project_index
            .read()
            .get(&project_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get count of edges for a project (O(1))
    pub fn get_project_edge_count(&self, project_id: u16) -> usize {
        self.project_index
            .read()
            .get(&project_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Prune old buckets to limit memory usage
    /// Call periodically (e.g., hourly)
    pub fn prune_old_buckets(&self, max_minute_age_us: u64, max_hour_age_us: u64) {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let minute_cutoff = now_us.saturating_sub(max_minute_age_us);
        let hour_cutoff = now_us.saturating_sub(max_hour_age_us);

        // Prune minute buckets
        {
            let mut buckets = self.minute_buckets.write();
            buckets.retain(|(ts, _), _| *ts >= minute_cutoff);
        }

        // Prune hour buckets
        {
            let mut buckets = self.hour_buckets.write();
            buckets.retain(|(ts, _), _| *ts >= hour_cutoff);
        }

        // Day buckets are kept indefinitely for long-term trends
    }

    /// Get total bucket counts for monitoring
    pub fn bucket_counts(&self) -> (usize, usize, usize) {
        (
            self.minute_buckets.read().len(),
            self.hour_buckets.read().len(),
            self.day_buckets.read().len(),
        )
    }
}

impl Default for MetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for a time range
#[derive(Debug, Default, Clone)]
pub struct MetricsSummary {
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_tokens: u64,
    pub total_duration_us: u64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
    pub total_cost_micros: u64,
}

impl MetricsSummary {
    pub fn total_cost(&self) -> f64 {
        self.total_cost_micros as f64 / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_alignment() {
        let minute_size = 60 * 1_000_000u64;

        // Timestamp at 1:30:45 should align to 1:30:00
        let ts = 90 * 1_000_000 + 45 * 1_000_000; // 90 seconds + 45 seconds = 135 seconds
        let aligned = MetricsAggregator::align_to_bucket(ts, minute_size);
        assert_eq!(aligned, 2 * minute_size); // 2 minutes
    }

    #[test]
    fn test_record_and_query() {
        let agg = MetricsAggregator::new();

        let edge = AgentFlowEdge {
            edge_id: 1,
            timestamp_us: 100 * 1_000_000, // 100 seconds
            duration_us: 500_000,          // 500ms
            token_count: 100,
            project_id: 1,
            session_id: 42,
            ..Default::default()
        };

        agg.record_edge(&edge, 1000, false); // 0.001 dollars

        let buckets = agg.query_metrics(0, 200 * 1_000_000, Some(1));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].request_count, 1);
        assert_eq!(buckets[0].total_tokens, 100);

        // Test session index
        let session_edges = agg.get_session_edges(42);
        assert_eq!(session_edges, vec![1]);
    }
}
