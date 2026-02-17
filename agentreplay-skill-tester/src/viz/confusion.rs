// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Confusion matrix rendering data exporter

use crate::evaluators::skill_selection::SelectionEvalResult;
use serde::{Deserialize, Serialize};

/// Confusion matrix data formatted for UI rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfusionMatrixData {
    pub labels: Vec<String>,
    pub matrix: Vec<Vec<usize>>,
    pub per_skill_metrics: Vec<SkillMetricRow>,
    pub macro_f1: f64,
    pub micro_f1: f64,
    pub total_samples: usize,
    pub misclassification_spotlight: Vec<MisclassificationSpotlight>,
}

/// Per-skill metric row for table display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetricRow {
    pub skill: String,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub support: usize,
}

/// Misclassification spotlight for UI highlighting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MisclassificationSpotlight {
    pub expected: String,
    pub actual: String,
    pub count: usize,
    pub pattern: Option<String>,
    pub recommendation: String,
}

/// Confusion matrix renderer/exporter
pub struct ConfusionMatrixRenderer;

impl ConfusionMatrixRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Convert a SelectionEvalResult into renderable confusion matrix data
    pub fn render(&self, result: &SelectionEvalResult) -> ConfusionMatrixData {
        let per_skill_metrics: Vec<SkillMetricRow> = result.per_skill.iter().map(|s| {
            SkillMetricRow {
                skill: s.skill_name.clone(),
                precision: s.precision,
                recall: s.recall,
                f1: s.f1,
                support: s.support,
            }
        }).collect();

        let total_samples: usize = result.per_skill.iter().map(|s| s.support).sum();

        let misclassification_spotlight: Vec<MisclassificationSpotlight> = result.misclassifications
            .iter()
            .take(5) // Top 5
            .map(|m| MisclassificationSpotlight {
                expected: m.expected.clone(),
                actual: m.actual.clone(),
                count: m.count,
                pattern: m.pattern.clone(),
                recommendation: m.recommendation.clone().unwrap_or_default(),
            })
            .collect();

        ConfusionMatrixData {
            labels: result.labels.clone(),
            matrix: result.confusion_matrix.clone(),
            per_skill_metrics,
            macro_f1: result.macro_f1,
            micro_f1: result.micro_f1,
            total_samples,
            misclassification_spotlight,
        }
    }
}

impl Default for ConfusionMatrixRenderer {
    fn default() -> Self {
        Self::new()
    }
}
