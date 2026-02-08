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

//! Metrics aggregation module for GenAI traces
//!
//! Provides pre-aggregated metrics for dashboards.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Metrics aggregator for GenAI operations
pub struct MetricsAggregator {
    data: Arc<RwLock<HashMap<MetricKey, MetricValue>>>,
}

/// Key for metrics aggregation
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MetricKey {
    pub tenant_id: u64,
    pub model: String,
    pub metric_name: String,
    pub bucket_time: u64,
}

/// Aggregated metric value
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricValue {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl MetricValue {
    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
}

impl MetricsAggregator {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn record_latency(&self, tenant_id: u64, model: &str, latency_us: u64) {
        let bucket_time = self.get_bucket_time();
        let key = MetricKey {
            tenant_id,
            model: model.to_string(),
            metric_name: "latency".to_string(),
            bucket_time,
        };

        let mut data = self.data.write().unwrap();
        let entry = data.entry(key).or_default();
        entry.count += 1;
        entry.sum += latency_us as f64;
        if entry.min == 0.0 || (latency_us as f64) < entry.min {
            entry.min = latency_us as f64;
        }
        if (latency_us as f64) > entry.max {
            entry.max = latency_us as f64;
        }
    }

    pub fn record_tokens(&self, tenant_id: u64, model: &str, tokens: u32) {
        let bucket_time = self.get_bucket_time();
        let key = MetricKey {
            tenant_id,
            model: model.to_string(),
            metric_name: "tokens".to_string(),
            bucket_time,
        };

        let mut data = self.data.write().unwrap();
        let entry = data.entry(key).or_default();
        entry.count += 1;
        entry.sum += tokens as f64;
        if entry.min == 0.0 || (tokens as f64) < entry.min {
            entry.min = tokens as f64;
        }
        if (tokens as f64) > entry.max {
            entry.max = tokens as f64;
        }
    }

    pub fn record_cost(&self, tenant_id: u64, model: &str, cost: f64) {
        let bucket_time = self.get_bucket_time();
        let key = MetricKey {
            tenant_id,
            model: model.to_string(),
            metric_name: "cost".to_string(),
            bucket_time,
        };

        let mut data = self.data.write().unwrap();
        let entry = data.entry(key).or_default();
        entry.count += 1;
        entry.sum += cost;
        if entry.min == 0.0 || cost < entry.min {
            entry.min = cost;
        }
        if cost > entry.max {
            entry.max = cost;
        }
    }

    fn get_bucket_time(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Bucket by hour
        (now / 3600) * 3600
    }
}

impl Default for MetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}
