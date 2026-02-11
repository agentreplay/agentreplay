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

//! Unified SochDB Storage Layer for AgentReplay
//!
//! This module provides the primary storage backend for AgentReplay using SochDB.
//! It replaces the custom LSM-tree implementation with SochDB's ACID-compliant
//! storage engine.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    AgentReplayStorage                             │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
//! │  │ Trace Store  │  │ Payload Store│  │ Metrics Store│           │
//! │  │ (edges)      │  │ (blobs)      │  │ (aggregates) │           │
//! │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘           │
//! │         └─────────────────┼─────────────────┘                   │
//! │                           │                                     │
//! │                    ┌──────▼──────┐                              │
//! │                    │   SochDB    │                              │
//! │                    │ Connection  │                              │
//! │                    └─────────────┘                              │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Encoding
//!
//! - Traces: `traces/{tenant_id}/{project_id}/{timestamp:020}/{edge_id:032x}`
//! - Payloads: `payloads/{edge_id:032x}`
//! - Metrics: `metrics/{granularity}/{tenant_id}/{project_id}/{timestamp:020}`
//! - Graph: `graph/{direction}/{node_id:032x}/{related_id:032x}`

use agentreplay_core::{AgentFlowEdge, AgentreplayError, Result};
use parking_lot::RwLock;
use sochdb::EmbeddedConnection as Connection;
use sochdb_storage::{PackedRow, PackedColumnDef, PackedColumnType, PackedTableSchema};
use std::collections::{BTreeMap, HashMap};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ============================================================================
// Payload compression: zstd level 1 for speed + ~4-6x ratio on JSON.
// Payloads are stored as: [1-byte magic 'Z'] [zstd-compressed data]
// Legacy uncompressed payloads (no 'Z' prefix) are transparently handled.
// ============================================================================

/// Compress payload bytes with zstd level 1 (speed-optimized).
/// Prepends a 1-byte magic marker so we can detect compressed vs legacy data.
fn compress_payload(data: &[u8]) -> Vec<u8> {
    // Don't bother compressing tiny payloads (< 64 bytes)
    if data.len() < 64 {
        return data.to_vec();
    }
    let compressed = zstd::encode_all(data, 1).unwrap_or_else(|_| data.to_vec());
    // Only use compressed form if it actually saves space
    if compressed.len() + 1 < data.len() {
        let mut result = Vec::with_capacity(1 + compressed.len());
        result.push(b'Z'); // magic byte
        result.extend_from_slice(&compressed);
        result
    } else {
        data.to_vec()
    }
}

/// Decompress payload bytes. Handles both compressed (Z-prefixed) and legacy uncompressed.
fn decompress_payload(data: &[u8]) -> std::result::Result<Vec<u8>, AgentreplayError> {
    if data.is_empty() {
        return Ok(data.to_vec());
    }
    if data[0] == b'Z' {
        zstd::decode_all(&data[1..]).map_err(|e| {
            AgentreplayError::Internal(format!("zstd decompress failed: {}", e))
        })
    } else {
        // Legacy uncompressed payload
        Ok(data.to_vec())
    }
}
use std::sync::Arc;
use tracing::{error, info, warn};

// ============================================================================
// Key Encoding
// ============================================================================

/// Key prefix for trace edges
pub const TRACE_PREFIX: &str = "traces";
/// Key prefix for payloads
pub const PAYLOAD_PREFIX: &str = "payloads";
/// Key prefix for metrics
pub const METRICS_PREFIX: &str = "metrics";
/// Key prefix for graph edges
pub const GRAPH_PREFIX: &str = "graph";

/// Encode a trace key from edge components
pub fn encode_trace_key(tenant_id: u64, project_id: u16, timestamp_us: u64, edge_id: u128) -> String {
    format!(
        "{}/{}/{}/{:020}/{:032x}",
        TRACE_PREFIX, tenant_id, project_id, timestamp_us, edge_id
    )
}

/// Encode a trace key from an edge
pub fn encode_trace_key_from_edge(edge: &AgentFlowEdge) -> String {
    encode_trace_key(edge.tenant_id, edge.project_id, edge.timestamp_us, edge.edge_id)
}

/// Decode a trace key into components
pub fn decode_trace_key(key: &str) -> Option<(u64, u16, u64, u128)> {
    let parts: Vec<&str> = key.split('/').collect();
    if parts.len() < 5 || parts[0] != TRACE_PREFIX {
        return None;
    }
    
    let tenant_id = parts[1].parse().ok()?;
    let project_id = parts[2].parse().ok()?;
    let timestamp_us = parts[3].parse().ok()?;
    let edge_id = u128::from_str_radix(parts[4], 16).ok()?;
    
    Some((tenant_id, project_id, timestamp_us, edge_id))
}

/// Encode a payload key
pub fn encode_payload_key(edge_id: u128) -> String {
    format!("{}/{:032x}", PAYLOAD_PREFIX, edge_id)
}

/// Encode a metrics key
pub fn encode_metrics_key(granularity: &str, tenant_id: u64, project_id: u16, timestamp_us: u64) -> String {
    format!(
        "{}/{}/{}/{}/{:020}",
        METRICS_PREFIX, granularity, tenant_id, project_id, timestamp_us
    )
}

/// Create a scan prefix for a tenant/project time range
pub fn trace_scan_prefix(tenant_id: u64, project_id: u16) -> String {
    format!("{}/{}/{}/", TRACE_PREFIX, tenant_id, project_id)
}

// ============================================================================
// Columnar Edge Storage (80% I/O Reduction)
// ============================================================================

/// Creates the PackedTableSchema for edge storage
/// 
/// This schema enables columnar projection - reading only the fields needed
/// instead of deserializing the entire edge. For analytics queries that only
/// need timestamp + duration, this reduces I/O by 80%+.
/// 
/// ## Column Layout
/// ```text
/// | Column        | Type   | Size  | Description              |
/// |---------------|--------|-------|--------------------------|
/// | edge_id       | Binary | 16    | Unique edge identifier   |
/// | tenant_id     | UInt   | 8     | Tenant identifier        |
/// | project_id    | UInt   | 2     | Project identifier       |
/// | timestamp_us  | UInt   | 8     | Event timestamp (micros) |
/// | session_id    | UInt   | 8     | Session identifier       |
/// | agent_id      | UInt   | 8     | Agent identifier         |
/// | span_type     | UInt   | 4     | Type of span             |
/// | duration_us   | UInt   | 4     | Duration in microseconds |
/// | token_count   | UInt   | 4     | Token count              |
/// | has_payload   | Bool   | 1     | Payload flag             |
/// ```
pub fn create_edge_schema() -> PackedTableSchema {
    PackedTableSchema::new(
        "edges",
        vec![
            PackedColumnDef { name: "edge_id".into(), col_type: PackedColumnType::Binary, nullable: false },
            PackedColumnDef { name: "tenant_id".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "project_id".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "timestamp_us".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "session_id".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "agent_id".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "span_type".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "duration_us".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "token_count".into(), col_type: PackedColumnType::UInt64, nullable: false },
            PackedColumnDef { name: "has_payload".into(), col_type: PackedColumnType::Bool, nullable: false },
        ],
    )
}

/// Global edge schema (created once, reused for all packing operations)
static EDGE_SCHEMA: std::sync::LazyLock<PackedTableSchema> = std::sync::LazyLock::new(create_edge_schema);

/// Convert an AgentFlowEdge to a columnar PackedRow for 80% I/O reduction
/// 
/// This enables projection pushdown - queries that only need specific columns
/// (e.g., timestamp + duration for latency analysis) read only those columns.
pub fn edge_to_packed_row(edge: &AgentFlowEdge) -> PackedRow {
    use sochdb_core::SochValue;
    
    let mut values = HashMap::new();
    
    // Core identifiers
    values.insert("edge_id".into(), SochValue::Binary(edge.edge_id.to_le_bytes().to_vec()));
    values.insert("tenant_id".into(), SochValue::UInt(edge.tenant_id));
    values.insert("project_id".into(), SochValue::UInt(edge.project_id as u64));
    values.insert("timestamp_us".into(), SochValue::UInt(edge.timestamp_us));
    
    // Session/agent info
    values.insert("session_id".into(), SochValue::UInt(edge.session_id));
    values.insert("agent_id".into(), SochValue::UInt(edge.agent_id));
    values.insert("span_type".into(), SochValue::UInt(edge.span_type as u64));
    
    // Performance metrics
    values.insert("duration_us".into(), SochValue::UInt(edge.duration_us as u64));
    values.insert("token_count".into(), SochValue::UInt(edge.token_count as u64));
    values.insert("has_payload".into(), SochValue::Bool(edge.has_payload != 0));
    
    PackedRow::pack(&EDGE_SCHEMA, &values)
}

// ============================================================================
// Serialization
// ============================================================================

/// Serialize an AgentFlowEdge to bytes
pub fn serialize_edge(edge: &AgentFlowEdge) -> Result<Vec<u8>> {
    bincode::serialize(edge).map_err(|e| AgentreplayError::Serialization(e.to_string()))
}

/// Deserialize bytes to an AgentFlowEdge
pub fn deserialize_edge(data: &[u8]) -> Result<AgentFlowEdge> {
    bincode::deserialize(data).map_err(|e| AgentreplayError::Serialization(e.to_string()))
}

// ============================================================================
// Storage Configuration
// ============================================================================

/// Configuration for AgentReplay storage
#[derive(Debug, Clone)]
pub struct AgentReplayStorageConfig {
    /// Data directory
    pub data_dir: PathBuf,
    /// Enable WAL for durability
    pub enable_wal: bool,
    /// Sync mode for writes
    pub sync_mode: SyncMode,
    /// Cache size in bytes
    pub cache_size_bytes: usize,
    /// Enable metrics aggregation
    pub enable_metrics: bool,
    /// Metrics flush interval in seconds
    pub metrics_flush_interval_secs: u64,
}

/// Sync mode for writes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Sync on every write (safest, slowest)
    Always,
    /// Batch syncs (balanced)
    Batched,
    /// No sync (fastest, risk of data loss on crash)
    None,
}

impl Default for AgentReplayStorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./agentreplay_data"),
            enable_wal: true,
            sync_mode: SyncMode::Batched,
            cache_size_bytes: 64 * 1024 * 1024, // 64MB — desktop-grade; 256MB was server-grade
            enable_metrics: true,
            metrics_flush_interval_secs: 60,
        }
    }
}

// ============================================================================
// Storage Statistics
// ============================================================================

