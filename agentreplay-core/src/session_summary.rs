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

//! Session Summary Generation
//!
//! End-of-session summaries for progress tracking and session continuity.
//!
//! # Summary Structure
//!
//! Session summaries capture:
//! - Original user request
//! - What was investigated/explored
//! - Key discoveries and learnings
//! - What was accomplished
//! - Pending work and next steps
//!
//! # Key Encoding
//!
//! - Storage key: `summaries/{project_id}/{session_id:032x}`

use crate::edge::HlcTimestamp;
use serde::{Deserialize, Serialize};

/// End-of-session summary with progress tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Unique session identifier.
    pub session_id: u128,
    /// Project this session belongs to.
    pub project_id: u128,
    /// Original user request that started the session.
    pub request: String,
    /// What was investigated/explored during the session.
    pub investigated: String,
    /// Key discoveries and learnings.
    pub learned: String,
    /// What was accomplished/completed.
    pub completed: String,
    /// Pending work and next steps.
    pub next_steps: String,
    /// Number of observations generated during the session.
    pub observation_count: u32,
    /// Number of prompts in the session.
    pub prompt_count: u32,
    /// Session duration in milliseconds.
    pub duration_ms: u64,
    /// Files that were read during the session.
    pub files_read: Vec<String>,
    /// Files that were modified during the session.
    pub files_modified: Vec<String>,
    /// Key concepts explored during the session.
    pub key_concepts: Vec<String>,
    /// When the summary was created.
    pub created_at: HlcTimestamp,
    /// Session start time (microseconds since epoch).
    pub session_start_us: u64,
    /// Session end time (microseconds since epoch).
    pub session_end_us: u64,
    /// Total tokens used by the memory agent.
    pub total_tokens_used: u64,
}

impl SessionSummary {
    /// Create a new session summary builder.
    pub fn builder(session_id: u128, project_id: u128) -> SessionSummaryBuilder {
        SessionSummaryBuilder::new(session_id, project_id)
    }

    /// Get the storage key for this summary.
    pub fn storage_key(&self) -> String {
        format!(
            "summary/{:032x}/{:032x}",
            self.project_id, self.session_id
        )
    }

    /// Check if the session has pending work.
    pub fn has_pending_work(&self) -> bool {
        !self.next_steps.is_empty() && !self.next_steps.to_lowercase().contains("none")
    }

    /// Calculate session productivity score.
    pub fn productivity_score(&self) -> f64 {
        let mut score = 0.0;

        // Base score from observations
        score += (self.observation_count as f64).min(10.0) / 10.0 * 0.3;

        // Score from completions
        if !self.completed.is_empty() {
            score += 0.3;
        }

        // Score from learnings
        if !self.learned.is_empty() {
            score += 0.2;
        }

        // Score from files modified
        score += (self.files_modified.len() as f64).min(5.0) / 5.0 * 0.2;

        score.min(1.0)
    }
}

/// Builder for session summaries.
pub struct SessionSummaryBuilder {
    session_id: u128,
    project_id: u128,
    request: String,
    investigated: String,
    learned: String,
    completed: String,
    next_steps: String,
    observation_count: u32,
    prompt_count: u32,
    duration_ms: u64,
    files_read: Vec<String>,
    files_modified: Vec<String>,
    key_concepts: Vec<String>,
    created_at: HlcTimestamp,
    session_start_us: u64,
    session_end_us: u64,
    total_tokens_used: u64,
}

