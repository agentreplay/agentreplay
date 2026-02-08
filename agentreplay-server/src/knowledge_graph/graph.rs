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

//! Semantic Knowledge Graph
//!
//! The main graph data structure storing entities and relationships.
//! Uses adjacency lists for efficient traversal and supports:
//! - Entity lookup by ID or name
//! - Relationship queries (outgoing, incoming, bidirectional)
//! - Community/cluster assignment
//! - Persistence to disk

use crate::knowledge_graph::entities::{
    Community, Entity, EntityId, EntityType, GraphStats, RelationType, Triple,
};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Edge in the knowledge graph (directed relationship)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source entity ID
    pub from: EntityId,
    /// Target entity ID
    pub to: EntityId,
    /// Relationship type
    pub relation: RelationType,
    /// Confidence score
    pub confidence: f64,
    /// Source trace edge ID (provenance)
    pub source_edge_id: Option<u128>,
    /// Number of times this relationship was observed
    pub occurrence_count: u32,
}

/// Semantic Knowledge Graph
pub struct SemanticGraph {
    /// Entity storage (ID -> Entity)
    entities: DashMap<EntityId, Entity>,
    /// Name to ID mapping for fast lookup
    name_index: DashMap<String, EntityId>,
    /// Outgoing edges (from -> Vec<Edge>)
    outgoing: DashMap<EntityId, Vec<GraphEdge>>,
    /// Incoming edges (to -> Vec<Edge>)
    incoming: DashMap<EntityId, Vec<GraphEdge>>,
    /// Community storage
    communities: RwLock<HashMap<u32, Community>>,
    /// Next entity ID
    next_id: AtomicU64,
    /// Persistence path
    persist_path: Option<PathBuf>,
}

impl SemanticGraph {
    /// Create a new empty semantic graph
    pub fn new() -> Self {
        Self {
            entities: DashMap::new(),
            name_index: DashMap::new(),
            outgoing: DashMap::new(),
            incoming: DashMap::new(),
            communities: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            persist_path: None,
        }
    }

    /// Create a semantic graph with persistence
    pub fn with_persistence<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();

