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

//! Entity and Relationship Types
//!
//! Defines the core types for the knowledge graph.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A unique identifier for entities in the knowledge graph
pub type EntityId = u64;

/// A knowledge graph entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier
    pub id: EntityId,
    /// Entity name (normalized)
    pub name: String,
    /// Original names before normalization
    pub aliases: Vec<String>,
    /// Entity type (e.g., "service", "file", "function", "variable")
    pub entity_type: EntityType,
    /// Additional attributes
    pub attributes: HashMap<String, String>,
    /// Number of times this entity appears in traces
    pub occurrence_count: u64,
    /// Community/cluster ID (set by Leiden algorithm)
    pub community_id: Option<u32>,
}

/// Entity types in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    /// A service (e.g., "Auth Service", "Payment Gateway")
    Service,
    /// A file (e.g., "auth.rs", "main.py")
    File,
    /// A function or method
    Function,
    /// A variable or parameter
    Variable,
    /// An error type
    Error,
    /// A model (e.g., "gpt-4", "claude-3")
    Model,
    /// A user or agent
    Agent,
    /// A generic concept
    Concept,
    /// Unknown type
    Unknown,
}

impl EntityType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "service" => EntityType::Service,
            "file" => EntityType::File,
            "function" | "method" => EntityType::Function,
            "variable" | "param" | "parameter" => EntityType::Variable,
            "error" | "exception" => EntityType::Error,
            "model" | "llm" => EntityType::Model,
            "agent" | "user" => EntityType::Agent,
            "concept" | "idea" => EntityType::Concept,
            _ => EntityType::Unknown,
        }
    }
}

/// Relationship types between entities
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationType {
    /// A depends on B
    DependsOn,
    /// A calls B
    Calls,
    /// A uses B
    Uses,
    /// A breaks B (negative dependency)
    Breaks,
    /// A is fixed by B
    FixedBy,
    /// A contains B
    Contains,
    /// A is part of B
    PartOf,
    /// A produces B (output)
    Produces,
    /// A consumes B (input)
    Consumes,
    /// A causes B (causal relationship)
    Causes,
    /// A is related to B (generic)
    RelatedTo,
    /// A is similar to B
    SimilarTo,
}

impl RelationType {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().replace(" ", "_").as_str() {
            "DEPENDS_ON" | "DEPENDS" | "REQUIRES" => RelationType::DependsOn,
            "CALLS" | "INVOKES" => RelationType::Calls,
            "USES" | "UTILIZES" => RelationType::Uses,
            "BREAKS" | "BREAKS_WHEN" | "FAILS" => RelationType::Breaks,
            "FIXED_BY" | "RESOLVED_BY" | "SOLVED_BY" => RelationType::FixedBy,
            "CONTAINS" | "HAS" | "INCLUDES" => RelationType::Contains,
            "PART_OF" | "BELONGS_TO" | "IN" => RelationType::PartOf,
            "PRODUCES" | "OUTPUTS" | "RETURNS" => RelationType::Produces,
            "CONSUMES" | "INPUTS" | "TAKES" => RelationType::Consumes,
            "CAUSES" | "LEADS_TO" | "RESULTS_IN" => RelationType::Causes,
            "SIMILAR_TO" | "LIKE" => RelationType::SimilarTo,
            _ => RelationType::RelatedTo,
        }
    }

    /// Check if this relationship type indicates a dependency
    pub fn is_dependency(&self) -> bool {
        matches!(
            self,
            RelationType::DependsOn
                | RelationType::Calls
                | RelationType::Uses
                | RelationType::Consumes
        )
    }

    /// Check if this relationship type indicates a negative/breaking relationship
    pub fn is_breaking(&self) -> bool {
        matches!(self, RelationType::Breaks | RelationType::Causes)
    }
}

/// A triple representing a relationship between two entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triple {
    /// Subject entity
    pub subject: String,
    /// Subject entity type (optional, inferred if not provided)
    pub subject_type: Option<EntityType>,
    /// Relationship/predicate
    pub predicate: RelationType,
    /// Object entity
    pub object: String,
    /// Object entity type (optional, inferred if not provided)
    pub object_type: Option<EntityType>,
    /// Confidence score (0.0 - 1.0) from LLM extraction
    pub confidence: f64,
    /// Source trace edge ID
    pub source_edge_id: Option<u128>,
    /// Extraction timestamp
    pub extracted_at: u64,
}

impl Triple {
    /// Create a new triple
    pub fn new(
        subject: impl Into<String>,
        predicate: RelationType,
        object: impl Into<String>,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        Self {
            subject: subject.into(),
            subject_type: None,
            predicate,
            object: object.into(),
            object_type: None,
            confidence: 1.0,
            source_edge_id: None,
            extracted_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
        }
    }

    /// Create a triple with types
    pub fn with_types(
        subject: impl Into<String>,
        subject_type: EntityType,
        predicate: RelationType,
        object: impl Into<String>,
        object_type: EntityType,
    ) -> Self {
        let mut triple = Self::new(subject, predicate, object);
        triple.subject_type = Some(subject_type);
        triple.object_type = Some(object_type);
        triple
    }

    /// Set the confidence score
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the source edge ID
    pub fn with_source(mut self, edge_id: u128) -> Self {
        self.source_edge_id = Some(edge_id);
        self
    }
}

/// A community/cluster of related entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    /// Community ID
    pub id: u32,
    /// Human-readable name (generated from member entities)
    pub name: String,
    /// LLM-generated summary of the community
    pub summary: Option<String>,
    /// Entity IDs in this community
    pub members: Vec<EntityId>,
    /// Representative keywords
    pub keywords: Vec<String>,
    /// Parent community ID (for hierarchical clustering)
    pub parent_id: Option<u32>,
    /// Child community IDs
    pub children: Vec<u32>,
    /// Internal connectivity (modularity contribution)
    pub modularity: f64,
}

impl Community {
    /// Create a new community
    pub fn new(id: u32) -> Self {
        Self {
            id,
            name: format!("Community {}", id),
            summary: None,
            members: Vec::new(),
            keywords: Vec::new(),
            parent_id: None,
            children: Vec::new(),
            modularity: 0.0,
        }
    }

    /// Add a member to the community
    pub fn add_member(&mut self, entity_id: EntityId) {
        if !self.members.contains(&entity_id) {
            self.members.push(entity_id);
        }
    }

    /// Set the community summary
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Add keywords
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }
}

/// Statistics about the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    /// Total number of entities
    pub entity_count: usize,
    /// Total number of relationships (edges)
    pub relationship_count: usize,
    /// Number of communities
    pub community_count: usize,
    /// Average relationships per entity
    pub avg_degree: f64,
    /// Graph density (actual edges / possible edges)
    pub density: f64,
    /// Distribution of entity types
    pub entity_type_distribution: HashMap<String, usize>,
    /// Distribution of relationship types
    pub relationship_type_distribution: HashMap<String, usize>,
}

impl Default for GraphStats {
    fn default() -> Self {
        Self {
            entity_count: 0,
            relationship_count: 0,
            community_count: 0,
            avg_degree: 0.0,
            density: 0.0,
            entity_type_distribution: HashMap::new(),
            relationship_type_distribution: HashMap::new(),
        }
    }
}
