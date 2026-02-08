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

//! Context packing and export
//!
//! Packs relevant memory into a context artifact (MDC file, JSON, or raw text)
//! for injection into editors and agents.

use crate::observation::{Observation, ObservationCategory};
use crate::session::SessionSummary;
use serde::{Deserialize, Serialize};

/// Specification for what context to pack
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSpec {
    /// Workspace to get context for
    pub workspace_id: String,
    /// Maximum token budget for the context
    pub token_budget: usize,
    /// Sections to include
    pub sections: Vec<ContextSection>,
    /// Semantic query to prioritize relevant content
    pub semantic_query: Option<String>,
    /// Output format
    pub format: ContextFormat,
}

/// Sections that can be included in context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSection {
    /// Key decisions made in this project
    Decisions,
    /// Code patterns and conventions
    Patterns,
    /// Recent session summaries
    RecentSessions,
    /// Saved observations
    Observations,
    /// User preferences
    Preferences,
    /// Open TODOs
    Todos,
    /// Rolling summary of project
    RollingSummary,
}

impl Default for ContextSection {
    fn default() -> Self {
        Self::Observations
    }
}

/// Output format for packed context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContextFormat {
    /// Markdown with frontmatter (for .mdc files)
    #[default]
    Mdc,
    /// Plain markdown
    Markdown,
    /// JSON structure
    Json,
    /// Plain text
    Text,
}

impl ContextSpec {
    /// Create a spec for a workspace with default sections
    pub fn for_workspace(workspace_id: impl Into<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            token_budget: 4_000,
            sections: vec![
                ContextSection::Decisions,
                ContextSection::Patterns,
                ContextSection::RecentSessions,
                ContextSection::Observations,
            ],
            semantic_query: None,
            format: ContextFormat::Mdc,
        }
    }

    /// Set token budget
    pub fn token_budget(mut self, budget: usize) -> Self {
        self.token_budget = budget;
        self
    }

    /// Set sections
    pub fn sections(mut self, sections: Vec<ContextSection>) -> Self {
        self.sections = sections;
        self
    }

    /// Add semantic query for relevance ranking
    pub fn semantic_query(mut self, query: impl Into<String>) -> Self {
        self.semantic_query = Some(query.into());
        self
    }

    /// Set output format
    pub fn format(mut self, format: ContextFormat) -> Self {
        self.format = format;
        self
    }
}

/// Packed context ready for injection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackedContext {
    /// Workspace this context is for
    pub workspace_id: String,
    /// Rendered content
    pub content: String,
    /// Format of the content
    pub format: ContextFormat,
    /// Approximate token count
    pub token_count: usize,
    /// Sections included
    pub sections_included: Vec<ContextSection>,
    /// Number of observations included
    pub observation_count: usize,
    /// Number of sessions included
    pub session_count: usize,
    /// Whether content was truncated due to token budget
    pub truncated: bool,
}

/// Context packer that assembles context from memory
pub struct ContextPacker {
    /// Token budget
    token_budget: usize,
    /// Approximate tokens per character (for estimation)
    tokens_per_char: f32,
}

impl Default for ContextPacker {
    fn default() -> Self {
        Self::new(4_000)
    }
}

impl ContextPacker {
    /// Create a new context packer with a token budget
    pub fn new(token_budget: usize) -> Self {
        Self {
            token_budget,
            tokens_per_char: 0.25, // Rough approximation
        }
    }

    /// Pack context from observations and session summaries
    pub fn pack(
        &self,
        spec: &ContextSpec,
        observations: &[Observation],
        sessions: &[SessionSummary],
    ) -> PackedContext {
        let mut sections_content: Vec<(ContextSection, String)> = Vec::new();
        let mut total_tokens = 0;
        let mut truncated = false;

        for section in &spec.sections {
            let content = self.render_section(*section, observations, sessions);
            let section_tokens = self.estimate_tokens(&content);

            if total_tokens + section_tokens <= self.token_budget {
                total_tokens += section_tokens;
                sections_content.push((*section, content));
            } else {
                // Try to include partial content
                let remaining = self.token_budget.saturating_sub(total_tokens);
                if remaining > 100 {
                    let truncated_content =
                        self.truncate_to_tokens(&content, remaining);
                    total_tokens += self.estimate_tokens(&truncated_content);
                    sections_content.push((*section, truncated_content));
                }
                truncated = true;
                break;
            }
        }

        let content = self.format_output(&spec.format, &sections_content, &spec.workspace_id);
        let sections_included: Vec<ContextSection> =
            sections_content.iter().map(|(s, _)| *s).collect();

        PackedContext {
            workspace_id: spec.workspace_id.clone(),
            content,
            format: spec.format,
            token_count: total_tokens,
            sections_included,
            observation_count: observations.len(),
            session_count: sessions.len(),
            truncated,
        }
    }

