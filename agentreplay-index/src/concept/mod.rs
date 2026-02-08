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

//! Concept Extraction and Indexing
//!
//! Extract concepts from observations and build a searchable index.
//!
//! # Key Encoding
//!
//! ```text
//! concept/{project_id}/{concept_normalized}/{observation_id}
//! ```
//!
//! # Concept Normalization
//!
//! Concepts are normalized to lowercase-hyphenated form:
//! - "User Authentication" → "user-authentication"
//! - "API_endpoint" → "api-endpoint"
//! - "HTTPClient" → "http-client"

mod extractor;
mod index;

pub use extractor::{ConceptExtractor, ConceptExtractionConfig, ConceptSource, ExtractedConcept};
pub use index::{ConceptIndex, ConceptEntry, ConceptQuery};
