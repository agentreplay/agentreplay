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

//! Concept index for efficient concept-based queries.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use parking_lot::RwLock;

/// A concept index entry linking concept to observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptEntry {
    /// Project ID.
    pub project_id: u128,
    /// Normalized concept.
    pub concept: String,
    /// Observation ID.
    pub observation_id: u128,
    /// Confidence score.
    pub confidence: f32,
    /// Source of the concept.
    pub source: String,
    /// Timestamp when indexed.
    pub indexed_at: u64,
}

impl ConceptEntry {
    /// Create storage key.
    pub fn storage_key(&self) -> String {
        format!(
            "concept/{:032x}/{}/{:032x}",
            self.project_id, self.concept, self.observation_id
        )
    }

    /// Create prefix for concept queries.
    pub fn concept_prefix(project_id: u128, concept: &str) -> String {
        format!("concept/{:032x}/{}/", project_id, concept)
    }

    /// Create prefix for project queries.
    pub fn project_prefix(project_id: u128) -> String {
        format!("concept/{:032x}/", project_id)
    }
}

/// Query options for concept lookups.
#[derive(Debug, Clone, Default)]
pub struct ConceptQuery {
    /// Project ID to search within.
    pub project_id: u128,
    /// Concepts to search for (OR query).
    pub concepts: Vec<String>,
    /// Minimum confidence score.
    pub min_confidence: Option<f32>,
    /// Maximum results.
    pub limit: Option<usize>,
}

impl ConceptQuery {
    /// Create a new query.
    pub fn new(project_id: u128) -> Self {
        Self {
            project_id,
            ..Default::default()
        }
    }

    /// Add concepts to search for.
    pub fn concepts(mut self, concepts: Vec<String>) -> Self {
        self.concepts = concepts;
        self
    }

    /// Set minimum confidence.
    pub fn min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = Some(confidence);
        self
    }

    /// Set result limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// In-memory concept index.
///
/// Production should use SochDB for persistence.
pub struct ConceptIndex {
    /// Primary index: concept -> observation IDs
    by_concept: RwLock<BTreeMap<String, Vec<ConceptEntry>>>,
    /// Reverse index: observation ID -> concepts
    by_observation: RwLock<HashMap<u128, Vec<String>>>,
    /// Concept frequency for ranking
    frequency: RwLock<HashMap<String, usize>>,
}

impl Default for ConceptIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ConceptIndex {
    /// Create a new concept index.
    pub fn new() -> Self {
        Self {
            by_concept: RwLock::new(BTreeMap::new()),
            by_observation: RwLock::new(HashMap::new()),
            frequency: RwLock::new(HashMap::new()),
        }
    }

    /// Index a concept entry.
    pub fn index(&self, entry: ConceptEntry) {
        let key = entry.storage_key();

        // Add to concept index
        {
            let mut by_concept = self.by_concept.write();
            by_concept
                .entry(key.clone())
                .or_insert_with(Vec::new)
                .push(entry.clone());
        }

        // Add to observation index
        {
            let mut by_observation = self.by_observation.write();
            by_observation
                .entry(entry.observation_id)
                .or_insert_with(Vec::new)
                .push(entry.concept.clone());
        }

        // Update frequency
        {
            let mut frequency = self.frequency.write();
            *frequency.entry(entry.concept.clone()).or_insert(0) += 1;
        }
    }

    /// Index multiple entries.
    pub fn index_batch(&self, entries: Vec<ConceptEntry>) {
        for entry in entries {
            self.index(entry);
        }
    }

    /// Find observations matching concept query.
    pub fn find_observations(&self, query: &ConceptQuery) -> Vec<u128> {
        let by_concept = self.by_concept.read();
        let mut observation_ids: HashSet<u128> = HashSet::new();

        for concept in &query.concepts {
            let prefix = ConceptEntry::concept_prefix(query.project_id, concept);

            for (key, entries) in by_concept.range(prefix.clone()..) {
                if !key.starts_with(&prefix) {
                    break;
                }

                for entry in entries {
                    if let Some(min_conf) = query.min_confidence {
                        if entry.confidence < min_conf {
                            continue;
                        }
                    }
                    observation_ids.insert(entry.observation_id);
                }
            }
        }

        let mut result: Vec<_> = observation_ids.into_iter().collect();

        if let Some(limit) = query.limit {
            result.truncate(limit);
        }

        result
    }