        if path.exists() {
            Self::load_from_disk(&path)
        } else {
            let mut graph = Self::new();
            graph.persist_path = Some(path);
            Ok(graph)
        }
    }

    /// Get or create an entity by name
    pub fn get_or_create_entity(&self, name: &str, entity_type: EntityType) -> EntityId {
        // Normalize name
        let normalized = normalize_entity_name(name);

        // Check if exists
        if let Some(id) = self.name_index.get(&normalized) {
            return *id;
        }

        // Create new entity
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let entity = Entity {
            id,
            name: normalized.clone(),
            aliases: vec![name.to_string()],
            entity_type,
            attributes: HashMap::new(),
            occurrence_count: 1,
            community_id: None,
        };

        self.entities.insert(id, entity);
        self.name_index.insert(normalized, id);

        id
    }

    /// Add a triple to the graph
    pub fn add_triple(&self, triple: &Triple) {
        // Get or create entities
        let from_type = triple.subject_type.clone().unwrap_or(EntityType::Unknown);
        let to_type = triple.object_type.clone().unwrap_or(EntityType::Unknown);

        let from_id = self.get_or_create_entity(&triple.subject, from_type);
        let to_id = self.get_or_create_entity(&triple.object, to_type);

        // Check for existing edge
        let mut edge_exists = false;
        if let Some(mut edges) = self.outgoing.get_mut(&from_id) {
            for edge in edges.iter_mut() {
                if edge.to == to_id && edge.relation == triple.predicate {
                    edge.occurrence_count += 1;
                    edge.confidence = (edge.confidence + triple.confidence) / 2.0;
                    edge_exists = true;
                    break;
                }
            }
        }

        if !edge_exists {
            let edge = GraphEdge {
                from: from_id,
                to: to_id,
                relation: triple.predicate.clone(),
                confidence: triple.confidence,
                source_edge_id: triple.source_edge_id,
                occurrence_count: 1,
            };

            // Add to outgoing edges
            self.outgoing.entry(from_id).or_default().push(edge.clone());
            // Add to incoming edges
            self.incoming.entry(to_id).or_default().push(edge);
        }

        // Update occurrence counts
        if let Some(mut entity) = self.entities.get_mut(&from_id) {
            entity.occurrence_count += 1;
        }
        if let Some(mut entity) = self.entities.get_mut(&to_id) {
            entity.occurrence_count += 1;
        }
    }

    /// Add multiple triples in batch
    pub fn add_triples(&self, triples: &[Triple]) {
        for triple in triples {
            self.add_triple(triple);
        }
    }

    /// Get entity by ID
    pub fn get_entity(&self, id: EntityId) -> Option<Entity> {
        self.entities.get(&id).map(|e| e.clone())
    }

    /// Get entity by name
    pub fn get_entity_by_name(&self, name: &str) -> Option<Entity> {
        let normalized = normalize_entity_name(name);
        self.name_index
            .get(&normalized)
            .and_then(|id| self.entities.get(&*id).map(|e| e.clone()))
    }

    /// Get outgoing relationships for an entity
    pub fn get_outgoing(&self, entity_id: EntityId) -> Vec<GraphEdge> {
        self.outgoing
            .get(&entity_id)
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    /// Get incoming relationships for an entity
    pub fn get_incoming(&self, entity_id: EntityId) -> Vec<GraphEdge> {
        self.incoming
            .get(&entity_id)
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    /// Query: "What depends on X?"
    pub fn what_depends_on(&self, entity_name: &str) -> Vec<(Entity, RelationType, f64)> {
        let normalized = normalize_entity_name(entity_name);
        let entity_id = match self.name_index.get(&normalized) {
            Some(id) => *id,
            None => return Vec::new(),
        };

        let incoming = self.get_incoming(entity_id);
        incoming
            .into_iter()
            .filter(|e| e.relation.is_dependency())
            .filter_map(|edge| {
                self.get_entity(edge.from)
                    .map(|entity| (entity, edge.relation, edge.confidence))
            })
            .collect()
    }

    /// Query: "What does X depend on?"
    pub fn depends_on(&self, entity_name: &str) -> Vec<(Entity, RelationType, f64)> {
        let normalized = normalize_entity_name(entity_name);
        let entity_id = match self.name_index.get(&normalized) {
            Some(id) => *id,
            None => return Vec::new(),
        };

        let outgoing = self.get_outgoing(entity_id);
        outgoing
            .into_iter()
            .filter(|e| e.relation.is_dependency())
            .filter_map(|edge| {
                self.get_entity(edge.to)
                    .map(|entity| (entity, edge.relation, edge.confidence))
            })
            .collect()
    }

    /// Query: "What breaks when I modify X?"
    pub fn what_breaks(&self, entity_name: &str) -> Vec<(Entity, RelationType, f64)> {
        let normalized = normalize_entity_name(entity_name);
        let entity_id = match self.name_index.get(&normalized) {
            Some(id) => *id,
            None => return Vec::new(),
        };

        // Get entities that depend on X (they might break)
        let incoming = self.get_incoming(entity_id);

        // Also check for explicit BREAKS relationships
        let mut results: Vec<(Entity, RelationType, f64)> = incoming
            .into_iter()
            .filter(|e| e.relation.is_dependency() || e.relation.is_breaking())
            .filter_map(|edge| {
                self.get_entity(edge.from)
                    .map(|entity| (entity, edge.relation, edge.confidence))
            })
            .collect();

        // Recursively find transitive dependencies (up to depth 3)
        let mut visited: HashSet<EntityId> = HashSet::new();
        visited.insert(entity_id);

        let mut frontier: Vec<EntityId> = results.iter().map(|(e, _, _)| e.id).collect();

        for _depth in 0..3 {
            let mut next_frontier = Vec::new();
            for id in &frontier {
                if visited.insert(*id) {
                    let incoming = self.get_incoming(*id);
                    for edge in incoming {
                        if edge.relation.is_dependency() {
                            if let Some(entity) = self.get_entity(edge.from) {
                                // Reduce confidence for transitive dependencies
                                let reduced_confidence = edge.confidence * 0.7;
                                results.push((entity.clone(), edge.relation, reduced_confidence));
                                next_frontier.push(entity.id);
                            }
                        }
                    }
                }
            }
            frontier = next_frontier;
        }

        results
    }

    /// Get entities in a community
    pub fn get_community_members(&self, community_id: u32) -> Vec<Entity> {
        self.entities
            .iter()
            .filter(|e| e.community_id == Some(community_id))
            .map(|e| e.clone())
            .collect()
    }

    /// Set community assignment for an entity
    pub fn set_community(&self, entity_id: EntityId, community_id: u32) {
        if let Some(mut entity) = self.entities.get_mut(&entity_id) {
            entity.community_id = Some(community_id);
        }
    }

    /// Add or update a community
    pub fn add_community(&self, community: Community) {
        let mut communities = self.communities.write();
        communities.insert(community.id, community);
    }

    /// Get all communities
    pub fn get_communities(&self) -> Vec<Community> {
        self.communities.read().values().cloned().collect()
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        let entity_count = self.entities.len();
        let relationship_count: usize = self.outgoing.iter().map(|e| e.len()).sum();
        let community_count = self.communities.read().len();

        let avg_degree = if entity_count > 0 {
            relationship_count as f64 / entity_count as f64
        } else {
            0.0
        };

        let density = if entity_count > 1 {
            relationship_count as f64 / (entity_count * (entity_count - 1)) as f64
        } else {
            0.0
        };

        // Count entity types
        let mut entity_type_distribution: HashMap<String, usize> = HashMap::new();
        for entity in self.entities.iter() {
            let type_name = format!("{:?}", entity.entity_type);
            *entity_type_distribution.entry(type_name).or_default() += 1;
        }

        // Count relationship types
        let mut relationship_type_distribution: HashMap<String, usize> = HashMap::new();
        for edges in self.outgoing.iter() {
            for edge in edges.iter() {
                let type_name = format!("{:?}", edge.relation);
                *relationship_type_distribution.entry(type_name).or_default() += 1;
            }
        }

        GraphStats {
            entity_count,
            relationship_count,
            community_count,
            avg_degree,
            density,
            entity_type_distribution,
            relationship_type_distribution,
        }
    }

    /// Get all entities
    pub fn all_entities(&self) -> Vec<Entity> {
        self.entities.iter().map(|e| e.clone()).collect()
    }

    /// Get all edges
    pub fn all_edges(&self) -> Vec<GraphEdge> {
        self.outgoing.iter().flat_map(|e| e.clone()).collect()
    }

    /// Build adjacency matrix for algorithms
    pub fn adjacency_matrix(&self) -> (Vec<EntityId>, Vec<Vec<f64>>) {
        let entity_ids: Vec<EntityId> = self.entities.iter().map(|e| *e.key()).collect();
        let n = entity_ids.len();
        let id_to_idx: HashMap<EntityId, usize> = entity_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        let mut matrix = vec![vec![0.0; n]; n];

        for edges in self.outgoing.iter() {
            for edge in edges.iter() {
                if let (Some(&from_idx), Some(&to_idx)) =
                    (id_to_idx.get(&edge.from), id_to_idx.get(&edge.to))
                {
                    matrix[from_idx][to_idx] = edge.confidence;
                }
            }
        }

        (entity_ids, matrix)
    }

    /// Save graph to disk
    pub fn save_to_disk(&self) -> std::io::Result<()> {
        let Some(ref path) = self.persist_path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }

        let data = GraphPersistence {
            entities: self.entities.iter().map(|e| e.clone()).collect(),
            edges: self.all_edges(),
            communities: self.communities.read().clone(),
            next_id: self.next_id.load(Ordering::SeqCst),
        };

        let temp_path = path.with_extension("tmp");
        let file = File::create(&temp_path)?;
        let mut writer = BufWriter::new(file);

        serde_json::to_writer(&mut writer, &data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        writer.flush()?;
        std::fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Load graph from disk
    fn load_from_disk(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let data: GraphPersistence = serde_json::from_reader(reader)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let graph = Self {
            entities: DashMap::new(),
            name_index: DashMap::new(),
            outgoing: DashMap::new(),
            incoming: DashMap::new(),
            communities: RwLock::new(data.communities),
            next_id: AtomicU64::new(data.next_id),
            persist_path: Some(path.to_path_buf()),
        };

        // Rebuild entities and name index
        for entity in data.entities {
            graph.name_index.insert(entity.name.clone(), entity.id);
            graph.entities.insert(entity.id, entity);
        }

        // Rebuild edge indices
        for edge in data.edges {
            graph
                .outgoing
                .entry(edge.from)
                .or_default()
                .push(edge.clone());
            graph.incoming.entry(edge.to).or_default().push(edge);
        }

        Ok(graph)
    }
}

impl Default for SemanticGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistence format
#[derive(Debug, Serialize, Deserialize)]
struct GraphPersistence {
    entities: Vec<Entity>,
    edges: Vec<GraphEdge>,
    communities: HashMap<u32, Community>,
    next_id: u64,
}

/// Normalize entity name for consistent lookup
fn normalize_entity_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '.', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_triple() {
        let graph = SemanticGraph::new();

        let triple = Triple::with_types(
            "auth.rs",
            EntityType::File,
            RelationType::DependsOn,
            "jwt_lib",
            EntityType::Service,
        );

        graph.add_triple(&triple);

        // Check entities created
        let auth = graph.get_entity_by_name("auth.rs");
        assert!(auth.is_some());
        assert_eq!(auth.unwrap().entity_type, EntityType::File);

        let jwt = graph.get_entity_by_name("jwt_lib");
        assert!(jwt.is_some());
    }

    #[test]
    fn test_what_depends_on() {
        let graph = SemanticGraph::new();

        // auth.rs -> jwt_lib
        // payment.rs -> jwt_lib
        graph.add_triple(&Triple::new("auth.rs", RelationType::DependsOn, "jwt_lib"));
        graph.add_triple(&Triple::new(
            "payment.rs",
            RelationType::DependsOn,
            "jwt_lib",
        ));

        let deps = graph.what_depends_on("jwt_lib");
        assert_eq!(deps.len(), 2);

        let names: Vec<&str> = deps.iter().map(|(e, _, _)| e.name.as_str()).collect();
        assert!(names.contains(&"auth.rs"));
        assert!(names.contains(&"payment.rs"));
    }

    #[test]
    fn test_what_breaks() {
        let graph = SemanticGraph::new();

        // auth.rs -> jwt_lib -> crypto_lib
        graph.add_triple(&Triple::new("auth.rs", RelationType::DependsOn, "jwt_lib"));
        graph.add_triple(&Triple::new(
            "jwt_lib",
            RelationType::DependsOn,
            "crypto_lib",
        ));

        let breaks = graph.what_breaks("crypto_lib");

        // Should include jwt_lib (direct) and auth.rs (transitive)
        let names: Vec<&str> = breaks.iter().map(|(e, _, _)| e.name.as_str()).collect();
        assert!(names.contains(&"jwt_lib"));
        assert!(names.contains(&"auth.rs"));
    }
}