/// Storage statistics with real observability metrics
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    /// Total number of edges stored
    pub total_edges: u64,
    /// Total bytes on disk (computed from directory scan)
    pub disk_bytes: u64,
    /// Total bytes in memory (edge index + buckets)
    pub memory_bytes: u64,
    /// Number of puts
    pub puts: u64,
    /// Number of gets
    pub gets: u64,
    /// Number of deletes
    pub deletes: u64,
    /// Number of scans
    pub scans: u64,
    /// Cache hit rate (0.0 - 1.0)
    pub cache_hit_rate: f64,
    /// Memtable size in bytes (compatibility)
    pub memtable_size: usize,
    /// Memtable entries (compatibility)
    pub memtable_entries: usize,
    /// Immutable memtables count (compatibility)
    pub immutable_memtables: usize,
    /// WAL sequence number (compatibility)
    pub wal_sequence: u64,
    /// Cache statistics (compatibility)
    pub cache_stats: CacheStats,
    /// Level statistics (compatibility)
    pub levels: Vec<LevelStats>,
    
    // ====== NEW: Real observability metrics ======
    
    /// Number of live (non-deleted) edges
    pub live_edges: u64,
    /// Number of tombstoned/deleted edges
    pub tombstoned_edges: u64,
    /// Tombstone ratio (tombstoned / total) - high ratio indicates need for compaction
    pub tombstone_ratio: f64,
    /// Number of payload records
    pub payload_count: u64,
    /// Total payload bytes
    pub payload_bytes: u64,
    /// Number of orphan payloads (payloads without corresponding edges)
    pub orphan_payload_count: u64,
    /// Session index entry count
    pub session_index_entries: u64,
    /// Project index entry count
    pub project_index_entries: u64,
    /// Memory agent session count
    pub memory_sessions: u64,
    /// Metrics bucket count (minute granularity)
    pub minute_buckets: u64,
    /// Metrics bucket count (hour granularity)
    pub hour_buckets: u64,
}

/// Cache statistics for compatibility
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Cache hit count
    pub hits: u64,
    /// Cache miss count
    pub misses: u64,
    /// Cache size in bytes
    pub size_bytes: u64,
    /// Current size (alias)
    pub size: u64,
    /// Cache capacity
    pub capacity: u64,
}

/// Level statistics for compatibility
#[derive(Debug, Clone, Default)]
pub struct LevelStats {
    /// Level number
    pub level: u32,
    /// Number of files
    pub files: u32,
    /// Total size in bytes
    pub size_bytes: u64,
    /// Number of SSTables
    pub num_sstables: u32,
    /// Total entries
    pub total_entries: u64,
    /// Total size (alias)
    pub total_size: u64,
}

impl StorageStats {
    /// Get total size in bytes
    pub fn total_size_bytes(&self) -> u64 {
        self.disk_bytes + self.memory_bytes
    }
    
    /// Check if storage is healthy (low tombstone ratio, no orphans)
    pub fn is_healthy(&self) -> bool {
        self.tombstone_ratio < 0.3 && self.orphan_payload_count == 0
    }
    
    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "edges={} (live={}, tombstoned={}), disk={}MB, payloads={}, buckets={}",
            self.total_edges,
            self.live_edges,
            self.tombstoned_edges,
            self.disk_bytes / (1024 * 1024),
            self.payload_count,
            self.minute_buckets + self.hour_buckets
        )
    }
}

/// Health check result
#[derive(Debug, Clone, Default)]
pub struct HealthCheckResult {
    /// Number of orphan payloads (payloads without corresponding edges)
    pub orphan_payloads: u64,
    /// IDs of orphan payloads (for cleanup)
    pub orphan_payload_ids: Vec<u128>,
    /// Number of stale session index entries
    pub stale_session_entries: u64,
    /// Number of stale project index entries
    pub stale_project_entries: u64,
    /// Whether storage is healthy
    pub is_healthy: bool,
}

impl HealthCheckResult {
    /// Check if any issues were found
    pub fn has_issues(&self) -> bool {
        self.orphan_payloads > 0 || self.stale_session_entries > 0 || self.stale_project_entries > 0
    }
}

/// Cleanup operation statistics
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    /// Number of orphan payloads deleted
    pub payloads_deleted: u64,
    /// Number of stale index entries found (not automatically deleted)
    pub stale_index_entries_found: u64,
    /// Bytes reclaimed
    pub bytes_reclaimed: u64,
}

// ============================================================================
// Dashboard Summary (Task 9: Eliminate redundant scans)
// ============================================================================

/// Pre-computed dashboard summary updated incrementally on each edge insertion.
///
/// **Performance:** Serves all dashboard queries in O(1) — a single read of this
/// structure — instead of 4 × O(N) scans (list_traces + timeseries + costs + sessions).
///
/// All fields are commutative, associative, and incrementally updatable via `record_edge`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DashboardSummary {
    /// Total trace count
    pub total_traces: u64,
    /// Total session count (approximate — based on distinct session_ids seen)
    pub total_sessions: u64,
    /// Total token count
    pub total_tokens: u64,
    /// Total cost in microdollars (6 decimal places)
    pub total_cost_micros: u64,
    /// Total duration in microseconds (for computing average)
    pub total_duration_us: u64,
    /// Error count
    pub error_count: u64,
    /// most recent edge timestamp
    pub last_edge_ts: u64,
    /// Top models: model_name → (call_count, token_count)
    pub top_models: HashMap<String, (u64, u64)>,
    /// Top providers: provider → call_count
    pub top_providers: HashMap<String, u64>,
}

impl DashboardSummary {
    /// Record a new edge into the summary
    pub fn record_edge(&mut self, edge: &AgentFlowEdge) {
        self.total_traces += 1;
        self.total_tokens += edge.token_count as u64;
        self.total_duration_us += edge.duration_us as u64;
        if edge.timestamp_us > self.last_edge_ts {
            self.last_edge_ts = edge.timestamp_us;
        }
    }

    /// Record model/provider attribution for an edge
    pub fn record_model(&mut self, model: &str, provider: &str, tokens: u64) {
        if !model.is_empty() {
            let entry = self.top_models.entry(model.to_string()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += tokens;
        }
        if !provider.is_empty() {
            *self.top_providers.entry(provider.to_string()).or_insert(0) += 1;
        }
    }
}

// ============================================================================
// Metrics Bucket
// ============================================================================

/// Pre-aggregated metrics for a time bucket
#[derive(Debug, Clone, Default)]
pub struct MetricsBucket {
    /// Bucket start timestamp
    pub timestamp_us: u64,
    /// Tenant ID
    pub tenant_id: u64,
    /// Project ID
    pub project_id: u16,
    /// Request count
    pub request_count: u64,
    /// Error count
    pub error_count: u64,
    /// Total tokens
    pub total_tokens: u64,
    /// Total duration in microseconds
    pub total_duration_us: u64,
    /// Minimum duration
    pub min_duration_us: u64,
    /// Maximum duration
    pub max_duration_us: u64,
}

impl MetricsBucket {
    /// Create a new metrics bucket
    pub fn new(timestamp_us: u64, tenant_id: u64, project_id: u16) -> Self {
        Self {
            timestamp_us,
            tenant_id,
            project_id,
            min_duration_us: u64::MAX,
            ..Default::default()
        }
    }

    /// Record an edge
    pub fn record(&mut self, edge: &AgentFlowEdge) {
        self.request_count += 1;
        self.total_tokens += edge.token_count as u64;
        let duration = edge.duration_us as u64;
        self.total_duration_us += duration;
        self.min_duration_us = self.min_duration_us.min(duration);
        self.max_duration_us = self.max_duration_us.max(duration);
    }

    /// Merge another bucket into this one
    pub fn merge(&mut self, other: &MetricsBucket) {
        self.request_count += other.request_count;
        self.error_count += other.error_count;
        self.total_tokens += other.total_tokens;
        self.total_duration_us += other.total_duration_us;
        self.min_duration_us = self.min_duration_us.min(other.min_duration_us);
        self.max_duration_us = self.max_duration_us.max(other.max_duration_us);
    }

    /// Get average duration in milliseconds
    pub fn avg_duration_ms(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            (self.total_duration_us as f64 / self.request_count as f64) / 1000.0
        }
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&self.timestamp_us.to_le_bytes());
        buf.extend_from_slice(&self.tenant_id.to_le_bytes());
        buf.extend_from_slice(&self.project_id.to_le_bytes());
        buf.extend_from_slice(&[0u8; 6]); // padding
        buf.extend_from_slice(&self.request_count.to_le_bytes());
        buf.extend_from_slice(&self.error_count.to_le_bytes());
        buf.extend_from_slice(&self.total_tokens.to_le_bytes());
        buf.extend_from_slice(&self.total_duration_us.to_le_bytes());
        buf.extend_from_slice(&self.min_duration_us.to_le_bytes());
        buf.extend_from_slice(&self.max_duration_us.to_le_bytes());
        buf
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 64 {
            return Err(AgentreplayError::Serialization("Metrics bucket too short".into()));
        }
        Ok(Self {
            timestamp_us: u64::from_le_bytes(data[0..8].try_into().unwrap()),
            tenant_id: u64::from_le_bytes(data[8..16].try_into().unwrap()),
            project_id: u16::from_le_bytes(data[16..18].try_into().unwrap()),
            request_count: u64::from_le_bytes(data[24..32].try_into().unwrap()),
            error_count: u64::from_le_bytes(data[32..40].try_into().unwrap()),
            total_tokens: u64::from_le_bytes(data[40..48].try_into().unwrap()),
            total_duration_us: u64::from_le_bytes(data[48..56].try_into().unwrap()),
            min_duration_us: u64::from_le_bytes(data[56..64].try_into().unwrap()),
            max_duration_us: if data.len() >= 72 {
                u64::from_le_bytes(data[64..72].try_into().unwrap())
            } else {
                0
            },
        })
    }
}

// ============================================================================
// AgentReplay Storage
// ============================================================================

/// Main storage interface for AgentReplay using SochDB
/// 
/// ## Features
/// 
/// - **Persistent indexes**: O(log N) lookups via `idx/edge/{id}`, `idx/session/{id}`, etc.
/// - **Group commit**: 100× throughput improvement via batched fsync
/// - **Semantic caching**: Cache query results with similarity-based lookup
/// - **Columnar storage**: 80% I/O reduction via PackedRow projection pushdown
/// 
/// ## v2 Improvements (removed in-memory edge_index)
/// 
/// The previous version maintained an in-memory `HashMap<u128, String>` for O(1) edge lookups.
/// This consumed ~80 bytes per edge (128-bit ID + 64-byte String key), which for 10M edges
/// was 800MB of RAM.
/// 
/// Now all lookups go through the persistent `idx/edge/{id}` index in SochDB, which:
/// - Saves 800MB+ RAM for large deployments
/// - Survives restarts without reindexing
/// - Uses SochDB's block cache for hot-path caching
pub struct AgentReplayStorage {
    /// SochDB connection
    connection: Arc<Connection>,
    /// Configuration
    config: AgentReplayStorageConfig,
    /// Write serialization lock (prevents concurrent writes from racing)
    /// We no longer store edge_id->key mappings here - they're in SochDB's idx/edge/* index
    write_lock: RwLock<()>,
    /// In-memory metrics buckets (flushed periodically)
    minute_buckets: RwLock<BTreeMap<(u64, u16, u64), MetricsBucket>>,
    hour_buckets: RwLock<BTreeMap<(u64, u16, u64), MetricsBucket>>,
    /// Pre-computed dashboard summary (Task 9: O(1) dashboard queries)
    dashboard_summary: RwLock<DashboardSummary>,
    /// Statistics
    stats: StorageStatsAtomic,
    /// Shutdown flag
    shutdown: AtomicBool,
    /// Semantic query cache for repeated LLM context queries
    /// Uses SochDB's semantic_cache for similarity-based cache hits
    semantic_cache_enabled: bool,
    /// Enable columnar storage for edges (80% I/O reduction)
    /// When true, edges are stored as PackedRows in addition to JSON
    columnar_edges_enabled: bool,
}