impl SessionSummaryBuilder {
    /// Create a new builder.
    pub fn new(session_id: u128, project_id: u128) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        Self {
            session_id,
            project_id,
            request: String::new(),
            investigated: String::new(),
            learned: String::new(),
            completed: String::new(),
            next_steps: String::new(),
            observation_count: 0,
            prompt_count: 0,
            duration_ms: 0,
            files_read: Vec::new(),
            files_modified: Vec::new(),
            key_concepts: Vec::new(),
            created_at: HlcTimestamp::default(),
            session_start_us: now,
            session_end_us: now,
            total_tokens_used: 0,
        }
    }

    /// Set the original request.
    pub fn request(mut self, request: impl Into<String>) -> Self {
        self.request = request.into();
        self
    }

    /// Set what was investigated.
    pub fn investigated(mut self, investigated: impl Into<String>) -> Self {
        self.investigated = investigated.into();
        self
    }

    /// Set what was learned.
    pub fn learned(mut self, learned: impl Into<String>) -> Self {
        self.learned = learned.into();
        self
    }

    /// Set what was completed.
    pub fn completed(mut self, completed: impl Into<String>) -> Self {
        self.completed = completed.into();
        self
    }

    /// Set next steps.
    pub fn next_steps(mut self, next_steps: impl Into<String>) -> Self {
        self.next_steps = next_steps.into();
        self
    }

    /// Set observation count.
    pub fn observation_count(mut self, count: u32) -> Self {
        self.observation_count = count;
        self
    }

    /// Set prompt count.
    pub fn prompt_count(mut self, count: u32) -> Self {
        self.prompt_count = count;
        self
    }

    /// Set duration.
    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    /// Set files read.
    pub fn files_read(mut self, files: Vec<String>) -> Self {
        self.files_read = files;
        self
    }

    /// Set files modified.
    pub fn files_modified(mut self, files: Vec<String>) -> Self {
        self.files_modified = files;
        self
    }

    /// Set key concepts.
    pub fn key_concepts(mut self, concepts: Vec<String>) -> Self {
        self.key_concepts = concepts;
        self
    }

    /// Set creation timestamp.
    pub fn created_at(mut self, ts: HlcTimestamp) -> Self {
        self.created_at = ts;
        self
    }

    /// Set session times.
    pub fn session_times(mut self, start_us: u64, end_us: u64) -> Self {
        self.session_start_us = start_us;
        self.session_end_us = end_us;
        self.duration_ms = (end_us - start_us) / 1000;
        self
    }

    /// Set total tokens used.
    pub fn total_tokens_used(mut self, tokens: u64) -> Self {
        self.total_tokens_used = tokens;
        self
    }

    /// Build the summary.
    pub fn build(self) -> SessionSummary {
        SessionSummary {
            session_id: self.session_id,
            project_id: self.project_id,
            request: self.request,
            investigated: self.investigated,
            learned: self.learned,
            completed: self.completed,
            next_steps: self.next_steps,
            observation_count: self.observation_count,
            prompt_count: self.prompt_count,
            duration_ms: self.duration_ms,
            files_read: self.files_read,
            files_modified: self.files_modified,
            key_concepts: self.key_concepts,
            created_at: self.created_at,
            session_start_us: self.session_start_us,
            session_end_us: self.session_end_us,
            total_tokens_used: self.total_tokens_used,
        }
    }
}

/// Parser for session summaries from LLM output.
pub struct SessionSummaryParser;

impl SessionSummaryParser {
    /// Parse a session summary from XML.
    pub fn parse(
        xml: &str,
        session_id: u128,
        project_id: u128,
    ) -> SessionSummary {
        let mut builder = SessionSummary::builder(session_id, project_id);

        if let Some(request) = Self::extract_tag(xml, "request") {
            builder = builder.request(request);
        }
        if let Some(investigated) = Self::extract_tag(xml, "investigated") {
            builder = builder.investigated(investigated);
        }
        if let Some(learned) = Self::extract_tag(xml, "learned") {
            builder = builder.learned(learned);
        }
        if let Some(completed) = Self::extract_tag(xml, "completed") {
            builder = builder.completed(completed);
        }
        if let Some(next_steps) = Self::extract_tag(xml, "next_steps") {
            builder = builder.next_steps(next_steps);
        }

        builder.build()
    }

    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);

        let start = xml.find(&open_tag)? + open_tag.len();
        let end = xml[start..].find(&close_tag)?;

        Some(xml[start..start + end].trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_builder() {
        let summary = SessionSummary::builder(1, 2)
            .request("Implement feature X")
            .completed("Feature X implemented")
            .next_steps("Add tests")
            .observation_count(5)
            .build();

        assert_eq!(summary.session_id, 1);
        assert_eq!(summary.project_id, 2);
        assert_eq!(summary.request, "Implement feature X");
        assert!(summary.has_pending_work());
    }

    #[test]
    fn test_storage_key() {
        let summary = SessionSummary::builder(123, 456).build();
        let key = summary.storage_key();
        assert!(key.starts_with("summary/"));
    }

    #[test]
    fn test_productivity_score() {
        let low_productivity = SessionSummary::builder(1, 1).build();
        assert!(low_productivity.productivity_score() < 0.5);

        let high_productivity = SessionSummary::builder(1, 1)
            .observation_count(10)
            .completed("Done")
            .learned("New stuff")
            .files_modified(vec!["a.rs".to_string(), "b.rs".to_string()])
            .build();
        assert!(high_productivity.productivity_score() > 0.7);
    }

    #[test]
    fn test_parse_summary() {
        let xml = r#"
        <summary>
            <request>Fix bug in parser</request>
            <investigated>Root cause analysis</investigated>
            <learned>Edge case handling</learned>
            <completed>Bug fixed</completed>
            <next_steps>Add regression tests</next_steps>
        </summary>
        "#;

        let summary = SessionSummaryParser::parse(xml, 1, 2);
        assert_eq!(summary.request, "Fix bug in parser");
        assert_eq!(summary.completed, "Bug fixed");
    }
}
