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

//! Structured Observation Schema with XML Parsing
//!
//! This module defines the observation data model used to compress tool events
//! into structured, queryable observations.
//!
//! # Observation Format
//!
//! Observations are generated in XML format by the memory agent and parsed
//! into structured Rust types:
//!
//! ```xml
//! <observation>
//!     <type>implementation</type>
//!     <title>Short action description (5-10 words)</title>
//!     <subtitle>One-sentence explanation (24 words max)</subtitle>
//!     <facts>
//!         <fact>Concise factual statement 1</fact>
//!         <fact>Concise factual statement 2</fact>
//!     </facts>
//!     <narrative>Full context paragraph: what, how, why</narrative>
//!     <concepts>
//!         <concept>category-type-knowledge</concept>
//!     </concepts>
//!     <files_read><file>path/to/file</file></files_read>
//!     <files_modified><file>path/to/file</file></files_modified>
//! </observation>
//! ```
//!
//! # Key Encoding
//!
//! - Primary: `observations/{session_id:032x}/{timestamp:020}/{id:032x}`
//! - Concepts index: `concepts/{concept}/{observation_id}`
//! - Files index: `files/{path_hash}/{observation_id}`

use crate::edge::HlcTimestamp;
use crate::observation_types::ObservationType;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// A semantic concept extracted from an observation.
///
/// Concepts follow the category-type-knowledge pattern, e.g.:
/// - `rust-error-handling`
/// - `api-authentication-oauth`
/// - `database-migration-schema`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Concept {
    /// The normalized concept string (lowercase, hyphenated).
    pub value: String,
    /// Category component (first part before first hyphen).
    pub category: String,
}

impl Concept {
    /// Create a new concept from a raw string.
    pub fn new(raw: impl Into<String>) -> Self {
        let value = Self::normalize(raw.into());
        let category = value
            .split('-')
            .next()
            .unwrap_or(&value)
            .to_string();

        Self { value, category }
    }

    /// Normalize a concept string: lowercase and hyphenate.
    pub fn normalize(s: String) -> String {
        s.to_lowercase()
            .replace('_', "-")
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Get the concept parts split by hyphen.
    pub fn parts(&self) -> Vec<&str> {
        self.value.split('-').collect()
    }
}

impl std::fmt::Display for Concept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// A structured observation generated from a tool event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Unique observation identifier.
    pub id: u128,
    /// Session this observation belongs to.
    pub session_id: u128,
    /// Project this observation belongs to.
    pub project_id: u128,
    /// Type of observation.
    pub observation_type: ObservationType,
    /// Short action description (5-10 words).
    pub title: String,
    /// One-sentence explanation (24 words max).
    pub subtitle: String,
    /// List of concise factual statements.
    pub facts: Vec<String>,
    /// Full context paragraph: what, how, why.
    pub narrative: String,
    /// Semantic concepts extracted from the observation.
    pub concepts: Vec<Concept>,
    /// Files that were read.
    pub files_read: Vec<PathBuf>,
    /// Files that were modified.
    pub files_modified: Vec<PathBuf>,
    /// ID of the source trace edge.
    pub source_edge_id: Option<u128>,
    /// ID of the tool invocation that generated this observation.
    pub tool_invocation_id: Option<u128>,
    /// Name of the tool that was used.
    pub tool_name: Option<String>,
    /// Whether the observation contains redacted private content.
    pub had_private_content: bool,
    /// Observation creation timestamp.
    pub created_at: HlcTimestamp,
    /// Token count for this observation.
    pub token_count: u32,
}

impl Observation {
    /// Create a new observation builder.
    pub fn builder(id: u128, session_id: u128, project_id: u128) -> ObservationBuilder {
        ObservationBuilder::new(id, session_id, project_id)
    }

    /// Get all files (read and modified).
    pub fn all_files(&self) -> Vec<&PathBuf> {
        self.files_read
            .iter()
            .chain(self.files_modified.iter())
            .collect()
    }

    /// Calculate completeness score (fields filled / total fields).
    pub fn completeness_score(&self) -> f64 {
        let mut filled = 0;
        let total = 8; // title, subtitle, facts, narrative, concepts, files_read, files_modified, observation_type

        if !self.title.is_empty() {
            filled += 1;
        }
        if !self.subtitle.is_empty() {
            filled += 1;
        }
        if !self.facts.is_empty() {
            filled += 1;
        }
        if !self.narrative.is_empty() {
            filled += 1;
        }
        if !self.concepts.is_empty() {
            filled += 1;
        }
        if !self.files_read.is_empty() {
            filled += 1;
        }
        if !self.files_modified.is_empty() {
            filled += 1;
        }
        // observation_type is always filled
        filled += 1;

        filled as f64 / total as f64
    }

