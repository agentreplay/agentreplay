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

//! Observation Quality Metrics
//!
//! Scoring and validation of generated observations.
//!
//! # Quality Dimensions
//!
//! - **Completeness**: All required fields present
//! - **Specificity**: Concrete vs vague content
//! - **Actionability**: Useful for future context
//! - **Correctness**: Valid observation type, concepts

use serde::{Deserialize, Serialize};

/// Quality score for an observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    /// Overall quality score (0.0-1.0).
    pub overall: f32,
    /// Completeness score.
    pub completeness: f32,
    /// Specificity score.
    pub specificity: f32,
    /// Actionability score.
    pub actionability: f32,
    /// Correctness score.
    pub correctness: f32,
    /// Individual dimension scores.
    pub dimensions: Vec<DimensionScore>,
    /// Validation issues found.
    pub issues: Vec<QualityIssue>,
}

impl QualityScore {
    /// Check if observation passes minimum quality threshold.
    pub fn passes_threshold(&self, threshold: f32) -> bool {
        self.overall >= threshold
    }

    /// Get primary issues (most severe).
    pub fn primary_issues(&self) -> Vec<&QualityIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::High)
            .collect()
    }
}

/// Individual dimension score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    /// Dimension name.
    pub name: String,
    /// Score (0.0-1.0).
    pub score: f32,
    /// Weight in overall calculation.
    pub weight: f32,
    /// Explanation.
    pub reason: String,
}

/// Quality issue found during validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    /// Issue code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Issue severity.
    pub severity: IssueSeverity,
    /// Field that has the issue.
    pub field: Option<String>,
    /// Suggestion for fixing.
    pub suggestion: Option<String>,
}

/// Issue severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
}

/// Observation input for quality scoring.
#[derive(Debug, Clone)]
pub struct ObservationInput {
    pub observation_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub facts: Vec<String>,
    pub narrative: String,
    pub concepts: Vec<String>,
    pub files_read: Vec<String>,
    pub files_modified: Vec<String>,
}

/// Quality metrics calculator.
pub struct QualityMetrics {
    /// Minimum title length.
    pub min_title_length: usize,
    /// Maximum title length.
    pub max_title_length: usize,
    /// Minimum narrative length.
    pub min_narrative_length: usize,
    /// Minimum facts for good completeness.
    pub min_facts: usize,
    /// Minimum concepts for good completeness.
    pub min_concepts: usize,
    /// Valid observation types.
    pub valid_types: Vec<String>,
    /// Vague words to penalize.
    pub vague_words: Vec<String>,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            min_title_length: 10,
            max_title_length: 100,
            min_narrative_length: 50,
            min_facts: 1,
            min_concepts: 1,
            valid_types: vec![
                "implementation".to_string(),
                "debugging".to_string(),
                "refactoring".to_string(),
                "testing".to_string(),
                "architecture".to_string(),
                "design".to_string(),
                "research".to_string(),
                "documentation".to_string(),
                "configuration".to_string(),
                "review".to_string(),
                "learning".to_string(),
                "planning".to_string(),
            ],
            vague_words: vec![
                "something".to_string(),
                "stuff".to_string(),
                "things".to_string(),
                "various".to_string(),
                "etc".to_string(),
                "some".to_string(),
                "maybe".to_string(),
                "probably".to_string(),
            ],
        }
    }
}

