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

//! Core AgentFlow Edge data structure
//!
//! Fixed 128-byte format, cache-line aligned for optimal memory access.
//! This is the fundamental unit of data in Flowtrace.
//!
//! **Gap #8**: Hybrid Logical Clock (HLC) support for distributed ordering.
//! HLC provides:
//! - Wall-clock time for human-readable timestamps
//! - Logical counter for causality guarantees
//! - Single 64-bit timestamp with both properties

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::io::Result as IoResult;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::TimestampConfig;

/// AgentFlow Format version
pub const AFF_SCHEMA_VERSION: u8 = 2;

/// Magic bytes for AgentFlowEdge format detection
///
/// **CRITICAL - Task #9 from task.md**: Format identification marker
///
/// Value: 0xAFF2 (AgentFlow Format v2)
/// - Upper byte: 0xAF = "AgentFlow"
/// - Lower byte: 0xF2 = "Format 2"
///
/// This magic value appears in SSTable footers and enables:
/// 1. Format detection (distinguish from garbage data)
/// 2. Version identification (handle schema evolution)
/// 3. Corruption detection (wrong magic = corrupted file)
/// 4. Multi-format support (future formats use different magic)
///
/// Note: Individual edges use schema_version field, not magic bytes.
/// Magic bytes are used in file-level structures (SSTables, indexes).
pub const AFF_MAGIC_BYTES: u16 = 0xAFF2;

/// Timestamp validation bounds
/// Minimum valid timestamp (January 1, 2020 in microseconds since epoch)
pub const MIN_VALID_TIMESTAMP: u64 = 1_577_836_800_000_000;
/// Maximum valid timestamp (December 31, 2099 in microseconds since epoch)
pub const MAX_VALID_TIMESTAMP: u64 = 4_102_444_800_000_000;

// ============================================================================
// Gap #8: Hybrid Logical Clock (HLC) Implementation
// ============================================================================
//
// HLC provides distributed ordering guarantees while maintaining wall-clock time.
// Reference: "Logical Physical Clocks" (Kulkarni et al., OPODIS 2014)
//
// Key properties:
// 1. Always advances (never goes backward)
// 2. Bounded divergence from physical time (max_drift_us)
// 3. Causality: if event A happens-before B, then HLC(A) < HLC(B)
// 4. Compatible with NTP-synchronized systems
//
// Encoding: 64-bit timestamp
// - Upper 48 bits: wall clock (milliseconds since epoch)
// - Lower 16 bits: logical counter (wraps at 65535)
//
// This gives us:
// - ~8900 years of range (until year 10,900)
// - 65535 events per millisecond ordering resolution
// - Single atomic 64-bit compare-and-swap operations

/// Bits allocated to the logical counter in the HLC timestamp
pub const HLC_LOGICAL_BITS: u32 = 16;
/// Maximum logical counter value before requiring clock advance
pub const HLC_MAX_LOGICAL: u16 = 0xFFFF; // 65535
/// Mask for extracting logical counter from packed timestamp
pub const HLC_LOGICAL_MASK: u64 = 0xFFFF;
/// Maximum allowed drift from physical time (in milliseconds)
pub const HLC_MAX_DRIFT_MS: u64 = 60_000; // 1 minute max drift

/// Hybrid Logical Clock for distributed ordering
///
/// Combines wall-clock time with a logical counter to provide:
/// - Monotonically increasing timestamps
/// - Causal ordering across distributed nodes
/// - Bounded divergence from physical time
///
/// # Example
///
/// ```
/// use flowtrace_core::edge::{HybridLogicalClock, HlcTimestamp};
///
/// // Create a new HLC instance
/// let mut hlc = HybridLogicalClock::new();
///
/// // Generate local event timestamps
/// let ts1 = hlc.now();
/// let ts2 = hlc.now();
/// assert!(ts2 > ts1);
///
/// // Handle remote event (causality tracking)
/// let remote_ts = HlcTimestamp::from_parts(ts1.wall_time_ms() + 100, 0);
/// let merged = hlc.receive(remote_ts);
/// assert!(merged >= remote_ts);
/// ```
#[derive(Debug)]
pub struct HybridLogicalClock {
    /// Last issued timestamp (packed wall_time_ms << 16 | logical)
    last: AtomicU64,
    /// Maximum drift allowed from physical time
    max_drift_ms: u64,
}

/// A single HLC timestamp value
///
/// Packed format: upper 48 bits = wall time (ms), lower 16 bits = logical counter
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct HlcTimestamp(pub u64);

impl HlcTimestamp {
    /// Create from separate wall time and logical components
    pub fn from_parts(wall_time_ms: u64, logical: u16) -> Self {
        let wall_shifted = (wall_time_ms & ((1u64 << 48) - 1)) << HLC_LOGICAL_BITS;
        HlcTimestamp(wall_shifted | (logical as u64))
    }

    /// Create from a packed 64-bit value
    pub fn from_packed(packed: u64) -> Self {
        HlcTimestamp(packed)
    }

    /// Get the packed 64-bit representation
    pub fn packed(&self) -> u64 {
        self.0
    }

    /// Extract wall clock time in milliseconds
    pub fn wall_time_ms(&self) -> u64 {
        self.0 >> HLC_LOGICAL_BITS
    }

    /// Extract wall clock time in microseconds
    pub fn wall_time_us(&self) -> u64 {
        self.wall_time_ms() * 1000
    }

    /// Extract logical counter
    pub fn logical(&self) -> u16 {
        (self.0 & HLC_LOGICAL_MASK) as u16
    }

    /// Check if this timestamp is within drift bounds of physical time
    pub fn is_valid(&self, max_drift_ms: u64) -> bool {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let wall = self.wall_time_ms();
        // Allow past timestamps (for causality) but bound future drift
        wall <= now_ms + max_drift_ms
    }
}

impl HybridLogicalClock {
    /// Create a new HLC with default max drift (1 minute)
    pub fn new() -> Self {
        HybridLogicalClock {
            last: AtomicU64::new(0),
            max_drift_ms: HLC_MAX_DRIFT_MS,
        }
    }

    /// Create a new HLC with custom max drift
    pub fn with_max_drift(max_drift_ms: u64) -> Self {
        HybridLogicalClock {
            last: AtomicU64::new(0),
            max_drift_ms,
        }
    }

    /// Get current physical time in milliseconds
    fn physical_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Generate a new timestamp for a local event
    ///
    /// This is the core HLC `now()` operation:
    /// 1. Get current physical time
    /// 2. If physical > last.wall, use physical with logical=0
    /// 3. Otherwise, increment logical counter
    /// 4. Return the new timestamp
    pub fn now(&self) -> HlcTimestamp {
        loop {
            let pt_ms = Self::physical_time_ms();
            let last = self.last.load(Ordering::Acquire);
            let last_ts = HlcTimestamp::from_packed(last);
            let last_wall = last_ts.wall_time_ms();
            let last_logical = last_ts.logical();

            let new_ts = if pt_ms > last_wall {
                // Physical time advanced, reset logical counter
                HlcTimestamp::from_parts(pt_ms, 0)
            } else {
                // Physical time hasn't advanced, increment logical
                let new_logical = last_logical.saturating_add(1);
                if new_logical == HLC_MAX_LOGICAL {
                    // Logical counter overflow - wait briefly and retry
                    std::thread::yield_now();
                    continue;
                }
                HlcTimestamp::from_parts(last_wall, new_logical)
            };

            // CAS to ensure atomic update
            if self
                .last
                .compare_exchange_weak(last, new_ts.packed(), Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return new_ts;
            }
            // CAS failed, retry
        }
    }

