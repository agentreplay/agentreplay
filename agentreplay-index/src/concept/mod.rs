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
