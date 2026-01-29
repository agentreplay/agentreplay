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

//! Persistent Evaluation Store
//!
//! Stores evaluation metrics and results on disk for historical analysis.
//! Uses a simple append-only log format with periodic compaction.
//! No in-memory caching - relies on SochDB's page cache for performance.

use agentreplay_core::EvalMetric;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

const EVAL_STORE_MAGIC: &[u8; 4] = b"EVAL";
const EVAL_STORE_VERSION: u32 = 1;
const ENTRY_TYPE_METRIC: u8 = 1;
const ENTRY_TYPE_SUMMARY: u8 = 2;

/// Serializable evaluation metric entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMetricEntry {
    pub edge_id: u128,
    pub metric_name: String,
    pub metric_value: f64,
    pub evaluator: String,
    pub timestamp_us: u64,
    pub passed: Option<bool>,
    pub confidence: Option<f64>,
    pub explanation: Option<String>,
}

impl From<&EvalMetric> for EvalMetricEntry {
    fn from(m: &EvalMetric) -> Self {
        Self {
            edge_id: m.edge_id,
            metric_name: m.get_metric_name().to_string(),
            metric_value: m.metric_value,
            evaluator: m.get_evaluator().to_string(),
            timestamp_us: m.timestamp_us,
            passed: None,
            confidence: None,
            explanation: None,
        }
    }
}

/// Evaluation summary for a trace
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalSummary {
    pub trace_id: u128,
    pub total_evaluations: usize,
    pub passed: usize,
    pub failed: usize,
    pub avg_score: f64,
    pub avg_confidence: f64,
    pub last_updated_us: u64,
}

/// Persistent Evaluation Store - no in-memory cache, relies on OS/SochDB caching
pub struct EvalStore {
    data_dir: PathBuf,
    log_path: PathBuf,
}

impl EvalStore {
    /// Open or create an eval store
    pub fn open(data_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)?;

        let log_path = data_dir.join("eval_metrics.log");

        // Initialize file with header if it doesn't exist
        if !log_path.exists() {
            let mut file = File::create(&log_path)?;
            file.write_all(EVAL_STORE_MAGIC)?;
            file.write_all(&EVAL_STORE_VERSION.to_le_bytes())?;
            file.flush()?;
        }

