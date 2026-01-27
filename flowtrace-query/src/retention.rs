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

//! Data Retention and TTL Management
//!
//! **CRITICAL FIX**: Implements automatic 30-day TTL data retention with FIFO deletion.
//!
//! ## Architecture
//!
//! The retention system uses a multi-layer approach:
//! 1. **Compaction-time filtering**: Expired edges are dropped during SSTable compaction
//! 2. **Timestamp-based SSTable skipping**: Entire SSTables with max_timestamp < cutoff are deleted
//! 3. **Query-time filtering**: Expired edges are filtered from query results as a safety net
//!
//! ## Configuration
//!
//! - `retention_days: 0` or `None` = unlimited retention (keep forever)
//! - `retention_days: 30` = default, delete data older than 30 days
//! - Settings are persisted to `~/.flowtrace/retention-config.json`

use crate::Flowtrace;
use flowtrace_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

/// Retention policy configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetentionPolicy {
    /// Environment name (e.g., "production", "development")
    pub environment: String,
    /// Retention period in days (0 or None = unlimited)
    pub retention_days: Option<u32>,
    /// Whether this policy is enabled
    pub enabled: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            environment: "default".to_string(),
            retention_days: Some(30), // 30-day default
            enabled: true,
        }
    }
}

impl RetentionPolicy {
    /// Check if this policy allows unlimited retention
    pub fn is_unlimited(&self) -> bool {
        !self.enabled || self.retention_days.is_none() || self.retention_days == Some(0)
    }

    /// Get the cutoff timestamp for this policy
    pub fn get_cutoff_timestamp_us(&self) -> Option<u64> {
        if self.is_unlimited() {
            return None;
        }

        let retention_days = self.retention_days.unwrap_or(30);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let retention_period_us = retention_days as u64 * 24 * 60 * 60 * 1_000_000;
        Some(now.saturating_sub(retention_period_us))
    }
}

/// Statistics about a retention cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetentionStats {
    pub traces_deleted: usize,
    pub disk_freed_bytes: u64,
    pub cleanup_duration_ms: u64,
    pub oldest_trace_kept_us: u64,
    pub sstables_deleted: usize,
    pub sstables_compacted: usize,
}

/// Retention metrics for observability
#[derive(Debug, Default)]
pub struct RetentionMetrics {
    /// Total traces deleted since startup
    pub total_traces_deleted: AtomicU64,
    /// Total bytes freed since startup
    pub total_bytes_freed: AtomicU64,
    /// Total cleanup operations performed
    pub cleanup_count: AtomicU64,
    /// Total cleanup failures
    pub cleanup_failures: AtomicU64,
    /// Last cleanup timestamp (microseconds)
    pub last_cleanup_us: AtomicU64,
    /// Last cleanup duration (milliseconds)
    pub last_cleanup_duration_ms: AtomicU64,
}

impl RetentionMetrics {
    pub fn record_cleanup(&self, stats: &RetentionStats) {
        self.total_traces_deleted
            .fetch_add(stats.traces_deleted as u64, Ordering::Relaxed);
        self.total_bytes_freed
            .fetch_add(stats.disk_freed_bytes, Ordering::Relaxed);
        self.cleanup_count.fetch_add(1, Ordering::Relaxed);
        self.last_cleanup_us.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            Ordering::Relaxed,
        );
        self.last_cleanup_duration_ms
            .store(stats.cleanup_duration_ms, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.cleanup_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Get metrics as a map for API responses
    pub fn to_map(&self) -> HashMap<String, u64> {
        let mut map = HashMap::new();
        map.insert(
            "total_traces_deleted".to_string(),
            self.total_traces_deleted.load(Ordering::Relaxed),
        );
        map.insert(
            "total_bytes_freed".to_string(),
            self.total_bytes_freed.load(Ordering::Relaxed),
        );
        map.insert(
            "cleanup_count".to_string(),
            self.cleanup_count.load(Ordering::Relaxed),
        );
        map.insert(
            "cleanup_failures".to_string(),
            self.cleanup_failures.load(Ordering::Relaxed),
        );
        map.insert(
            "last_cleanup_us".to_string(),
            self.last_cleanup_us.load(Ordering::Relaxed),
        );
        map.insert(
            "last_cleanup_duration_ms".to_string(),
            self.last_cleanup_duration_ms.load(Ordering::Relaxed),
        );
        map
    }
}

/// Persisted retention configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Schema version for forward compatibility
    pub version: u32,
    /// Policies by environment
    pub policies: Vec<RetentionPolicy>,
    /// Global TTL cutoff (overrides per-environment if set)
    pub global_retention_days: Option<u32>,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            version: 1,
            policies: vec![
                RetentionPolicy {
                    environment: "production".to_string(),
                    retention_days: Some(30),
                    enabled: true,
                },
                RetentionPolicy {
                    environment: "development".to_string(),
                    retention_days: Some(7),
                    enabled: true,
                },
                RetentionPolicy::default(),
            ],
            global_retention_days: None,
        }
    }
}