    /// Get the storage key for this observation.
    pub fn storage_key(&self) -> String {
        format!(
            "obs/{:032x}/{:032x}/{:020}/{:032x}",
            self.project_id,
            self.session_id,
            self.created_at.packed(),
            self.id
        )
    }
}

/// Builder for creating observations.
pub struct ObservationBuilder {
    id: u128,
    session_id: u128,
    project_id: u128,
    observation_type: ObservationType,
    title: String,
    subtitle: String,
    facts: Vec<String>,
    narrative: String,
    concepts: Vec<Concept>,
    files_read: Vec<PathBuf>,
    files_modified: Vec<PathBuf>,
    source_edge_id: Option<u128>,
    tool_invocation_id: Option<u128>,
    tool_name: Option<String>,
    had_private_content: bool,
    created_at: HlcTimestamp,
    token_count: u32,
}

impl ObservationBuilder {
    /// Create a new observation builder.
    pub fn new(id: u128, session_id: u128, project_id: u128) -> Self {
        Self {
            id,
            session_id,
            project_id,
            observation_type: ObservationType::default(),
            title: String::new(),
            subtitle: String::new(),
            facts: Vec::new(),
            narrative: String::new(),
            concepts: Vec::new(),
            files_read: Vec::new(),
            files_modified: Vec::new(),
            source_edge_id: None,
            tool_invocation_id: None,
            tool_name: None,
            had_private_content: false,
            created_at: HlcTimestamp::default(),
            token_count: 0,
        }
    }

    /// Set the observation type.
    pub fn observation_type(mut self, t: ObservationType) -> Self {
        self.observation_type = t;
        self
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// Add a fact.
    pub fn add_fact(mut self, fact: impl Into<String>) -> Self {
        self.facts.push(fact.into());
        self
    }

    /// Set all facts.
    pub fn facts(mut self, facts: Vec<String>) -> Self {
        self.facts = facts;
        self
    }

    /// Set the narrative.
    pub fn narrative(mut self, narrative: impl Into<String>) -> Self {
        self.narrative = narrative.into();
        self
    }

    /// Add a concept.
    pub fn add_concept(mut self, concept: impl Into<String>) -> Self {
        self.concepts.push(Concept::new(concept));
        self
    }

    /// Set all concepts.
    pub fn concepts(mut self, concepts: Vec<String>) -> Self {
        self.concepts = concepts.into_iter().map(Concept::new).collect();
        self
    }

    /// Add a file that was read.
    pub fn add_file_read(mut self, path: impl Into<PathBuf>) -> Self {
        self.files_read.push(path.into());
        self
    }

    /// Set all files read.
    pub fn files_read(mut self, files: Vec<PathBuf>) -> Self {
        self.files_read = files;
        self
    }

    /// Add a file that was modified.
    pub fn add_file_modified(mut self, path: impl Into<PathBuf>) -> Self {
        self.files_modified.push(path.into());
        self
    }

    /// Set all files modified.
    pub fn files_modified(mut self, files: Vec<PathBuf>) -> Self {
        self.files_modified = files;
        self
    }

    /// Set the source edge ID.
    pub fn source_edge_id(mut self, id: u128) -> Self {
        self.source_edge_id = Some(id);
        self
    }

    /// Set the tool invocation ID.
    pub fn tool_invocation_id(mut self, id: u128) -> Self {
        self.tool_invocation_id = Some(id);
        self
    }

    /// Set the tool name.
    pub fn tool_name(mut self, name: impl Into<String>) -> Self {
        self.tool_name = Some(name.into());
        self
    }

    /// Set whether the observation had private content.
    pub fn had_private_content(mut self, had: bool) -> Self {
        self.had_private_content = had;
        self
    }

    /// Set the creation timestamp.
    pub fn created_at(mut self, ts: HlcTimestamp) -> Self {
        self.created_at = ts;
        self
    }

    /// Set the token count.
    pub fn token_count(mut self, count: u32) -> Self {
        self.token_count = count;
        self
    }

    /// Build the observation.
    pub fn build(self) -> Observation {
        Observation {
            id: self.id,
            session_id: self.session_id,
            project_id: self.project_id,
            observation_type: self.observation_type,
            title: self.title,
            subtitle: self.subtitle,
            facts: self.facts,
            narrative: self.narrative,
            concepts: self.concepts,
            files_read: self.files_read,
            files_modified: self.files_modified,
            source_edge_id: self.source_edge_id,
            tool_invocation_id: self.tool_invocation_id,
            tool_name: self.tool_name,
            had_private_content: self.had_private_content,
            created_at: self.created_at,
            token_count: self.token_count,
        }
    }
}

/// Errors that can occur during observation parsing.
#[derive(Debug, Error)]
pub enum ObservationParseError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid XML structure: {0}")]
    InvalidXml(String),

