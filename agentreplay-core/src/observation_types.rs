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

//! Observation Type Taxonomy
//!
//! Hierarchical classification of development observations.
//! Enables semantic categorization and filtered queries.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Categories of observation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationCategory {
    /// Development activities (implementation, debugging, etc.)
    Development,
    /// Architecture and design activities
    Architecture,
    /// Investigation and research activities
    Investigation,
    /// Documentation activities
    Documentation,
    /// Other/custom activities
    Other,
}

impl fmt::Display for ObservationCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObservationCategory::Development => write!(f, "development"),
            ObservationCategory::Architecture => write!(f, "architecture"),
            ObservationCategory::Investigation => write!(f, "investigation"),
            ObservationCategory::Documentation => write!(f, "documentation"),
            ObservationCategory::Other => write!(f, "other"),
        }
    }
}

/// Type of observation representing a development activity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationType {
    // === Development ===
    /// Code implementation activity.
    Implementation,
    /// Debugging and bug fixing.
    Debugging,
    /// Code refactoring.
    Refactoring,
    /// Testing and test writing.
    Testing,
    /// Code review activity.
    CodeReview,

    // === Architecture ===
    /// Architectural decisions.
    Architecture,
    /// System or API design.
    Design,
    /// Project planning.
    Planning,
    /// Performance optimization.
    Optimization,

    // === Investigation ===
    /// Research activity.
    Research,
    /// Code or system analysis.
    Analysis,
    /// Exploratory work.
    Exploration,
    /// Learning new concepts.
    Learning,

    // === Documentation ===
    /// Documentation writing.
    Documentation,
    /// Configuration changes.
    Configuration,
    /// Setup and installation.
    Setup,

    // === Other ===
    /// Custom observation type.
    Custom(String),
}

impl Default for ObservationType {
    fn default() -> Self {
        ObservationType::Implementation
    }
}

impl ObservationType {
    /// Get the category for this observation type.
    pub fn category(&self) -> ObservationCategory {
        match self {
            ObservationType::Implementation
            | ObservationType::Debugging
            | ObservationType::Refactoring
            | ObservationType::Testing
            | ObservationType::CodeReview => ObservationCategory::Development,

            ObservationType::Architecture
            | ObservationType::Design
            | ObservationType::Planning
            | ObservationType::Optimization => ObservationCategory::Architecture,

            ObservationType::Research
            | ObservationType::Analysis
            | ObservationType::Exploration
            | ObservationType::Learning => ObservationCategory::Investigation,

            ObservationType::Documentation
            | ObservationType::Configuration
            | ObservationType::Setup => ObservationCategory::Documentation,

            ObservationType::Custom(_) => ObservationCategory::Other,
        }
    }

    /// Get all standard observation types.
    pub fn all_standard() -> Vec<ObservationType> {
        vec![
            ObservationType::Implementation,
            ObservationType::Debugging,
            ObservationType::Refactoring,
            ObservationType::Testing,
            ObservationType::CodeReview,
            ObservationType::Architecture,
            ObservationType::Design,
            ObservationType::Planning,
            ObservationType::Optimization,
            ObservationType::Research,
            ObservationType::Analysis,
            ObservationType::Exploration,
            ObservationType::Learning,
            ObservationType::Documentation,
            ObservationType::Configuration,
            ObservationType::Setup,
        ]
    }

    /// Get all types in a specific category.
    pub fn types_in_category(category: ObservationCategory) -> Vec<ObservationType> {
        Self::all_standard()
            .into_iter()
            .filter(|t| t.category() == category)
            .collect()
    }

    /// Check if this is a custom type.
    pub fn is_custom(&self) -> bool {
        matches!(self, ObservationType::Custom(_))
    }

    /// Convert to string representation for storage/indexing.
    pub fn as_str(&self) -> &str {
        match self {
            ObservationType::Implementation => "implementation",
            ObservationType::Debugging => "debugging",
            ObservationType::Refactoring => "refactoring",
            ObservationType::Testing => "testing",
            ObservationType::CodeReview => "code_review",
            ObservationType::Architecture => "architecture",
            ObservationType::Design => "design",
            ObservationType::Planning => "planning",
            ObservationType::Optimization => "optimization",
            ObservationType::Research => "research",
            ObservationType::Analysis => "analysis",
            ObservationType::Exploration => "exploration",
            ObservationType::Learning => "learning",
            ObservationType::Documentation => "documentation",
            ObservationType::Configuration => "configuration",
            ObservationType::Setup => "setup",
            ObservationType::Custom(s) => s.as_str(),
        }
    }
}

impl fmt::Display for ObservationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ObservationType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "implementation" => Ok(ObservationType::Implementation),
            "debugging" => Ok(ObservationType::Debugging),
            "refactoring" => Ok(ObservationType::Refactoring),
            "testing" => Ok(ObservationType::Testing),
            "code_review" | "codereview" => Ok(ObservationType::CodeReview),
            "architecture" => Ok(ObservationType::Architecture),
            "design" => Ok(ObservationType::Design),
            "planning" => Ok(ObservationType::Planning),
            "optimization" => Ok(ObservationType::Optimization),
            "research" => Ok(ObservationType::Research),
            "analysis" => Ok(ObservationType::Analysis),
            "exploration" => Ok(ObservationType::Exploration),
            "learning" => Ok(ObservationType::Learning),
            "documentation" => Ok(ObservationType::Documentation),
            "configuration" => Ok(ObservationType::Configuration),
            "setup" => Ok(ObservationType::Setup),
            _ => Ok(ObservationType::Custom(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_mapping() {
        assert_eq!(
            ObservationType::Implementation.category(),
            ObservationCategory::Development
        );
        assert_eq!(
            ObservationType::Architecture.category(),
            ObservationCategory::Architecture
        );
        assert_eq!(
            ObservationType::Research.category(),
            ObservationCategory::Investigation
        );
        assert_eq!(
            ObservationType::Documentation.category(),
            ObservationCategory::Documentation
        );
        assert_eq!(
            ObservationType::Custom("custom".to_string()).category(),
            ObservationCategory::Other
        );
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "implementation".parse::<ObservationType>().unwrap(),
            ObservationType::Implementation
        );
        assert_eq!(
            "DEBUGGING".parse::<ObservationType>().unwrap(),
            ObservationType::Debugging
        );
        assert!(matches!(
            "unknown_type".parse::<ObservationType>().unwrap(),
            ObservationType::Custom(s) if s == "unknown_type"
        ));
    }

    #[test]
    fn test_all_standard() {
        let all = ObservationType::all_standard();
        assert!(all.len() >= 16);
        assert!(!all.iter().any(|t| t.is_custom()));
    }

    #[test]
    fn test_types_in_category() {
        let dev_types = ObservationType::types_in_category(ObservationCategory::Development);
        assert!(dev_types.contains(&ObservationType::Implementation));
        assert!(dev_types.contains(&ObservationType::Debugging));
        assert!(!dev_types.contains(&ObservationType::Research));
    }
}
