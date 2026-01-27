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

//! Leiden Community Detection Algorithm
//!
//! Implements the Leiden algorithm for detecting communities in the knowledge graph.
//! Leiden is an improvement over Louvain that ensures well-connected communities
//! through an intermediate refinement phase.
//!
//! ## Algorithm Overview
//!
//! 1. **Local Moving Phase**: Move nodes between communities to maximize modularity
//! 2. **Refinement Phase**: Ensure communities are well-connected (Leiden improvement)
//! 3. **Aggregation Phase**: Create super-nodes from communities
//! 4. **Repeat**: Until no improvement possible
//!
//! ## Modularity
//!
//! Q = (1/2m) * Σij[Aij - (ki*kj)/(2m)] * δ(ci, cj)
//!
//! Where:
//! - Aij = edge weight between i and j
//! - ki, kj = degree of nodes i, j
//! - m = total edge weight
//! - δ(ci, cj) = 1 if nodes in same community
//!
//! Reference: Traag et al., "From Louvain to Leiden: guaranteeing well-connected communities"
//! https://www.nature.com/articles/s41598-019-41695-z

use crate::knowledge_graph::entities::{Community, EntityId};
use crate::knowledge_graph::graph::SemanticGraph;
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::{HashMap, HashSet};

/// Leiden clustering configuration
#[derive(Debug, Clone)]
pub struct LeidenConfig {
    /// Resolution parameter (higher = more communities)
    pub resolution: f64,
    /// Maximum iterations per phase
    pub max_iterations: usize,
    /// Minimum modularity improvement to continue
    pub min_improvement: f64,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
}

impl Default for LeidenConfig {
    fn default() -> Self {
        Self {
            resolution: 1.0,
            max_iterations: 100,
            min_improvement: 1e-6,
            seed: None,
        }
    }
}

/// Leiden community detection algorithm
pub struct LeidenClustering {
    config: LeidenConfig,
}