    #[error("Invalid observation type: {0}")]
    InvalidType(String),

    #[error("Regex error: {0}")]
    RegexError(String),
}

/// Parser for observations from XML.
pub struct ObservationParser;

impl ObservationParser {
    /// Parse an observation from XML string.
    pub fn parse(
        xml: &str,
        id: u128,
        session_id: u128,
        project_id: u128,
    ) -> Result<Observation, ObservationParseError> {
        let mut builder = Observation::builder(id, session_id, project_id);

        // Parse type
        if let Some(obs_type) = Self::extract_tag(xml, "type") {
            builder = builder.observation_type(obs_type.parse().unwrap_or_default());
        }

        // Parse title
        if let Some(title) = Self::extract_tag(xml, "title") {
            builder = builder.title(title);
        }

        // Parse subtitle
        if let Some(subtitle) = Self::extract_tag(xml, "subtitle") {
            builder = builder.subtitle(subtitle);
        }

        // Parse facts
        let facts = Self::extract_list(xml, "facts", "fact");
        builder = builder.facts(facts);

        // Parse narrative
        if let Some(narrative) = Self::extract_tag(xml, "narrative") {
            builder = builder.narrative(narrative);
        }

        // Parse concepts
        let concepts = Self::extract_list(xml, "concepts", "concept");
        builder = builder.concepts(concepts);

        // Parse files_read
        let files_read: Vec<PathBuf> = Self::extract_list(xml, "files_read", "file")
            .into_iter()
            .map(PathBuf::from)
            .collect();
        builder = builder.files_read(files_read);

        // Parse files_modified
        let files_modified: Vec<PathBuf> = Self::extract_list(xml, "files_modified", "file")
            .into_iter()
            .map(PathBuf::from)
            .collect();
        builder = builder.files_modified(files_modified);

        Ok(builder.build())
    }

    /// Parse with lenient fallback for malformed XML.
    pub fn parse_lenient(
        xml: &str,
        id: u128,
        session_id: u128,
        project_id: u128,
    ) -> Observation {
        Self::parse(xml, id, session_id, project_id).unwrap_or_else(|_| {
            // Fallback: create minimal observation with the raw text as narrative
            Observation::builder(id, session_id, project_id)
                .observation_type(ObservationType::Custom("unparsed".to_string()))
                .narrative(xml.to_string())
                .build()
        })
    }

    /// Extract content between XML tags.
    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);

        let start = xml.find(&open_tag)? + open_tag.len();
        let end = xml[start..].find(&close_tag)?;

        Some(xml[start..start + end].trim().to_string())
    }

    /// Extract a list of items from nested XML tags.
    fn extract_list(xml: &str, container_tag: &str, item_tag: &str) -> Vec<String> {
        let open_container = format!("<{}>", container_tag);
        let close_container = format!("</{}>", container_tag);

        let container_content = match xml.find(&open_container) {
            Some(start) => {
                let content_start = start + open_container.len();
                match xml[content_start..].find(&close_container) {
                    Some(end) => &xml[content_start..content_start + end],
                    None => return Vec::new(),
                }
            }
            None => return Vec::new(),
        };

        let open_item = format!("<{}>", item_tag);
        let close_item = format!("</{}>", item_tag);

        let mut items = Vec::new();
        let mut remaining = container_content;

        while let Some(start) = remaining.find(&open_item) {
            remaining = &remaining[start + open_item.len()..];
            if let Some(end) = remaining.find(&close_item) {
                items.push(remaining[..end].trim().to_string());
                remaining = &remaining[end + close_item.len()..];
            } else {
                break;
            }
        }

        items
    }
}

/// Filter for querying observations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservationFilter {
    /// Filter by project ID.
    pub project_id: Option<u128>,
    /// Filter by session ID.
    pub session_id: Option<u128>,
    /// Filter by observation types.
    pub types: Option<Vec<ObservationType>>,
    /// Filter by concepts (any match).
    pub concepts: Option<Vec<String>>,
    /// Filter by file path (any match).
    pub files: Option<Vec<PathBuf>>,
    /// Filter by time range start (inclusive).
    pub time_start: Option<HlcTimestamp>,
    /// Filter by time range end (exclusive).
    pub time_end: Option<HlcTimestamp>,
    /// Maximum number of results.
    pub limit: Option<usize>,
    /// Offset for pagination.
    pub offset: Option<usize>,
}

impl ObservationFilter {
    /// Create a new filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by project.
    pub fn project(mut self, project_id: u128) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Filter by session.
    pub fn session(mut self, session_id: u128) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Filter by types.
    pub fn types(mut self, types: Vec<ObservationType>) -> Self {
        self.types = Some(types);
        self
    }

    /// Filter by concepts.
    pub fn concepts(mut self, concepts: Vec<String>) -> Self {
        self.concepts = Some(concepts);
        self
    }