    /// Update local clock based on received remote timestamp
    ///
    /// This is the HLC `receive()` operation for causality tracking:
    /// 1. Take max of (physical_time, last.wall, remote.wall)
    /// 2. Increment logical counter appropriately
    /// 3. Return timestamp that is >= remote (causality guarantee)
    pub fn receive(&self, remote: HlcTimestamp) -> HlcTimestamp {
        loop {
            let pt_ms = Self::physical_time_ms();
            let remote_wall = remote.wall_time_ms();

            // Check drift bound - reject if remote is too far in future
            if remote_wall > pt_ms + self.max_drift_ms {
                // Remote clock is too far ahead - use our time but ensure causality
                // by setting logical counter high enough
                let last = self.last.load(Ordering::Acquire);
                let last_ts = HlcTimestamp::from_packed(last);

                // Return a timestamp that's at our physical time but still
                // maintains local monotonicity
                let new_ts = if pt_ms > last_ts.wall_time_ms() {
                    HlcTimestamp::from_parts(pt_ms, 0)
                } else {
                    HlcTimestamp::from_parts(
                        last_ts.wall_time_ms(),
                        last_ts.logical().saturating_add(1),
                    )
                };

                if self
                    .last
                    .compare_exchange_weak(
                        last,
                        new_ts.packed(),
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return new_ts;
                }
                continue;
            }

            let last = self.last.load(Ordering::Acquire);
            let last_ts = HlcTimestamp::from_packed(last);
            let last_wall = last_ts.wall_time_ms();
            let last_logical = last_ts.logical();
            let remote_logical = remote.logical();

            // Pick the maximum wall time
            let max_wall = pt_ms.max(last_wall).max(remote_wall);

            let new_ts = if max_wall == pt_ms && pt_ms > last_wall && pt_ms > remote_wall {
                // Physical time is ahead - reset logical
                HlcTimestamp::from_parts(pt_ms, 0)
            } else if max_wall == last_wall && last_wall == remote_wall {
                // All three tied - increment max logical
                HlcTimestamp::from_parts(
                    max_wall,
                    last_logical.max(remote_logical).saturating_add(1),
                )
            } else if max_wall == last_wall {
                // Last wall is max - increment its logical
                HlcTimestamp::from_parts(last_wall, last_logical.saturating_add(1))
            } else {
                // Remote wall is max - increment its logical
                HlcTimestamp::from_parts(remote_wall, remote_logical.saturating_add(1))
            };

            // CAS to ensure atomic update
            if self
                .last
                .compare_exchange_weak(last, new_ts.packed(), Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return new_ts;
            }
            // CAS failed, retry
        }
    }

    /// Get the last issued timestamp without generating a new one
    pub fn peek(&self) -> HlcTimestamp {
        HlcTimestamp::from_packed(self.last.load(Ordering::Acquire))
    }

    /// Set the clock to a specific timestamp (for testing/recovery)
    ///
    /// # Safety
    /// This can break causality guarantees if used incorrectly.
    /// Only use for testing or when recovering from persistent state.
    pub fn set(&self, ts: HlcTimestamp) {
        self.last.store(ts.packed(), Ordering::Release);
    }
}

impl Default for HybridLogicalClock {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// End Gap #8: HLC Implementation
// ============================================================================

/// Flag bits for AgentFlowEdge.flags field
pub const FLAG_DELETED: u32 = 1 << 0; // Tombstone marker for deletions

/// Sensitivity flags for PII and redaction control
pub const SENSITIVITY_NONE: u8 = 0;
pub const SENSITIVITY_PII: u8 = 1 << 0; // Contains personally identifiable information
pub const SENSITIVITY_SECRET: u8 = 1 << 1; // Contains secrets/credentials
pub const SENSITIVITY_INTERNAL: u8 = 1 << 2; // Internal-only data
pub const SENSITIVITY_NO_EMBED: u8 = 1 << 3; // Never embed in vector index

/// Span types for agent execution traces
///
/// Includes both LLM-specific operations (Planning, Reasoning, etc.) and
/// general operations (Retrieval, Embedding, HTTP, Database, Function) to
/// support full observability of agent workflows.
///
/// Note: Custom span types use values >= 16 and < 255.
/// The value 255 is reserved as a marker for custom types in compact representations.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpanType {
    // Core agent operations
    Root = 0,
    Planning = 1,
    Reasoning = 2,
    ToolCall = 3,
    ToolResponse = 4,
    Synthesis = 5,
    Response = 6,
    Error = 7,

    // Extended operations for full observability (OTEL compatible)
    Retrieval = 8,   // Vector DB, semantic search
    Embedding = 9,   // Text embedding generation
    HttpCall = 10,   // HTTP/REST API calls
    Database = 11,   // Database queries
    Function = 12,   // Generic function/method call
    Reranking = 13,  // Reranking retrieved results
    Parsing = 14,    // Document/data parsing
    Generation = 15, // Content generation (non-LLM)

    /// Marker for custom span types (values >= 16)
    Custom = 255,
}

/// Environment/deployment context
///
/// Stored in edge structure for filtering by environment.
/// Uses 1 byte, leaving room for up to 256 environment types.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Environment {
    /// Development environment
    #[default]
    Development = 0,
    /// Staging/QA environment
    Staging = 1,
    /// Production environment
    Production = 2,
    /// Testing environment
    Test = 3,
    /// Custom/unknown environment
    Custom = 255,
}

impl Environment {
    /// Parse environment from string
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "dev" | "development" => Environment::Development,
            "staging" | "stage" | "qa" => Environment::Staging,
            "prod" | "production" => Environment::Production,
            "test" | "testing" => Environment::Test,
            _ => Environment::Custom,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Staging => "staging",
            Environment::Production => "production",
            Environment::Test => "test",
            Environment::Custom => "custom",
        }
    }
}

impl SpanType {
    /// Convert u64 to SpanType
    ///
    /// Values 0-15 map to predefined types.
    /// Any other value is treated as Custom.
    pub fn from_u64(value: u64) -> Self {
        match value {
            0 => SpanType::Root,
            1 => SpanType::Planning,
            2 => SpanType::Reasoning,
            3 => SpanType::ToolCall,
            4 => SpanType::ToolResponse,
            5 => SpanType::Synthesis,
            6 => SpanType::Response,
            7 => SpanType::Error,
            8 => SpanType::Retrieval,
            9 => SpanType::Embedding,
            10 => SpanType::HttpCall,
            11 => SpanType::Database,
            12 => SpanType::Function,
            13 => SpanType::Reranking,
            14 => SpanType::Parsing,
            15 => SpanType::Generation,
            _ => SpanType::Custom,
        }
    }