impl LeidenClustering {
    /// Create new Leiden clustering with default config
    pub fn new() -> Self {
        Self {
            config: LeidenConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: LeidenConfig) -> Self {
        Self { config }
    }

    /// Run Leiden algorithm on the semantic graph
    ///
    /// Returns a mapping of entity ID -> community ID
    pub fn detect_communities(&self, graph: &SemanticGraph) -> HashMap<EntityId, u32> {
        // Get adjacency matrix and entity IDs
        let (entity_ids, matrix) = graph.adjacency_matrix();
        let n = entity_ids.len();

        if n == 0 {
            return HashMap::new();
        }

        // Initialize: each node in its own community
        let mut communities: Vec<u32> = (0..n as u32).collect();
        let mut _num_communities = n as u32;

        // Compute initial degrees and total weight
        let degrees: Vec<f64> = matrix.iter().map(|row| row.iter().sum()).collect();
        let total_weight: f64 = degrees.iter().sum::<f64>() / 2.0;

        if total_weight == 0.0 {
            // No edges, each node is its own community
            return entity_ids
                .into_iter()
                .enumerate()
                .map(|(i, id)| (id, i as u32))
                .collect();
        }

        let mut rng = match self.config.seed {
            Some(seed) => {
                use rand::SeedableRng;
                rand::rngs::StdRng::seed_from_u64(seed)
            }
            None => {
                use rand::SeedableRng;
                rand::rngs::StdRng::from_entropy()
            }
        };

        // Main loop
        for _iteration in 0..self.config.max_iterations {
            let old_modularity =
                self.compute_modularity(&matrix, &communities, &degrees, total_weight);

            // Phase 1: Local moving
            let improved = self.local_moving_phase(
                &matrix,
                &mut communities,
                &degrees,
                total_weight,
                &mut rng,
            );

            // Phase 2: Refinement (Leiden improvement over Louvain)
            self.refinement_phase(&matrix, &mut communities, &degrees, total_weight);

            // Check for improvement
            let new_modularity =
                self.compute_modularity(&matrix, &communities, &degrees, total_weight);
            let improvement = new_modularity - old_modularity;

            if !improved || improvement < self.config.min_improvement {
                break;
            }

            // Phase 3: Aggregation (renumber communities)
            let (new_communities, new_num) = self.renumber_communities(&communities);
            communities = new_communities;
            _num_communities = new_num;
        }

        // Build result mapping
        entity_ids.into_iter().zip(communities).collect()
    }

    /// Local moving phase: move nodes to maximize modularity gain
    fn local_moving_phase<R: Rng>(
        &self,
        matrix: &[Vec<f64>],
        communities: &mut [u32],
        degrees: &[f64],
        total_weight: f64,
        rng: &mut R,
    ) -> bool {
        let n = communities.len();
        let mut improved = false;

        // Random order for visiting nodes
        let mut order: Vec<usize> = (0..n).collect();
        order.shuffle(rng);

        for &node in &order {
            let current_community = communities[node];

            // Find neighboring communities
            let mut neighbor_communities: HashMap<u32, f64> = HashMap::new();
            for (j, &weight) in matrix[node].iter().enumerate() {
                if weight > 0.0 && j != node {
                    *neighbor_communities.entry(communities[j]).or_default() += weight;
                }
            }

            // Also consider current community
            neighbor_communities.entry(current_community).or_default();

            // Find best community
            let mut best_community = current_community;
            let mut best_gain = 0.0;

            for (&community, &edge_weight_to_community) in &neighbor_communities {
                if community == current_community {
                    continue;
                }

                let gain = self.modularity_gain(
                    node,
                    community,
                    current_community,
                    edge_weight_to_community,
                    *neighbor_communities.get(&current_community).unwrap_or(&0.0),
                    communities,
                    degrees,
                    total_weight,
                );

                if gain > best_gain {
                    best_gain = gain;
                    best_community = community;
                }
            }

            // Move node if improvement found
            if best_community != current_community {
                communities[node] = best_community;
                improved = true;
            }
        }

        improved
    }

    /// Refinement phase: ensure communities are well-connected (Leiden improvement)
    fn refinement_phase(
        &self,
        matrix: &[Vec<f64>],
        communities: &mut [u32],
        _degrees: &[f64],
        _total_weight: f64,
    ) {
        let n = communities.len();

        // For each community, check internal connectivity
        let mut community_nodes: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, &c) in communities.iter().enumerate() {
            community_nodes.entry(c).or_default().push(i);
        }

        for (_, nodes) in community_nodes {
            if nodes.len() <= 1 {
                continue;
            }

            // Check if community is well-connected (all nodes reachable)
            let mut visited: HashSet<usize> = HashSet::new();
            let mut stack = vec![nodes[0]];
            visited.insert(nodes[0]);

            while let Some(node) = stack.pop() {
                for &neighbor in &nodes {
                    if !visited.contains(&neighbor) && matrix[node][neighbor] > 0.0 {
                        visited.insert(neighbor);
                        stack.push(neighbor);
                    }
                }
            }

            // If not all nodes visited, community is disconnected
            // Split disconnected parts (simplified - just remove isolated nodes)
            for &node in &nodes {
                if !visited.contains(&node) {
                    // Assign isolated node to singleton community
                    communities[node] = (n + node) as u32;
                }
            }
        }
    }

    /// Compute modularity gain for moving a node
    fn modularity_gain(
        &self,
        node: usize,
        new_community: u32,
        old_community: u32,
        edge_weight_to_new: f64,
        edge_weight_to_old: f64,
        communities: &[u32],
        degrees: &[f64],
        total_weight: f64,
    ) -> f64 {
        let resolution = self.config.resolution;
        let node_degree = degrees[node];

        // Sum of degrees in new community (excluding node)
        let new_comm_degree: f64 = communities
            .iter()
            .zip(degrees.iter())
            .filter(|(&c, _)| c == new_community)
            .map(|(_, &d)| d)
            .sum();

        // Sum of degrees in old community (excluding node)
        let old_comm_degree: f64 = communities
            .iter()
            .zip(degrees.iter())
            .filter(|(&c, _)| c == old_community)
            .map(|(_, &d)| d)
            .sum::<f64>()
            - node_degree;

        // Modularity gain formula
        let gain_new =
            edge_weight_to_new - resolution * node_degree * new_comm_degree / (2.0 * total_weight);
        let gain_old =
            edge_weight_to_old - resolution * node_degree * old_comm_degree / (2.0 * total_weight);

        gain_new - gain_old
    }

