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

use anyhow::Result;
use agentreplay_core::AgentFlowEdge;
use parking_lot::RwLock;
use rand::Rng;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error; // Using AgentFlowEdge as trace context proxy for now

#[derive(Error, Debug)]
pub enum DatasetError {
    #[error("Dataset not found")]
    NotFound,
    #[error("Invalid version: {0}")]
    InvalidVersion(String),
    #[error("Storage error: {0}")]
    StorageError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: u128,
    pub input: String,
    pub expected_output: Option<String>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedDataset {
    pub id: u128,
    pub name: String,
    pub semantic_version: Version, // 1.2.3
    pub description: String,
    pub test_cases: Vec<TestCase>,
    pub metadata: DatasetMetadata,
    pub parent_version: Option<u128>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetMetadata {
    pub author: String,
    pub tags: Vec<String>,
    pub source: DatasetSource,
    pub size: usize,
    pub stratification: Option<StratificationInfo>,
    pub quality_metrics: DatasetQualityMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatasetSource {
    Manual {
        created_by: String,
    },
    ProductionImport {
        query: ProductionImportConfig,
        imported_at: u64,
        trace_count: usize,
    },
    Synthetic {
        generator: String,
        seed: u64,
    },
    Fork {
        parent_id: u128,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratificationInfo {
    pub strategy: StratificationStrategy,
    pub buckets: HashMap<String, usize>, // category -> count
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StratificationStrategy {
    Uniform, // Equal count per category
    Weighted { weights: HashMap<String, f64> },
    Proportional, // Match production distribution
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatasetQualityMetrics {
    pub coverage: f64,       // % of production scenarios covered
    pub diversity: f64,      // Shannon entropy of categories
    pub difficulty: f64,     // Avg failure rate on this dataset
    pub staleness_days: u64, // Days since last update
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionImportConfig {
    pub name: String,
    pub created_by: String,
    pub date_range: (u64, u64),
    pub metric_filter: MetricFilter,
    pub limit: usize,
    pub stratification: Option<StratificationStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricFilter {
    pub metric: String,
    pub operator: String, // GT, LT
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct DatasetChanges {
    pub description: Option<String>,
    pub added_cases: Vec<TestCase>,
    pub removed_case_ids: Vec<u128>,
    pub modified_cases: Vec<TestCase>,
    pub is_breaking: bool,
    pub author: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetDiff {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub size_change: i64,
    pub quality_delta: DatasetQualityDelta,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetQualityDelta {
    pub coverage_change: f64,
    pub diversity_change: f64,
}

#[async_trait::async_trait]
pub trait DatasetStorage: Send + Sync {
    async fn save(&self, dataset: &ManagedDataset) -> Result<()>;
    async fn get(&self, id: u128) -> Result<ManagedDataset>;
}

pub struct DatasetManager {
    storage: Arc<dyn DatasetStorage>,
    version_control: Arc<DatasetVersionControl>,
    // db: Arc<Agentreplay>, // Simplified: passed in logic
}

impl DatasetManager {
    pub fn new(storage: Arc<dyn DatasetStorage>) -> Self {
        Self {
            storage,
            version_control: Arc::new(DatasetVersionControl),
        }
    }

    // In a real impl, we would inject a TraceQuery trait to query traces.
    // For now, assume traces are passed or we have a method to query them.

    pub async fn import_from_production(
        &self,
        config: ProductionImportConfig,
        traces: Vec<AgentFlowEdge>,
    ) -> Result<ManagedDataset> {
        // 1. Filter traces (assuming traces passed here are already filtered by DB query)

        // 2. Apply stratification
        let sampled_traces = if let Some(strategy) = &config.stratification {
            self.stratified_sample(&traces, strategy)?
        } else {
            traces.clone() // Assuming traces fits in memory for this task
        };

        // 3. Convert to test cases
        let test_cases: Vec<TestCase> = sampled_traces
            .iter()
            .map(|trace| {
                // Mock extraction - simplified as AgentFlowEdge doesn't have input/output payload
                TestCase {
                    id: generate_id(),
                    input: "input_placeholder".to_string(), // trace.input.clone().unwrap_or_default(),
                    expected_output: Some("output_placeholder".to_string()), // trace.output.clone(),
                    metadata: serde_json::json!({
                        "source_trace_id": format!("{}", trace.edge_id),
                        "imported_at": current_timestamp(),
                    }),
                    tags: vec!["production-import".to_string()],
                }
            })
            .collect();

        // 4. Create dataset
        let dataset = ManagedDataset {
            id: generate_id(),
            name: config.name.clone(),
            semantic_version: Version::new(1, 0, 0),
            description: format!("Imported {} traces from production", test_cases.len()),
            test_cases: test_cases.clone(),
            metadata: DatasetMetadata {
                author: config.created_by.clone(),
                tags: vec![
                    "production-import".to_string(),
                    config.metric_filter.metric.clone(),
                ],
                source: DatasetSource::ProductionImport {
                    query: config.clone(),
                    imported_at: current_timestamp(),
                    trace_count: sampled_traces.len(),
                },
                size: sampled_traces.len(),
                stratification: None,                              // Simplified
                quality_metrics: DatasetQualityMetrics::default(), // Simplified
            },
            parent_version: None,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
        };

        self.storage.save(&dataset).await?;
        Ok(dataset)
    }

    pub async fn create_version(
        &self,
        base_id: u128,
        changes: DatasetChanges,
    ) -> Result<ManagedDataset> {
        let base = self.storage.get(base_id).await?;

        let new_version = self
            .version_control
            .bump_version(&base.semantic_version, changes.is_breaking);

        let mut new_test_cases = base.test_cases.clone();
        new_test_cases.extend(changes.added_cases);
        new_test_cases.retain(|tc| !changes.removed_case_ids.contains(&tc.id));

        for modified in changes.modified_cases {
            if let Some(tc) = new_test_cases.iter_mut().find(|t| t.id == modified.id) {
                *tc = modified;
            }
        }

        let dataset = ManagedDataset {
            id: generate_id(),
            name: base.name.clone(),
            semantic_version: new_version,
            description: changes.description.unwrap_or(base.description),
            test_cases: new_test_cases.clone(),
            metadata: DatasetMetadata {
                author: changes.author,
                tags: base.metadata.tags.clone(),
                source: DatasetSource::Fork { parent_id: base_id },
                size: new_test_cases.len(),
                stratification: base.metadata.stratification.clone(),
                quality_metrics: DatasetQualityMetrics::default(),
            },
            parent_version: Some(base_id),
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
        };

        self.storage.save(&dataset).await?;
        Ok(dataset)
    }

    pub async fn diff(&self, v1_id: u128, v2_id: u128) -> Result<DatasetDiff> {
        let v1 = self.storage.get(v1_id).await?;
        let v2 = self.storage.get(v2_id).await?;

        let v1_ids: HashSet<u128> = v1.test_cases.iter().map(|tc| tc.id).collect();
        let v2_ids: HashSet<u128> = v2.test_cases.iter().map(|tc| tc.id).collect();

        Ok(DatasetDiff {
            added: v2_ids.difference(&v1_ids).count(),
            removed: v1_ids.difference(&v2_ids).count(),
            modified: v1
                .test_cases
                .iter()
                .filter(|tc1| {
                    v2.test_cases
                        .iter()
                        .any(|tc2| tc1.id == tc2.id && tc1.input != tc2.input)
                })
                .count(),
            size_change: v2.test_cases.len() as i64 - v1.test_cases.len() as i64,
            quality_delta: DatasetQualityDelta {
                coverage_change: v2.metadata.quality_metrics.coverage
                    - v1.metadata.quality_metrics.coverage,
                diversity_change: v2.metadata.quality_metrics.diversity
                    - v1.metadata.quality_metrics.diversity,
            },
        })
    }

    fn stratified_sample(
        &self,
        traces: &[AgentFlowEdge],
        strategy: &StratificationStrategy,
    ) -> Result<Vec<AgentFlowEdge>> {
        // Simplified implementation: just random sample
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();

        let count = match strategy {
            StratificationStrategy::Uniform => 10,
            StratificationStrategy::Weighted { .. } => 20,
            StratificationStrategy::Proportional => 15,
        };

        let sampled: Vec<_> = traces.choose_multiple(&mut rng, count).cloned().collect();
        Ok(sampled)
    }
}

pub struct DatasetVersionControl;
impl DatasetVersionControl {
    pub fn bump_version(&self, current: &Version, breaking: bool) -> Version {
        let mut new = current.clone();
        if breaking {
            new.major += 1;
            new.minor = 0;
            new.patch = 0;
        } else {
            new.minor += 1;
            new.patch = 0;
        }
        new
    }
}

// In-memory implementation
pub struct InMemoryDatasetStorage {
    datasets: RwLock<HashMap<u128, ManagedDataset>>,
}

impl Default for InMemoryDatasetStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryDatasetStorage {
    pub fn new() -> Self {
        Self {
            datasets: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl DatasetStorage for InMemoryDatasetStorage {
    async fn save(&self, dataset: &ManagedDataset) -> Result<()> {
        self.datasets.write().insert(dataset.id, dataset.clone());
        Ok(())
    }

    async fn get(&self, id: u128) -> Result<ManagedDataset> {
        self.datasets
            .read()
            .get(&id)
            .cloned()
            .ok_or(DatasetError::NotFound.into())
    }
}

fn generate_id() -> u128 {
    rand::thread_rng().gen()
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

/// LSM-based persistent dataset storage with version history
pub struct LsmDatasetStorage {
    /// Path to the datasets directory
    data_dir: std::path::PathBuf,
    /// In-memory cache of recently accessed datasets
    cache: RwLock<HashMap<u128, ManagedDataset>>,
    /// Index of all dataset IDs by name
    name_index: RwLock<HashMap<String, Vec<u128>>>,
}

impl LsmDatasetStorage {
    pub fn new(data_dir: impl Into<std::path::PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir)?;

        let storage = Self {
            data_dir,
            cache: RwLock::new(HashMap::new()),
            name_index: RwLock::new(HashMap::new()),
        };

        // Load index from disk
        storage.load_index()?;

        Ok(storage)
    }

    fn dataset_path(&self, id: u128) -> std::path::PathBuf {
        self.data_dir.join(format!("{:032x}.json", id))
    }

    fn index_path(&self) -> std::path::PathBuf {
        self.data_dir.join("_index.json")
    }

    fn load_index(&self) -> Result<()> {
        let index_path = self.index_path();
        if index_path.exists() {
            let data = std::fs::read_to_string(&index_path)?;
            let index: HashMap<String, Vec<u128>> = serde_json::from_str(&data)?;
            *self.name_index.write() = index;
        }
        Ok(())
    }

    fn save_index(&self) -> Result<()> {
        let index_path = self.index_path();
        let data = serde_json::to_string_pretty(&*self.name_index.read())?;
        std::fs::write(index_path, data)?;
        Ok(())
    }

    /// List all datasets with optional name filter
    pub fn list_datasets(&self, name_filter: Option<&str>) -> Result<Vec<ManagedDataset>> {
        let index = self.name_index.read();
        let mut datasets = Vec::new();

        for (name, ids) in index.iter() {
            if let Some(filter) = name_filter {
                if !name.contains(filter) {
                    continue;
                }
            }
            for &id in ids {
                if let Ok(dataset) = self.get_sync(id) {
                    datasets.push(dataset);
                }
            }
        }

        // Sort by updated_at descending
        datasets.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(datasets)
    }

    /// Get version history for a dataset by name
    pub fn get_version_history(&self, name: &str) -> Result<Vec<ManagedDataset>> {
        let index = self.name_index.read();
        let ids = index.get(name).cloned().unwrap_or_default();
        drop(index);

        let mut versions: Vec<ManagedDataset> = ids
            .iter()
            .filter_map(|&id| self.get_sync(id).ok())
            .collect();

        // Sort by semantic version
        versions.sort_by(|a, b| a.semantic_version.cmp(&b.semantic_version));
        Ok(versions)
    }

    /// Get the latest version of a dataset by name
    pub fn get_latest_version(&self, name: &str) -> Result<ManagedDataset> {
        let versions = self.get_version_history(name)?;
        versions
            .into_iter()
            .last()
            .ok_or_else(|| DatasetError::NotFound.into())
    }

    fn get_sync(&self, id: u128) -> Result<ManagedDataset> {
        // Check cache first
        if let Some(dataset) = self.cache.read().get(&id) {
            return Ok(dataset.clone());
        }

        // Load from disk
        let path = self.dataset_path(id);
        if !path.exists() {
            return Err(DatasetError::NotFound.into());
        }

        let data = std::fs::read_to_string(&path)?;
        let dataset: ManagedDataset = serde_json::from_str(&data)?;

        // Update cache
        self.cache.write().insert(id, dataset.clone());

        Ok(dataset)
    }

    fn save_sync(&self, dataset: &ManagedDataset) -> Result<()> {
        let path = self.dataset_path(dataset.id);
        let data = serde_json::to_string_pretty(dataset)?;
        std::fs::write(path, data)?;

        // Update cache
        self.cache.write().insert(dataset.id, dataset.clone());

        // Update name index
        self.name_index
            .write()
            .entry(dataset.name.clone())
            .or_default()
            .push(dataset.id);

        self.save_index()?;

        Ok(())
    }

    /// Delete a dataset version
    pub fn delete(&self, id: u128) -> Result<()> {
        let path = self.dataset_path(id);
        if path.exists() {
            // Remove from index
            if let Ok(dataset) = self.get_sync(id) {
                let mut index = self.name_index.write();
                if let Some(ids) = index.get_mut(&dataset.name) {
                    ids.retain(|&x| x != id);
                    if ids.is_empty() {
                        index.remove(&dataset.name);
                    }
                }
            }

            // Remove from cache
            self.cache.write().remove(&id);

            // Remove file
            std::fs::remove_file(path)?;

            self.save_index()?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl DatasetStorage for LsmDatasetStorage {
    async fn save(&self, dataset: &ManagedDataset) -> Result<()> {
        self.save_sync(dataset)
    }

    async fn get(&self, id: u128) -> Result<ManagedDataset> {
        self.get_sync(id)
    }
}