    fn render_section(
        &self,
        section: ContextSection,
        observations: &[Observation],
        sessions: &[SessionSummary],
    ) -> String {
        match section {
            ContextSection::Decisions => {
                let decisions: Vec<_> = observations
                    .iter()
                    .filter(|o| o.category == ObservationCategory::Decision)
                    .collect();
                if decisions.is_empty() {
                    return String::new();
                }
                let mut content = String::from("## Decisions\n\n");
                for obs in decisions.iter().take(10) {
                    content.push_str(&format!("- {}\n", obs.content));
                }
                content
            }
            ContextSection::Patterns => {
                let patterns: Vec<_> = observations
                    .iter()
                    .filter(|o| o.category == ObservationCategory::Pattern)
                    .collect();
                if patterns.is_empty() {
                    return String::new();
                }
                let mut content = String::from("## Patterns\n\n");
                for obs in patterns.iter().take(10) {
                    content.push_str(&format!("- {}\n", obs.content));
                }
                content
            }
            ContextSection::RecentSessions => {
                if sessions.is_empty() {
                    return String::new();
                }
                let mut content = String::from("## Recent Sessions\n\n");
                for session in sessions.iter().take(5) {
                    content.push_str(&format!(
                        "### {} - {}\n{}\n\n",
                        session.started_at.format("%Y-%m-%d %H:%M"),
                        session
                            .topics
                            .first()
                            .map(|t| t.as_str())
                            .unwrap_or("Session"),
                        session.summary
                    ));
                }
                content
            }
            ContextSection::Observations => {
                let general: Vec<_> = observations
                    .iter()
                    .filter(|o| {
                        o.category == ObservationCategory::Note
                            || o.category == ObservationCategory::Fact
                            || o.category == ObservationCategory::Insight
                    })
                    .collect();
                if general.is_empty() {
                    return String::new();
                }
                let mut content = String::from("## Observations\n\n");
                for obs in general.iter().take(15) {
                    content.push_str(&format!("- {}\n", obs.content));
                }
                content
            }
            ContextSection::Preferences => {
                let prefs: Vec<_> = observations
                    .iter()
                    .filter(|o| o.category == ObservationCategory::Preference)
                    .collect();
                if prefs.is_empty() {
                    return String::new();
                }
                let mut content = String::from("## User Preferences\n\n");
                for obs in prefs.iter().take(10) {
                    content.push_str(&format!("- {}\n", obs.content));
                }
                content
            }
            ContextSection::Todos => {
                let todos: Vec<_> = observations
                    .iter()
                    .filter(|o| o.category == ObservationCategory::Todo)
                    .collect();
                if todos.is_empty() {
                    return String::new();
                }
                let mut content = String::from("## TODOs\n\n");
                for obs in todos.iter().take(10) {
                    content.push_str(&format!("- [ ] {}\n", obs.content));
                }
                content
            }
            ContextSection::RollingSummary => {
                // This would come from a computed rolling summary
                String::new()
            }
        }
    }

    fn format_output(
        &self,
        format: &ContextFormat,
        sections: &[(ContextSection, String)],
        workspace_id: &str,
    ) -> String {
        let body: String = sections
            .iter()
            .filter(|(_, content)| !content.is_empty())
            .map(|(_, content)| content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        match format {
            ContextFormat::Mdc => {
                format!(
                    r#"---
description: Agentreplay memory context for {}
alwaysApply: true
---

# Memory Context

{}
"#,
                    workspace_id, body
                )
            }
            ContextFormat::Markdown => {
                format!("# Memory Context\n\n{}", body)
            }
            ContextFormat::Json => {
                let json_sections: Vec<_> = sections
                    .iter()
                    .filter(|(_, content)| !content.is_empty())
                    .map(|(section, content)| {
                        serde_json::json!({
                            "section": format!("{:?}", section),
                            "content": content
                        })
                    })
                    .collect();
                serde_json::to_string_pretty(&json_sections).unwrap_or_default()
            }
            ContextFormat::Text => body,
        }
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        (text.len() as f32 * self.tokens_per_char) as usize
    }

    fn truncate_to_tokens(&self, text: &str, max_tokens: usize) -> String {
        let max_chars = (max_tokens as f32 / self.tokens_per_char) as usize;
        if text.len() <= max_chars {
            return text.to_string();
        }
        // Find a good break point
        let truncated = &text[..max_chars.min(text.len())];
        if let Some(pos) = truncated.rfind('\n') {
            format!("{}...", &text[..pos])
        } else {
            format!("{}...", truncated)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::Observation;

    #[test]
    fn test_context_packer() {
        let observations = vec![
            Observation::new("ws", "s1")
                .content("Use explicit error handling")
                .category(ObservationCategory::Decision),
            Observation::new("ws", "s1")
                .content("All public functions should have documentation")
                .category(ObservationCategory::Pattern),
            Observation::new("ws", "s1")
                .content("The auth module needs refactoring")
                .category(ObservationCategory::Todo),
        ];

        let sessions = vec![];

        let spec = ContextSpec::for_workspace("ws")
            .sections(vec![
                ContextSection::Decisions,
                ContextSection::Patterns,
                ContextSection::Todos,
            ])
            .token_budget(2000);

        let packer = ContextPacker::new(2000);
        let packed = packer.pack(&spec, &observations, &sessions);

        assert!(packed.content.contains("Use explicit error handling"));
        assert!(packed.content.contains("public functions should have documentation"));
        assert!(packed.content.contains("auth module needs refactoring"));
        assert_eq!(packed.observation_count, 3);
    }

    #[test]
    fn test_mdc_format() {
        let observations = vec![Observation::new("my-project", "s1")
            .content("Important decision")
            .category(ObservationCategory::Decision)];

        let spec = ContextSpec::for_workspace("my-project")
            .format(ContextFormat::Mdc)
            .sections(vec![ContextSection::Decisions]);

        let packer = ContextPacker::new(2000);
        let packed = packer.pack(&spec, &observations, &[]);

        assert!(packed.content.contains("---"));
        assert!(packed.content.contains("description:"));
        assert!(packed.content.contains("alwaysApply: true"));
    }
}