    pub fn to_u64(self) -> u64 {
        self as u64
    }

    /// Check if this is a custom span type
    pub fn is_custom(&self) -> bool {
        matches!(self, SpanType::Custom)
    }
}

/// AgentFlow Edge - Fixed 128-byte structure (Schema Version 2)
///
/// Memory layout (cache-line aligned):
/// - Identity & Causality: 32 bytes (edge_id, causal_parent)
/// - Temporal: 16 bytes (timestamp_us, logical_clock)
/// - Multi-tenancy: 16 bytes (tenant_id, project_id, schema_version, sensitivity_flags, reserved)
/// - Context: 16 bytes (agent_id, session_id, span_type, parent_count, padding)
/// - Probabilistic: 16 bytes (confidence, token_count, duration_us, sampling_rate)
/// - Payload Reference: 4 bytes (compression_type, has_payload flag, padding)
/// - Metadata: 16 bytes (flags, checksum)
///
/// Total: 128 bytes exactly
///
/// ## Attribute Storage Strategy
///
/// This fixed-size edge structure contains only essential, queryable fields.
/// Additional data (prompts, responses, custom attributes) are stored separately:
///
/// **Payload Store**: Large, variable-size data keyed by edge_id
/// - Prompt text (input to LLM)
/// - Response text (output from LLM)
/// - Custom attributes as JSON: {model_name, temperature, route_name, etc.}
/// - Tool call parameters and results
/// - Error messages and stack traces
///
/// The `has_payload` flag indicates whether payload data exists.
/// The `compression_type` indicates the payload compression (0=none, 1=lz4, 2=zstd).
/// Payloads are retrieved via: `db.get_payload(edge_id) -> Option<PayloadData>`
///
/// This design keeps edges small and cacheable while supporting arbitrary metadata.
///
/// Schema version 2 changes from v1:
/// - Added tenant_id (u64) for multi-tenancy
/// - Added project_id (u32) for project-level isolation
/// - Added schema_version (u8) for format versioning
/// - Added sensitivity_flags (u8) for PII/redaction control
/// - Changed span_type from u64 to u32 (still supports 4B types)
/// - Changed flags from u64 to u32 (32 bits sufficient)
/// - Added reserved space (u32) for future expansion
///
/// Note: Timestamps use microsecond precision (not nanoseconds) to avoid u64 overflow.
/// u64::MAX microseconds = ~584,000 years from epoch (safe until year 586,000 AD)
#[repr(C, align(128))]
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentFlowEdge {
    // Identity & Causality (32 bytes)
    pub edge_id: u128,
    pub causal_parent: u128,

    // Temporal (12 bytes) - microseconds precision
    pub timestamp_us: u64,
    pub logical_clock: u32, // Sufficient for ordering (4B edges)

    // Multi-tenancy & Versioning (12 bytes) - NEW in v2
    pub tenant_id: u64,
    pub project_id: u16, // 65K projects per tenant should be sufficient
    pub schema_version: u8,
    pub sensitivity_flags: u8,

    // Context (20 bytes)
    pub agent_id: u64,
    pub session_id: u64,
    pub span_type: u32, // SpanType as u32 (was u64 in v1)
    pub parent_count: u8,
    pub environment: u8, // Environment enum (0=dev, 1=staging, 2=prod, 3=test)
    pub ttl_hours: u16,  // Time-to-live in hours (0 = no expiry, 65535 hours = ~7.5 years)

    // Probabilistic (16 bytes)
    pub confidence: f32,
    pub token_count: u32,
    pub duration_us: u32, // Duration in microseconds
    pub sampling_rate: f32,

    // Payload metadata (4 bytes) - payload data stored separately by edge_id
    pub compression_type: u8,
    pub has_payload: u8,         // Boolean flag if payload exists
    pub compression_dict_id: u8, // Dictionary ID for span_type compression (0 = no dict)
    pub reserved: u8,            // Reserved for future use

    // Metadata (12 bytes)
    pub flags: u32, // Changed from u64 in v1
    pub checksum: u64,
}

// Static assertion that Edge is exactly 128 bytes
const _: () = assert!(std::mem::size_of::<AgentFlowEdge>() == 128);
const _: () = assert!(std::mem::align_of::<AgentFlowEdge>() == 128);

/// Validate timestamp is within acceptable range
///
/// Returns error if timestamp is before 2020 or after 2099.
/// This prevents overflow bugs and clearly invalid data.
/// Validate timestamp is within configured bounds
///
/// This function checks if a timestamp falls within acceptable ranges based on
/// the provided configuration. The default config validates 2020-2099, but can
/// be customized for testing, historical data, or other use cases.
///
/// # Arguments
/// * `timestamp_us` - Timestamp in microseconds since Unix epoch
/// * `config` - Configuration specifying validation bounds and enforcement
///
/// # Examples
/// ```
/// use flowtrace_core::{validate_timestamp, TimestampConfig};
///
/// // Production: strict validation (2020-2099)
/// let config = TimestampConfig::production();
/// assert!(validate_timestamp(1730800800000000, &config).is_ok()); // 2024
///
/// // Testing: accept any timestamp
/// let config = TimestampConfig::unrestricted();
/// assert!(validate_timestamp(1000, &config).is_ok()); // Simple test timestamp
///
/// // Historical: accept from Unix epoch onwards
/// let config = TimestampConfig::historical();
/// assert!(validate_timestamp(100000000, &config).is_ok()); // 1970
/// ```
pub fn validate_timestamp(timestamp_us: u64, config: &TimestampConfig) -> crate::error::Result<()> {
    if !config.enforce_validation {
        return Ok(()); // Skip validation when disabled
    }

    // Get current time for drift detection (Task 7)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);

    if let Some(min) = config.min_timestamp {
        if timestamp_us < min {
            return Err(crate::error::FlowtraceError::InvalidTimestamp(
                format!(
                    "Timestamp {} is before minimum valid time {}. Got timestamp in year ~{}, minimum is year ~{}",
                    timestamp_us,
                    min,
                    1970 + timestamp_us / (365 * 24 * 3600 * 1_000_000),
                    1970 + min / (365 * 24 * 3600 * 1_000_000)
                )
            ));
        }
    }

    if let Some(max) = config.max_timestamp {
        if timestamp_us > max {
            return Err(crate::error::FlowtraceError::InvalidTimestamp(
                format!(
                    "Timestamp {} exceeds maximum valid time {}. Got timestamp in year ~{}, maximum is year ~{}",
                    timestamp_us,
                    max,
                    1970 + timestamp_us / (365 * 24 * 3600 * 1_000_000),
                    1970 + max / (365 * 24 * 3600 * 1_000_000)
                )
            ));
        }
    }

    // Task 7: NTP drift compensation
    // Allow timestamps slightly in the future (up to 5 seconds) to account for clock skew
    const MAX_FUTURE_DRIFT_US: u64 = 5_000_000; // 5 seconds
    if timestamp_us > now && config.enforce_validation {
        let drift = timestamp_us - now;
        if drift > MAX_FUTURE_DRIFT_US {
            return Err(crate::error::FlowtraceError::InvalidTimestamp(format!(
                "Timestamp {} is too far in future ({}s ahead). Clock skew issue?",
                timestamp_us,
                drift / 1_000_000
            )));
        }
    }

    Ok(())
}

