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

//! Context builder implementation.

use super::config::ContextConfig;
use super::token_calculator::TokenCalculator;
use crate::observation::Observation;
use crate::session_summary::SessionSummary;
use serde::{Deserialize, Serialize};

/// A compiled context document ready for injection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDocument {
    /// The formatted context as markdown.
    pub content: String,
    /// Estimated token count.
    pub token_count: usize,
    /// Number of observations included.
    pub observation_count: usize,
    /// Number of summaries included.
    pub summary_count: usize,
    /// Project ID this context was built for.
    pub project_id: u128,
}

impl ContextDocument {
    /// Check if the context is empty.
    pub fn is_empty(&self) -> bool {
        self.observation_count == 0 && self.summary_count == 0
    }
}

/// Builder for generating context documents.
pub struct ContextBuilder {
    config: ContextConfig,
    token_calculator: TokenCalculator,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new(ContextConfig::default())
    }
}

impl ContextBuilder {
    /// Create a new context builder with the given configuration.
    pub fn new(config: ContextConfig) -> Self {
        Self {
            config,
            token_calculator: TokenCalculator::new(),
        }
    }

    /// Build context from observations and summaries.
    pub fn build(
        &self,
        project_id: u128,
        observations: Vec<Observation>,
        summaries: Vec<SessionSummary>,
    ) -> ContextDocument {
        let mut sections = Vec::new();
        let mut total_tokens = 0;

        // Add header
        let header = self.build_header(project_id);
        total_tokens += self.token_calculator.estimate(&header);
        sections.push(header);

        // Add session summaries section
        let summary_budget = self.config.summary_budget();
        let summaries_section = self.build_summaries_section(&summaries, summary_budget);
        let summary_count = summaries.len().min(self.config.max_summaries);
        total_tokens += self.token_calculator.estimate(&summaries_section);
        sections.push(summaries_section);

        // Add observations section
        let obs_budget = self.config.observation_budget();
        let (observations_section, obs_count) =
            self.build_observations_section(&observations, obs_budget);
        total_tokens += self.token_calculator.estimate(&observations_section);
        sections.push(observations_section);

        // Add footer
        let footer = self.build_footer(obs_count, summary_count);
        total_tokens += self.token_calculator.estimate(&footer);
        sections.push(footer);

        let content = sections.join("\n\n");

        ContextDocument {
            content,
            token_count: total_tokens,
            observation_count: obs_count,
            summary_count,
            project_id,
        }
    }

    fn build_header(&self, project_id: u128) -> String {
        format!(
            "# Project Memory Context\n\n\
            *Project ID: {:032x}*\n\n\
            This context contains relevant historical information from your previous sessions.",
            project_id
        )
    }

    fn build_footer(&self, obs_count: usize, summary_count: usize) -> String {
        format!(
            "---\n\n*Context includes {} observations and {} session summaries.*",
            obs_count, summary_count
        )
    }

    fn build_summaries_section(&self, summaries: &[SessionSummary], budget: usize) -> String {
        if summaries.is_empty() {
            return "## Recent Sessions\n\n*No previous sessions recorded.*".to_string();
        }

        let mut section = String::from("## Recent Sessions\n\n");
        let mut used_tokens = self.token_calculator.estimate(&section);

        for (i, summary) in summaries.iter().take(self.config.max_summaries).enumerate() {
            let entry = self.format_summary(summary);
            let entry_tokens = self.token_calculator.estimate(&entry);

            if used_tokens + entry_tokens > budget {
                break;
            }

            section.push_str(&entry);
            section.push_str("\n\n");
            used_tokens += entry_tokens;

            if i >= self.config.max_summaries - 1 {
                break;
            }
        }

        section
    }

    fn format_summary(&self, summary: &SessionSummary) -> String {
        let mut parts = vec![format!("### Session: {}", summary.request)];

        if !summary.completed.is_empty() {
            parts.push(format!("**Completed:** {}", summary.completed));
        }

        if !summary.learned.is_empty() {
            parts.push(format!("**Learned:** {}", summary.learned));
        }

        if summary.has_pending_work() {
            parts.push(format!("**Next Steps:** {}", summary.next_steps));
        }

        parts.join("\n")
    }

