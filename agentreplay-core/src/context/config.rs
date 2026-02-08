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

//! Context configuration.

use serde::{Deserialize, Serialize};

/// Configuration for context building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Maximum token budget for context.
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,

    /// Percentage of budget for observations (0.0-1.0).
    #[serde(default = "default_observation_budget")]
    pub observation_budget_pct: f64,

    /// Percentage of budget for summaries (0.0-1.0).
    #[serde(default = "default_summary_budget")]
    pub summary_budget_pct: f64,

    /// Number of recent observations to include at full detail.
    #[serde(default = "default_full_detail_count")]
    pub full_detail_count: usize,

    /// Number of observations to include at condensed detail.
    #[serde(default = "default_condensed_count")]
    pub condensed_count: usize,

    /// Maximum number of session summaries to include.
    #[serde(default = "default_max_summaries")]
    pub max_summaries: usize,

    /// Whether to include file lists in context.
    #[serde(default = "default_include_files")]
    pub include_files: bool,

    /// Whether to include concepts in context.
    #[serde(default = "default_include_concepts")]
    pub include_concepts: bool,
}

fn default_token_budget() -> usize {
    8000
}

fn default_observation_budget() -> f64 {
    0.60
}

fn default_summary_budget() -> f64 {
    0.30
}

fn default_full_detail_count() -> usize {
    10
}

fn default_condensed_count() -> usize {
    20
}

fn default_max_summaries() -> usize {
    5
}

fn default_include_files() -> bool {
    true
}

fn default_include_concepts() -> bool {
    true
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            token_budget: default_token_budget(),
            observation_budget_pct: default_observation_budget(),
            summary_budget_pct: default_summary_budget(),
            full_detail_count: default_full_detail_count(),
            condensed_count: default_condensed_count(),
            max_summaries: default_max_summaries(),
            include_files: default_include_files(),
            include_concepts: default_include_concepts(),
        }
    }
}

impl ContextConfig {
    /// Get the observation token budget.
    pub fn observation_budget(&self) -> usize {
        (self.token_budget as f64 * self.observation_budget_pct) as usize
    }

    /// Get the summary token budget.
    pub fn summary_budget(&self) -> usize {
        (self.token_budget as f64 * self.summary_budget_pct) as usize
    }

    /// Get the header/footer token budget.
    pub fn header_budget(&self) -> usize {
        self.token_budget - self.observation_budget() - self.summary_budget()
    }

    /// Create a configuration with a custom token budget.
    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.token_budget = budget;
        self
    }

    /// Create a configuration with custom detail counts.
    pub fn with_detail_counts(mut self, full: usize, condensed: usize) -> Self {
        self.full_detail_count = full;
        self.condensed_count = condensed;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ContextConfig::default();
        assert_eq!(config.token_budget, 8000);
        assert!((config.observation_budget_pct - 0.60).abs() < f64::EPSILON);
    }

    #[test]
    fn test_budget_allocation() {
        let config = ContextConfig::default();
        let obs = config.observation_budget();
        let sum = config.summary_budget();
        let hdr = config.header_budget();

        assert_eq!(obs + sum + hdr, config.token_budget);
    }
}
