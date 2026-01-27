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

//! Storage Metrics
//!
//! Comprehensive metrics for LSM-tree storage engine operations,
//! including write amplification, compaction efficiency, and I/O tracking.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Write amplification metrics for storage operations
///
/// Tracks the ratio of physical bytes written to disk vs logical bytes written by users.
/// Lower write amplification = better performance and longer SSD/HDD lifespan.
///
/// **Target thresholds for agent trace workload:**
/// - Overall WA: < 10x (leveled compaction baseline)
/// - L0→L1 WA: < 3x
/// - Li→Li+1 WA: < 5x per level
///
/// **Information-theoretic bound:** Minimum WA for LSM with k levels ≥ k
/// (proven in "The Log-Structured Merge-Tree" by O'Neil et al.)
#[derive(Debug, Clone)]
pub struct WriteAmplificationMetrics {
    /// Actual physical bytes written to disk (including compaction)
    physical_writes: Arc<AtomicU64>,

    /// Logical bytes from user writes (memtable → SSTable flushes)
    logical_writes: Arc<AtomicU64>,

    /// Per-level write tracking (for detailed analysis)
    level_writes: Arc<Vec<AtomicU64>>,

    /// Compaction-specific metrics
    compaction_bytes_read: Arc<AtomicU64>,
    compaction_bytes_written: Arc<AtomicU64>,

    /// WAL writes (for durability cost analysis)
    wal_writes: Arc<AtomicU64>,
}

impl Default for WriteAmplificationMetrics {
    fn default() -> Self {
        Self::new(7) // Default: 7 levels (L0..L6)
    }
}

