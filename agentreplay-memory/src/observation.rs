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

//! Observation types and storage
//!
//! Observations are the atomic unit of memory in Agentreplay. They represent
//! facts, decisions, patterns, or notes captured during coding sessions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an observation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(pub String);

impl ObservationId {
    /// Generate a new unique ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }
}

impl Default for ObservationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ObservationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Category of an observation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationCategory {
    /// A decision made about architecture, design, or implementation
    Decision,
    /// A code pattern or convention observed
    Pattern,
    /// A fact about the codebase or domain
    Fact,
    /// A preference expressed by the user
    Preference,
    /// A TODO or action item
    Todo,
    /// An error or issue encountered
    Issue,
    /// A learned insight or lesson
    Insight,
    /// General note
    Note,
}

impl Default for ObservationCategory {
    fn default() -> Self {
        Self::Note
    }
}

/// An observation stored in memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Unique observation ID
    pub id: ObservationId,
    /// Workspace this observation belongs to
    pub workspace_id: String,
    /// Session where this observation was made
    pub session_id: String,
    /// Observation content
    pub content: String,
    /// Category
    pub category: ObservationCategory,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// When the observation was created
    pub created_at: DateTime<Utc>,
    /// When the observation was last updated
    pub updated_at: DateTime<Utc>,
    /// Source of the observation (e.g., "user", "auto", "session-end")
    pub source: String,
    /// Optional trace ID linking to a specific trace span
    pub trace_id: Option<String>,
    /// Optional span ID linking to a specific span
    pub span_id: Option<String>,
    /// Relevance score (computed, not stored)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f32>,
    /// Embedding vector (if computed)
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
}

impl Observation {
    /// Create a new observation builder
    pub fn new(workspace_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ObservationId::new(),
            workspace_id: workspace_id.into(),
            session_id: session_id.into(),
            content: String::new(),
            category: ObservationCategory::Note,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            source: "user".to_string(),
            trace_id: None,
            span_id: None,
            relevance_score: None,
            embedding: None,
        }
    }

    /// Set the content
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Set the category
    pub fn category(mut self, category: ObservationCategory) -> Self {
        self.category = category;
        self
    }

    /// Set tags
    pub fn tags(mut self, tags: Vec<impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Add a single tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set the source
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Link to a trace
    pub fn with_trace(mut self, trace_id: impl Into<String>, span_id: Option<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self.span_id = span_id;
        self
    }

    /// Compute content hash for deduplication
    pub fn content_hash(&self) -> String {
        let hash = blake3::hash(self.content.as_bytes());
        hex::encode(&hash.as_bytes()[..8])
    }
}

/// Query for retrieving observations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservationQuery {
    /// Filter by workspace
    pub workspace_id: Option<String>,
    /// Filter by session
    pub session_id: Option<String>,
    /// Filter by categories
    pub categories: Vec<ObservationCategory>,
    /// Filter by tags (any match)
    pub tags: Vec<String>,
    /// Full-text search query
    pub text_query: Option<String>,
    /// Semantic search query (uses embeddings)
    pub semantic_query: Option<String>,
    /// Filter by time range (start)
    pub from_time: Option<DateTime<Utc>>,
    /// Filter by time range (end)
    pub to_time: Option<DateTime<Utc>>,
    /// Maximum results to return
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Sort order: "newest", "oldest", "relevant"
    pub sort: Option<String>,
}

impl ObservationQuery {
    /// Create a new query for a workspace
    pub fn for_workspace(workspace_id: impl Into<String>) -> Self {
        Self {
            workspace_id: Some(workspace_id.into()),
            ..Default::default()
        }
    }

    /// Filter by category
    pub fn category(mut self, category: ObservationCategory) -> Self {
        self.categories.push(category);
        self
    }

    /// Filter by tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set semantic search query
    pub fn semantic(mut self, query: impl Into<String>) -> Self {
        self.semantic_query = Some(query.into());
        self
    }

    /// Limit results
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_builder() {
        let obs = Observation::new("workspace-1", "session-1")
            .content("User prefers explicit error handling")
            .category(ObservationCategory::Preference)
            .tags(vec!["error-handling", "style"])
            .source("user");

        assert_eq!(obs.workspace_id, "workspace-1");
        assert_eq!(obs.session_id, "session-1");
        assert_eq!(obs.content, "User prefers explicit error handling");
        assert_eq!(obs.category, ObservationCategory::Preference);
        assert_eq!(obs.tags, vec!["error-handling", "style"]);
    }

    #[test]
    fn test_observation_hash() {
        let obs1 = Observation::new("w", "s").content("same content");
        let obs2 = Observation::new("w", "s").content("same content");
        let obs3 = Observation::new("w", "s").content("different content");

        assert_eq!(obs1.content_hash(), obs2.content_hash());
        assert_ne!(obs1.content_hash(), obs3.content_hash());
    }

    #[test]
    fn test_query_builder() {
        let query = ObservationQuery::for_workspace("my-project")
            .category(ObservationCategory::Decision)
            .tag("architecture")
            .semantic("error handling patterns")
            .limit(10);

        assert_eq!(query.workspace_id, Some("my-project".to_string()));
        assert!(query.categories.contains(&ObservationCategory::Decision));
        assert!(query.tags.contains(&"architecture".to_string()));
        assert_eq!(
            query.semantic_query,
            Some("error handling patterns".to_string())
        );
        assert_eq!(query.limit, Some(10));
    }
}
