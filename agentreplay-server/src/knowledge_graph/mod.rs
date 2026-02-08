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

//! Knowledge Graph Module
//!
//! Implements GraphRAG-style semantic knowledge graph with:
//! - Triple extraction from trace payloads using LLM
//! - Entity resolution and normalization
//! - Leiden community detection algorithm
//! - Graph-based queries for dependency analysis
//!
//! ## Architecture
//!
//! ```text
//! Trace Payloads → Triple Extraction → Entity Resolution → Knowledge Graph
//!                       ↓                    ↓                    ↓
//!                  (LLM/Ollama)        (Normalization)      (Leiden Clusters)
//!                                                                 ↓
//!                                                          Community Summaries
//! ```
//!
//! ## Triple Format
//!
//! Triples are stored as (subject, predicate, object) tuples:
//! - `(auth.rs, DEPENDS_ON, Auth Service)`
//! - `(user_id, BREAKS, Payment Gateway)`
//! - `(Bug in auth.rs, FIXED_BY, Commit abc123)`
//!
//! ## Queries
//!
//! - "What depends on auth.rs?"
//! - "What breaks when I modify user_id?"
//! - "Show me the authentication cluster"

pub mod entities;
pub mod extractor;
pub mod graph;
pub mod leiden;
pub mod queries;

pub use entities::*;
pub use extractor::TripleExtractor;
pub use graph::SemanticGraph;
pub use leiden::LeidenClustering;
pub use queries::GraphQueryEngine;