    /// Filter by files.
    pub fn files(mut self, files: Vec<PathBuf>) -> Self {
        self.files = Some(files);
        self
    }

    /// Filter by time range.
    pub fn time_range(mut self, start: HlcTimestamp, end: HlcTimestamp) -> Self {
        self.time_start = Some(start);
        self.time_end = Some(end);
        self
    }

    /// Set limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset.
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Check if an observation matches this filter.
    pub fn matches(&self, obs: &Observation) -> bool {
        if let Some(project_id) = self.project_id {
            if obs.project_id != project_id {
                return false;
            }
        }

        if let Some(session_id) = self.session_id {
            if obs.session_id != session_id {
                return false;
            }
        }

        if let Some(types) = &self.types {
            if !types.contains(&obs.observation_type) {
                return false;
            }
        }

        if let Some(concepts) = &self.concepts {
            let obs_concepts: Vec<&str> = obs.concepts.iter().map(|c| c.value.as_str()).collect();
            if !concepts.iter().any(|c| obs_concepts.contains(&c.as_str())) {
                return false;
            }
        }

        if let Some(files) = &self.files {
            let all_obs_files = obs.all_files();
            if !files.iter().any(|f| all_obs_files.contains(&f)) {
                return false;
            }
        }

        if let Some(start) = self.time_start {
            if obs.created_at < start {
                return false;
            }
        }

        if let Some(end) = self.time_end {
            if obs.created_at >= end {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concept_normalization() {
        assert_eq!(Concept::normalize("Rust Error Handling".to_string()), "rust-error-handling");
        assert_eq!(Concept::normalize("api_authentication".to_string()), "api-authentication");
        assert_eq!(Concept::normalize("--multiple--hyphens--".to_string()), "multiple-hyphens");
    }

    #[test]
    fn test_concept_parts() {
        let concept = Concept::new("rust-error-handling");
        assert_eq!(concept.parts(), vec!["rust", "error", "handling"]);
        assert_eq!(concept.category, "rust");
    }

    #[test]
    fn test_observation_builder() {
        let obs = Observation::builder(1, 2, 3)
            .title("Test observation")
            .subtitle("This is a test")
            .add_fact("Fact 1")
            .add_concept("rust-testing")
            .add_file_read("/src/main.rs")
            .build();

        assert_eq!(obs.id, 1);
        assert_eq!(obs.session_id, 2);
        assert_eq!(obs.project_id, 3);
        assert_eq!(obs.title, "Test observation");
        assert_eq!(obs.facts.len(), 1);
        assert_eq!(obs.concepts.len(), 1);
    }

    #[test]
    fn test_xml_parsing() {
        let xml = r#"
        <observation>
            <type>implementation</type>
            <title>Added error handling</title>
            <subtitle>Implemented Result-based error handling for file operations</subtitle>
            <facts>
                <fact>Added Result return type</fact>
                <fact>Created custom error enum</fact>
            </facts>
            <narrative>This change introduces proper error handling...</narrative>
            <concepts>
                <concept>rust-error-handling</concept>
                <concept>api-design</concept>
            </concepts>
            <files_read>
                <file>src/lib.rs</file>
            </files_read>
            <files_modified>
                <file>src/error.rs</file>
            </files_modified>
        </observation>
        "#;

        let obs = ObservationParser::parse(xml, 1, 2, 3).unwrap();

        assert_eq!(obs.observation_type, ObservationType::Implementation);
        assert_eq!(obs.title, "Added error handling");
        assert_eq!(obs.facts.len(), 2);
        assert_eq!(obs.concepts.len(), 2);
        assert_eq!(obs.files_read.len(), 1);
        assert_eq!(obs.files_modified.len(), 1);
    }

    #[test]
    fn test_observation_filter() {
        let obs = Observation::builder(1, 2, 3)
            .observation_type(ObservationType::Debugging)
            .add_concept("rust-testing")
            .build();

        let filter = ObservationFilter::new()
            .project(3)
            .types(vec![ObservationType::Debugging]);

        assert!(filter.matches(&obs));

        let non_matching_filter = ObservationFilter::new()
            .types(vec![ObservationType::Implementation]);

        assert!(!non_matching_filter.matches(&obs));
    }

    #[test]
    fn test_completeness_score() {
        let minimal = Observation::builder(1, 1, 1).build();
        assert!(minimal.completeness_score() < 0.5);

        let complete = Observation::builder(1, 1, 1)
            .title("Title")
            .subtitle("Subtitle")
            .add_fact("Fact")
            .narrative("Narrative")
            .add_concept("concept")
            .add_file_read("/file")
            .add_file_modified("/file2")
            .build();
        assert!((complete.completeness_score() - 1.0).abs() < f64::EPSILON);
    }
}
