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

//! Agentreplay Storage Layer
//!
//! SochDB-based storage engine providing ACID-compliant storage for trace data.
//!
//! ## Architecture
//!
//! This storage layer uses SochDB as the backing store, providing:
//!
//! - **ACID Transactions**: Full durability and consistency guarantees
//! - **MVCC**: Multi-version concurrency control for isolation
//! - **Range Scans**: Efficient temporal and key-based queries
//! - **Metrics Aggregation**: Pre-aggregated metrics with configurable granularity
//! - **Payload Storage**: Binary payload storage with compression
//!
//! ## Usage
//!
//! ```rust,ignore
//! use agentreplay_storage::{AgentReplayStorage, AgentReplayStorageConfig};
//! use sochdb::InMemoryConnection;
//!
//! let conn = InMemoryConnection::new();
//! let config = AgentReplayStorageConfig::default();
//! let storage = AgentReplayStorage::new(conn, config);
//! ```

// Core SochDB-based storage module
pub mod sochdb_unified;

// Auxiliary modules that don't depend on LSM
pub mod aff;
pub mod analytics_bucket;
pub mod backend;
pub mod bloom;
pub mod compression;
pub mod eval_store;
pub mod event_store;
pub mod metrics_agg;
pub mod memory_agent_store;
pub mod observation_store;
pub mod pending_queue;
pub mod response_git;
pub mod sharded_metrics;
pub mod sketches;

// Re-export core types from sochdb_unified
pub use sochdb_unified::{
    AgentReplayStorage, AgentReplayStorageConfig, MetricsBucket, StorageStats, SyncMode,
    CacheStats, LevelStats, HealthCheckResult, CleanupStats,
    decode_trace_key, deserialize_edge, encode_metrics_key, encode_payload_key,
    encode_trace_key, serialize_edge,
};

// Re-export auxiliary module types
pub use aff::{AFFHeader, AFFReader, AFFWriter, AFF_MAGIC, AFF_VERSION};
pub use analytics_bucket::{
    AggregatedMetrics, AnalyticsBucket, BloomFilterStats, QueryExecutionStats, QueryFilters,
};
pub use backend::{LocalFsBackend, ObjectMetadata, StorageBackend};
pub use compression::{CompressionEngine, CompressionStats, StorageTier};
pub use eval_store::{EvalAggregateStats, EvalMetricEntry, EvalStore, EvalSummary};
pub use event_store::EventStore;
pub use metrics_agg::{BucketKey, BucketStats, MetricsAggregator, MetricsSummary};
pub use memory_agent_store::{MemoryAgentStoreError, PersistentMemoryStore, SessionDeleteStats};
pub use response_git::{
    Author, Blob, Branch, Commit, CommitDiff, ContentType, DiffConfig, DiffEngine, DiffHunk,
    DiffLine, DiffStats, EntryMode, Experiment, ExperimentVariant, GitObject, LineChange, LogEntry,
    ObjectId, ObjectStore, ObjectType, Ref, RefError, RefStore, RepositoryError,
    ResponseRepository, ResponseSnapshot, StoreError, StoreStats, Tag, TokenUsage, Tree, TreeDiff,
    TreeEntry,
};
pub use sketches::{AdaptiveSketch, CountMinSketch, DDSketch, ExponentialHistogram, HyperLogLog};

// Observation and queue storage for memory agent
pub use observation_store::{ObservationKey, ObservationQuery, ObservationStore, ObservationStoreError, StoredObservation};
pub use pending_queue::{ClaimResult, PendingMessage, PendingMessageQueue, PendingQueueConfig, QueueError};

// Compatibility aliases for migration from old storage layer
// UnifiedStorage is now AgentReplayStorage
pub type UnifiedStorage = AgentReplayStorage;
pub type UnifiedStorageConfig = AgentReplayStorageConfig;

// VersionStore is now ResponseRepository
pub type VersionStore = ResponseRepository;

// Metrics bucket snapshot compatibility
pub use sharded_metrics::{
    AtomicMetricsBucket, MergeableMetricsBucket, MetricsBucketSnapshot, ProjectSummary,
    SessionSummary, ShardedBucketKey, ShardedMetricsAggregator, ShardedMetricsSummary,
};

/// Storage backend type selector (simplified to SochDB only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageBackendType {
    #[default]
    SochDB,
}

/// Compatibility alias for LSM stats (now returns SochDB stats)
pub type LSMStats = StorageStats;

/// Compatibility alias for MVCC version stats
#[derive(Debug, Clone, Default)]
pub struct VersionSetStatsSnapshot {
    pub current_version: u64,
    pub total_versions: u64,
    pub active_readers: u64,
}

/// Backup manager (stub for compatibility)
pub struct BackupManager {
    _backup_dir: std::path::PathBuf,
}

impl BackupManager {
    pub fn new<P: AsRef<std::path::Path>>(backup_dir: P) -> Self {
        BackupManager { _backup_dir: backup_dir.as_ref().to_path_buf() }
    }

    pub fn create_backup<P: AsRef<std::path::Path>>(&self, _destination: P) -> std::io::Result<BackupMetadata> {
        Ok(BackupMetadata::default())
    }

    pub fn list_backups<P: AsRef<std::path::Path>>(_location: P) -> std::io::Result<Vec<BackupMetadata>> {
        Ok(Vec::new())
    }

    pub fn restore_backup<P: AsRef<std::path::Path>>(&self, _backup_path: P) -> std::io::Result<()> {
        Ok(())
    }

    pub fn delete_backup(&self, _backup_id: &str) -> std::io::Result<()> {
        Ok(())
    }

    pub fn verify_backup<P: AsRef<std::path::Path>>(_backup_path: P) -> std::io::Result<bool> {
        Ok(true)
    }
}

/// Backup metadata (stub for compatibility)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BackupMetadata {
    pub id: String,
    pub timestamp: u64,
    pub timestamp_us: u64,
    pub created_at: String,
    pub size_bytes: u64,
    pub edge_count: u64,
    pub file_count: usize,
    pub database_version: String,
}

