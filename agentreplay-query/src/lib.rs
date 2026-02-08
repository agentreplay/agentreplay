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
