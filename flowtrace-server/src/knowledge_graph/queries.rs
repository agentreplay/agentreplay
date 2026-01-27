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

//! Graph Query Engine
//!
//! High-level query interface for the semantic knowledge graph.
//! Supports natural language queries like:
//! - "What depends on auth.rs?"
//! - "What breaks when I modify user_id?"
//! - "Show me the authentication cluster"

use crate::knowledge_graph::entities::{Entity, GraphStats};
use crate::knowledge_graph::graph::SemanticGraph;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Query result types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyResult {
    pub entity: String,
    pub entity_type: String,
    pub relationship: String,
    pub confidence: f64,
    pub is_transitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    pub query_entity: String,
    pub directly_affected: Vec<DependencyResult>,
    pub transitively_affected: Vec<DependencyResult>,
    pub risk_score: f64,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub id: u32,
    pub name: String,
    pub summary: Option<String>,
    pub members: Vec<String>,
    pub keywords: Vec<String>,
    pub internal_edges: usize,
}

/// Query engine for the semantic graph
pub struct GraphQueryEngine {
    graph: Arc<SemanticGraph>,
}

impl GraphQueryEngine {
    /// Create a new query engine
    pub fn new(graph: Arc<SemanticGraph>) -> Self {
        Self { graph }
    }

    /// Parse and execute a natural language query
    pub fn query(&self, query: &str) -> QueryResult {
        let query_lower = query.to_lowercase();

        // Pattern: "What does X depend on?" - extract X from the middle
        if query_lower.contains("what does") && query_lower.contains("depend on") {
            if let Some(entity) =
                self.extract_entity_between(&query_lower, "what does", "depend on")
            {
                return self.query_dependencies(&entity);
            }
        }

        // Pattern matching for common queries
        if query_lower.contains("depends on") || query_lower.contains("dependencies of") {
            // Extract entity name after the pattern
            if let Some(entity) =
                self.extract_entity_after(&query_lower, &["depends on", "dependencies of"])
            {
                return self.query_dependencies(&entity);
            }
        }

        if query_lower.contains("what depends on") || query_lower.contains("used by") {
            if let Some(entity) =
                self.extract_entity_after(&query_lower, &["what depends on", "used by"])
            {
                return self.query_dependents(&entity);
            }
        }

        if query_lower.contains("breaks") || query_lower.contains("impact") {
            // Pattern: "What breaks when I modify X?"
            if let Some(entity) =
                self.extract_entity_after(&query_lower, &["modify", "changing", "change"])
            {
                return self.query_impact(&entity);
            }
            if let Some(entity) = self.extract_entity_after(
                &query_lower,
                &["breaks when", "impact of modifying", "impact of changing"],
            ) {
                return self.query_impact(&entity);
            }
        }

        if query_lower.contains("cluster") || query_lower.contains("community") {
            if let Some(name) = self.extract_entity_after(&query_lower, &["cluster", "community"]) {
                return self.query_cluster(&name);
            }
            // If no specific cluster, return all clusters
            return self.query_all_clusters();
        }

        if query_lower.contains("stats") || query_lower.contains("statistics") {
            return self.query_stats();
        }

        // Default: search for entity
        QueryResult::NotFound(format!("Could not parse query: {}", query))
    }

    /// Query: "What does X depend on?"
    pub fn query_dependencies(&self, entity_name: &str) -> QueryResult {
        let deps = self.graph.depends_on(entity_name);

        if deps.is_empty() {
            return QueryResult::NotFound(format!("No dependencies found for '{}'", entity_name));
        }

        let results: Vec<DependencyResult> = deps
            .into_iter()
            .map(|(entity, relation, confidence)| DependencyResult {
                entity: entity.name,
                entity_type: format!("{:?}", entity.entity_type),
                relationship: format!("{:?}", relation),
                confidence,
                is_transitive: false,
            })
            .collect();

        QueryResult::Dependencies {
            entity: entity_name.to_string(),
            dependencies: results,
        }
    }