    /// Compute total modularity
    fn compute_modularity(
        &self,
        matrix: &[Vec<f64>],
        communities: &[u32],
        degrees: &[f64],
        total_weight: f64,
    ) -> f64 {
        if total_weight == 0.0 {
            return 0.0;
        }

        let n = communities.len();
        let mut modularity = 0.0;

        for i in 0..n {
            for j in 0..n {
                if communities[i] == communities[j] {
                    let expected = degrees[i] * degrees[j] / (2.0 * total_weight);
                    modularity += matrix[i][j] - self.config.resolution * expected;
                }
            }
        }

        modularity / (2.0 * total_weight)
    }

    /// Renumber communities to be contiguous
    fn renumber_communities(&self, communities: &[u32]) -> (Vec<u32>, u32) {
        let mut mapping: HashMap<u32, u32> = HashMap::new();
        let mut next_id = 0u32;

        let new_communities: Vec<u32> = communities
            .iter()
            .map(|&c| {
                *mapping.entry(c).or_insert_with(|| {
                    let id = next_id;
                    next_id += 1;
                    id
                })
            })
            .collect();

        (new_communities, next_id)
    }

    /// Apply clustering results to the graph
    pub fn apply_to_graph(&self, graph: &SemanticGraph, clustering: &HashMap<EntityId, u32>) {
        // Group entities by community
        let mut community_members: HashMap<u32, Vec<EntityId>> = HashMap::new();
        for (&entity_id, &community_id) in clustering {
            community_members
                .entry(community_id)
                .or_default()
                .push(entity_id);
        }

        // Update entity community assignments
        for (&entity_id, &community_id) in clustering {
            graph.set_community(entity_id, community_id);
        }

        // Create community objects
        for (community_id, members) in community_members {
            let mut community = Community::new(community_id);
            community.members = members.clone();

            // Generate name from top entities
            let entity_names: Vec<String> = members
                .iter()
                .take(3)
                .filter_map(|&id| graph.get_entity(id).map(|e| e.name))
                .collect();
            if !entity_names.is_empty() {
                community.name = entity_names.join(", ");
            }

            // Extract keywords from entity names
            let keywords: HashSet<String> = members
                .iter()
                .filter_map(|&id| graph.get_entity(id))
                .flat_map(|e| e.name.split('_').map(|s| s.to_string()).collect::<Vec<_>>())
                .filter(|s| s.len() > 2)
                .collect();
            community.keywords = keywords.into_iter().take(10).collect();

            graph.add_community(community);
        }
    }
}

impl Default for LeidenClustering {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::entities::{EntityType, RelationType, Triple};

    #[test]
    fn test_leiden_basic() {
        let graph = SemanticGraph::new();

        // Create two clusters
        // Cluster 1: auth.rs <-> jwt.rs <-> user.rs
        graph.add_triple(&Triple::new("auth.rs", RelationType::DependsOn, "jwt.rs"));
        graph.add_triple(&Triple::new("jwt.rs", RelationType::DependsOn, "auth.rs"));
        graph.add_triple(&Triple::new("jwt.rs", RelationType::DependsOn, "user.rs"));
        graph.add_triple(&Triple::new("user.rs", RelationType::DependsOn, "jwt.rs"));

        // Cluster 2: payment.rs <-> billing.rs
        graph.add_triple(&Triple::new(
            "payment.rs",
            RelationType::DependsOn,
            "billing.rs",
        ));
        graph.add_triple(&Triple::new(
            "billing.rs",
            RelationType::DependsOn,
            "payment.rs",
        ));

        // Weak link between clusters
        graph.add_triple(&Triple::new("auth.rs", RelationType::Uses, "payment.rs"));

        let leiden = LeidenClustering::with_config(LeidenConfig {
            resolution: 1.0,
            seed: Some(42),
            ..Default::default()
        });

        let communities = leiden.detect_communities(&graph);

        // Should detect at least 2 communities
        let unique_communities: HashSet<_> = communities.values().collect();
        assert!(
            unique_communities.len() >= 2,
            "Should detect at least 2 communities"
        );
    }

    #[test]
    fn test_leiden_empty_graph() {
        let graph = SemanticGraph::new();
        let leiden = LeidenClustering::new();
        let communities = leiden.detect_communities(&graph);
        assert!(communities.is_empty());
    }

    #[test]
    fn test_leiden_single_node() {
        let graph = SemanticGraph::new();
        graph.get_or_create_entity("single_node", EntityType::Service);

        let leiden = LeidenClustering::new();
        let communities = leiden.detect_communities(&graph);

        assert_eq!(communities.len(), 1);
    }
}