impl QualityMetrics {
    /// Score an observation.
    pub fn score(&self, observation: &ObservationInput) -> QualityScore {
        let mut dimensions = Vec::new();
        let mut issues = Vec::new();

        // Completeness
        let (completeness, comp_issues) = self.score_completeness(observation);
        dimensions.push(DimensionScore {
            name: "completeness".to_string(),
            score: completeness,
            weight: 0.3,
            reason: format!("{} required fields present", if completeness > 0.8 { "All" } else { "Some" }),
        });
        issues.extend(comp_issues);

        // Specificity
        let (specificity, spec_issues) = self.score_specificity(observation);
        dimensions.push(DimensionScore {
            name: "specificity".to_string(),
            score: specificity,
            weight: 0.3,
            reason: format!("Content is {}", if specificity > 0.7 { "specific" } else { "vague" }),
        });
        issues.extend(spec_issues);

        // Actionability
        let (actionability, action_issues) = self.score_actionability(observation);
        dimensions.push(DimensionScore {
            name: "actionability".to_string(),
            score: actionability,
            weight: 0.2,
            reason: "Usefulness for future context".to_string(),
        });
        issues.extend(action_issues);

        // Correctness
        let (correctness, corr_issues) = self.score_correctness(observation);
        dimensions.push(DimensionScore {
            name: "correctness".to_string(),
            score: correctness,
            weight: 0.2,
            reason: "Schema validity".to_string(),
        });
        issues.extend(corr_issues);

        // Calculate weighted overall score
        let overall = dimensions
            .iter()
            .map(|d| d.score * d.weight)
            .sum::<f32>()
            / dimensions.iter().map(|d| d.weight).sum::<f32>();

        QualityScore {
            overall,
            completeness,
            specificity,
            actionability,
            correctness,
            dimensions,
            issues,
        }
    }

    fn score_completeness(&self, obs: &ObservationInput) -> (f32, Vec<QualityIssue>) {
        let mut score: f32 = 1.0;
        let mut issues = Vec::new();

        // Title
        if obs.title.is_empty() {
            score -= 0.3;
            issues.push(QualityIssue {
                code: "MISSING_TITLE".to_string(),
                message: "Title is required".to_string(),
                severity: IssueSeverity::High,
                field: Some("title".to_string()),
                suggestion: Some("Add a descriptive title".to_string()),
            });
        }

        // Facts
        if obs.facts.is_empty() {
            score -= 0.2;
            issues.push(QualityIssue {
                code: "NO_FACTS".to_string(),
                message: "No facts recorded".to_string(),
                severity: IssueSeverity::Medium,
                field: Some("facts".to_string()),
                suggestion: Some("Add at least one concrete fact".to_string()),
            });
        }

        // Narrative
        if obs.narrative.is_empty() {
            score -= 0.3;
            issues.push(QualityIssue {
                code: "MISSING_NARRATIVE".to_string(),
                message: "Narrative is required".to_string(),
                severity: IssueSeverity::High,
                field: Some("narrative".to_string()),
                suggestion: Some("Add a narrative description".to_string()),
            });
        }

        // Concepts
        if obs.concepts.is_empty() {
            score -= 0.1;
            issues.push(QualityIssue {
                code: "NO_CONCEPTS".to_string(),
                message: "No concepts extracted".to_string(),
                severity: IssueSeverity::Low,
                field: Some("concepts".to_string()),
                suggestion: Some("Add relevant concepts for indexing".to_string()),
            });
        }

        // Observation type
        if obs.observation_type.is_empty() {
            score -= 0.1;
            issues.push(QualityIssue {
                code: "MISSING_TYPE".to_string(),
                message: "Observation type is required".to_string(),
                severity: IssueSeverity::Medium,
                field: Some("observation_type".to_string()),
                suggestion: None,
            });
        }

        (score.max(0.0), issues)
    }

    fn score_specificity(&self, obs: &ObservationInput) -> (f32, Vec<QualityIssue>) {
        let mut score: f32 = 1.0;
        let mut issues = Vec::new();

        // Check for vague words in narrative
        let narrative_lower = obs.narrative.to_lowercase();
        let vague_count = self
            .vague_words
            .iter()
            .filter(|w| narrative_lower.contains(w.as_str()))
            .count();

        if vague_count > 2 {
            score -= 0.3;
            issues.push(QualityIssue {
                code: "VAGUE_NARRATIVE".to_string(),
                message: format!("Narrative contains {} vague terms", vague_count),
                severity: IssueSeverity::Medium,
                field: Some("narrative".to_string()),
                suggestion: Some("Use more specific language".to_string()),
            });
        }

        // Check title length
        if obs.title.len() < self.min_title_length {
            score -= 0.2;
            issues.push(QualityIssue {
                code: "SHORT_TITLE".to_string(),
                message: "Title is too short".to_string(),
                severity: IssueSeverity::Low,
                field: Some("title".to_string()),
                suggestion: Some("Use a more descriptive title".to_string()),
            });
        }

        // Check narrative length
        if obs.narrative.len() < self.min_narrative_length {
            score -= 0.2;
            issues.push(QualityIssue {
                code: "SHORT_NARRATIVE".to_string(),
                message: "Narrative is too brief".to_string(),
                severity: IssueSeverity::Low,
                field: Some("narrative".to_string()),
                suggestion: Some("Provide more context in the narrative".to_string()),
            });
        }

        // Check for file references
        if obs.files_read.is_empty() && obs.files_modified.is_empty() {
            score -= 0.1;
        }

        (score.max(0.0), issues)
    }