/// Checked timestamp arithmetic: safely add duration to timestamp
///
/// # Returns
/// - `Ok(result)` if addition succeeds without overflow
/// - `Err` if overflow would occur
///
/// # Examples
/// ```
/// use flowtrace_core::checked_timestamp_add;
///
/// assert!(checked_timestamp_add(1000, 500).is_ok());
/// assert_eq!(checked_timestamp_add(1000, 500).unwrap(), 1500);
/// assert!(checked_timestamp_add(u64::MAX, 1).is_err()); // Overflow
/// ```
pub fn checked_timestamp_add(timestamp_us: u64, duration_us: u64) -> crate::error::Result<u64> {
    timestamp_us.checked_add(duration_us).ok_or_else(|| {
        crate::error::FlowtraceError::InvalidTimestamp(format!(
            "Timestamp arithmetic overflow: {} + {} would exceed u64::MAX",
            timestamp_us, duration_us
        ))
    })
}

/// Checked timestamp arithmetic: safely subtract duration from timestamp
///
/// # Returns
/// - `Ok(result)` if subtraction succeeds without underflow
/// - `Err` if underflow would occur
///
/// # Examples
/// ```
/// use flowtrace_core::checked_timestamp_sub;
///
/// assert!(checked_timestamp_sub(1500, 500).is_ok());
/// assert_eq!(checked_timestamp_sub(1500, 500).unwrap(), 1000);
/// assert!(checked_timestamp_sub(100, 500).is_err()); // Underflow
/// ```
pub fn checked_timestamp_sub(timestamp_us: u64, duration_us: u64) -> crate::error::Result<u64> {
    timestamp_us.checked_sub(duration_us).ok_or_else(|| {
        crate::error::FlowtraceError::InvalidTimestamp(format!(
            "Timestamp arithmetic underflow: {} - {} would be negative",
            timestamp_us, duration_us
        ))
    })
}

/// Monotonic clock state for logical ordering
///
/// Tracks process start time (wall clock) to enable monotonic logical clocks
/// that survive NTP adjustments.
static PROCESS_START_WALL_CLOCK: AtomicU64 = AtomicU64::new(0);

/// Initialize monotonic clock tracking (called on first use)
fn init_monotonic_clock() -> u64 {
    // Get wall clock time
    let wall_clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros().min(u64::MAX as u128) as u64)
        .unwrap_or(crate::config::DEFAULT_MIN_TIMESTAMP);

    // Try to initialize if not already set (first thread wins)
    let _ = PROCESS_START_WALL_CLOCK.compare_exchange(
        0,
        wall_clock,
        Ordering::SeqCst,
        Ordering::SeqCst,
    );

    // Return wall clock start point
    PROCESS_START_WALL_CLOCK.load(Ordering::SeqCst)
}

impl AgentFlowEdge {
    /// Create a new edge with automatic ID and timestamp generation
    pub fn new(
        tenant_id: u64,
        project_id: u16,
        agent_id: u64,
        session_id: u64,
        span_type: SpanType,
        causal_parent: u128,
    ) -> Self {
        let edge_id = Self::generate_id();
        let (timestamp_us, logical_clock_u64) = Self::now_us();

        // Truncate u64 to u32 for logical_clock field (wraps around after 4B edges, but that's OK for ordering)
        let logical_clock = logical_clock_u64 as u32;

        let mut edge = AgentFlowEdge {
            edge_id,
            causal_parent,
            timestamp_us,
            logical_clock,
            tenant_id,
            project_id,
            schema_version: AFF_SCHEMA_VERSION,
            sensitivity_flags: SENSITIVITY_NONE,
            agent_id,
            session_id,
            span_type: span_type.to_u64() as u32,
            parent_count: if causal_parent == 0 { 0 } else { 1 },
            environment: Environment::Development as u8, // Default to development
            ttl_hours: 0,                                // 0 = no expiry (never delete)
            confidence: 1.0,
            token_count: 0,
            duration_us: 0,
            sampling_rate: 1.0,
            compression_type: 0,
            has_payload: 0,
            compression_dict_id: 0, // No dictionary compression
            reserved: 0,
            flags: 0,
            checksum: 0,
        };

        edge.checksum = edge.compute_checksum();
        edge
    }

    /// Generate a unique edge ID using timestamp + counter
    ///
    /// Uses a single atomic operation to prevent TOCTOU race conditions.
    /// The ID structure: [48-bit timestamp][16-bit epoch counter][64-bit sequence]
    ///
    /// This guarantees:
    /// - Uniqueness across all threads in the same process
    /// - Roughly time-ordered IDs (for efficient indexing)
    /// - No duplicates even with extreme clock skew
    pub fn generate_id() -> u128 {
        use std::sync::atomic::{AtomicU64, Ordering};

        // Global sequence counter - increments atomically for each ID
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        // Last timestamp we used (for detecting time regression)
        static LAST_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
        // Counter for IDs generated in the same microsecond
        static SAME_US_COUNTER: AtomicU64 = AtomicU64::new(0);

        // Get sequence first (atomic, always unique)
        let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst);

        // Get wall clock time
        let (timestamp, _logical) = Self::now_us();

