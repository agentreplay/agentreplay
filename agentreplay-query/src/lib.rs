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

//! Agentreplay Query Engine
//!
//! High-level API for querying agent flow data.

pub mod aggregation;
pub mod annotations;
pub mod comparison;
pub mod cost_engine;
pub mod dataset_manager;
pub mod engine;
pub mod enterprise_methods;
pub mod merge;
pub mod nl_query_parser;
pub mod prompt_manager;
pub mod retention;
pub mod semantic;
pub mod session;

pub use aggregation::{AggregationKey, AggregationType, AggregationValue};
pub use cost_engine::{CostCalculator, ModelPricing};
pub use engine::{
    DatabaseStats,
    Agentreplay,
    QueryBuilder,
    // Query complexity constants (for documentation/configuration)
    DEFAULT_QUERY_LIMIT,
    MAX_QUERY_LIMIT,
    MAX_TIME_RANGE_SECS,
};
pub use merge::KWayMerge;
pub use nl_query_parser::{NLQueryParser, ParsedQuery, QueryIntent};
pub use retention::{RetentionConfig, RetentionManager, RetentionPolicy, RetentionStats};
pub use semantic::{
    QueryFilters, SemanticQuery, SemanticSearchConfig, SemanticSearchError, SemanticSearchResult,
    TimeRange,
};
pub use session::{MessageMetadata, MessageType, SessionMessage, SessionTimeline, TimelineEvent};