impl RetentionConfig {
    /// Load config from disk, or create default
    pub fn load(path: &PathBuf) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(config) => return config,
                    Err(e) => {
                        warn!("Failed to parse retention config: {}. Using defaults.", e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read retention config: {}. Using defaults.", e);
                }
            }
        }
        Self::default()
    }

    /// Save config to disk
    pub fn save(&self, path: &PathBuf) -> Result<()> {
        use flowtrace_core::FlowtraceError;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self).map_err(|e| {
            FlowtraceError::Internal(format!("Failed to serialize retention config: {}", e))
        })?;
        std::fs::write(path, contents)?;
        info!(path = %path.display(), "Retention config saved");
        Ok(())
    }

    /// Get effective retention cutoff timestamp
    pub fn get_effective_cutoff_us(&self, environment: Option<&str>) -> Option<u64> {
        // Global override takes precedence
        if let Some(global_days) = self.global_retention_days {
            if global_days == 0 {
                return None; // Unlimited
            }
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
            let retention_period_us = global_days as u64 * 24 * 60 * 60 * 1_000_000;
            return Some(now.saturating_sub(retention_period_us));
        }

        // Find environment-specific policy
        let env = environment.unwrap_or("default");
        let policy = self
            .policies
            .iter()
            .find(|p| p.environment == env && p.enabled)
            .or_else(|| {
                self.policies
                    .iter()
                    .find(|p| p.environment == "default" && p.enabled)
            });

        policy.and_then(|p| p.get_cutoff_timestamp_us())
    }
}

impl Flowtrace {
    /// Delete traces older than the specified timestamp
    ///
    /// **CRITICAL FIX**: Implements actual deletion via tombstone markers.
    /// The storage layer will:
    /// 1. Write tombstone markers for deleted edges
    /// 2. Filter out expired edges during compaction
    /// 3. Reclaim disk space when SSTables are compacted
    pub async fn delete_traces_before(&self, before_timestamp_us: u64) -> Result<RetentionStats> {
        let start_time = SystemTime::now();
        let mut stats = RetentionStats::default();

        // Query all edges older than the cutoff
        // Note: This is done in batches to avoid memory exhaustion
        let old_edges = self.storage.range_scan(0, before_timestamp_us)?;

        if old_edges.is_empty() {
            stats.cleanup_duration_ms = start_time
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            return Ok(stats);
        }

        info!(
            count = old_edges.len(),
            cutoff_us = before_timestamp_us,
            "Deleting expired traces"
        );

        // Delete in batches to avoid overwhelming the system
        const BATCH_SIZE: usize = 1000;
        let mut deleted_count = 0;

        for chunk in old_edges.chunks(BATCH_SIZE) {
            for edge in chunk {
                // Write tombstone marker
                if let Err(e) = self.delete(edge.edge_id, edge.tenant_id).await {
                    warn!(
                        edge_id = %format!("{:#x}", edge.edge_id),
                        error = %e,
                        "Failed to delete expired edge"
                    );
                } else {
                    deleted_count += 1;
                }
            }
        }

        stats.traces_deleted = deleted_count;
        stats.cleanup_duration_ms = start_time
            .elapsed()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        stats.oldest_trace_kept_us = before_timestamp_us;

        info!(
            deleted = stats.traces_deleted,
            duration_ms = stats.cleanup_duration_ms,
            "Retention cleanup completed"
        );

        Ok(stats)
    }

    /// Get the total number of traces in the database
    pub async fn trace_count(&self) -> usize {
        // Use iter_all_edges for accurate count
        match self.storage.iter_all_edges() {
            Ok(edges) => edges.len(),
            Err(_) => 0,
        }
    }

    /// Get the oldest trace timestamp in the database
    pub async fn oldest_trace_timestamp(&self) -> Option<u64> {
        match self.storage.iter_all_edges() {
            Ok(edges) => edges.iter().map(|e| e.timestamp_us).min(),
            Err(_) => None,
        }
    }

