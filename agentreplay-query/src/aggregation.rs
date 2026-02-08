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