    /// Get concepts for an observation.
    pub fn get_observation_concepts(&self, observation_id: u128) -> Vec<String> {
        self.by_observation
            .read()
            .get(&observation_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get most frequent concepts for a project.
    pub fn get_top_concepts(&self, project_id: u128, limit: usize) -> Vec<(String, usize)> {
        let by_concept = self.by_concept.read();
        let prefix = ConceptEntry::project_prefix(project_id);

        let mut concept_counts: HashMap<String, usize> = HashMap::new();

        for (key, entries) in by_concept.range(prefix.clone()..) {
            if !key.starts_with(&prefix) {
                break;
            }

            for entry in entries {
                *concept_counts.entry(entry.concept.clone()).or_insert(0) += 1;
            }
        }

        let mut sorted: Vec<_> = concept_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);

        sorted
    }

    /// Find related concepts (co-occurring).
    pub fn find_related(&self, project_id: u128, concept: &str, limit: usize) -> Vec<String> {
        // Find observations with this concept
        let obs_ids = self.find_observations(
            &ConceptQuery::new(project_id).concepts(vec![concept.to_string()]),
        );

        // Get all concepts from those observations
        let by_observation = self.by_observation.read();
        let mut related_counts: HashMap<String, usize> = HashMap::new();

        for obs_id in obs_ids {
            if let Some(concepts) = by_observation.get(&obs_id) {
                for c in concepts {
                    if c != concept {
                        *related_counts.entry(c.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut sorted: Vec<_> = related_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);

        sorted.into_iter().map(|(c, _)| c).collect()
    }

    /// Remove observation from index.
    pub fn remove_observation(&self, observation_id: u128) {
        let concepts = {
            let mut by_observation = self.by_observation.write();
            by_observation.remove(&observation_id).unwrap_or_default()
        };

        let mut by_concept = self.by_concept.write();
        let mut frequency = self.frequency.write();

        for concept in concepts {
            // Update frequency
            if let Some(count) = frequency.get_mut(&concept) {
                *count = count.saturating_sub(1);
            }

            // Remove from concept index
            for entries in by_concept.values_mut() {
                entries.retain(|e| e.observation_id != observation_id);
            }
        }
    }

    /// Get index statistics.
    pub fn stats(&self) -> ConceptIndexStats {
        ConceptIndexStats {
            total_concepts: self.by_concept.read().len(),
            total_observations: self.by_observation.read().len(),
            unique_concepts: self.frequency.read().len(),
        }
    }

    /// Clear the index.
    pub fn clear(&self) {
        self.by_concept.write().clear();
        self.by_observation.write().clear();
        self.frequency.write().clear();
    }
}

/// Concept index statistics.
#[derive(Debug, Clone)]
pub struct ConceptIndexStats {
    /// Total concept entries.
    pub total_concepts: usize,
    /// Total indexed observations.
    pub total_observations: usize,
    /// Unique concept count.
    pub unique_concepts: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_entry(project_id: u128, concept: &str, observation_id: u128) -> ConceptEntry {
        ConceptEntry {
            project_id,
            concept: concept.to_string(),
            observation_id,
            confidence: 1.0,
            source: "explicit".to_string(),
            indexed_at: 0,
        }
    }

    #[test]
    fn test_index_and_find() {
        let index = ConceptIndex::new();

        index.index(create_entry(100, "authentication", 1));
        index.index(create_entry(100, "authentication", 2));
        index.index(create_entry(100, "session", 2));

        let obs = index.find_observations(
            &ConceptQuery::new(100).concepts(vec!["authentication".to_string()]),
        );

        assert_eq!(obs.len(), 2);
    }

    #[test]
    fn test_get_observation_concepts() {
        let index = ConceptIndex::new();

        index.index(create_entry(100, "auth", 1));
        index.index(create_entry(100, "session", 1));

        let concepts = index.get_observation_concepts(1);
        assert_eq!(concepts.len(), 2);
    }

    #[test]
    fn test_find_related() {
        let index = ConceptIndex::new();

        // Observation 1: auth, session
        index.index(create_entry(100, "auth", 1));
        index.index(create_entry(100, "session", 1));

        // Observation 2: auth, token
        index.index(create_entry(100, "auth", 2));
        index.index(create_entry(100, "token", 2));

        let related = index.find_related(100, "auth", 10);
        assert!(related.contains(&"session".to_string()));
        assert!(related.contains(&"token".to_string()));
    }

    #[test]
    fn test_top_concepts() {
        let index = ConceptIndex::new();

        for i in 0..5 {
            index.index(create_entry(100, "common", i));
        }
        for i in 0..2 {
            index.index(create_entry(100, "rare", i + 10));
        }

        let top = index.get_top_concepts(100, 10);
        assert_eq!(top[0].0, "common");
        assert_eq!(top[0].1, 5);
    }
}