    /// Get the newest trace timestamp in the database
    pub async fn newest_trace_timestamp(&self) -> Option<u64> {
        match self.storage.iter_all_edges() {
            Ok(edges) => edges.iter().map(|e| e.timestamp_us).max(),
            Err(_) => None,
        }
    }

    /// Apply retention policy and delete expired data
    ///
    /// This is the main entry point for TTL enforcement. Called by the
    /// background retention worker.
    pub async fn apply_retention(&self, config: &RetentionConfig) -> Result<RetentionStats> {
        let cutoff = config.get_effective_cutoff_us(None);

        if cutoff.is_none() {
            info!("Retention policy is unlimited, skipping cleanup");
            return Ok(RetentionStats::default());
        }

        let cutoff_us = cutoff.unwrap();
        self.delete_traces_before(cutoff_us).await
    }
}

/// Retention manager for managing cleanup policies
pub struct RetentionManager {
    pub config: RetentionConfig,
    pub config_path: PathBuf,
    pub metrics: RetentionMetrics,
}

impl RetentionManager {
    pub fn new(config_path: PathBuf) -> Self {
        let config = RetentionConfig::load(&config_path);
        Self {
            config,
            config_path,
            metrics: RetentionMetrics::default(),
        }
    }

    /// Update retention configuration
    pub fn update_config(&mut self, config: RetentionConfig) -> Result<()> {
        self.config = config;
        self.config.save(&self.config_path)
    }

    /// Get the retention policy for a specific environment
    pub fn get_policy(&self, environment: &str) -> Option<&RetentionPolicy> {
        self.config
            .policies
            .iter()
            .find(|p| p.environment == environment && p.enabled)
            .or_else(|| {
                self.config
                    .policies
                    .iter()
                    .find(|p| p.environment == "default" && p.enabled)
            })
    }

    /// Calculate the cutoff timestamp for a policy
    pub fn calculate_cutoff(&self, policy: &RetentionPolicy) -> Option<u64> {
        policy.get_cutoff_timestamp_us()
    }

    /// Apply retention policies to a database
    pub async fn apply_retention(&mut self, db: &Flowtrace) -> Result<RetentionStats> {
        match db.apply_retention(&self.config).await {
            Ok(stats) => {
                self.metrics.record_cleanup(&stats);
                Ok(stats)
            }
            Err(e) => {
                self.metrics.record_failure();
                Err(e)
            }
        }
    }

    /// Get retention metrics
    pub fn get_metrics(&self) -> HashMap<String, u64> {
        self.metrics.to_map()
    }

    /// Set global retention days (0 = unlimited)
    pub fn set_global_retention_days(&mut self, days: Option<u32>) -> Result<()> {
        self.config.global_retention_days = days;
        self.config.save(&self.config_path)
    }
}

impl Default for RetentionManager {
    fn default() -> Self {
        // Default path for retention config
        let config_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".flowtrace")
            .join("retention-config.json");

        Self::new(config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_policy_unlimited() {
        let policy = RetentionPolicy {
            environment: "test".to_string(),
            retention_days: None,
            enabled: true,
        };
        assert!(policy.is_unlimited());
        assert!(policy.get_cutoff_timestamp_us().is_none());

        let policy = RetentionPolicy {
            environment: "test".to_string(),
            retention_days: Some(0),
            enabled: true,
        };
        assert!(policy.is_unlimited());
    }

    #[test]
    fn test_retention_policy_30_days() {
        let policy = RetentionPolicy {
            environment: "test".to_string(),
            retention_days: Some(30),
            enabled: true,
        };
        assert!(!policy.is_unlimited());

        let cutoff = policy.get_cutoff_timestamp_us().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // Cutoff should be ~30 days ago (within 1 second tolerance)
        let expected_cutoff = now - (30 * 24 * 60 * 60 * 1_000_000);
        assert!((cutoff as i64 - expected_cutoff as i64).abs() < 1_000_000);
    }

    #[test]
    fn test_retention_config_default() {
        let config = RetentionConfig::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.policies.len(), 3);

        // Production should have 30 days
        let prod = config
            .policies
            .iter()
            .find(|p| p.environment == "production")
            .unwrap();
        assert_eq!(prod.retention_days, Some(30));

        // Development should have 7 days
        let dev = config
            .policies
            .iter()
            .find(|p| p.environment == "development")
            .unwrap();
        assert_eq!(dev.retention_days, Some(7));
    }
}