    fn score_actionability(&self, obs: &ObservationInput) -> (f32, Vec<QualityIssue>) {
        let mut score = 0.5; // Start at neutral
        let issues = Vec::new();

        // More facts = more actionable
        score += (obs.facts.len() as f32 * 0.1).min(0.2);

        // More concepts = more discoverable
        score += (obs.concepts.len() as f32 * 0.05).min(0.15);

        // File references = more traceable
        if !obs.files_read.is_empty() || !obs.files_modified.is_empty() {
            score += 0.15;
        }

        (score.min(1.0), issues)
    }

    fn score_correctness(&self, obs: &ObservationInput) -> (f32, Vec<QualityIssue>) {
        let mut score: f32 = 1.0;
        let mut issues = Vec::new();

        // Valid observation type
        if !self.valid_types.contains(&obs.observation_type.to_lowercase()) {
            score -= 0.5;
            issues.push(QualityIssue {
                code: "INVALID_TYPE".to_string(),
                message: format!("Invalid observation type: {}", obs.observation_type),
                severity: IssueSeverity::High,
                field: Some("observation_type".to_string()),
                suggestion: Some(format!("Use one of: {}", self.valid_types.join(", "))),
            });
        }

        // Title length bounds
        if obs.title.len() > self.max_title_length {
            score -= 0.2;
            issues.push(QualityIssue {
                code: "TITLE_TOO_LONG".to_string(),
                message: "Title exceeds maximum length".to_string(),
                severity: IssueSeverity::Low,
                field: Some("title".to_string()),
                suggestion: Some("Shorten the title".to_string()),
            });
        }

        (score.max(0.0), issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_good_observation() -> ObservationInput {
        ObservationInput {
            observation_type: "implementation".to_string(),
            title: "Implemented user authentication flow".to_string(),
            subtitle: Some("OAuth2 integration".to_string()),
            facts: vec![
                "Added OAuth2 client configuration".to_string(),
                "Token refresh is handled automatically".to_string(),
            ],
            narrative: "Implemented the complete OAuth2 authentication flow with automatic token refresh. The implementation follows the authorization code grant type and stores tokens securely.".to_string(),
            concepts: vec!["oauth2".to_string(), "authentication".to_string()],
            files_read: vec!["src/auth/config.rs".to_string()],
            files_modified: vec!["src/auth/oauth.rs".to_string()],
        }
    }

    #[test]
    fn test_good_observation_score() {
        let metrics = QualityMetrics::default();
        let obs = create_good_observation();
        let score = metrics.score(&obs);

        assert!(score.overall > 0.8, "Good observation should score > 0.8");
        assert!(score.issues.is_empty() || score.issues.iter().all(|i| i.severity != IssueSeverity::High));
    }

    #[test]
    fn test_missing_title_low_score() {
        let metrics = QualityMetrics::default();
        let mut obs = create_good_observation();
        obs.title = String::new();

        let score = metrics.score(&obs);
        assert!(score.completeness < 0.8);
        assert!(score.issues.iter().any(|i| i.code == "MISSING_TITLE"));
    }

    #[test]
    fn test_invalid_type() {
        let metrics = QualityMetrics::default();
        let mut obs = create_good_observation();
        obs.observation_type = "invalid_type".to_string();

        let score = metrics.score(&obs);
        assert!(score.correctness < 0.7);
        assert!(score.issues.iter().any(|i| i.code == "INVALID_TYPE"));
    }

    #[test]
    fn test_vague_narrative() {
        let metrics = QualityMetrics::default();
        let mut obs = create_good_observation();
        obs.narrative = "Did something with various things and stuff etc.".to_string();

        let score = metrics.score(&obs);
        assert!(score.specificity < 0.8);
    }
}