impl WriteAmplificationMetrics {
    /// Create new metrics tracker with specified number of levels
    pub fn new(num_levels: usize) -> Self {
        let level_writes = (0..num_levels).map(|_| AtomicU64::new(0)).collect();

        Self {
            physical_writes: Arc::new(AtomicU64::new(0)),
            logical_writes: Arc::new(AtomicU64::new(0)),
            level_writes: Arc::new(level_writes),
            compaction_bytes_read: Arc::new(AtomicU64::new(0)),
            compaction_bytes_written: Arc::new(AtomicU64::new(0)),
            wal_writes: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record a user write (logical write)
    ///
    /// This represents bytes written by the application (memtable flush).
    pub fn record_logical_write(&self, bytes: u64) {
        self.logical_writes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record a physical write to disk
    ///
    /// This includes all disk writes: WAL, memtable flushes, compaction output.
    pub fn record_physical_write(&self, bytes: u64) {
        self.physical_writes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record write to a specific level
    ///
    /// Used for per-level write amplification analysis.
    pub fn record_level_write(&self, level: usize, bytes: u64) {
        if level < self.level_writes.len() {
            self.level_writes[level].fetch_add(bytes, Ordering::Relaxed);
        }
    }

    /// Record compaction read
    pub fn record_compaction_read(&self, bytes: u64) {
        self.compaction_bytes_read
            .fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record compaction write
    pub fn record_compaction_write(&self, bytes: u64) {
        self.compaction_bytes_written
            .fetch_add(bytes, Ordering::Relaxed);
        self.record_physical_write(bytes);
    }

    /// Record WAL write
    pub fn record_wal_write(&self, bytes: u64) {
        self.wal_writes.fetch_add(bytes, Ordering::Relaxed);
        self.record_physical_write(bytes);
    }

    /// Calculate overall write amplification factor
    ///
    /// **Formula:** WA = physical_writes / logical_writes
    ///
    /// **Interpretation:**
    /// - WA = 1.0: Perfect (no amplification)
    /// - WA = 5.0: Every 1 byte written by user causes 5 bytes written to disk
    /// - WA = 10+: High amplification (may need tuning)
    pub fn write_amplification_factor(&self) -> f64 {
        let physical = self.physical_writes.load(Ordering::Relaxed);
        let logical = self.logical_writes.load(Ordering::Relaxed);

        if logical == 0 {
            0.0
        } else {
            physical as f64 / logical as f64
        }
    }

    /// Calculate write amplification for a specific level
    ///
    /// **Formula:** WA_level_i = (bytes_written_to_i + bytes_written_to_i+1) / bytes_written_to_i
    pub fn per_level_wa(&self, level: usize) -> f64 {
        if level >= self.level_writes.len() - 1 {
            return 0.0;
        }

        let this_level = self.level_writes[level].load(Ordering::Relaxed) as f64;
        let next_level = self.level_writes[level + 1].load(Ordering::Relaxed) as f64;

        if this_level == 0.0 {
            0.0
        } else {
            (this_level + next_level) / this_level
        }
    }

    /// Calculate compaction-only write amplification
    ///
    /// **Formula:** Compaction_WA = compaction_bytes_written / compaction_bytes_read
    ///
    /// This isolates compaction overhead from user writes.
    pub fn compaction_wa(&self) -> f64 {
        let read = self.compaction_bytes_read.load(Ordering::Relaxed);
        let written = self.compaction_bytes_written.load(Ordering::Relaxed);

        if read == 0 {
            0.0
        } else {
            written as f64 / read as f64
        }
    }

    /// Get total physical writes
    pub fn total_physical_writes(&self) -> u64 {
        self.physical_writes.load(Ordering::Relaxed)
    }

    /// Get total logical writes
    pub fn total_logical_writes(&self) -> u64 {
        self.logical_writes.load(Ordering::Relaxed)
    }

    /// Get writes to a specific level
    pub fn level_writes(&self, level: usize) -> u64 {
        self.level_writes
            .get(level)
            .map(|a| a.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get total compaction bytes read
    pub fn total_compaction_read(&self) -> u64 {
        self.compaction_bytes_read.load(Ordering::Relaxed)
    }

    /// Get total compaction bytes written
    pub fn total_compaction_written(&self) -> u64 {
        self.compaction_bytes_written.load(Ordering::Relaxed)
    }

    /// Get total WAL writes
    pub fn total_wal_writes(&self) -> u64 {
        self.wal_writes.load(Ordering::Relaxed)
    }

    /// Generate comprehensive report
    pub fn report(&self) -> WriteAmplificationReport {
        WriteAmplificationReport {
            overall_wa: self.write_amplification_factor(),
            compaction_wa: self.compaction_wa(),
            total_physical: self.total_physical_writes(),
            total_logical: self.total_logical_writes(),
            wal_writes: self.total_wal_writes(),
            compaction_read: self.total_compaction_read(),
            compaction_written: self.total_compaction_written(),
            per_level_wa: (0..self.level_writes.len() - 1)
                .map(|l| self.per_level_wa(l))
                .collect(),
            per_level_bytes: (0..self.level_writes.len())
                .map(|l| self.level_writes(l))
                .collect(),
        }
    }

    /// Reset all metrics to zero
    ///
    /// **Use case:** Testing, benchmarking, or periodic metric windows
    pub fn reset(&self) {
        self.physical_writes.store(0, Ordering::Relaxed);
        self.logical_writes.store(0, Ordering::Relaxed);
        self.compaction_bytes_read.store(0, Ordering::Relaxed);
        self.compaction_bytes_written.store(0, Ordering::Relaxed);
        self.wal_writes.store(0, Ordering::Relaxed);

        for level in self.level_writes.iter() {
            level.store(0, Ordering::Relaxed);
        }
    }
}

/// Comprehensive write amplification report
#[derive(Debug, Clone)]
pub struct WriteAmplificationReport {
    /// Overall write amplification (physical / logical)
    pub overall_wa: f64,

    /// Compaction-only write amplification
    pub compaction_wa: f64,

    /// Total physical bytes written
    pub total_physical: u64,

    /// Total logical bytes written
    pub total_logical: u64,

    /// Total WAL writes
    pub wal_writes: u64,

    /// Total compaction reads
    pub compaction_read: u64,

    /// Total compaction writes
    pub compaction_written: u64,

    /// Per-level write amplification
    pub per_level_wa: Vec<f64>,

    /// Per-level bytes written
    pub per_level_bytes: Vec<u64>,
}

impl std::fmt::Display for WriteAmplificationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Write Amplification Report ===")?;
        writeln!(f, "Overall WA:       {:.2}x", self.overall_wa)?;
        writeln!(f, "Compaction WA:    {:.2}x", self.compaction_wa)?;
        writeln!(f)?;
        writeln!(
            f,
            "Total Physical:   {} bytes ({:.2} MB)",
            self.total_physical,
            self.total_physical as f64 / 1_048_576.0
        )?;
        writeln!(
            f,
            "Total Logical:    {} bytes ({:.2} MB)",
            self.total_logical,
            self.total_logical as f64 / 1_048_576.0
        )?;
        writeln!(
            f,
            "WAL Writes:       {} bytes ({:.2} MB)",
            self.wal_writes,
            self.wal_writes as f64 / 1_048_576.0
        )?;
        writeln!(f)?;
        writeln!(f, "Compaction Stats:")?;
        writeln!(
            f,
            "  Read:           {} bytes ({:.2} MB)",
            self.compaction_read,
            self.compaction_read as f64 / 1_048_576.0
        )?;
        writeln!(
            f,
            "  Written:        {} bytes ({:.2} MB)",
            self.compaction_written,
            self.compaction_written as f64 / 1_048_576.0
        )?;
        writeln!(f)?;
        writeln!(f, "Per-Level Breakdown:")?;
        for (level, (wa, bytes)) in self
            .per_level_wa
            .iter()
            .zip(self.per_level_bytes.iter())
            .enumerate()
        {
            writeln!(
                f,
                "  L{}: {:.2}x WA, {} bytes ({:.2} MB)",
                level,
                wa,
                bytes,
                *bytes as f64 / 1_048_576.0
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wa_calculation_basic() {
        let metrics = WriteAmplificationMetrics::new(3);

        // User writes 100 bytes
        metrics.record_logical_write(100);

        // Physical writes: WAL (100) + SSTable (100) = 200
        metrics.record_wal_write(100);
        metrics.record_physical_write(100);

        assert_eq!(metrics.write_amplification_factor(), 2.0);
    }

    #[test]
    fn test_compaction_wa() {
        let metrics = WriteAmplificationMetrics::new(3);

        // Compaction reads 1000 bytes from L0
        metrics.record_compaction_read(1000);

        // Compaction writes 800 bytes to L1 (some deduplication)
        metrics.record_compaction_write(800);

        assert_eq!(metrics.compaction_wa(), 0.8);
    }

    #[test]
    fn test_per_level_wa() {
        let metrics = WriteAmplificationMetrics::new(3);

        // L0 writes
        metrics.record_level_write(0, 100);

        // L1 writes (result of L0 compaction + new compactions)
        metrics.record_level_write(1, 150);

        // L0→L1 WA = (100 + 150) / 100 = 2.5x
        assert_eq!(metrics.per_level_wa(0), 2.5);
    }

    #[test]
    fn test_report_generation() {
        let metrics = WriteAmplificationMetrics::new(3);

        metrics.record_logical_write(1000);
        metrics.record_wal_write(1000);
        metrics.record_compaction_read(2000);
        metrics.record_compaction_write(1800);

        let report = metrics.report();

        assert_eq!(report.total_logical, 1000);
        assert!(report.overall_wa > 1.0);
        assert_eq!(report.compaction_wa, 0.9);
    }

    #[test]
    fn test_reset() {
        let metrics = WriteAmplificationMetrics::new(3);

        metrics.record_logical_write(100);
        metrics.record_physical_write(200);

        assert_eq!(metrics.write_amplification_factor(), 2.0);

        metrics.reset();

        assert_eq!(metrics.total_logical_writes(), 0);
        assert_eq!(metrics.total_physical_writes(), 0);
        assert_eq!(metrics.write_amplification_factor(), 0.0);
    }
}
