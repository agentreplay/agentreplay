// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Skill selection evaluator with confusion matrix
//!
//! Evaluates whether the correct skill was selected for a given input.
//! Computes K×K confusion matrix, per-skill Precision/Recall/F1,
//! and macro/micro averages.
//!
//! Mathematical formulation:
//!   Precision_k = TP_k / (TP_k + FP_k)
//!   Recall_k = TP_k / (TP_k + FN_k)
//!   F1_k = 2 · P_k · R_k / (P_k + R_k)
//!   Macro F1 = (1/K) Σ F1_k
//!   Micro F1 = 2·TP / (2·TP + FP + FN)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-skill metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetrics {
    pub skill_name: String,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub support: usize, // number of actual samples for this skill
}

/// Confusion matrix cell
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfusionCell {
    pub expected: String,
    pub actual: String,
    pub count: usize,
}

/// Selection evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionEvalResult {
    /// K×K confusion matrix
    pub confusion_matrix: Vec<Vec<usize>>,
    /// Skill labels in matrix order
    pub labels: Vec<String>,
    /// Per-skill metrics
    pub per_skill: Vec<SkillMetrics>,
    /// Macro-averaged F1
    pub macro_f1: f64,
    /// Micro-averaged F1
    pub micro_f1: f64,
    /// Misclassification details
    pub misclassifications: Vec<Misclassification>,
}

/// A misclassification case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Misclassification {
    pub expected: String,
    pub actual: String,
    pub count: usize,
    pub pattern: Option<String>,
    pub recommendation: Option<String>,
}

/// Evaluator for skill selection accuracy
pub struct SkillSelectionEvaluator {
    /// Skill names (including "none" for no-skill-should-trigger cases)
    skill_names: Vec<String>,
}

impl SkillSelectionEvaluator {
    pub fn new(skill_names: Vec<String>) -> Self {
        Self { skill_names }
    }

    /// Evaluate skill selection across a batch of (expected, actual) pairs
    pub fn evaluate(&self, predictions: &[(String, String)]) -> SelectionEvalResult {
        let k = self.skill_names.len();
        let label_to_idx: HashMap<&str, usize> = self.skill_names.iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i))
            .collect();

        // Build confusion matrix
        let mut matrix = vec![vec![0usize; k]; k];
        for (expected, actual) in predictions {
            if let (Some(&ei), Some(&ai)) = (label_to_idx.get(expected.as_str()), label_to_idx.get(actual.as_str())) {
                matrix[ei][ai] += 1;
            }
        }

        // Compute per-skill metrics
        let mut per_skill = Vec::new();
        let mut total_tp = 0usize;
        let mut total_fp = 0usize;
        let mut total_fn = 0usize;

        for i in 0..k {
            let tp = matrix[i][i];
            let fp: usize = (0..k).filter(|&r| r != i).map(|r| matrix[r][i]).sum();
            let fn_: usize = (0..k).filter(|&c| c != i).map(|c| matrix[i][c]).sum();
            let support = tp + fn_;

            let precision = if tp + fp > 0 { tp as f64 / (tp + fp) as f64 } else { 0.0 };
            let recall = if tp + fn_ > 0 { tp as f64 / (tp + fn_) as f64 } else { 0.0 };
            let f1 = if precision + recall > 0.0 {
                2.0 * precision * recall / (precision + recall)
            } else {
                0.0
            };

            total_tp += tp;
            total_fp += fp;
            total_fn += fn_;

            per_skill.push(SkillMetrics {
                skill_name: self.skill_names[i].clone(),
                precision,
                recall,
                f1,
                support,
            });
        }

        // Macro F1
        let macro_f1 = if k > 0 {
            per_skill.iter().map(|s| s.f1).sum::<f64>() / k as f64
        } else {
            0.0
        };

        // Micro F1
        let micro_precision = if total_tp + total_fp > 0 {
            total_tp as f64 / (total_tp + total_fp) as f64
        } else {
            0.0
        };
        let micro_recall = if total_tp + total_fn > 0 {
            total_tp as f64 / (total_tp + total_fn) as f64
        } else {
            0.0
        };
        let micro_f1 = if micro_precision + micro_recall > 0.0 {
            2.0 * micro_precision * micro_recall / (micro_precision + micro_recall)
        } else {
            0.0
        };

        // Find misclassifications
        let mut misclassifications = Vec::new();
        for i in 0..k {
            for j in 0..k {
                if i != j && matrix[i][j] > 0 {
                    misclassifications.push(Misclassification {
                        expected: self.skill_names[i].clone(),
                        actual: self.skill_names[j].clone(),
                        count: matrix[i][j],
                        pattern: None,
                        recommendation: Some(format!(
                            "Add gating predicate to distinguish '{}' from '{}'",
                            self.skill_names[i], self.skill_names[j]
                        )),
                    });
                }
            }
        }
        misclassifications.sort_by(|a, b| b.count.cmp(&a.count));

        SelectionEvalResult {
            confusion_matrix: matrix,
            labels: self.skill_names.clone(),
            per_skill,
            macro_f1,
            micro_f1,
            misclassifications,
        }
    }
}