    /// Query: "What depends on X?"
    pub fn query_dependents(&self, entity_name: &str) -> QueryResult {
        let deps = self.graph.what_depends_on(entity_name);

        if deps.is_empty() {
            return QueryResult::NotFound(format!("No dependents found for '{}'", entity_name));
        }

        let results: Vec<DependencyResult> = deps
            .into_iter()
            .map(|(entity, relation, confidence)| DependencyResult {
                entity: entity.name,
                entity_type: format!("{:?}", entity.entity_type),
                relationship: format!("{:?}", relation),
                confidence,
                is_transitive: false,
            })
            .collect();

        QueryResult::Dependents {
            entity: entity_name.to_string(),
            dependents: results,
        }
    }

    /// Query: "What breaks when I modify X?"
    pub fn query_impact(&self, entity_name: &str) -> QueryResult {
        let affected = self.graph.what_breaks(entity_name);

        if affected.is_empty() {
            return QueryResult::Impact(ImpactAnalysis {
                query_entity: entity_name.to_string(),
                directly_affected: Vec::new(),
                transitively_affected: Vec::new(),
                risk_score: 0.0,
                recommendation: format!(
                    "'{}' has no known dependencies. Safe to modify.",
                    entity_name
                ),
            });
        }

        // Separate direct and transitive
        let mut direct: Vec<DependencyResult> = Vec::new();
        let mut transitive: Vec<DependencyResult> = Vec::new();

        for (entity, relation, confidence) in affected {
            let result = DependencyResult {
                entity: entity.name,
                entity_type: format!("{:?}", entity.entity_type),
                relationship: format!("{:?}", relation),
                confidence,
                is_transitive: confidence < 0.8, // Lower confidence = likely transitive
            };

            if result.is_transitive {
                transitive.push(result);
            } else {
                direct.push(result);
            }
        }

        // Compute risk score
        let direct_weight = direct.len() as f64 * 1.0;
        let transitive_weight = transitive.len() as f64 * 0.3;
        let risk_score = ((direct_weight + transitive_weight) / 10.0).min(1.0);

        let recommendation = if risk_score > 0.7 {
            format!(
                "HIGH RISK: Modifying '{}' affects {} direct and {} transitive dependencies. Consider thorough testing.",
                entity_name, direct.len(), transitive.len()
            )
        } else if risk_score > 0.3 {
            format!(
                "MEDIUM RISK: '{}' has {} direct dependencies. Review affected components before modifying.",
                entity_name, direct.len()
            )
        } else {
            format!(
                "LOW RISK: '{}' has limited dependencies. Should be safe to modify with standard testing.",
                entity_name
            )
        };

        QueryResult::Impact(ImpactAnalysis {
            query_entity: entity_name.to_string(),
            directly_affected: direct,
            transitively_affected: transitive,
            risk_score,
            recommendation,
        })
    }

    /// Query: "Show me cluster X"
    pub fn query_cluster(&self, name: &str) -> QueryResult {
        let communities = self.graph.get_communities();

        let name_lower = name.to_lowercase();
        let matching: Vec<ClusterInfo> = communities
            .into_iter()
            .filter(|c| {
                c.name.to_lowercase().contains(&name_lower)
                    || c.keywords
                        .iter()
                        .any(|k| k.to_lowercase().contains(&name_lower))
            })
            .map(|c| {
                let members: Vec<String> = c
                    .members
                    .iter()
                    .filter_map(|&id| self.graph.get_entity(id).map(|e| e.name))
                    .collect();

                ClusterInfo {
                    id: c.id,
                    name: c.name,
                    summary: c.summary,
                    members,
                    keywords: c.keywords,
                    internal_edges: 0, // TODO: compute
                }
            })
            .collect();

        if matching.is_empty() {
            QueryResult::NotFound(format!("No cluster matching '{}' found", name))
        } else {
            QueryResult::Clusters(matching)
        }
    }

    /// Query: "Show all clusters"
    pub fn query_all_clusters(&self) -> QueryResult {
        let communities = self.graph.get_communities();

        let clusters: Vec<ClusterInfo> = communities
            .into_iter()
            .map(|c| {
                let members: Vec<String> = c
                    .members
                    .iter()
                    .filter_map(|&id| self.graph.get_entity(id).map(|e| e.name))
                    .collect();

                ClusterInfo {
                    id: c.id,
                    name: c.name,
                    summary: c.summary,
                    members,
                    keywords: c.keywords,
                    internal_edges: 0,
                }
            })
            .collect();

        QueryResult::Clusters(clusters)
    }

    /// Query: "Show statistics"
    pub fn query_stats(&self) -> QueryResult {
        QueryResult::Stats(self.graph.stats())
    }