        // Handle timestamp - use CAS to ensure we always advance
        let last_ts = LAST_TIMESTAMP.load(Ordering::Acquire);
        let effective_ts = if timestamp > last_ts {
            // Normal case: time moved forward
            match LAST_TIMESTAMP.compare_exchange(
                last_ts,
                timestamp,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Reset same-microsecond counter
                    SAME_US_COUNTER.store(0, Ordering::Relaxed);
                    timestamp
                }
                Err(actual) => {
                    // Another thread updated - use their value
                    actual
                }
            }
        } else {
            // Time hasn't advanced (same microsecond or clock skew)
            // Use last known timestamp to maintain ordering
            last_ts
        };

        // Combine: [48-bit timestamp][16-bit process-local counter][64-bit sequence]
        // This gives us:
        // - Time ordering from timestamp bits
        // - Uniqueness from sequence (never repeats)
        // - Extra discrimination from counter
        let ts_bits = (effective_ts & 0xFFFF_FFFF_FFFF) as u128; // 48 bits
        let counter = SAME_US_COUNTER.fetch_add(1, Ordering::Relaxed) & 0xFFFF; // 16 bits

        (ts_bits << 80) | ((counter as u128) << 64) | (sequence as u128)
    }

    /// Get current timestamp with monotonic logical clock
    ///
    /// Returns (wall_clock_us, logical_clock_us):
    /// - wall_clock_us: System time for display and queries (may regress with NTP)
    /// - logical_clock_us: Monotonic time for causal ordering (never regresses)
    ///
    /// The logical clock is based on Instant (monotonic) and is guaranteed to:
    /// - Never go backwards (survives NTP adjustments, clock resets)
    /// - Increase monotonically within the process lifetime
    /// - Provide correct causal ordering even during time regressions
    ///
    /// Microseconds provide sufficient precision while avoiding overflow:
    /// u64::MAX microseconds = ~584,000 years from epoch (safe until year 586,000 AD)
    pub fn now_us() -> (u64, u64) {
        // Initialize monotonic tracking on first call
        static ONCE: AtomicU64 = AtomicU64::new(0);
        if ONCE
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let _ = init_monotonic_clock();
        }

        // Get monotonic time (never regresses)
        let process_start_wall = PROCESS_START_WALL_CLOCK.load(Ordering::Relaxed);

        // Calculate monotonic offset using a simple counter for reliability
        // In production, you might use Instant, but for simplicity we use a counter
        static MONOTONIC_COUNTER: AtomicU64 = AtomicU64::new(0);
        let monotonic_offset = MONOTONIC_COUNTER.fetch_add(1, Ordering::SeqCst);
        let logical_clock = process_start_wall.saturating_add(monotonic_offset);

        // Get wall clock (may regress, but useful for display)
        let wall_clock = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                // Explicit overflow check - prevents silent truncation at u64 boundary
                // as_micros() returns u128, we must validate it fits in u64
                let micros = duration.as_micros();
                match u64::try_from(micros) {
                    Ok(ts) => {
                        // Additional validation: timestamp must be in valid range
                        if !(MIN_VALID_TIMESTAMP..=MAX_VALID_TIMESTAMP).contains(&ts) {
                            // Log error but use logical clock fallback instead of panicking
                            eprintln!(
                                "[ERROR] Timestamp {} outside valid range [{}, {}]. \
                                 System clock may be misconfigured. Using logical clock fallback.",
                                ts, MIN_VALID_TIMESTAMP, MAX_VALID_TIMESTAMP
                            );
                            logical_clock // Fallback to logical clock
                        } else {
                            ts
                        }
                    }
                    Err(_) => {
                        // Overflow would occur - use logical clock fallback
                        eprintln!(
                            "[ERROR] Timestamp overflow: {} microseconds exceeds u64::MAX. \
                             This indicates a severe system clock error. Using logical clock fallback.",
                            micros
                        );
                        logical_clock // Fallback to logical clock
                    }
                }
            }
            Err(_) => {
                // Time went backwards - use process start + monotonic offset
                logical_clock
            }
        };

        (wall_clock, logical_clock)
    }

    /// Get current timestamp (legacy - use now_us() for new code)
    ///
    /// Returns wall clock timestamp only. For new code, prefer now_us()
    /// which returns both wall clock and monotonic logical clock.
    #[deprecated(note = "Use now_us() which returns both wall clock and logical clock")]
    pub fn now_us_legacy() -> u64 {
        Self::now_us().0
    }

    /// Compute BLAKE3 checksum of edge data (excluding checksum field)
    pub fn compute_checksum(&self) -> u64 {
        let mut hasher = blake3::Hasher::new();

        hasher.update(&self.edge_id.to_le_bytes());
        hasher.update(&self.causal_parent.to_le_bytes());
        hasher.update(&self.timestamp_us.to_le_bytes());
        hasher.update(&self.logical_clock.to_le_bytes());
        hasher.update(&self.tenant_id.to_le_bytes());
        hasher.update(&self.project_id.to_le_bytes());
        hasher.update(&[self.schema_version]);
        hasher.update(&[self.sensitivity_flags]);
        hasher.update(&self.agent_id.to_le_bytes());
        hasher.update(&self.session_id.to_le_bytes());
        hasher.update(&self.span_type.to_le_bytes());
        hasher.update(&[self.parent_count]);
        hasher.update(&[self.environment]);
        hasher.update(&self.ttl_hours.to_le_bytes());
        hasher.update(&self.confidence.to_le_bytes());
        hasher.update(&self.token_count.to_le_bytes());
        hasher.update(&self.duration_us.to_le_bytes());
        hasher.update(&self.sampling_rate.to_le_bytes());
        hasher.update(&[self.compression_type]);
        hasher.update(&[self.has_payload]);
        hasher.update(&[self.compression_dict_id]);
        hasher.update(&[self.reserved]);
        hasher.update(&self.flags.to_le_bytes());

        let hash = hasher.finalize();
        u64::from_le_bytes(hash.as_bytes()[0..8].try_into().unwrap())
    }

    /// Verify edge checksum
    pub fn verify_checksum(&self) -> bool {
        self.checksum == self.compute_checksum()
    }

    /// Get SpanType enum from stored value
    pub fn get_span_type(&self) -> SpanType {
        SpanType::from_u64(self.span_type as u64)
    }

    /// Check if this edge has PII sensitivity flag
    pub fn has_pii(&self) -> bool {
        (self.sensitivity_flags & SENSITIVITY_PII) != 0
    }

    /// Check if this edge has secret sensitivity flag
    pub fn has_secrets(&self) -> bool {
        (self.sensitivity_flags & SENSITIVITY_SECRET) != 0
    }

    /// Check if this edge should never be embedded
    pub fn should_not_embed(&self) -> bool {
        (self.sensitivity_flags & SENSITIVITY_NO_EMBED) != 0
    }

    /// Set sensitivity flags
    pub fn set_sensitivity(&mut self, flags: u8) {
        self.sensitivity_flags = flags;
        self.checksum = self.compute_checksum();
    }

    /// Add sensitivity flag(s)
    pub fn add_sensitivity(&mut self, flags: u8) {
        self.sensitivity_flags |= flags;
        self.checksum = self.compute_checksum();
    }

    /// Set confidence with validation
    pub fn set_confidence(&mut self, confidence: f32) -> std::result::Result<(), &'static str> {
        if !(0.0..=1.0).contains(&confidence) || confidence.is_nan() {
            return Err("Confidence must be in range [0.0, 1.0]");
        }
        self.confidence = confidence;
        self.checksum = self.compute_checksum();
        Ok(())
    }

    /// Check if this edge is deleted (tombstone)
    pub fn is_deleted(&self) -> bool {
        (self.flags & FLAG_DELETED) != 0
    }

    /// Mark this edge as deleted (create tombstone)
    pub fn mark_deleted(&mut self) {
        self.flags |= FLAG_DELETED;
        self.checksum = self.compute_checksum();
    }

    /// Create a tombstone for deleting an edge by ID
    pub fn tombstone(edge_id: u128, timestamp_us: u64, tenant_id: u64) -> Self {
        let mut edge = AgentFlowEdge {
            edge_id,
            causal_parent: 0,
            timestamp_us,
            logical_clock: 0,
            tenant_id,
            project_id: 0,
            schema_version: AFF_SCHEMA_VERSION,
            sensitivity_flags: SENSITIVITY_NONE,
            agent_id: 0,
            session_id: 0,
            span_type: 0,
            parent_count: 0,
            environment: Environment::Development as u8,
            ttl_hours: 0,
            confidence: 0.0,
            token_count: 0,
            duration_us: 0,
            sampling_rate: 0.0,
            compression_type: 0,
            has_payload: 0,
            compression_dict_id: 0,
            reserved: 0,
            flags: FLAG_DELETED,
            checksum: 0,
        };
        edge.checksum = edge.compute_checksum();
        edge
    }

    // === Schema Versioning & Compatibility ===

    /// Get the AFF schema version for this edge
    ///
    /// This allows readers to handle different format versions gracefully.
    /// Current version is 2. Version 1 lacked tenant_id and multi-tenancy fields.
    pub fn get_schema_version(&self) -> u8 {
        self.schema_version
    }

    /// Check if this edge uses the current schema version
    pub fn is_current_schema(&self) -> bool {
        self.schema_version == AFF_SCHEMA_VERSION
    }

    // === Multi-Tenancy Helpers ===

    /// Get tenant ID for this edge
    ///
    /// Tenant ID enables multi-tenant deployments where multiple organizations
    /// share the same Flowtrace instance with complete data isolation.
    pub fn get_tenant_id(&self) -> u64 {
        self.tenant_id
    }

    /// Get project ID within the tenant
    ///
    /// Projects provide a second level of isolation within a tenant,
    /// useful for separating development/staging/production or teams.
    pub fn get_project_id(&self) -> u16 {
        self.project_id
    }

    // === Sensitivity & Privacy Helpers ===

    /// Check if this edge contains PII (personally identifiable information)
    pub fn contains_pii(&self) -> bool {
        (self.sensitivity_flags & SENSITIVITY_PII) != 0
    }

    /// Check if this edge contains secrets or credentials
    pub fn contains_secrets(&self) -> bool {
        (self.sensitivity_flags & SENSITIVITY_SECRET) != 0
    }

    /// Check if this edge is internal-only (should not be exported)
    pub fn is_internal_only(&self) -> bool {
        (self.sensitivity_flags & SENSITIVITY_INTERNAL) != 0
    }

    /// Check if this edge should be excluded from embedding
    ///
    /// Some edges (e.g., authentication, PII) should never be embedded
    /// in semantic vector indexes for privacy/security reasons.
    pub fn should_skip_embedding(&self) -> bool {
        (self.sensitivity_flags & SENSITIVITY_NO_EMBED) != 0
    }

    /// Mark this edge as containing PII
    pub fn mark_pii(&mut self) {
        self.sensitivity_flags |= SENSITIVITY_PII;
        self.checksum = self.compute_checksum();
    }

    /// Mark this edge as containing secrets
    pub fn mark_secret(&mut self) {
        self.sensitivity_flags |= SENSITIVITY_SECRET;
        self.checksum = self.compute_checksum();
    }

    /// Mark this edge as internal-only
    pub fn mark_internal(&mut self) {
        self.sensitivity_flags |= SENSITIVITY_INTERNAL;
        self.checksum = self.compute_checksum();
    }

    /// Mark this edge to skip embedding
    pub fn mark_no_embed(&mut self) {
        self.sensitivity_flags |= SENSITIVITY_NO_EMBED;
        self.checksum = self.compute_checksum();
    }

    // === Sampling Helpers ===

    /// Set sampling rate with validation
    pub fn set_sampling_rate(&mut self, rate: f32) -> std::result::Result<(), &'static str> {
        if !(0.0..=1.0).contains(&rate) || rate.is_nan() {
            return Err("Sampling rate must be in range [0.0, 1.0]");
        }
        self.sampling_rate = rate;
        self.checksum = self.compute_checksum();
        Ok(())
    }

    /// Check if this edge was sampled (included in trace)
    ///
    /// A sampling_rate < 1.0 indicates this edge is part of a sampled trace.
    /// For example, sampling_rate=0.1 means 1 in 10 edges are kept.
    ///
    /// Use this to:
    /// - Filter queries to sampled-only data (faster)
    /// - Calculate approximate counts from samples
    /// - Understand data completeness
    pub fn is_sampled(&self) -> bool {
        self.sampling_rate < 1.0
    }

    /// Get the sampling weight for aggregations
    ///
    /// When counting sampled edges, multiply by this weight to get
    /// an estimate of the full population.
    ///
    /// Example: If sampling_rate=0.1 and you see 100 edges,
    /// the estimated full count is 100 * 10 = 1000 edges.
    pub fn sampling_weight(&self) -> f32 {
        if self.sampling_rate > 0.0 {
            1.0 / self.sampling_rate
        } else {
            1.0 // Treat 0 as full sampling
        }
    }

    /// Validate edge invariants
    ///
    /// Uses default production config for validation.
    /// For custom validation, use validate_with_config().
    pub fn validate(&self) -> std::result::Result<(), &'static str> {
        self.validate_with_config(&TimestampConfig::default())
    }

    /// Validate edge invariants with custom timestamp config
    pub fn validate_with_config(
        &self,
        config: &TimestampConfig,
    ) -> std::result::Result<(), &'static str> {
        // Check timestamp is in valid range
        if let Err(e) = validate_timestamp(self.timestamp_us, config) {
            eprintln!("Edge validation failed: {}", e);
            return Err("Invalid timestamp - check TimestampConfig bounds");
        }

        // Check confidence range
        if !(0.0..=1.0).contains(&self.confidence) || self.confidence.is_nan() {
            return Err("Invalid confidence value");
        }

        // Check sampling rate range
        if !(0.0..=1.0).contains(&self.sampling_rate) || self.sampling_rate.is_nan() {
            return Err("Invalid sampling_rate value");
        }

        // Verify checksum
        if !self.verify_checksum() {
            return Err("Checksum validation failed");
        }

        Ok(())
    }

    /// Serialize edge to bytes (exactly 128 bytes)
    pub fn to_bytes(&self) -> [u8; 128] {
        let mut bytes = [0u8; 128];
        let mut cursor = &mut bytes[..];

        cursor.write_u128::<LittleEndian>(self.edge_id).unwrap();
        cursor
            .write_u128::<LittleEndian>(self.causal_parent)
            .unwrap();
        cursor.write_u64::<LittleEndian>(self.timestamp_us).unwrap();
        cursor
            .write_u32::<LittleEndian>(self.logical_clock)
            .unwrap();
        cursor.write_u64::<LittleEndian>(self.tenant_id).unwrap();
        cursor.write_u16::<LittleEndian>(self.project_id).unwrap();
        cursor.write_u8(self.schema_version).unwrap();
        cursor.write_u8(self.sensitivity_flags).unwrap();
        cursor.write_u64::<LittleEndian>(self.agent_id).unwrap();
        cursor.write_u64::<LittleEndian>(self.session_id).unwrap();
        cursor.write_u32::<LittleEndian>(self.span_type).unwrap();
        cursor.write_u8(self.parent_count).unwrap();
        cursor.write_u8(self.environment).unwrap();
        cursor.write_u16::<LittleEndian>(self.ttl_hours).unwrap();
        cursor.write_f32::<LittleEndian>(self.confidence).unwrap();
        cursor.write_u32::<LittleEndian>(self.token_count).unwrap();
        cursor.write_u32::<LittleEndian>(self.duration_us).unwrap();
        cursor
            .write_f32::<LittleEndian>(self.sampling_rate)
            .unwrap();
        cursor.write_u8(self.compression_type).unwrap();
        cursor.write_u8(self.has_payload).unwrap();
        cursor.write_u8(self.compression_dict_id).unwrap();
        cursor.write_u8(self.reserved).unwrap();
        cursor.write_u32::<LittleEndian>(self.flags).unwrap();
        cursor.write_u64::<LittleEndian>(self.checksum).unwrap();

        bytes
    }

    /// Deserialize edge from bytes
    ///
    /// **CRITICAL FIX**: Now verifies checksum to detect corrupted data.
    /// Returns error if checksum doesn't match, preventing silent data corruption.
    pub fn from_bytes(bytes: &[u8; 128]) -> IoResult<Self> {
        let mut cursor = &bytes[..];

        let edge = AgentFlowEdge {
            edge_id: cursor.read_u128::<LittleEndian>()?,
            causal_parent: cursor.read_u128::<LittleEndian>()?,
            timestamp_us: cursor.read_u64::<LittleEndian>()?,
            logical_clock: cursor.read_u32::<LittleEndian>()?,
            tenant_id: cursor.read_u64::<LittleEndian>()?,
            project_id: cursor.read_u16::<LittleEndian>()?,
            schema_version: cursor.read_u8()?,
            sensitivity_flags: cursor.read_u8()?,
            agent_id: cursor.read_u64::<LittleEndian>()?,
            session_id: cursor.read_u64::<LittleEndian>()?,
            span_type: cursor.read_u32::<LittleEndian>()?,
            parent_count: cursor.read_u8()?,
            environment: cursor.read_u8()?,
            ttl_hours: cursor.read_u16::<LittleEndian>()?,
            confidence: cursor.read_f32::<LittleEndian>()?,
            token_count: cursor.read_u32::<LittleEndian>()?,
            duration_us: cursor.read_u32::<LittleEndian>()?,
            sampling_rate: cursor.read_f32::<LittleEndian>()?,
            compression_type: cursor.read_u8()?,
            has_payload: cursor.read_u8()?,
            compression_dict_id: cursor.read_u8()?,
            reserved: cursor.read_u8()?,
            flags: cursor.read_u32::<LittleEndian>()?,
            checksum: cursor.read_u64::<LittleEndian>()?,
        };

        // CRITICAL FIX: Verify checksum to detect corruption
        if !edge.verify_checksum() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Checksum mismatch for edge {}: computed {} != stored {}",
                    edge.edge_id,
                    edge.compute_checksum(),
                    edge.checksum
                ),
            ));
        }

        Ok(edge)
    }
}