    fn build_observations_section(
        &self,
        observations: &[Observation],
        budget: usize,
    ) -> (String, usize) {
        if observations.is_empty() {
            return (
                "## Recent Activity\n\n*No recent activity recorded.*".to_string(),
                0,
            );
        }

        let mut section = String::from("## Recent Activity\n\n");
        let mut used_tokens = self.token_calculator.estimate(&section);
        let mut count = 0;

        // Recent observations (full detail)
        let full_detail_count = self.config.full_detail_count.min(observations.len());
        for obs in observations.iter().take(full_detail_count) {
            let entry = self.format_observation_full(obs);
            let entry_tokens = self.token_calculator.estimate(&entry);

            if used_tokens + entry_tokens > budget {
                break;
            }

            section.push_str(&entry);
            section.push_str("\n\n");
            used_tokens += entry_tokens;
            count += 1;
        }

        // Older observations (condensed)
        if count < observations.len() {
            let condensed_start = full_detail_count;
            let condensed_end = (condensed_start + self.config.condensed_count).min(observations.len());

            for obs in observations.iter().skip(condensed_start).take(condensed_end - condensed_start) {
                let entry = self.format_observation_condensed(obs);
                let entry_tokens = self.token_calculator.estimate(&entry);

                if used_tokens + entry_tokens > budget {
                    break;
                }

                section.push_str(&entry);
                section.push_str("\n");
                used_tokens += entry_tokens;
                count += 1;
            }
        }

        (section, count)
    }

    fn format_observation_full(&self, obs: &Observation) -> String {
        let mut parts = vec![
            format!("### {} [{}]", obs.title, obs.observation_type),
            obs.subtitle.clone(),
        ];

        if !obs.narrative.is_empty() {
            parts.push(obs.narrative.clone());
        }

        if self.config.include_concepts && !obs.concepts.is_empty() {
            let concepts: Vec<_> = obs.concepts.iter().map(|c| c.value.as_str()).collect();
            parts.push(format!("*Concepts: {}*", concepts.join(", ")));
        }

        if self.config.include_files {
            if !obs.files_modified.is_empty() {
                let files: Vec<_> = obs
                    .files_modified
                    .iter()
                    .filter_map(|p| p.to_str())
                    .collect();
                parts.push(format!("*Modified: {}*", files.join(", ")));
            }
        }

        parts.join("\n")
    }

    fn format_observation_condensed(&self, obs: &Observation) -> String {
        format!("- **{}**: {}", obs.title, obs.subtitle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation_types::ObservationType;

    fn test_observation(id: u128, title: &str) -> Observation {
        Observation::builder(id, 1, 1)
            .title(title)
            .subtitle("Test subtitle")
            .observation_type(ObservationType::Implementation)
            .build()
    }

    fn test_summary(request: &str) -> SessionSummary {
        SessionSummary::builder(1, 1)
            .request(request)
            .completed("Completed task")
            .build()
    }

    #[test]
    fn test_build_empty_context() {
        let builder = ContextBuilder::default();
        let doc = builder.build(1, vec![], vec![]);

        assert!(doc.observation_count == 0);
        assert!(doc.summary_count == 0);
        assert!(!doc.content.is_empty()); // Still has header/footer
    }

    #[test]
    fn test_build_with_observations() {
        let builder = ContextBuilder::default();
        let observations = vec![
            test_observation(1, "First task"),
            test_observation(2, "Second task"),
        ];

        let doc = builder.build(1, observations, vec![]);

        assert_eq!(doc.observation_count, 2);
        assert!(doc.content.contains("First task"));
        assert!(doc.content.contains("Second task"));
    }

    #[test]
    fn test_build_with_summaries() {
        let builder = ContextBuilder::default();
        let summaries = vec![test_summary("Implement feature X")];

        let doc = builder.build(1, vec![], summaries);

        assert_eq!(doc.summary_count, 1);
        assert!(doc.content.contains("Implement feature X"));
    }

    #[test]
    fn test_token_budget_respected() {
        let config = ContextConfig::default().with_token_budget(500);
        let builder = ContextBuilder::new(config);

        // Create many observations
        let observations: Vec<_> = (0..100)
            .map(|i| test_observation(i, &format!("Task {}", i)))
            .collect();

        let doc = builder.build(1, observations, vec![]);

        // Should be limited by budget
        assert!(doc.observation_count < 100);
        assert!(doc.token_count <= 600); // Some margin
    }
}