        Ok(Self { data_dir, log_path })
    }

    /// Load existing metrics from log file
    fn load_from_log(
        path: &Path,
    ) -> std::io::Result<(
        HashMap<u128, Vec<EvalMetricEntry>>,
        HashMap<u128, EvalSummary>,
    )> {
        let mut metrics: HashMap<u128, Vec<EvalMetricEntry>> = HashMap::new();
        let mut summaries: HashMap<u128, EvalSummary> = HashMap::new();

        if !path.exists() {
            return Ok((metrics, summaries));
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read header
        let mut magic = [0u8; 4];
        if reader.read_exact(&mut magic).is_err() {
            return Ok((metrics, summaries)); // Empty file
        }
        if &magic != EVAL_STORE_MAGIC {
            tracing::warn!("Invalid eval store magic, starting fresh");
            return Ok((metrics, summaries));
        }

        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        if version != EVAL_STORE_VERSION {
            tracing::warn!(
                "Eval store version mismatch ({} vs {}), starting fresh",
                version,
                EVAL_STORE_VERSION
            );
            return Ok((metrics, summaries));
        }

        // Read entries
        loop {
            let mut entry_type = [0u8; 1];
            if reader.read_exact(&mut entry_type).is_err() {
                break; // EOF
            }

            let mut len_bytes = [0u8; 4];
            if reader.read_exact(&mut len_bytes).is_err() {
                break;
            }
            let len = u32::from_le_bytes(len_bytes) as usize;

            let mut data = vec![0u8; len];
            if reader.read_exact(&mut data).is_err() {
                break;
            }

            let mut crc_bytes = [0u8; 4];
            if reader.read_exact(&mut crc_bytes).is_err() {
                break;
            }
            let stored_crc = u32::from_le_bytes(crc_bytes);
            let computed_crc = crc32fast::hash(&data);

            if stored_crc != computed_crc {
                tracing::warn!("CRC mismatch in eval store, skipping entry");
                continue;
            }

            match entry_type[0] {
                ENTRY_TYPE_METRIC => {
                    if let Ok(metric) = serde_json::from_slice::<EvalMetricEntry>(&data) {
                        metrics.entry(metric.edge_id).or_default().push(metric);
                    }
                }
                ENTRY_TYPE_SUMMARY => {
                    if let Ok(summary) = serde_json::from_slice::<EvalSummary>(&data) {
                        summaries.insert(summary.trace_id, summary);
                    }
                }
                _ => {
                    tracing::warn!("Unknown entry type {} in eval store", entry_type[0]);
                }
            }
        }

        Ok((metrics, summaries))
    }

    /// Store evaluation metrics
    pub fn store_metrics(&self, metrics: Vec<EvalMetricEntry>) -> std::io::Result<()> {
        let file = OpenOptions::new().append(true).open(&self.log_path)?;
        let mut writer = BufWriter::new(file);

        for metric in metrics {
            let data = serde_json::to_vec(&metric)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            let crc = crc32fast::hash(&data);

            writer.write_all(&[ENTRY_TYPE_METRIC])?;
            writer.write_all(&(data.len() as u32).to_le_bytes())?;
            writer.write_all(&data)?;
            writer.write_all(&crc.to_le_bytes())?;
        }
        writer.flush()?;

        Ok(())
    }

    /// Store evaluation summary
    pub fn store_summary(&self, summary: EvalSummary) -> std::io::Result<()> {
        let file = OpenOptions::new().append(true).open(&self.log_path)?;
        let mut writer = BufWriter::new(file);

        let data = serde_json::to_vec(&summary)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let crc = crc32fast::hash(&data);

        writer.write_all(&[ENTRY_TYPE_SUMMARY])?;
        writer.write_all(&(data.len() as u32).to_le_bytes())?;
        writer.write_all(&data)?;
        writer.write_all(&crc.to_le_bytes())?;
        writer.flush()?;

        Ok(())
    }

    /// Get evaluation metrics for an edge (loads from disk)
    pub fn get_metrics(&self, edge_id: u128) -> std::io::Result<Vec<EvalMetricEntry>> {
        let (metrics, _) = Self::load_from_log(&self.log_path)?;
        Ok(metrics.get(&edge_id).cloned().unwrap_or_default())
    }

    /// Get evaluation summary for a trace (loads from disk)
    pub fn get_summary(&self, trace_id: u128) -> std::io::Result<Option<EvalSummary>> {
        let (_, summaries) = Self::load_from_log(&self.log_path)?;
        Ok(summaries.get(&trace_id).cloned())
    }

    /// Get all metrics for a time range
    pub fn get_metrics_in_range(
        &self,
        start_us: u64,
        end_us: u64,
    ) -> std::io::Result<Vec<EvalMetricEntry>> {
        let (metrics, _) = Self::load_from_log(&self.log_path)?;
        Ok(metrics
            .values()
            .flatten()
            .filter(|m| m.timestamp_us >= start_us && m.timestamp_us <= end_us)
            .cloned()
            .collect())
    }

    /// Get aggregate statistics
    pub fn get_aggregate_stats(&self) -> std::io::Result<EvalAggregateStats> {
        let (metrics, summaries) = Self::load_from_log(&self.log_path)?;

        let total_metrics = metrics.values().map(|v| v.len()).sum();
        let total_traces = metrics.len();

        let mut avg_score = 0.0;
        let mut count = 0;
        for m in metrics.values().flatten() {
            avg_score += m.metric_value;
            count += 1;
        }
        if count > 0 {
            avg_score /= count as f64;
        }

        let total_passed = summaries.values().map(|s| s.passed).sum();
        let total_failed = summaries.values().map(|s| s.failed).sum();

        Ok(EvalAggregateStats {
            total_metrics,
            total_traces,
            avg_score,
            total_passed,
            total_failed,
        })
    }

    /// Compact the log by rewriting only current data
    pub fn compact(&self) -> std::io::Result<()> {
        let (metrics, summaries) = Self::load_from_log(&self.log_path)?;

        let new_path = self.data_dir.join("eval_metrics.log.new");

        {
            let file = File::create(&new_path)?;
            let mut writer = BufWriter::new(file);

            // Write header
            writer.write_all(EVAL_STORE_MAGIC)?;
            writer.write_all(&EVAL_STORE_VERSION.to_le_bytes())?;

            // Write all metrics
            for metric_list in metrics.values() {
                for metric in metric_list {
                    let data = serde_json::to_vec(metric)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    let crc = crc32fast::hash(&data);

                    writer.write_all(&[ENTRY_TYPE_METRIC])?;
                    writer.write_all(&(data.len() as u32).to_le_bytes())?;
                    writer.write_all(&data)?;
                    writer.write_all(&crc.to_le_bytes())?;
                }
            }

            // Write all summaries
            for summary in summaries.values() {
                let data = serde_json::to_vec(summary)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                let crc = crc32fast::hash(&data);

                writer.write_all(&[ENTRY_TYPE_SUMMARY])?;
                writer.write_all(&(data.len() as u32).to_le_bytes())?;
                writer.write_all(&data)?;
                writer.write_all(&crc.to_le_bytes())?;
            }

            writer.flush()?;
        }

        // Rename files
        std::fs::rename(&new_path, &self.log_path)?;

        Ok(())
    }
}

/// Aggregate evaluation statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalAggregateStats {
    pub total_metrics: usize,
    pub total_traces: usize,
    pub avg_score: f64,
    pub total_passed: usize,
    pub total_failed: usize,
}
