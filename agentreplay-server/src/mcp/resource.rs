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

//! MCP Context Resource implementation.

use serde::{Deserialize, Serialize};

/// MCP context resource configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResourceConfig {
    /// Maximum observations to return.
    pub max_observations: usize,
    /// Maximum token budget for context.
    pub max_tokens: usize,
    /// Whether to include summaries.
    pub include_summaries: bool,
    /// Whether to filter by recency.
    pub recency_weight: f32,
}

impl Default for ContextResourceConfig {
    fn default() -> Self {
        Self {
            max_observations: 20,
            max_tokens: 8000,
            include_summaries: true,
            recency_weight: 0.3,
        }
    }
}

/// Request for context from MCP client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequest {
    /// Project ID.
    pub project_id: u128,
    /// Session ID (optional - for session-specific context).
    pub session_id: Option<u128>,
    /// Maximum observations.
    pub max_observations: Option<usize>,
    /// Maximum tokens.
    pub max_tokens: Option<usize>,
    /// Concepts to filter by.
    pub concepts: Option<Vec<String>>,
    /// Minimum timestamp.
    pub since: Option<u64>,
    /// Query string for semantic matching.
    pub query: Option<String>,
}

/// Response with context for MCP client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResponse {
    /// Formatted context string.
    pub context: String,
    /// Observations included.
    pub observation_ids: Vec<u128>,
    /// Token count estimate.
    pub token_count: usize,
    /// Session summaries if included.
    pub summaries: Vec<SessionSummaryResponse>,
    /// Metadata.
    pub metadata: ContextMetadata,
}

/// Session summary in response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryResponse {
    /// Session ID.
    pub session_id: u128,
    /// Summary content.
    pub summary: String,
    /// Observation count in session.
    pub observation_count: usize,
}

/// Context response metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// Total observations available.
    pub total_observations: usize,
    /// Observations returned.
    pub returned_observations: usize,
    /// Was result truncated.
    pub truncated: bool,
    /// Time range covered.
    pub time_range: Option<(u64, u64)>,
}

/// MCP Context Resource.
pub struct McpContextResource {
    config: ContextResourceConfig,
}

impl Default for McpContextResource {
    fn default() -> Self {
        Self::new(ContextResourceConfig::default())
    }
}

impl McpContextResource {
    /// Create a new context resource.
    pub fn new(config: ContextResourceConfig) -> Self {
        Self { config }
    }

    /// Get resource URI for MCP.
    pub fn resource_uri(&self, project_id: u128) -> String {
        format!("agentreplay://context/{:032x}", project_id)
    }

    /// Get resource description for MCP.
    pub fn resource_description(&self) -> &'static str {
        "AgentReplay observations and session context for the current project"
    }

    /// Get resource mime type.
    pub fn mime_type(&self) -> &'static str {
        "text/markdown"
    }

    /// Build context from observations.
    ///
    /// This is a placeholder - actual implementation would query
    /// ObservationStore and build formatted context.
    pub fn build_context(&self, request: &ContextRequest) -> ContextResponse {
        let _max_obs = request
            .max_observations
            .unwrap_or(self.config.max_observations);
        let _max_tokens = request.max_tokens.unwrap_or(self.config.max_tokens);

        // Placeholder - would query storage
        ContextResponse {
            context: format!(
                "# AgentReplay Context\n\n*Project: {:032x}*\n\n_Context would be generated here from observations._",
                request.project_id
            ),
            observation_ids: vec![],
            token_count: 50,
            summaries: vec![],
            metadata: ContextMetadata {
                total_observations: 0,
                returned_observations: 0,
                truncated: false,
                time_range: None,
            },
        }
    }

    /// Format observations as markdown.
    pub fn format_observation_markdown(
        _id: u128,
        obs_type: &str,
        title: &str,
        narrative: &str,
        facts: &[String],
        concepts: &[String],
    ) -> String {
        let mut output = String::new();

        output.push_str(&format!("## {}\n\n", title));
        output.push_str(&format!("*Type: {}*\n\n", obs_type));
        output.push_str(&format!("{}\n\n", narrative));

        if !facts.is_empty() {
            output.push_str("**Key Facts:**\n");
            for fact in facts {
                output.push_str(&format!("- {}\n", fact));
            }
            output.push('\n');
        }

        if !concepts.is_empty() {
            output.push_str(&format!("*Concepts: {}*\n\n", concepts.join(", ")));
        }

        output.push_str("---\n\n");

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_uri() {
        let resource = McpContextResource::default();
        let uri = resource.resource_uri(12345);
        assert!(uri.starts_with("agentreplay://context/"));
    }

    #[test]
    fn test_build_context() {
        let resource = McpContextResource::default();
        let request = ContextRequest {
            project_id: 100,
            session_id: None,
            max_observations: Some(10),
            max_tokens: None,
            concepts: None,
            since: None,
            query: None,
        };

        let response = resource.build_context(&request);
        assert!(response.context.contains("AgentReplay Context"));
    }

    #[test]
    fn test_format_observation() {
        let md = McpContextResource::format_observation_markdown(
            1,
            "implementation",
            "Added authentication",
            "Implemented OAuth2 flow",
            &["Uses JWT".to_string()],
            &["oauth2".to_string(), "auth".to_string()],
        );

        assert!(md.contains("Added authentication"));
        assert!(md.contains("OAuth2"));
        assert!(md.contains("Uses JWT"));
    }
}
