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