    /// Extract entity name AFTER a pattern in query
    fn extract_entity_after(&self, query: &str, patterns: &[&str]) -> Option<String> {
        for pattern in patterns {
            if let Some(idx) = query.find(pattern) {
                let after = &query[idx + pattern.len()..];
                let entity = after
                    .split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
                    .to_string();

                if !entity.is_empty() {
                    return Some(entity);
                }
            }
        }
        None
    }

    /// Extract entity name BETWEEN two patterns (e.g., "what does X depend on")
    fn extract_entity_between(
        &self,
        query: &str,
        start_pattern: &str,
        end_pattern: &str,
    ) -> Option<String> {
        if let Some(start_idx) = query.find(start_pattern) {
            let after_start = &query[start_idx + start_pattern.len()..];
            if let Some(end_idx) = after_start.find(end_pattern) {
                let between = &after_start[..end_idx];
                let entity = between
                    .trim()
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
                    .to_string();

                if !entity.is_empty() {
                    return Some(entity);
                }
            }
        }
        None
    }

    /// Search entities by name pattern
    pub fn search_entities(&self, pattern: &str) -> Vec<Entity> {
        let pattern_lower = pattern.to_lowercase();
        self.graph
            .all_entities()
            .into_iter()
            .filter(|e| e.name.to_lowercase().contains(&pattern_lower))
            .collect()
    }

    /// Get entity details
    pub fn get_entity(&self, name: &str) -> Option<EntityDetails> {
        self.graph.get_entity_by_name(name).map(|entity| {
            let outgoing = self.graph.get_outgoing(entity.id);
            let incoming = self.graph.get_incoming(entity.id);

            EntityDetails {
                entity,
                outgoing_count: outgoing.len(),
                incoming_count: incoming.len(),
                relationships: outgoing
                    .into_iter()
                    .chain(incoming)
                    .take(20)
                    .map(|e| format!("{:?}", e.relation))
                    .collect(),
            }
        })
    }
}

/// Query result types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum QueryResult {
    Dependencies {
        entity: String,
        dependencies: Vec<DependencyResult>,
    },
    Dependents {
        entity: String,
        dependents: Vec<DependencyResult>,
    },
    Impact(ImpactAnalysis),
    Clusters(Vec<ClusterInfo>),
    Stats(GraphStats),
    NotFound(String),
}

/// Detailed entity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDetails {
    pub entity: Entity,
    pub outgoing_count: usize,
    pub incoming_count: usize,
    pub relationships: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::entities::Triple;
    use crate::knowledge_graph::RelationType;

    fn create_test_graph() -> Arc<SemanticGraph> {
        let graph = SemanticGraph::new();

        graph.add_triple(&Triple::new("auth.rs", RelationType::DependsOn, "jwt_lib"));
        graph.add_triple(&Triple::new(
            "payment.rs",
            RelationType::DependsOn,
            "jwt_lib",
        ));
        graph.add_triple(&Triple::new("jwt_lib", RelationType::DependsOn, "crypto"));

        Arc::new(graph)
    }

    #[test]
    fn test_query_dependencies() {
        let graph = create_test_graph();
        let engine = GraphQueryEngine::new(graph);

        let result = engine.query("What does auth.rs depend on?");
        match result {
            QueryResult::Dependencies {
                entity,
                dependencies,
            } => {
                assert_eq!(entity, "auth.rs");
                assert!(!dependencies.is_empty());
            }
            _ => panic!("Expected Dependencies result"),
        }
    }

    #[test]
    fn test_query_impact() {
        let graph = create_test_graph();
        let engine = GraphQueryEngine::new(graph);

        let result = engine.query("What breaks when I modify jwt_lib?");
        match result {
            QueryResult::Impact(analysis) => {
                assert!(!analysis.directly_affected.is_empty());
            }
            _ => panic!("Expected Impact result"),
        }
    }

    #[test]
    fn test_query_stats() {
        let graph = create_test_graph();
        let engine = GraphQueryEngine::new(graph);

        let result = engine.query("Show statistics");
        match result {
            QueryResult::Stats(stats) => {
                assert!(stats.entity_count > 0);
                assert!(stats.relationship_count > 0);
            }
            _ => panic!("Expected Stats result"),
        }
    }
}
