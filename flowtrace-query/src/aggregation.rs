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

//! Aggregation support for analytics queries
//!
//! Provides structures for pre-computed aggregations.

use serde::{Deserialize, Serialize};

/// Aggregation key for grouping metrics
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct AggregationKey {
    pub tenant_id: u64,
    pub agg_type: AggregationType,
    pub dimensions: Vec<String>,
    pub bucket_time: u64,
}

/// Type of aggregation
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum AggregationType {
    TokenSum,
    TokenAvg,
    TraceCount,
    CostSum,
    LatencyAvg,
    ErrorCount,
}

/// Aggregated value
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregationValue {
    pub sum: f64,
    pub count: u64,
    pub min: f64,
    pub max: f64,
}

impl AggregationValue {
    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
}