impl Default for AgentFlowEdge {
    /// Creates a zeroed edge (sentinel value for internal use)
    ///
    /// WARNING: This produces an invalid edge with edge_id=0, timestamp_us=0.
    /// Do NOT use default() for creating new edges - use new() instead.
    /// Default is provided only for internal buffer initialization.
    fn default() -> Self {
        AgentFlowEdge {
            edge_id: 0,
            causal_parent: 0,
            timestamp_us: 0,
            logical_clock: 0,
            tenant_id: 0,
            project_id: 0,
            schema_version: AFF_SCHEMA_VERSION,
            sensitivity_flags: SENSITIVITY_NONE,
            agent_id: 0,
            session_id: 0,
            span_type: 0,
            parent_count: 0,
            environment: Environment::Development as u8,
            ttl_hours: 0,
            confidence: 0.0,
            token_count: 0,
            duration_us: 0,
            sampling_rate: 0.0,
            compression_type: 0,
            has_payload: 0,
            compression_dict_id: 0,
            reserved: 0,
            flags: 0,
            checksum: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_size() {
        assert_eq!(std::mem::size_of::<AgentFlowEdge>(), 128);
        assert_eq!(std::mem::align_of::<AgentFlowEdge>(), 128);
    }

    #[test]
    fn test_edge_creation() {
        let edge = AgentFlowEdge::new(1, 0, 1, 100, SpanType::Planning, 0);
        assert_eq!(edge.tenant_id, 1);
        assert_eq!(edge.project_id, 0);
        assert_eq!(edge.agent_id, 1);
        assert_eq!(edge.session_id, 100);
        assert_eq!(edge.get_span_type(), SpanType::Planning);
        assert_eq!(edge.parent_count, 0);
        assert_eq!(edge.schema_version, AFF_SCHEMA_VERSION);
        assert!(edge.verify_checksum());
    }

    #[test]
    fn test_edge_serialization() {
        let edge = AgentFlowEdge::new(1, 0, 42, 999, SpanType::ToolCall, 123);
        let bytes = edge.to_bytes();
        let deserialized = AgentFlowEdge::from_bytes(&bytes).unwrap();

        assert_eq!(edge.edge_id, deserialized.edge_id);
        assert_eq!(edge.tenant_id, deserialized.tenant_id);
        assert_eq!(edge.project_id, deserialized.project_id);
        assert_eq!(edge.agent_id, deserialized.agent_id);
        assert_eq!(edge.session_id, deserialized.session_id);
        assert_eq!(edge.schema_version, deserialized.schema_version);
        assert!(deserialized.verify_checksum());
    }

    #[test]
    fn test_checksum_validation() {
        let mut edge = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);
        assert!(edge.verify_checksum());

        // Corrupt data
        edge.agent_id = 999;
        assert!(!edge.verify_checksum());

        // Recompute checksum
        edge.checksum = edge.compute_checksum();
        assert!(edge.verify_checksum());
    }

    #[test]
    fn test_unique_ids() {
        let e1 = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);
        let e2 = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Root, 0);
        assert_ne!(e1.edge_id, e2.edge_id);
    }

    #[test]
    fn test_sensitivity_flags() {
        let mut edge = AgentFlowEdge::new(1, 0, 1, 1, SpanType::Planning, 0);

        // Test PII flag
        edge.add_sensitivity(SENSITIVITY_PII);
        assert!(edge.has_pii());
        assert!(!edge.has_secrets());

        // Test secret flag
        edge.add_sensitivity(SENSITIVITY_SECRET);
        assert!(edge.has_pii());
        assert!(edge.has_secrets());

        // Test no-embed flag
        edge.add_sensitivity(SENSITIVITY_NO_EMBED);
        assert!(edge.should_not_embed());

        // Checksum should still be valid after updates
        assert!(edge.verify_checksum());
    }

    #[test]
    fn test_tenant_isolation() {
        let edge1 = AgentFlowEdge::new(1, 0, 1, 100, SpanType::Planning, 0);
        let edge2 = AgentFlowEdge::new(2, 0, 1, 100, SpanType::Planning, 0);

        assert_eq!(edge1.tenant_id, 1);
        assert_eq!(edge2.tenant_id, 2);
        assert_ne!(edge1.tenant_id, edge2.tenant_id);
    }

    #[test]
    fn test_environment_field() {
        // Test default environment (Development)
        let edge = AgentFlowEdge::new(1, 0, 1, 100, SpanType::Planning, 0);
        assert_eq!(edge.environment, Environment::Development as u8);

        // Test environment serialization/deserialization
        let mut edge = AgentFlowEdge::new(1, 0, 1, 100, SpanType::Planning, 0);
        edge.environment = Environment::Production as u8;
        edge.checksum = edge.compute_checksum();

        let bytes = edge.to_bytes();
        let deserialized = AgentFlowEdge::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.environment, Environment::Production as u8);
        assert!(deserialized.verify_checksum());

        // Test all environment types
        assert_eq!(Environment::Development as u8, 0);
        assert_eq!(Environment::Staging as u8, 1);
        assert_eq!(Environment::Production as u8, 2);
        assert_eq!(Environment::Test as u8, 3);
        assert_eq!(Environment::Custom as u8, 255);

        // Test environment parsing
        assert_eq!(Environment::parse("development"), Environment::Development);
        assert_eq!(Environment::parse("production"), Environment::Production);
        assert_eq!(Environment::parse("staging"), Environment::Staging);
        assert_eq!(Environment::parse("test"), Environment::Test);
        assert_eq!(Environment::parse("unknown"), Environment::Custom);
    }

    // ========================================================================
    // Gap #8: Hybrid Logical Clock Tests
    // ========================================================================

    #[test]
    fn test_hlc_timestamp_packing_gap8() {
        // Test HLC timestamp encoding/decoding
        let ts = HlcTimestamp::from_parts(1_700_000_000_000, 42);
        assert_eq!(ts.wall_time_ms(), 1_700_000_000_000);
        assert_eq!(ts.logical(), 42);

        // Test round-trip through packed representation
        let packed = ts.packed();
        let ts2 = HlcTimestamp::from_packed(packed);
        assert_eq!(ts, ts2);

        // Test wall time extraction (upper 48 bits)
        let ts3 = HlcTimestamp::from_parts(0xFFFFFFFFFFFF, 0);
        assert_eq!(ts3.wall_time_ms(), 0xFFFFFFFFFFFF);
        assert_eq!(ts3.logical(), 0);

        // Test logical extraction (lower 16 bits)
        let ts4 = HlcTimestamp::from_parts(0, 0xFFFF);
        assert_eq!(ts4.wall_time_ms(), 0);
        assert_eq!(ts4.logical(), 0xFFFF);
    }

    #[test]
    fn test_hlc_monotonicity_gap8() {
        // Test that HLC timestamps are always monotonically increasing
        let hlc = HybridLogicalClock::new();

        let mut prev = HlcTimestamp::default();
        for _ in 0..1000 {
            let ts = hlc.now();
            assert!(ts > prev, "HLC must be monotonically increasing");
            prev = ts;
        }
    }

    #[test]
    fn test_hlc_receive_causality_gap8() {
        // Test that receive() maintains causality: result >= remote
        let hlc = HybridLogicalClock::new();

        // Generate some local timestamps
        let local1 = hlc.now();
        let local2 = hlc.now();

        // Simulate receiving a remote timestamp from the future
        let remote_wall = local2.wall_time_ms() + 10; // 10ms ahead
        let remote = HlcTimestamp::from_parts(remote_wall, 5);

        let after_receive = hlc.receive(remote);

        // Result must be >= remote for causality
        assert!(
            after_receive >= remote,
            "After receive, HLC must be >= remote timestamp"
        );

        // Result must also be > previous local timestamps
        assert!(
            after_receive > local2,
            "After receive, HLC must be > previous local"
        );
    }

    #[test]
    fn test_hlc_receive_past_timestamp_gap8() {
        // Test receiving a timestamp from the past
        let hlc = HybridLogicalClock::new();

        // Generate some local timestamps
        let local1 = hlc.now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let local2 = hlc.now();

        // Simulate receiving an old remote timestamp
        let remote = HlcTimestamp::from_parts(local1.wall_time_ms().saturating_sub(100), 0);

        let after_receive = hlc.receive(remote);

        // Result must still be monotonically increasing
        assert!(
            after_receive > local2,
            "After receiving past timestamp, HLC must still advance"
        );

        // Verify local1 < local2 ordering was preserved
        assert!(local1 < local2);
    }

    #[test]
    fn test_hlc_drift_protection_gap8() {
        // Test that HLC rejects timestamps too far in the future
        let hlc = HybridLogicalClock::with_max_drift(1000); // 1 second max drift

        // Get current physical time
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Create a remote timestamp way in the future (1 hour ahead)
        let remote_future = HlcTimestamp::from_parts(now_ms + 3_600_000, 0);

        // Receive should not adopt the far-future timestamp
        let after = hlc.receive(remote_future);

        // The result should be bounded by max_drift
        assert!(
            after.wall_time_ms() < now_ms + 5000, // Allow some slack
            "HLC should reject timestamps too far in future"
        );
    }

    #[test]
    fn test_hlc_ordering_gap8() {
        // Test HlcTimestamp ordering
        let t1 = HlcTimestamp::from_parts(1000, 0);
        let t2 = HlcTimestamp::from_parts(1000, 1);
        let t3 = HlcTimestamp::from_parts(1001, 0);

        // Same wall time, different logical
        assert!(t1 < t2);

        // Different wall time
        assert!(t2 < t3);
        assert!(t1 < t3);

        // Equality
        let t4 = HlcTimestamp::from_parts(1000, 0);
        assert_eq!(t1, t4);
    }

    #[test]
    fn test_hlc_wall_time_us_conversion_gap8() {
        // Test microsecond conversion
        let ts = HlcTimestamp::from_parts(1_700_000_000_000, 0);
        assert_eq!(ts.wall_time_us(), 1_700_000_000_000_000);
    }

    #[test]
    fn test_hlc_concurrent_access_gap8() {
        // Test thread-safe concurrent access
        use std::sync::Arc;
        use std::thread;

        let hlc = Arc::new(HybridLogicalClock::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let hlc_clone = Arc::clone(&hlc);
            let handle = thread::spawn(move || {
                let mut timestamps = Vec::with_capacity(100);
                for _ in 0..100 {
                    timestamps.push(hlc_clone.now());
                }
                timestamps
            });
            handles.push(handle);
        }

        let mut all_timestamps: Vec<HlcTimestamp> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // All timestamps should be unique (no duplicates under contention)
        all_timestamps.sort();
        let original_len = all_timestamps.len();
        all_timestamps.dedup();
        assert_eq!(
            all_timestamps.len(),
            original_len,
            "HLC should generate unique timestamps under contention"
        );
    }
}