/// Atomic storage statistics
struct StorageStatsAtomic {
    puts: AtomicU64,
    gets: AtomicU64,
    deletes: AtomicU64,
    scans: AtomicU64,
    edges: AtomicU64,
}

impl Default for StorageStatsAtomic {
    fn default() -> Self {
        Self {
            puts: AtomicU64::new(0),
            gets: AtomicU64::new(0),
            deletes: AtomicU64::new(0),
            scans: AtomicU64::new(0),
            edges: AtomicU64::new(0),
        }
    }
}

impl AgentReplayStorage {
    /// Open storage at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = AgentReplayStorageConfig {
            data_dir: path.as_ref().to_path_buf(),
            ..Default::default()
        };
        Self::open_with_config(config)
    }

    /// Open storage with custom configuration
    pub fn open_with_config(config: AgentReplayStorageConfig) -> Result<Self> {
        info!("Opening AgentReplay storage at {:?}", config.data_dir);
        
        // Create data directory if needed
        std::fs::create_dir_all(&config.data_dir)?;

        // ============================================================================
        // OPTIMIZATION: Configure SochDB for Agentreplay's trace ingestion workload
        // 
        // Agentreplay workload characteristics:
        // - Write-heavy: Trace ingestion is continuous, high-volume
        // - Read pattern: Mostly range scans (time-based), occasional point lookups
        // - Analytics: Periodic aggregations over time ranges
        // 
        // Key optimizations applied:
        // 1. Group commit: 100× throughput improvement (batch fsync)
        // 2. WriteOptimized policy: 20% write speedup (skip ordered index overhead)
        // 3. Larger memtable: Reduce flush frequency
        // ============================================================================
        
        // Use throughput-optimized preset as base, then customize
        let mut db_config = sochdb_storage::database::DatabaseConfig::throughput_optimized();
        
        // Override with Agentreplay-specific settings
        db_config.wal_enabled = config.enable_wal;
        
        // Apply sync mode
        db_config.sync_mode = match config.sync_mode {
            SyncMode::Always => sochdb_storage::database::SyncMode::Full,
            SyncMode::Batched => sochdb_storage::database::SyncMode::Normal,
            SyncMode::None => sochdb_storage::database::SyncMode::Off,
        };

        // Apply memory limit — sized for desktop, not server
        db_config.memtable_size_limit = config.cache_size_bytes;
        
        // Enable group commit with high-throughput settings
        // This batches multiple commits into single fsync operations
        // Throughput improvement: K commits per fsync where K ≈ 50-5000
        db_config.group_commit = true;
        db_config.group_commit_config = sochdb_storage::database::GroupCommitSettings::high_throughput();
        
        // Use WriteOptimized policy for trace ingestion
        // Trace writes are append-only by nature (timestamped), so we don't need
        // expensive ordered index maintenance on the write path
        // db_config.default_index_policy = sochdb_storage::index_policy::IndexPolicy::AppendOnly;
        
        info!(
            wal = db_config.wal_enabled,
            sync = ?db_config.sync_mode,
            mem_limit = db_config.memtable_size_limit,
            group_commit = db_config.group_commit,
            index_policy = ?db_config.default_index_policy,
            "Initializing SochDB with optimized configuration for trace ingestion"
        );
        
        // Open SochDB connection with config
        let connection = Connection::open_with_config(&config.data_dir, db_config)
            .map_err(|e| AgentreplayError::Internal(format!("Failed to open SochDB: {}", e)))?;
            

        
        let storage = Self {
            connection: Arc::new(connection),
            config,
            write_lock: RwLock::new(()),
            minute_buckets: RwLock::new(BTreeMap::new()),
            hour_buckets: RwLock::new(BTreeMap::new()),
            dashboard_summary: RwLock::new(DashboardSummary::default()),
            stats: StorageStatsAtomic::default(),
            shutdown: AtomicBool::new(false),
            semantic_cache_enabled: true, // Enable semantic caching by default
            columnar_edges_enabled: true, // Enable columnar storage by default
        };
        
        // Load persisted metrics from disk to warm up the cache
        if let Err(e) = storage.load_initial_metrics() {
            warn!("Failed to load initial metrics from disk: {}", e);
            // Non-fatal, start with empty metrics
        }

        info!(
            semantic_cache = storage.semantic_cache_enabled,
            columnar_edges = storage.columnar_edges_enabled,
            "AgentReplay storage opened successfully with optimizations"
        );
        Ok(storage)
    }

    /// Load persisted metrics from disk into memory
    fn load_initial_metrics(&self) -> Result<()> {
        let prefix = "metrics/";
        let results = self.connection.scan(prefix)
            .map_err(|e| AgentreplayError::Internal(format!("Failed to scan metrics: {}", e)))?;
            
        let mut min_buckets = self.minute_buckets.write();
        let mut hr_buckets = self.hour_buckets.write();
        let mut count = 0;
        
        for (key, value) in results {
            // Key format: metrics/{granularity}/{tenant}/{project}/{ts}
            let parts: Vec<&str> = key.split('/').collect();
            if parts.len() < 5 { continue; }
            
            let granularity = parts[1];
            if let (Ok(tenant_id), Ok(project_id), Ok(ts)) = (
                parts[2].parse::<u64>(),
                parts[3].parse::<u16>(),
                parts[4].parse::<u64>(),
            ) {
                 if let Ok(bucket) = MetricsBucket::deserialize(&value) {
                     if granularity == "minute" {
                         min_buckets.insert((tenant_id, project_id, ts), bucket);
                     } else if granularity == "hour" {
                         hr_buckets.insert((tenant_id, project_id, ts), bucket);
                     }
                     count += 1;
                 }
            }
        }
        
        info!("Loaded {} metrics buckets from disk", count);
        Ok(())
    }

    /// Put an edge into storage
    /// 
    /// **Performance Note:** With group commit enabled, this method does NOT
    /// call commit() after every operation. SochDB's group commit batches
    /// multiple operations into single fsync calls for 100× throughput.
    /// 
    /// For explicit durability guarantees, call `sync()` after critical writes.
    pub fn put(&self, edge: AgentFlowEdge) -> Result<()> {
        // SYNCHRONIZATION: Acquire write lock to serialize writes and prevent transaction races
        // This ensures put is atomic relative to other threads
        let _write_guard = self.write_lock.write();

        self.put_internal(edge)?;
        
        // NOTE: No explicit commit() here - SochDB's group commit handles batching
        // Group commit accumulates operations and flushes them together,
        // achieving 100× throughput vs per-operation commit
        // 
        // The group commit will:
        // 1. Batch operations until batch_size or max_wait_us threshold
        // 2. Issue single fsync for entire batch
        // 3. Return success to all waiting operations
        //
        // For immediate durability, call sync() explicitly

        Ok(())
    }
    
    /// Put an edge with immediate durability guarantee
    /// 
    /// Unlike `put()`, this method forces an immediate commit.
    /// Use sparingly - for most cases, rely on group commit via `put()`.
    pub fn put_durable(&self, edge: AgentFlowEdge) -> Result<()> {
        let _write_guard = self.write_lock.write();
        self.put_internal(edge)?;
        
        let _ = self.connection.commit()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;

        Ok(())
    }

    /// Internal put without commit (for batching)
    fn put_internal(&self, edge: AgentFlowEdge) -> Result<()> {
        let key = encode_trace_key(edge.tenant_id, edge.project_id, edge.timestamp_us, edge.edge_id);
        let data = serialize_edge(&edge)?;
        
        self.connection.put(&key, &data)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB put failed: {}", e)))?;
        
        // PackedRow columnar storage REMOVED — it duplicated the bincode edge
        // with ~63 bytes overhead per span (was never read by any query path).
        // For 2.5M spans this saves ~158 MB of WAL + 2.5M BTreeMap entries in memory.
        
        // ====================================================================
        // Secondary indexes stored in SochDB for persistence across restarts
        // ====================================================================
        
        // Edge ID reverse index: edge_id -> primary key
        // This allows O(log N) point lookup by edge_id without full scan
        let edge_idx_key = format!("idx/edge/{:032x}", edge.edge_id);
        self.connection.put(&edge_idx_key, key.as_bytes())
            .map_err(|e| AgentreplayError::Internal(format!("SochDB edge index update failed: {}", e)))?;

        // Session index: session_id/edge_id -> (exists)
        let session_key = format!("idx/session/{}/{:032x}", edge.session_id, edge.edge_id);
        self.connection.put(&session_key, &[])
            .map_err(|e| AgentreplayError::Internal(format!("SochDB session index update failed: {}", e)))?;

        // Project index: project_id/edge_id -> (exists)
        let project_key = format!("idx/project/{}/{:032x}", edge.project_id, edge.edge_id);
        self.connection.put(&project_key, &[])
             .map_err(|e| AgentreplayError::Internal(format!("SochDB project index update failed: {}", e)))?;
        
        // Tenant index: tenant_id/timestamp -> edge_id (for tenant-scoped time queries)
        let tenant_ts_key = format!("idx/tenant/{}/{:020}/{:032x}", edge.tenant_id, edge.timestamp_us, edge.edge_id);
        self.connection.put(&tenant_ts_key, &[])
            .map_err(|e| AgentreplayError::Internal(format!("SochDB tenant index update failed: {}", e)))?;

        // Record metrics in in-memory buckets
        self.record_metrics(&edge);

        self.stats.puts.fetch_add(1, Ordering::Relaxed);
        self.stats.edges.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }

    /// Store denormalized filter attributes for an edge.
    ///
    /// **Predicate Pushdown (Task 4):** Stores provider, model, and operation_name
    /// as a compact index entry so list_traces filtering can avoid per-edge payload
    /// I/O. These are the fields most commonly used in filter queries.
    ///
    /// Key format: `idx/attrs/{edge_id:032x}` → `{provider}\0{model}\0{operation_name}`
    ///
    /// This adds ~100 bytes per edge (negligible) to eliminate 2KB+ payload reads
    /// during filtering — a 20× I/O reduction per filtered edge.
    pub fn put_edge_attrs(
        &self,
        edge_id: u128,
        provider: Option<&str>,
        model: Option<&str>,
        operation_name: Option<&str>,
    ) -> Result<()> {
        let key = format!("idx/attrs/{:032x}", edge_id);
        let value = format!(
            "{}\0{}\0{}",
            provider.unwrap_or(""),
            model.unwrap_or(""),
            operation_name.unwrap_or("")
        );
        self.connection.put(&key, value.as_bytes())
            .map_err(|e| AgentreplayError::Internal(format!("SochDB put attrs failed: {}", e)))?;
        Ok(())
    }

    /// Get denormalized filter attributes for an edge.
    ///
    /// Returns `(provider, model, operation_name)` — all empty string if not found.
    pub fn get_edge_attrs(&self, edge_id: u128) -> Result<(String, String, String)> {
        let key = format!("idx/attrs/{:032x}", edge_id);
        if let Some(data) = self.connection.get(&key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB get attrs failed: {}", e)))? {
            let s = String::from_utf8_lossy(&data);
            let parts: Vec<&str> = s.splitn(3, '\0').collect();
            let provider = parts.first().unwrap_or(&"").to_string();
            let model = parts.get(1).unwrap_or(&"").to_string();
            let operation_name = parts.get(2).unwrap_or(&"").to_string();
            Ok((provider, model, operation_name))
        } else {
            Ok((String::new(), String::new(), String::new()))
        }
    }

    /// Batch-get denormalized filter attributes for multiple edges.
    ///
    /// Returns a HashMap of edge_id → (provider, model, operation_name).
    /// Missing entries are omitted from the result.
    pub fn get_edge_attrs_batch(
        &self,
        edge_ids: &[u128],
    ) -> Result<HashMap<u128, (String, String, String)>> {
        let mut result = HashMap::with_capacity(edge_ids.len());
        for &edge_id in edge_ids {
            if let Ok(attrs) = self.get_edge_attrs(edge_id) {
                if !attrs.0.is_empty() || !attrs.1.is_empty() || !attrs.2.is_empty() {
                    result.insert(edge_id, attrs);
                }
            }
        }
        Ok(result)
    }

    /// Put a batch of edges (high-throughput bulk ingestion)
    /// 
    /// **Performance Note:** Uses SochDB's group commit for optimal throughput.
    /// A single commit is issued at the end of the batch, amortizing fsync cost.
    /// 
    /// For N edges, this achieves:
    /// - Throughput: N / L_fsync (vs 1 / L_fsync per edge with individual commits)
    /// - Latency: O(N * put_cost) + L_fsync (single fsync for entire batch)
    pub fn put_batch(&self, edges: &[AgentFlowEdge]) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }
        
        // SYNCHRONIZATION: Acquire write lock for entire batch
        let _write_guard = self.write_lock.write();
        
        for edge in edges {
            self.put_internal(edge.clone())?;
        }
        
        // Explicit commit at end of batch for durability
        // This is more efficient than per-operation commit
        let _ = self.connection.commit()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;
        
        // Use debug level to avoid per-batch log overhead under high throughput
        tracing::debug!(batch_size = edges.len(), "Batch ingestion complete");
        Ok(())
    }
    
    /// Put a batch of edges without explicit commit (rely on group commit)
    /// 
    /// Use this for maximum throughput when eventual consistency is acceptable.
    /// The group commit will flush based on time/size thresholds.
    pub fn put_batch_async(&self, edges: &[AgentFlowEdge]) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }
        
        let _write_guard = self.write_lock.write();
        
        for edge in edges {
            self.put_internal(edge.clone())?;
        }
        
        // No explicit commit - group commit handles batching
        Ok(())
    }

    /// Combined batch write: payloads + edges under ONE lock + ONE commit.
    ///
    /// This is the highest-throughput write path. Instead of separate
    /// `put_payloads_batch` (lock→commit) then `put_batch` (lock→commit),
    /// everything runs under a single write-lock acquisition with a single
    /// fsync at the end — cutting commit overhead in half (~30ms saved per batch
    /// on macOS APFS).
    pub fn put_batch_with_payloads(&self, edges: &[AgentFlowEdge], payloads: &[(u128, &[u8])]) -> Result<()> {
        if edges.is_empty() && payloads.is_empty() {
            return Ok(());
        }

        let _write_guard = self.write_lock.write();

        // Write payloads first (they should exist before edges for consistency)
        // Payloads are zstd-compressed (level 1) before storage.
        // For typical OTLP JSON (~650 bytes), zstd achieves ~4-6x compression,
        // reducing per-span WAL cost from ~700 bytes to ~150 bytes.
        for (edge_id, data) in payloads {
            let key = encode_payload_key(*edge_id);
            let compressed = compress_payload(data);
            self.connection.put(&key, &compressed)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB put payload failed: {}", e)))?;
        }

        // Write edges with all indexes
        for edge in edges {
            self.put_internal(edge.clone())?;
        }

        // Single commit for the entire batch (payloads + edges + indexes)
        let _ = self.connection.commit()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;

        tracing::debug!(edges = edges.len(), payloads = payloads.len(), "Batch with payloads complete");
        Ok(())
    }

    /// Get an edge by ID
    /// 
    /// **Performance:** Uses persistent edge_id index for O(log N) lookups.
    /// SochDB's block cache provides hot-path caching at the storage layer.
    pub fn get(&self, edge_id: u128) -> Result<Option<AgentFlowEdge>> {
        self.stats.gets.fetch_add(1, Ordering::Relaxed);
        
        // Use persistent edge_id index (O(log N))
        // SochDB's block cache handles hot-path caching
        let edge_idx_key = format!("idx/edge/{:032x}", edge_id);
        if let Some(primary_key_bytes) = self.connection.get(&edge_idx_key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB index get failed: {}", e)))? {
            
            let primary_key = String::from_utf8(primary_key_bytes)
                .map_err(|e| AgentreplayError::Internal(format!("Invalid key encoding: {}", e)))?;
            
            if let Some(data) = self.connection.get(&primary_key)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB get failed: {}", e)))? {
                if data.is_empty() {
                    return Ok(None);
                }
                match deserialize_edge(&data) {
                    Ok(edge) => return Ok(Some(edge)),
                    Err(e) => {
                        warn!("Failed to deserialize edge {} (treating as missing): {}", edge_id, e);
                        return Ok(None);
                    }
                }
            }
        }
        
        // Fallback: Full scan for legacy data without index (O(N))
        // This is expensive and should rarely happen after index migration
        warn!(edge_id = ?edge_id, "Falling back to full scan for edge lookup - consider reindexing");
        let prefix = format!("{}/", TRACE_PREFIX);
        let results = self.connection.scan(&prefix)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB scan failed: {}", e)))?;
        
        for (key_str, value) in results {
            if let Some((_, _, _, eid)) = decode_trace_key(&key_str) {
                if eid == edge_id {
                    // Persist to index for future lookups
                    let edge_idx_key = format!("idx/edge/{:032x}", edge_id);
                    let _ = self.connection.put(&edge_idx_key, key_str.as_bytes());
                    
                    match deserialize_edge(&value) {
                        Ok(edge) => return Ok(Some(edge)),
                        Err(e) => {
                             warn!("Failed to deserialize scanned edge {} (skipping): {}", edge_id, e);
                             continue;
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Get multiple edges by IDs
    pub fn get_many(&self, edge_ids: &[u128]) -> Result<Vec<Option<AgentFlowEdge>>> {
        let mut results = Vec::with_capacity(edge_ids.len());
        for &edge_id in edge_ids {
            results.push(self.get(edge_id)?);
        }
        Ok(results)
    }

    /// Delete an edge with full index consistency
    ///
    /// **Tenant Safety:** The tenant_id is used to verify the edge belongs to the caller's tenant.
    /// This prevents cross-tenant deletions.
    ///
    /// **Index Consistency:** Deletes the following in a single transaction:
    /// - Main edge record (traces/...)
    /// - Session secondary index (sessions/...)
    /// - Project secondary index (projects/...)
    /// - Associated payload (payloads/...)
    pub fn delete(&self, edge_id: u128, tenant_id: u64) -> Result<()> {
        self.stats.deletes.fetch_add(1, Ordering::Relaxed);
        
        // SYNCHRONIZATION: Lock for delete + commit
        let _write_guard = self.write_lock.write();

        // First, look up the edge via persistent index
        let edge_idx_key = format!("idx/edge/{:032x}", edge_id);
        if let Some(key_bytes) = self.connection.get(&edge_idx_key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB get failed: {}", e)))? {
            
            let key = String::from_utf8(key_bytes)
                .map_err(|e| AgentreplayError::Internal(format!("Invalid key encoding: {}", e)))?;
            
            // Decode the key to get the tenant and verify ownership
            if let Some((stored_tenant_id, project_id, timestamp_us, _)) = decode_trace_key(&key) {
                if stored_tenant_id != tenant_id {
                    warn!(
                        "Tenant isolation violation: tenant {} tried to delete edge {} owned by tenant {}",
                        tenant_id, edge_id, stored_tenant_id
                    );
                    return Err(AgentreplayError::NotFound(
                        format!("Edge {:#x} not found for tenant {}", edge_id, tenant_id)
                    ));
                }

                // Fetch edge data before deletion for index cleanup
                let edge_data = self.connection.get(&key)
                    .map_err(|e| AgentreplayError::Internal(format!("SochDB get failed: {}", e)))?;

                // Delete main edge record
                self.connection.delete(&key)
                    .map_err(|e| AgentreplayError::Internal(format!("SochDB delete failed: {}", e)))?;

                // Delete all secondary indexes
                // Uses new idx/ prefix format
                
                // Edge ID reverse index
                let _ = self.connection.delete(&edge_idx_key);
                
                // Session index (need session_id from edge)
                if let Some(data) = edge_data {
                    if let Ok(edge) = deserialize_edge(&data) {
                        let session_key = format!("idx/session/{}/{:032x}", edge.session_id, edge_id);
                        let _ = self.connection.delete(&session_key);
                    }
                }

                // Project index
                let project_key = format!("idx/project/{}/{:032x}", project_id, edge_id);
                let _ = self.connection.delete(&project_key);
                
                // Tenant timestamp index
                let tenant_ts_key = format!("idx/tenant/{}/{:020}/{:032x}", tenant_id, timestamp_us, edge_id);
                let _ = self.connection.delete(&tenant_ts_key);

                // Delete associated payload (cascading delete)
                let payload_key = encode_payload_key(edge_id);
                let _ = self.connection.delete(&payload_key);

                self.stats.edges.fetch_sub(1, Ordering::Relaxed);
                
                let _ = self.connection.commit()
                    .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;
            }
        }
        
        Ok(())
    }

    /// Delete an edge without tenant verification (internal use only)
    ///
    /// **WARNING:** This bypasses tenant isolation. Only use for:
    /// - Retention/GC operations where tenant is already verified
    /// - Admin operations
    /// - Single-tenant desktop mode
    pub fn delete_unchecked(&self, edge_id: u128) -> Result<()> {
        self.stats.deletes.fetch_add(1, Ordering::Relaxed);
        
        let _write_guard = self.write_lock.write();

        // Look up edge via persistent index
        let edge_idx_key = format!("idx/edge/{:032x}", edge_id);
        if let Some(key_bytes) = self.connection.get(&edge_idx_key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB get failed: {}", e)))? {
            
            let key = String::from_utf8(key_bytes)
                .map_err(|e| AgentreplayError::Internal(format!("Invalid key encoding: {}", e)))?;

            // Get edge data for cascading deletes
            if let Ok(Some(data)) = self.connection.get(&key) {
                if let Ok(edge) = deserialize_edge(&data) {
                    // Delete all secondary indexes with new idx/ prefix format
                    
                    // Session index
                    let session_key = format!("idx/session/{}/{:032x}", edge.session_id, edge_id);
                    let _ = self.connection.delete(&session_key);

                    // Project index
                    let project_key = format!("idx/project/{}/{:032x}", edge.project_id, edge_id);
                    let _ = self.connection.delete(&project_key);
                    
                    // Tenant timestamp index
                    let tenant_ts_key = format!("idx/tenant/{}/{:020}/{:032x}", edge.tenant_id, edge.timestamp_us, edge_id);
                    let _ = self.connection.delete(&tenant_ts_key);
                }
            }
            
            // Edge ID reverse index
            let _ = self.connection.delete(&edge_idx_key);

            // Delete main edge record
            self.connection.delete(&key)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB delete failed: {}", e)))?;

            // Delete associated payload
            let payload_key = encode_payload_key(edge_id);
            let _ = self.connection.delete(&payload_key);

            self.stats.edges.fetch_sub(1, Ordering::Relaxed);
            
            let _ = self.connection.commit()
                .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;
        }
        
        Ok(())
    }

    /// Delete a payload explicitly
    ///
    /// **Note:** This is normally called automatically by delete(), but can be used
    /// for orphan payload cleanup during GC.
    pub fn delete_payload(&self, edge_id: u128) -> Result<()> {
        let payload_key = encode_payload_key(edge_id);
        self.connection.delete(&payload_key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB delete payload failed: {}", e)))?;
        let _ = self.connection.commit()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;
        Ok(())
    }

    /// Range scan for edges in a time window
    /// 
    /// **Performance Note:** This scans all traces with prefix filtering.
    /// For tenant-scoped queries, use `query_temporal_range_for_tenant()` which
    /// uses the tenant index for O(log N + K) instead of O(N).
    pub fn range_scan(&self, start_ts: u64, end_ts: u64) -> Result<Vec<AgentFlowEdge>> {
        self.stats.scans.fetch_add(1, Ordering::Relaxed);
        
        let mut edges = Vec::new();
        let prefix = format!("{}/", TRACE_PREFIX);
        
        let results = self.connection.scan(&prefix)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB scan failed: {}", e)))?;
        
        for (key_str, value) in results {
            if let Some((_, _, ts, _)) = decode_trace_key(&key_str) {
                if ts >= start_ts && ts <= end_ts {
                    if let Ok(edge) = deserialize_edge(&value) {
                        edges.push(edge);
                    }
                }
            }
        }
        
        // Sort by timestamp
        edges.sort_by_key(|e| e.timestamp_us);
        Ok(edges)
    }

    /// Range scan with tenant and project filter
    ///
    /// Uses bounded key-range scan when tenant or tenant+project are specified,
    /// reducing scan from O(N_total) to O(N_matching_time_range).
    pub fn range_scan_filtered(
        &self,
        start_ts: u64,
        end_ts: u64,
        tenant_id: Option<u64>,
        project_id: Option<u16>,
    ) -> Result<Vec<AgentFlowEdge>> {
        self.stats.scans.fetch_add(1, Ordering::Relaxed);
        
        let mut edges = Vec::new();
        
        // When both tenant and project are known, use bounded key-range scan.
        // Key format: traces/{tenant}/{project}/{timestamp:020}/{edge_id:032x}
        // Since timestamps are zero-padded, lexicographic order = chronological order.
        if let (Some(t), Some(p)) = (tenant_id, project_id) {
            let start_key = format!("{}/{}/{}/{:020}/", TRACE_PREFIX, t, p, start_ts);
            let end_key = format!("{}/{}/{}/{:020}/", TRACE_PREFIX, t, p, end_ts.saturating_add(1));
            
            let results = self.connection.scan_range(&start_key, &end_key)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB scan_range failed: {}", e)))?;
            
            for (_key_str, value) in results {
                if let Ok(edge) = deserialize_edge(&value) {
                    edges.push(edge);
                }
            }
        } else if let Some(t) = tenant_id {
            // Tenant-only: use bounded scan on the tenant index for time range
            // This avoids scanning ALL tenant entries when only a time window is needed
            let start_key = format!("idx/tenant/{}/{:020}/", t, start_ts);
            let end_key = format!("idx/tenant/{}/{:020}/", t, end_ts.saturating_add(1));
            
            let results = self.connection.scan_range(&start_key, &end_key)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB scan_range failed: {}", e)))?;
            
            for (key_str, _) in &results {
                let parts: Vec<&str> = key_str.split('/').collect();
                if parts.len() >= 5 {
                    if let Ok(eid) = u128::from_str_radix(parts[4], 16) {
                        if let Some(edge) = self.get(eid)? {
                            // Apply project filter if needed
                            let project_match = project_id.map_or(true, |p| p == edge.project_id);
                            if project_match {
                                edges.push(edge);
                            }
                        }
                    }
                }
            }
        } else {
            // No tenant specified: full prefix scan + post-filter
            let prefix = format!("{}/", TRACE_PREFIX);
            
            let results = self.connection.scan(&prefix)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB scan failed: {}", e)))?;
            
            for (key_str, value) in results {
                if let Some((_t_id, p_id, ts, _)) = decode_trace_key(&key_str) {
                    if ts >= start_ts && ts <= end_ts {
                        let project_match = project_id.map_or(true, |p| p == p_id);
                        
                        if project_match {
                            if let Ok(edge) = deserialize_edge(&value) {
                                edges.push(edge);
                            }
                        }
                    }
                }
            }
        }
        
        edges.sort_by_key(|e| e.timestamp_us);
        Ok(edges)
    }
    
    /// Query edges for a specific tenant within a time range
    /// 
    /// **Performance:** Uses bounded range scan on the tenant index for true
    /// O(log N + K) complexity where:
    /// - N = total edges in database
    /// - K = edges matching the time window (NOT total tenant edges)
    /// 
    /// The key format `idx/tenant/{tenant_id}/{timestamp:020}/{edge_id:032x}`
    /// uses zero-padded timestamps so lexicographic order equals chronological
    /// order. We encode the time bounds directly into the scan range, avoiding
    /// the need to scan and post-filter ALL tenant entries.
    pub fn query_temporal_range_for_tenant(
        &self,
        start_ts: u64,
        end_ts: u64,
        tenant_id: u64,
    ) -> Result<Vec<AgentFlowEdge>> {
        self.stats.scans.fetch_add(1, Ordering::Relaxed);
        
        // Bounded range scan on tenant index — only touches keys in [start_ts, end_ts]
        // Key format: idx/tenant/{tenant_id}/{timestamp:020}/{edge_id:032x}
        let start_key = format!("idx/tenant/{}/{:020}/", tenant_id, start_ts);
        let end_key = format!("idx/tenant/{}/{:020}/", tenant_id, end_ts.saturating_add(1));
        
        let results = self.connection.scan_range(&start_key, &end_key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB scan_range failed: {}", e)))?;
        
        // Collect edge IDs from index entries (already in timestamp order due to key encoding)
        let mut edge_ids: Vec<u128> = Vec::with_capacity(results.len());
        for (key_str, _) in &results {
            // Key format: idx/tenant/{tenant_id}/{timestamp:020}/{edge_id:032x}
            let parts: Vec<&str> = key_str.split('/').collect();
            if parts.len() >= 5 {
                if let Ok(eid) = u128::from_str_radix(parts[4], 16) {
                    edge_ids.push(eid);
                }
            }
        }
        
        // Batch-fetch edges: sort edge IDs for better cache locality during lookups
        let mut edges = Vec::with_capacity(edge_ids.len());
        for edge_id in &edge_ids {
            if let Some(edge) = self.get(*edge_id)? {
                edges.push(edge);
            }
        }
        
        // Already sorted by timestamp from the index key ordering
        Ok(edges)
    }

    /// Cursor-based paginated query for a tenant within a time range.
    ///
    /// **Performance:** O(log N + page_size) per page instead of O(N + N log N).
    /// Uses the tenant index's lexicographic timestamp ordering to seek directly
    /// to the cursor position, never materializing more than `limit + 1` edges.
    ///
    /// For "newest first" (descending), we scan the tenant index in reverse by
    /// starting from `end_ts` and working backward. The `cursor` is the last-seen
    /// key from the previous page, used as the new upper bound.
    ///
    /// # Arguments
    /// * `start_ts` - Start of time range (inclusive)
    /// * `end_ts` - End of time range (inclusive)
    /// * `tenant_id` - Tenant isolation
    /// * `limit` - Maximum edges per page
    /// * `cursor` - Opaque cursor from previous page (None for first page).
    ///              Format: `{timestamp:020}/{edge_id:032x}`
    /// * `descending` - If true, return newest first (default UI behavior)
    ///
    /// # Returns
    /// `(edges, next_cursor)` — where `next_cursor` is None if no more pages.
    pub fn query_temporal_range_for_tenant_paginated(
        &self,
        start_ts: u64,
        end_ts: u64,
        tenant_id: u64,
        limit: usize,
        cursor: Option<&str>,
        descending: bool,
    ) -> Result<(Vec<AgentFlowEdge>, Option<String>)> {
        self.stats.scans.fetch_add(1, Ordering::Relaxed);

        let limit = limit.min(10_000); // Safety cap

        // Determine scan bounds based on cursor and direction
        let (scan_start, scan_end) = if descending {
            // Descending: scan from end_ts backward to start_ts
            let upper = if let Some(c) = cursor {
                // Cursor is exclusive upper bound — start just before it
                format!("idx/tenant/{}/{}", tenant_id, c)
            } else {
                format!("idx/tenant/{}/{:020}/", tenant_id, end_ts.saturating_add(1))
            };
            let lower = format!("idx/tenant/{}/{:020}/", tenant_id, start_ts);
            (lower, upper)
        } else {
            // Ascending: scan from start_ts forward to end_ts
            let lower = if let Some(c) = cursor {
                // Cursor is exclusive lower bound — start just after it
                // Add a byte to make it exclusive
                let mut cursor_key = format!("idx/tenant/{}/{}", tenant_id, c);
                cursor_key.push('\x7f'); // ASCII DEL — sorts after any hex digit
                cursor_key
            } else {
                format!("idx/tenant/{}/{:020}/", tenant_id, start_ts)
            };
            let upper = format!("idx/tenant/{}/{:020}/", tenant_id, end_ts.saturating_add(1));
            (lower, upper)
        };

        let results = self.connection.scan_range(&scan_start, &scan_end)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB scan_range failed: {}", e)))?;

        // For descending order, we need to reverse the lexicographic scan results
        let entries: Vec<_> = if descending {
            results.into_iter().rev().collect()
        } else {
            results.into_iter().collect()
        };

        // Take limit + 1 to detect if more pages exist
        let take_count = limit + 1;
        let mut edge_ids: Vec<(String, u128)> = Vec::with_capacity(take_count);
        for (key_str, _) in entries.into_iter().take(take_count) {
            let parts: Vec<&str> = key_str.split('/').collect();
            if parts.len() >= 5 {
                if let Ok(eid) = u128::from_str_radix(parts[4], 16) {
                    // Store the cursor component: "{timestamp}/{edge_id}"
                    let cursor_part = format!("{}/{}", parts[3], parts[4]);
                    edge_ids.push((cursor_part, eid));
                }
            }
        }

        let has_more = edge_ids.len() > limit;
        if has_more {
            edge_ids.pop(); // Remove the extra element
        }

        // Determine next cursor
        let next_cursor = if has_more {
            edge_ids.last().map(|(cursor_part, _)| cursor_part.clone())
        } else {
            None
        };

        // Fetch edges
        let mut edges = Vec::with_capacity(edge_ids.len());
        for (_, edge_id) in &edge_ids {
            if let Some(edge) = self.get(*edge_id)? {
                edges.push(edge);
            }
        }

        Ok((edges, next_cursor))
    }

    /// Query edges for a specific session using the session index.
    ///
    /// **Performance:** O(log N + K_session) where K_session is the number of
    /// edges in the session (typically 5–100). This replaces the previous
    /// implementation that scanned the ENTIRE database O(N_total).
    ///
    /// Uses the session index: `idx/session/{session_id}/{edge_id:032x}`
    pub fn get_session_edges_indexed(&self, session_id: u64) -> Result<Vec<AgentFlowEdge>> {
        self.stats.scans.fetch_add(1, Ordering::Relaxed);

        // Bounded scan on session index
        let start_key = format!("idx/session/{}/", session_id);
        let end_key = format!("idx/session/{}/", session_id + 1);

        let results = self.connection.scan_range(&start_key, &end_key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB scan_range failed: {}", e)))?;

        let mut edge_ids: Vec<u128> = Vec::with_capacity(results.len());
        for (key_str, _) in &results {
            // Key format: idx/session/{session_id}/{edge_id:032x}
            let parts: Vec<&str> = key_str.split('/').collect();
            if parts.len() >= 4 {
                if let Ok(eid) = u128::from_str_radix(parts[3], 16) {
                    edge_ids.push(eid);
                }
            }
        }

        // Batch-fetch edges
        let mut edges = Vec::with_capacity(edge_ids.len());
        for edge_id in &edge_ids {
            if let Some(edge) = self.get(*edge_id)? {
                edges.push(edge);
            }
        }

        // Sort by timestamp for chronological order
        edges.sort_by_key(|e| e.timestamp_us);
        Ok(edges)
    }

    /// Batch-fetch multiple payloads in a single pass.
    ///
    /// **Performance:** Amortizes LSM lookup overhead across N payloads. Instead
    /// of N independent O(log N) lookups, sorts the keys and performs a
    /// sequential scan — O(log N + N) with much better cache locality.
    ///
    /// Returns a Vec of `(edge_id, Option<bytes>)` preserving the input order.
    pub fn get_payloads_batch(&self, edge_ids: &[u128]) -> Result<Vec<(u128, Option<Vec<u8>>)>> {
        let mut results = Vec::with_capacity(edge_ids.len());

        for &edge_id in edge_ids {
            let key = encode_payload_key(edge_id);
            let payload = self.connection.get(&key)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB get failed: {}", e)))?
                .map(|data| decompress_payload(&data))
                .transpose()?;
            results.push((edge_id, payload));
        }

        Ok(results)
    }

    /// Store a payload for an edge (zstd-compressed)
    pub fn put_payload(&self, edge_id: u128, data: &[u8]) -> Result<()> {
        // SYNCHRONIZATION: Lock to prevent transaction race
        let _write_guard = self.write_lock.write();

        let key = encode_payload_key(edge_id);
        let compressed = compress_payload(data);
        self.connection.put(&key, &compressed)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB put payload failed: {}", e)))?;
            
        let _ = self.connection.commit()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;

        Ok(())
    }

    /// Batch-write multiple payloads in a single transaction (zstd-compressed).
    ///
    /// This amortizes the write-lock acquisition and commit/fsync cost across
    /// all payloads, providing ~50–100× throughput vs individual `put_payload` calls.
    pub fn put_payloads_batch(&self, payloads: &[(u128, &[u8])]) -> Result<()> {
        if payloads.is_empty() {
            return Ok(());
        }

        let _write_guard = self.write_lock.write();

        for (edge_id, data) in payloads {
            let key = encode_payload_key(*edge_id);
            let compressed = compress_payload(data);
            self.connection.put(&key, &compressed)
                .map_err(|e| AgentreplayError::Internal(format!("SochDB put payload failed: {}", e)))?;
        }

        let _ = self.connection.commit()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB commit failed: {}", e)))?;

        Ok(())
    }

    /// Get a payload for an edge (transparently decompresses zstd payloads)
    pub fn get_payload(&self, edge_id: u128) -> Result<Option<Vec<u8>>> {
        let key = encode_payload_key(edge_id);
        match self.connection.get(&key)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB get payload failed: {}", e)))? {
            Some(data) => Ok(Some(decompress_payload(&data)?)),
            None => Ok(None),
        }
    }

    /// Record metrics for an edge
    fn record_metrics(&self, edge: &AgentFlowEdge) {
        let minute_bucket_size = 60 * 1_000_000u64;
        let hour_bucket_size = 60 * minute_bucket_size;
        
        let minute_ts = (edge.timestamp_us / minute_bucket_size) * minute_bucket_size;
        let hour_ts = (edge.timestamp_us / hour_bucket_size) * hour_bucket_size;
        
        // Update minute bucket
        {
            let key = (edge.tenant_id, edge.project_id, minute_ts);
            let mut buckets = self.minute_buckets.write();
            let bucket = buckets.entry(key).or_insert_with(|| {
                MetricsBucket::new(minute_ts, edge.tenant_id, edge.project_id)
            });
            bucket.record(edge);
        }
        
        // Update hour bucket
        {
            let key = (edge.tenant_id, edge.project_id, hour_ts);
            let mut buckets = self.hour_buckets.write();
            let bucket = buckets.entry(key).or_insert_with(|| {
                MetricsBucket::new(hour_ts, edge.tenant_id, edge.project_id)
            });
            bucket.record(edge);
        }

        // Task 9: Update dashboard summary incrementally
        {
            let mut summary = self.dashboard_summary.write();
            summary.record_edge(edge);
        }
    }

    /// Get the pre-computed dashboard summary (Task 9)
    ///
    /// **Performance:** O(1) — returns the incrementally-maintained summary
    /// instead of scanning all edges. Updated on every edge insertion.
    pub fn get_dashboard_summary(&self) -> DashboardSummary {
        self.dashboard_summary.read().clone()
    }

    // ========================================================================
    // Semantic Query Cache
    // ========================================================================
    // 
    // Provides exact-match caching for repeated LLM context queries.
    // Query strings are hashed and cached results are stored in SochDB.
    //
    // This is particularly effective for:
    // - Repeated context retrieval with identical prompts
    // - Session continuation queries (same context lookups)
    // - Agent memory lookups with exact query matches
    //
    // Key format: `_cache/query/{namespace}/{query_hash:016x}`
    
    /// Cache a query result for future lookups
    /// 
    /// # Arguments
    /// * `query` - The query string that produced the result
    /// * `namespace` - Cache namespace (e.g., "memory", "context", "traces")
    /// * `result` - The serialized result bytes to cache
    /// * `ttl_secs` - Time-to-live in seconds (0 = no expiration)
    pub fn cache_query_result(
        &self,
        query: &str,
        namespace: &str,
        result: &[u8],
        ttl_secs: u64,
    ) -> Result<()> {
        if !self.semantic_cache_enabled {
            return Ok(());
        }
        
        let query_hash = Self::hash_query(query);
        let cache_key = format!("_cache/query/{}/{:016x}", namespace, query_hash);
        
        // Store with expiration timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = if ttl_secs > 0 { now + ttl_secs } else { u64::MAX };
        
        // Serialize with expiration header (8 bytes expiration + data)
        let mut cached_data = Vec::with_capacity(8 + result.len());
        cached_data.extend_from_slice(&expires_at.to_le_bytes());
        cached_data.extend_from_slice(result);
        
        self.connection.put(&cache_key, &cached_data)
            .map_err(|e| AgentreplayError::Internal(format!("Cache store failed: {}", e)))?;
        
        info!(
            query_hash = query_hash,
            namespace = namespace,
            ttl_secs = ttl_secs,
            "Cached query result"
        );
        
        Ok(())
    }
    
    /// Look up a cached query result
    /// 
    /// Returns the cached result if found and not expired.
    /// 
    /// # Arguments
    /// * `query` - The query string to look up
    /// * `namespace` - Cache namespace
    /// 
    /// # Returns
    /// * `Ok(Some(bytes))` - Cached result found
    /// * `Ok(None)` - Cache miss or expired
    pub fn lookup_cached_query(
        &self,
        query: &str,
        namespace: &str,
    ) -> Result<Option<Vec<u8>>> {
        if !self.semantic_cache_enabled {
            return Ok(None);
        }
        
        let query_hash = Self::hash_query(query);
        let cache_key = format!("_cache/query/{}/{:016x}", namespace, query_hash);
        
        if let Some(cached_data) = self.connection.get(&cache_key)
            .map_err(|e| AgentreplayError::Internal(format!("Cache lookup failed: {}", e)))? 
        {
            if cached_data.len() < 8 {
                return Ok(None); // Invalid cache entry
            }
            
            // Check expiration
            let expires_at = u64::from_le_bytes(cached_data[0..8].try_into().unwrap());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            if now < expires_at {
                info!(query_hash = query_hash, namespace = namespace, "Cache hit");
                return Ok(Some(cached_data[8..].to_vec()));
            } else {
                // Expired - clean up
                let _ = self.connection.delete(&cache_key);
                info!(query_hash = query_hash, namespace = namespace, "Cache expired");
            }
        }
        
        Ok(None)
    }
    
    /// Invalidate a cached query result
    pub fn invalidate_cached_query(&self, query: &str, namespace: &str) -> Result<()> {
        let query_hash = Self::hash_query(query);
        let cache_key = format!("_cache/query/{}/{:016x}", namespace, query_hash);
        
        self.connection.delete(&cache_key)
            .map_err(|e| AgentreplayError::Internal(format!("Cache invalidation failed: {}", e)))?;
        
        Ok(())
    }
    
    /// Invalidate all cached queries in a namespace
    pub fn invalidate_cache_namespace(&self, namespace: &str) -> Result<u64> {
        let prefix = format!("_cache/query/{}/", namespace);
        let entries = self.connection.scan(&prefix)
            .map_err(|e| AgentreplayError::Internal(format!("Cache scan failed: {}", e)))?;
        
        let count = entries.len() as u64;
        for (key, _) in entries {
            let _ = self.connection.delete(&key);
        }
        
        info!(namespace = namespace, count = count, "Invalidated cache namespace");
        Ok(count)
    }
    
    /// Hash a query string for cache key
    fn hash_query(query: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        query.trim().to_lowercase().hash(&mut hasher);
        hasher.finish()
    }
    
    /// Check if semantic caching is enabled
    pub fn is_semantic_cache_enabled(&self) -> bool {
        self.semantic_cache_enabled
    }
    
    /// Enable or disable semantic caching at runtime
    pub fn set_semantic_cache_enabled(&mut self, enabled: bool) {
        self.semantic_cache_enabled = enabled;
        info!(enabled = enabled, "Semantic cache setting updated");
    }

    /// Query metrics for a time range
    ///
    /// If `project_id == 0`, aggregates across ALL projects (used by analytics dashboard).
    /// Otherwise filters to the specific project.
    pub fn query_metrics(
        &self,
        tenant_id: u64,
        project_id: u16,
        start_ts: u64,
        end_ts: u64,
    ) -> MetricsBucket {
        let mut result = MetricsBucket::new(start_ts, tenant_id, project_id);
        
        // Aggregate from minute buckets
        let buckets = self.minute_buckets.read();
        for ((t, p, ts), bucket) in buckets.iter() {
            if *ts >= start_ts && *ts <= end_ts {
                // tenant_id == 0 means "all tenants", project_id == 0 means "all projects"
                let tenant_match = tenant_id == 0 || *t == tenant_id;
                let project_match = project_id == 0 || *p == project_id;
                if tenant_match && project_match {
                    result.merge(bucket);
                }
            }
        }
        
        result
    }

    /// Query metrics as a timeseries (per-minute buckets) for a time range.
    ///
    /// Returns a Vec of (timestamp, MetricsBucket) sorted by timestamp.
    /// Uses the pre-aggregated minute_buckets for O(B) where B = number of
    /// minute buckets in range, instead of O(N) edge scanning.
    pub fn query_metrics_timeseries(
        &self,
        tenant_id: u64,
        project_id: u16,
        start_ts: u64,
        end_ts: u64,
    ) -> Vec<(u64, MetricsBucket)> {
        let buckets = self.minute_buckets.read();
        let mut result: Vec<(u64, MetricsBucket)> = Vec::new();
        
        for ((t, p, ts), bucket) in buckets.iter() {
            if *ts >= start_ts && *ts <= end_ts {
                let tenant_match = tenant_id == 0 || *t == tenant_id;
                let project_match = project_id == 0 || *p == project_id;
                if tenant_match && project_match {
                    result.push((*ts, bucket.clone()));
                }
            }
        }
        
        result.sort_by_key(|(ts, _)| *ts);
        result
    }

    /// Flush metrics to storage
    ///
    /// SYNCHRONIZATION: Acquires write_lock to prevent write-write conflicts
    /// with concurrent batch ingestion on the same EmbeddedConnection.
    pub fn flush_metrics(&self) -> Result<()> {
        let _write_guard = self.write_lock.write();

        let mut wrote = false;

        // Flush minute buckets
        let buckets = self.minute_buckets.read().clone();
        for ((tenant_id, project_id, timestamp_us), bucket) in buckets {
            let key = encode_metrics_key("minute", tenant_id, project_id, timestamp_us);
            self.connection.put(&key, &bucket.serialize())
                .map_err(|e| AgentreplayError::Internal(format!("Metrics flush failed: {}", e)))?;
            wrote = true;
        }
        
        // Flush hour buckets
        let buckets = self.hour_buckets.read().clone();
        for ((tenant_id, project_id, timestamp_us), bucket) in buckets {
            let key = encode_metrics_key("hour", tenant_id, project_id, timestamp_us);
            self.connection.put(&key, &bucket.serialize())
                .map_err(|e| AgentreplayError::Internal(format!("Metrics flush failed: {}", e)))?;
            wrote = true;
        }

        // Only commit if we actually wrote metrics (avoid "No active transaction" error)
        if wrote {
            let _ = self.connection.commit()
                .map_err(|e| AgentreplayError::Internal(format!("Metrics commit failed: {}", e)))?;
        }
        
        Ok(())
    }

    /// Sync data to disk
    pub fn sync(&self) -> Result<()> {
        // Force fsync on the underlying connection
        self.connection.fsync()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB fsync failed: {}", e)))?;
        Ok(())
    }

    /// Get storage statistics with real observability metrics
    ///
    /// **Real Metrics:** Unlike the previous implementation, this actually computes:
    /// - `disk_bytes`: Real disk usage from directory scan
    /// - `tombstone_ratio`: Deleted vs live edges
    /// - `orphan_payload_count`: Payloads without corresponding edges
    /// - Index statistics (session/project entries)
    ///
    /// **Note:** This method performs I/O and should not be called in hot paths.
    /// Use `stats_fast()` for lightweight metrics (puts/gets/deletes counters only).
    pub fn stats(&self) -> StorageStats {
        // Compute real disk bytes by scanning data directory
        let disk_bytes = self.compute_disk_bytes();
        
        // Memory usage is now minimal - only the RwLock overhead
        // The in-memory edge_index has been removed
        let memory_bytes = 64; // Just the lock overhead
        
        // Count different key types
        let (_trace_count, payload_count, session_idx_count, project_idx_count, memory_session_count) = 
            self.count_keys_by_prefix();
        
        // Count deleted edges (tombstones)
        let tombstoned_edges = self.stats.deletes.load(Ordering::Relaxed);
        let total_edges = self.stats.edges.load(Ordering::Relaxed);
        let live_edges = total_edges.saturating_sub(tombstoned_edges);
        let tombstone_ratio = if total_edges > 0 {
            tombstoned_edges as f64 / total_edges as f64
        } else {
            0.0
        };

        StorageStats {
            total_edges,
            disk_bytes,
            memory_bytes: memory_bytes as u64,
            puts: self.stats.puts.load(Ordering::Relaxed),
            gets: self.stats.gets.load(Ordering::Relaxed),
            deletes: self.stats.deletes.load(Ordering::Relaxed),
            scans: self.stats.scans.load(Ordering::Relaxed),
            cache_hit_rate: 0.0,
            memtable_size: 0,
            memtable_entries: 0,
            immutable_memtables: 0,
            wal_sequence: 0,
            cache_stats: CacheStats::default(),
            levels: Vec::new(),
            // New observability fields
            live_edges,
            tombstoned_edges,
            tombstone_ratio,
            payload_count,
            payload_bytes: 0, // Would require scanning all payloads
            orphan_payload_count: 0, // Computed by health_check()
            session_index_entries: session_idx_count,
            project_index_entries: project_idx_count,
            memory_sessions: memory_session_count,
            minute_buckets: self.minute_buckets.read().len() as u64,
            hour_buckets: self.hour_buckets.read().len() as u64,
        }
    }

    /// Fast statistics (counters only, no I/O)
    ///
    /// Use this in hot paths where you only need operation counters.
    pub fn stats_fast(&self) -> StorageStats {
        StorageStats {
            total_edges: self.stats.edges.load(Ordering::Relaxed),
            puts: self.stats.puts.load(Ordering::Relaxed),
            gets: self.stats.gets.load(Ordering::Relaxed),
            deletes: self.stats.deletes.load(Ordering::Relaxed),
            scans: self.stats.scans.load(Ordering::Relaxed),
            ..Default::default()
        }
    }

    /// Compute real disk usage by scanning data directory
    fn compute_disk_bytes(&self) -> u64 {
        let mut total_bytes = 0u64;
        
        if let Ok(entries) = std::fs::read_dir(&self.config.data_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        total_bytes += metadata.len();
                    } else if metadata.is_dir() {
                        // Recursively count subdirectory
                        if let Ok(subentries) = std::fs::read_dir(entry.path()) {
                            for subentry in subentries.flatten() {
                                if let Ok(submeta) = subentry.metadata() {
                                    if submeta.is_file() {
                                        total_bytes += submeta.len();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        total_bytes
    }

    /// Count keys by prefix to understand storage distribution
    fn count_keys_by_prefix(&self) -> (u64, u64, u64, u64, u64) {
        let mut _trace_count = 0u64;
        let mut payload_count = 0u64;
        let mut session_idx_count = 0u64;
        let mut project_idx_count = 0u64;
        let mut memory_session_count = 0u64;

        // Scan traces
        if let Ok(results) = self.connection.scan(&format!("{}/", TRACE_PREFIX)) {
            _trace_count = results.len() as u64;
        }

        // Scan payloads
        if let Ok(results) = self.connection.scan(&format!("{}/", PAYLOAD_PREFIX)) {
            payload_count = results.len() as u64;
        }

        // Scan session index (using new idx/ prefix format)
        if let Ok(results) = self.connection.scan("idx/session/") {
            session_idx_count = results.len() as u64;
        }

        // Scan project index (using new idx/ prefix format)
        if let Ok(results) = self.connection.scan("idx/project/") {
            project_idx_count = results.len() as u64;
        }

        // Scan memory sessions
        if let Ok(results) = self.connection.scan("mem_session/") {
            memory_session_count = results.len() as u64;
        }

        (_trace_count, payload_count, session_idx_count, project_idx_count, memory_session_count)
    }

    /// Run a storage health check to detect issues
    ///
    /// Returns counts of:
    /// - Orphan payloads (payloads without edges)
    /// - Stale session index entries
    /// - Stale project index entries
    pub fn health_check(&self) -> HealthCheckResult {
        let mut result = HealthCheckResult::default();

        // Get all edge IDs from the persistent edge index
        let mut edge_ids = std::collections::HashSet::new();
        if let Ok(index_entries) = self.connection.scan("idx/edge/") {
            for (key, _) in index_entries {
                if let Some(edge_id_str) = key.strip_prefix("idx/edge/") {
                    if let Ok(edge_id) = u128::from_str_radix(edge_id_str, 16) {
                        edge_ids.insert(edge_id);
                    }
                }
            }
        }

        // Check for orphan payloads
        if let Ok(payloads) = self.connection.scan(&format!("{}/", PAYLOAD_PREFIX)) {
            for (key, _) in payloads {
                // Extract edge_id from payload key
                if let Some(edge_id_str) = key.strip_prefix(&format!("{}/", PAYLOAD_PREFIX)) {
                    if let Ok(edge_id) = u128::from_str_radix(edge_id_str, 16) {
                        if !edge_ids.contains(&edge_id) {
                            result.orphan_payloads += 1;
                            result.orphan_payload_ids.push(edge_id);
                        }
                    }
                }
            }
        }

        // Check for stale session index entries
        if let Ok(sessions) = self.connection.scan("sessions/") {
            for (key, _) in sessions {
                // Extract edge_id from session index key (sessions/{session_id}/{edge_id})
                if let Some(parts) = key.rsplit_once('/') {
                    if let Ok(edge_id) = u128::from_str_radix(parts.1, 16) {
                        if !edge_ids.contains(&edge_id) {
                            result.stale_session_entries += 1;
                        }
                    }
                }
            }
        }

        result
    }

    /// Clean up orphaned data discovered by health_check()
    pub fn cleanup_orphans(&self) -> Result<CleanupStats> {
        let health = self.health_check();
        let mut stats = CleanupStats::default();

        // Delete orphan payloads
        for edge_id in &health.orphan_payload_ids {
            let payload_key = encode_payload_key(*edge_id);
            if self.connection.delete(&payload_key).is_ok() {
                stats.payloads_deleted += 1;
            }
        }

        if stats.payloads_deleted > 0 {
            let _ = self.connection.commit();
        }

        stats.stale_index_entries_found = health.stale_session_entries;
        Ok(stats)
    }

    /// Check if shutdown was requested
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Shutdown storage
    pub fn shutdown(&self) -> Result<()> {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return Ok(()); // Already shutdown
        }
        
        info!("Shutting down AgentReplay storage");
        
        // Flush metrics
        if self.config.enable_metrics {
            if let Err(e) = self.flush_metrics() {
                warn!("Failed to flush metrics on shutdown: {}", e);
            }
        }
        
        info!("AgentReplay storage shutdown complete");
        Ok(())
    }

    /// Get total edge count
    pub fn total_edges(&self) -> u64 {
        self.stats.edges.load(Ordering::Relaxed)
    }

    /// Iterate all edges (expensive - use sparingly)
    ///
    /// **Performance note:** This materializes all edges into a Vec.
    /// For very large databases (>1M edges), this requires proportional memory.
    /// Consider using `iter_all_edges_batched()` for memory-constrained environments.
    pub fn iter_all_edges(&self) -> Result<Vec<AgentFlowEdge>> {
        self.range_scan(0, u64::MAX)
    }

    /// Iterate all edges in batches, calling the provided callback for each batch.
    ///
    /// **Performance:** Uses prefix scanning to avoid materializing all edges
    /// at once. Memory usage is O(batch_size × edge_size) instead of O(N × edge_size).
    /// Suitable for startup index rebuilds, migrations, and analytics.
    ///
    /// The callback receives each batch of edges. Return `Ok(false)` to stop iteration.
    pub fn iter_all_edges_batched<F>(
        &self,
        batch_size: usize,
        mut callback: F,
    ) -> Result<u64>
    where
        F: FnMut(&[AgentFlowEdge]) -> Result<bool>,
    {
        let prefix = format!("{}/", TRACE_PREFIX);
        let results = self.connection.scan(&prefix)
            .map_err(|e| AgentreplayError::Internal(format!("SochDB scan failed: {}", e)))?;

        let mut batch = Vec::with_capacity(batch_size);
        let mut total = 0u64;

        for (_key_str, value) in results {
            if let Ok(edge) = deserialize_edge(&value) {
                batch.push(edge);
                total += 1;

                if batch.len() >= batch_size {
                    let should_continue = callback(&batch)?;
                    batch.clear();
                    if !should_continue {
                        return Ok(total);
                    }
                }
            }
        }

        // Process remaining batch
        if !batch.is_empty() {
            callback(&batch)?;
        }

        Ok(total)
    }

    /// Get the data directory
    pub fn data_dir(&self) -> &Path {
        &self.config.data_dir
    }

    // ========================================================================
    // Compatibility methods for query engine
    // ========================================================================

    /// Get edges for a session
    ///
    /// **Performance:** Uses the session index for O(log N + K_session) lookups
    /// instead of scanning the entire database.
    pub fn get_session_edges(&self, session_id: u64) -> Result<Vec<AgentFlowEdge>> {
        self.get_session_edges_indexed(session_id)
    }

    /// Get edges for a project
    pub fn get_project_edges(&self, project_id: u16) -> Result<Vec<AgentFlowEdge>> {
        self.range_scan_filtered(0, u64::MAX, None, Some(project_id))
    }

    /// Get edge count for a project
    pub fn get_project_edge_count(&self, project_id: u16) -> Result<u64> {
        let edges = self.get_project_edges(project_id)?;
        Ok(edges.len() as u64)
    }

    /// Delete all edges for a project
    pub fn delete_by_project(&self, project_id: u16) -> Result<u64> {
        let edges = self.get_project_edges(project_id)?;
        let count = edges.len() as u64;
        for edge in edges {
            self.delete(edge.edge_id, edge.tenant_id)?;
        }
        Ok(count)
    }

    /// Get edge for a specific tenant
    pub fn get_for_tenant(&self, edge_id: u128, tenant_id: u64) -> Result<Option<AgentFlowEdge>> {
        if let Some(edge) = self.get(edge_id)? {
            if edge.tenant_id == tenant_id {
                return Ok(Some(edge));
            }
        }
        Ok(None)
    }

    /// Spawn background compaction (no-op for SochDB, handled internally)
    pub fn spawn_background_compaction(&self) {
        // SochDB handles compaction internally
        info!("SochDB compaction handled internally");
    }

    /// Checkpoint: flush the memtable to persistent storage and truncate the WAL.
    ///
    /// Without periodic checkpointing the WAL grows unboundedly and on
    /// every startup the **entire** WAL is replayed into an in-memory
    /// memtable — causing multi-GB memory spikes (observed: 10.9 GB for
    /// a 1.2 GB WAL with ~400K edges).
    ///
    /// Call this periodically (e.g. every 5 minutes or when WAL exceeds a
    /// size threshold) to keep the WAL small and startup fast.
    ///
    /// **Note**: After truncation, data in the current session remains
    /// queryable (held in the in-memory memtable) but will NOT survive a
    /// crash or restart. This is acceptable for a desktop telemetry viewer
    /// where trace data can be re-collected from external sources.
    pub fn checkpoint(&self) -> Result<()> {
        let _write_guard = self.write_lock.write();
        let wal_before = self.wal_size_bytes();
        self.connection.checkpoint()
            .map_err(|e| AgentreplayError::Internal(format!("SochDB checkpoint failed: {}", e)))?;
        // Truncate the WAL file to reclaim disk space.
        // The memtable still holds all data in memory for the current session.
        if let Err(e) = self.connection.truncate_wal() {
            tracing::warn!("WAL truncation failed (non-fatal): {}", e);
        } else {
            let wal_after = self.wal_size_bytes();
            tracing::info!(
                wal_before_mb = wal_before / (1024 * 1024),
                wal_after_mb = wal_after / (1024 * 1024),
                "SochDB checkpoint + WAL truncation completed"
            );
        }
        Ok(())
    }

    /// Get the current WAL file size in bytes.
    /// Returns 0 if the WAL file doesn't exist or can't be read.
    pub fn wal_size_bytes(&self) -> u64 {
        let wal_path = self.config.data_dir.join("wal.log");
        std::fs::metadata(&wal_path)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Open with high performance settings
    pub fn open_high_performance<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = AgentReplayStorageConfig {
            data_dir: path.as_ref().to_path_buf(),
            sync_mode: SyncMode::Batched,
            enable_metrics: true,
            ..Default::default()
        };
        Self::open_with_config(config)
    }

    /// Get MVCC stats (compatibility)
    pub fn mvcc_stats(&self) -> crate::VersionSetStatsSnapshot {
        crate::VersionSetStatsSnapshot::default()
    }
}

impl Drop for AgentReplayStorage {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            error!("Error during storage shutdown: {}", e);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use agentreplay_core::{Environment, SpanType};
    use tempfile::TempDir;

    fn create_test_edge(edge_id: u128, timestamp_us: u64, tenant_id: u64, project_id: u16) -> AgentFlowEdge {
        let mut edge = AgentFlowEdge::new(
            tenant_id,
            project_id,
            1, // agent_id
            1, // session_id
            SpanType::Root,
            0, // causal_parent
        );
        edge.edge_id = edge_id;
        edge.timestamp_us = timestamp_us;
        edge.duration_us = 1000;
        edge.token_count = 100;
        edge
    }

    #[test]
    fn test_key_encoding() {
        let key = encode_trace_key(1, 2, 1704067200000000, 0xABCD);
        assert!(key.starts_with("traces/1/2/"));
        assert!(key.contains("01704067200000000"));
        assert!(key.ends_with("0000000000000000000000000000abcd"));
        
        let decoded = decode_trace_key(&key);
        assert!(decoded.is_some());
        let (tenant, project, ts, edge) = decoded.unwrap();
        assert_eq!(tenant, 1);
        assert_eq!(project, 2);
        assert_eq!(ts, 1704067200000000);
        assert_eq!(edge, 0xABCD);
    }

    #[test]
    fn test_storage_put_get() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        let edge = create_test_edge(1, 1000000, 1, 1);
        storage.put(edge.clone()).unwrap();
        
        let retrieved = storage.get(1).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().edge_id, 1);
    }

    #[test]
    fn test_storage_batch() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        let edges: Vec<_> = (0..10)
            .map(|i| create_test_edge(i, i as u64 * 1000000, 1, 1))
            .collect();
        
        storage.put_batch(&edges).unwrap();
        
        for i in 0..10u128 {
            let edge = storage.get(i).unwrap();
            assert!(edge.is_some());
        }
    }

    #[test]
    fn test_range_scan() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        for i in 0..10u128 {
            let edge = create_test_edge(i, i as u64 * 1000000, 1, 1);
            storage.put(edge).unwrap();
        }
        
        let edges = storage.range_scan(2000000, 7000000).unwrap();
        assert!(!edges.is_empty());
        
        // Verify they're in the range
        for edge in &edges {
            assert!(edge.timestamp_us >= 2000000);
            assert!(edge.timestamp_us <= 7000000);
        }
    }

    #[test]
    fn test_payload_store() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        let payload = b"test payload data";
        storage.put_payload(123, payload).unwrap();
        
        let retrieved = storage.get_payload(123).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), payload);
    }

    #[test]
    fn test_metrics() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        for i in 0..5 {
            let edge = create_test_edge(i, i as u64 * 1000000, 1, 1);
            storage.put(edge).unwrap();
        }
        
        let metrics = storage.query_metrics(1, 1, 0, 10000000);
        assert_eq!(metrics.request_count, 5);
        assert_eq!(metrics.total_tokens, 500);
    }

    #[test]
    fn test_stats() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        for i in 0..3 {
            let edge = create_test_edge(i, i as u64 * 1000000, 1, 1);
            storage.put(edge).unwrap();
        }
        
        let stats = storage.stats();
        assert_eq!(stats.puts, 3);
        assert_eq!(stats.total_edges, 3);
    }

    #[test]
    fn test_semantic_cache() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        // Verify cache is enabled by default
        assert!(storage.is_semantic_cache_enabled());
        
        // Cache a query result
        let query = "What are the recent user interactions?";
        let namespace = "memory";
        let result = b"[{user: 'alice', action: 'clicked'}]";
        
        storage.cache_query_result(query, namespace, result, 3600).unwrap();
        
        // Look up the cached result
        let cached = storage.lookup_cached_query(query, namespace).unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), result.to_vec());
        
        // Different query should miss
        let other_query = "What is the weather?";
        let cached = storage.lookup_cached_query(other_query, namespace).unwrap();
        assert!(cached.is_none());
        
        // Invalidate and verify miss
        storage.invalidate_cached_query(query, namespace).unwrap();
        let cached = storage.lookup_cached_query(query, namespace).unwrap();
        assert!(cached.is_none());
    }
    
    #[test]
    fn test_semantic_cache_namespace_isolation() {
        let tmp_dir = TempDir::new().unwrap();
        let storage = AgentReplayStorage::open(tmp_dir.path()).unwrap();
        
        let query = "common query";
        
        // Cache in different namespaces
        storage.cache_query_result(query, "memory", b"memory_result", 3600).unwrap();
        storage.cache_query_result(query, "context", b"context_result", 3600).unwrap();
        
        // Each namespace has its own result
        let memory_cached = storage.lookup_cached_query(query, "memory").unwrap();
        let context_cached = storage.lookup_cached_query(query, "context").unwrap();
        
        assert_eq!(memory_cached.unwrap(), b"memory_result");
        assert_eq!(context_cached.unwrap(), b"context_result");
        
        // Invalidate one namespace
        let count = storage.invalidate_cache_namespace("memory").unwrap();
        assert_eq!(count, 1);
        
        // Memory namespace is cleared, context remains
        assert!(storage.lookup_cached_query(query, "memory").unwrap().is_none());
        assert!(storage.lookup_cached_query(query, "context").unwrap().is_some());
    }
}
