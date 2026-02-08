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
