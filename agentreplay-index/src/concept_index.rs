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

//! Persistent concept index for observation retrieval.

use agentreplay_core::observation::Concept;
use agentreplay_storage::bloom::BloomFilter;
use sochdb::Connection;
use std::sync::Arc;
use thiserror::Error;

const CONCEPT_PREFIX: &str = "idx_concept";

/// Errors for concept index operations.
#[derive(Debug, Error)]
pub enum ConceptIndexError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
}

/// Secondary index for concept-based observation queries.
pub struct ConceptIndexStore {
    connection: Arc<Connection>,
    bloom: parking_lot::RwLock<BloomFilter>,
}

impl ConceptIndexStore {
    /// Create a new concept index store.
    pub fn new(connection: Arc<Connection>, expected_items: usize, false_positive_rate: f64) -> Self {
        Self {
            connection,
            bloom: parking_lot::RwLock::new(BloomFilter::new(expected_items, false_positive_rate)),
        }
    }

    /// Index an observation's concepts.
    pub fn index_observation(&self, observation_id: u128, concepts: &[Concept]) -> Result<(), ConceptIndexError> {
        for concept in concepts {
            let normalized = concept.value.as_str();
            let key = format!("{}/{}/{:032x}", CONCEPT_PREFIX, normalized, observation_id);
            self.connection
                .put(key.as_bytes(), &[])
                .map_err(|e| ConceptIndexError::Storage(e.to_string()))?;
            self.bloom.write().insert(&normalized);
        }
        Ok(())
    }

    /// Remove an observation from the index.
    pub fn remove_observation(&self, observation_id: u128, concepts: &[Concept]) -> Result<(), ConceptIndexError> {
        for concept in concepts {
            let normalized = concept.value.as_str();
            let key = format!("{}/{}/{:032x}", CONCEPT_PREFIX, normalized, observation_id);
            self.connection
                .delete(key.as_bytes())
                .map_err(|e| ConceptIndexError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// Query observation IDs for a concept.
    pub fn query_concept(&self, concept: &str) -> Result<Vec<u128>, ConceptIndexError> {
        let normalized = Concept::normalize(concept.to_string());
        if !self.bloom.read().contains(&normalized) {
            return Ok(Vec::new());
        }

        let prefix = format!("{}/{}/", CONCEPT_PREFIX, normalized);
        let results = self
            .connection
            .scan(prefix.as_bytes())
            .map_err(|e| ConceptIndexError::Storage(e.to_string()))?;

        let mut ids = Vec::with_capacity(results.len());
        for (key, _) in results {
            let key_str = String::from_utf8(key).map_err(|e| ConceptIndexError::InvalidKey(e.to_string()))?;
            if let Some(id) = parse_observation_id(&key_str) {
                ids.push(id);
            }
        }
        Ok(ids)
    }
}

fn parse_observation_id(key: &str) -> Option<u128> {
    let mut parts = key.split('/').collect::<Vec<_>>();
    let id_str = parts.pop()?;
    u128::from_str_radix(id_str, 16).ok()
}
