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

//! Classification Metrics for Binary Evaluators
//!
//! Many evaluators (hallucination, toxicity, bias) are binary classifiers.
//! This module provides comprehensive classification metrics beyond basic accuracy.
//!
//! ## Metrics Provided
//!
//! - **ROC/AUC**: Threshold-independent performance across all operating points
//! - **PR Curves**: Precision-Recall curves (better for imbalanced datasets)
//! - **MCC**: Matthews Correlation Coefficient (robust to class imbalance)
//! - **Confusion Matrix**: Complete breakdown of TP/TN/FP/FN
//! - **Optimal Threshold**: Find best threshold for various objectives
//!
//! ## Why MCC?
//!
//! F1 score can be misleading on imbalanced datasets. Consider:
//! - 99% negative, 1% positive
//! - Always predict negative: Accuracy=99%, but MCC=0 (no skill)
//!
//! MCC ranges [-1, +1] where 0 = random, and correctly penalizes
//! classifiers that ignore the minority class.

use serde::{Deserialize, Serialize};

/// Complete classification metrics result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationMetrics {
    /// Confusion matrix components
    pub confusion_matrix: ConfusionMatrix,
    /// Basic metrics
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub specificity: f64,
    pub f1_score: f64,
    /// Matthews Correlation Coefficient (TASK 6)
    pub mcc: f64,
    /// Balanced accuracy (average of sensitivity and specificity)
    pub balanced_accuracy: f64,
    /// Positive/negative predictive values
    pub ppv: f64, // Same as precision
    pub npv: f64, // TN / (TN + FN)
    /// Likelihood ratios
    pub positive_likelihood_ratio: f64, // TPR / FPR
    pub negative_likelihood_ratio: f64, // FNR / TNR
}

/// Confusion matrix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfusionMatrix {
    pub true_positives: usize,
    pub true_negatives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub total: usize,
}

/// ROC curve result (TASK 5)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ROCCurve {
    /// (FPR, TPR) points for the curve
    pub points: Vec<ROCPoint>,
    /// Area Under ROC Curve
    pub auc: f64,
    /// Optimal threshold using Youden's J
    pub optimal_threshold_youden: f64,
    /// Optimal threshold for max F1
    pub optimal_threshold_f1: f64,
    /// Number of samples
    pub n_samples: usize,
    pub n_positive: usize,
    pub n_negative: usize,
}

/// Single point on ROC curve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ROCPoint {
    pub threshold: f64,
    pub tpr: f64, // True Positive Rate (sensitivity/recall)
    pub fpr: f64, // False Positive Rate (1 - specificity)
    pub precision: f64,
    pub f1: f64,
    pub youden_j: f64, // TPR - FPR
}

/// Precision-Recall curve result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRCurve {
    /// (Recall, Precision) points
    pub points: Vec<PRPoint>,
    /// Area Under PR Curve
    pub auprc: f64,
    /// Average Precision (AP)
    pub average_precision: f64,
    /// F1-optimal threshold
    pub optimal_threshold: f64,
    /// Baseline (random classifier's precision = positive rate)
    pub baseline: f64,
}

/// Single point on PR curve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRPoint {
    pub threshold: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

/// Classification analyzer
pub struct ClassificationAnalyzer {
    /// Threshold for binary classification (default: 0.5)
    threshold: f64,
    /// Whether higher scores mean positive class
    higher_is_positive: bool,
}

impl Default for ClassificationAnalyzer {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            higher_is_positive: true,
        }
    }
}

impl ClassificationAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_threshold(mut self, t: f64) -> Self {
        self.threshold = t;
        self
    }

    pub fn with_higher_is_positive(mut self, hip: bool) -> Self {
        self.higher_is_positive = hip;
        self
    }

    /// Compute classification metrics from (score, is_positive) pairs
    pub fn analyze(&self, predictions: &[(f64, bool)]) -> ClassificationMetrics {
        let cm = self.confusion_matrix(predictions, self.threshold);
        self.metrics_from_confusion_matrix(&cm)
    }

    /// Compute confusion matrix at a given threshold
    pub fn confusion_matrix(&self, predictions: &[(f64, bool)], threshold: f64) -> ConfusionMatrix {
        let mut tp = 0;
        let mut tn = 0;
        let mut fp = 0;
        let mut fn_ = 0;

        for (score, is_positive) in predictions {
            let predicted_positive = if self.higher_is_positive {
                *score >= threshold
            } else {
                *score <= threshold
            };

            match (predicted_positive, *is_positive) {
                (true, true) => tp += 1,
                (true, false) => fp += 1,
                (false, true) => fn_ += 1,
                (false, false) => tn += 1,
            }
        }

        ConfusionMatrix {
            true_positives: tp,
            true_negatives: tn,
            false_positives: fp,
            false_negatives: fn_,
            total: predictions.len(),
        }
    }

    /// Compute all metrics from confusion matrix
    fn metrics_from_confusion_matrix(&self, cm: &ConfusionMatrix) -> ClassificationMetrics {
        let tp = cm.true_positives as f64;
        let tn = cm.true_negatives as f64;
        let fp = cm.false_positives as f64;
        let fn_ = cm.false_negatives as f64;
        let total = cm.total as f64;

        let accuracy = if total > 0.0 { (tp + tn) / total } else { 0.0 };

        let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
        let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
        let specificity = if tn + fp > 0.0 { tn / (tn + fp) } else { 0.0 };
        let npv = if tn + fn_ > 0.0 { tn / (tn + fn_) } else { 0.0 };

        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        // Matthews Correlation Coefficient (TASK 6)
        let mcc_num = tp * tn - fp * fn_;
        let mcc_denom = ((tp + fp) * (tp + fn_) * (tn + fp) * (tn + fn_)).sqrt();
        let mcc = if mcc_denom > 0.0 {
            mcc_num / mcc_denom
        } else {
            0.0
        };

        let balanced_accuracy = (recall + specificity) / 2.0;

        // Likelihood ratios
        let fpr = 1.0 - specificity;
        let fnr = 1.0 - recall;
        let positive_lr = if fpr > 0.0 {
            recall / fpr
        } else {
            f64::INFINITY
        };
        let negative_lr = if specificity > 0.0 {
            fnr / specificity
        } else {
            0.0
        };

        ClassificationMetrics {
            confusion_matrix: cm.clone(),
            accuracy,
            precision,
            recall,
            specificity,
            f1_score: f1,
            mcc,
            balanced_accuracy,
            ppv: precision,
            npv,
            positive_likelihood_ratio: positive_lr,
            negative_likelihood_ratio: negative_lr,
        }
    }

    /// Compute ROC curve and AUROC (TASK 5)
    pub fn roc_curve(&self, predictions: &[(f64, bool)]) -> ROCCurve {
        if predictions.is_empty() {
            return ROCCurve {
                points: Vec::new(),
                auc: 0.0,
                optimal_threshold_youden: 0.5,
                optimal_threshold_f1: 0.5,
                n_samples: 0,
                n_positive: 0,
                n_negative: 0,
            };
        }

        let n_positive = predictions.iter().filter(|(_, p)| *p).count();
        let n_negative = predictions.len() - n_positive;

        if n_positive == 0 || n_negative == 0 {
            return ROCCurve {
                points: Vec::new(),
                auc: 0.5, // Undefined, return random baseline
                optimal_threshold_youden: 0.5,
                optimal_threshold_f1: 0.5,
                n_samples: predictions.len(),
                n_positive,
                n_negative,
            };
        }

        // Sort by score descending
        let mut sorted: Vec<_> = predictions.to_vec();
        sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        // Get unique thresholds
        let mut thresholds: Vec<f64> = sorted.iter().map(|(s, _)| *s).collect();
        thresholds.sort_by(|a, b| b.partial_cmp(a).unwrap());
        thresholds.dedup();

        // Add boundary thresholds
        let max_score = sorted.first().map(|(s, _)| *s).unwrap_or(1.0);
        let min_score = sorted.last().map(|(s, _)| *s).unwrap_or(0.0);
        thresholds.insert(0, max_score + 0.01);
        thresholds.push(min_score - 0.01);

        let mut points = Vec::with_capacity(thresholds.len());
        let mut best_youden = (0.0, 0.0f64);
        let mut best_f1 = (0.0, 0.0f64);

        for threshold in &thresholds {
            let cm = self.confusion_matrix(&sorted, *threshold);
            let metrics = self.metrics_from_confusion_matrix(&cm);

            let tpr = metrics.recall;
            let fpr = 1.0 - metrics.specificity;
            let youden_j = tpr - fpr;

            let point = ROCPoint {
                threshold: *threshold,
                tpr,
                fpr,
                precision: metrics.precision,
                f1: metrics.f1_score,
                youden_j,
            };

            if youden_j > best_youden.1 {
                best_youden = (*threshold, youden_j);
            }
            if metrics.f1_score > best_f1.1 {
                best_f1 = (*threshold, metrics.f1_score);
            }

            points.push(point);
        }

        // Sort points by FPR for AUC calculation
        points.sort_by(|a, b| a.fpr.partial_cmp(&b.fpr).unwrap());

        // Calculate AUC using trapezoidal rule
        let mut auc = 0.0;
        for i in 1..points.len() {
            let dx = points[i].fpr - points[i - 1].fpr;
            let avg_y = (points[i].tpr + points[i - 1].tpr) / 2.0;
            auc += dx * avg_y;
        }

        ROCCurve {
            points,
            auc,
            optimal_threshold_youden: best_youden.0,
            optimal_threshold_f1: best_f1.0,
            n_samples: predictions.len(),
            n_positive,
            n_negative,
        }
    }

    /// Compute Precision-Recall curve and AUPRC
    pub fn pr_curve(&self, predictions: &[(f64, bool)]) -> PRCurve {
        if predictions.is_empty() {
            return PRCurve {
                points: Vec::new(),
                auprc: 0.0,
                average_precision: 0.0,
                optimal_threshold: 0.5,
                baseline: 0.0,
            };
        }

        let n_positive = predictions.iter().filter(|(_, p)| *p).count();
        let baseline = n_positive as f64 / predictions.len() as f64;

        if n_positive == 0 {
            return PRCurve {
                points: Vec::new(),
                auprc: 0.0,
                average_precision: 0.0,
                optimal_threshold: 0.5,
                baseline,
            };
        }

        // Sort by score descending
        let mut sorted: Vec<_> = predictions.to_vec();
        sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        let mut points = Vec::new();
        let mut best_f1 = (0.0, 0.0f64);
        let mut tp_cumsum = 0;
        let mut ap_sum = 0.0;
        let mut prev_recall = 0.0;

        for (i, (score, is_positive)) in sorted.iter().enumerate() {
            if *is_positive {
                tp_cumsum += 1;
            }

            let predicted_positive = i + 1;
            let precision = tp_cumsum as f64 / predicted_positive as f64;
            let recall = tp_cumsum as f64 / n_positive as f64;
            let f1 = if precision + recall > 0.0 {
                2.0 * precision * recall / (precision + recall)
            } else {
                0.0
            };

            // Average Precision contribution
            if *is_positive {
                ap_sum += precision * (recall - prev_recall);
                prev_recall = recall;
            }

            if f1 > best_f1.1 {
                best_f1 = (*score, f1);
            }

            points.push(PRPoint {
                threshold: *score,
                precision,
                recall,
                f1,
            });
        }

        // Calculate AUPRC using trapezoidal rule
        // Sort by recall ascending
        points.sort_by(|a, b| a.recall.partial_cmp(&b.recall).unwrap());

        let mut auprc = 0.0;
        for i in 1..points.len() {
            let dx = points[i].recall - points[i - 1].recall;
            let avg_y = (points[i].precision + points[i - 1].precision) / 2.0;
            auprc += dx * avg_y;
        }

        PRCurve {
            points,
            auprc,
            average_precision: ap_sum,
            optimal_threshold: best_f1.0,
            baseline,
        }
    }

    /// Find optimal threshold based on objective
    pub fn find_optimal_threshold(
        &self,
        predictions: &[(f64, bool)],
        objective: ThresholdObjective,
    ) -> ThresholdResult {
        if predictions.is_empty() {
            return ThresholdResult {
                threshold: 0.5,
                score: 0.0,
                objective,
            };
        }

        // Get unique thresholds
        let mut thresholds: Vec<f64> = predictions.iter().map(|(s, _)| *s).collect();
        thresholds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        thresholds.dedup();

        let mut best = (0.5, f64::NEG_INFINITY);

        for threshold in thresholds {
            let cm = self.confusion_matrix(predictions, threshold);
            let metrics = self.metrics_from_confusion_matrix(&cm);

            let score = match objective {
                ThresholdObjective::MaxF1 => metrics.f1_score,
                ThresholdObjective::MaxYouden => metrics.recall + metrics.specificity - 1.0,
                ThresholdObjective::MaxMCC => metrics.mcc,
                ThresholdObjective::MaxBalancedAccuracy => metrics.balanced_accuracy,
                ThresholdObjective::FixedRecall(target) => {
                    if metrics.recall >= target {
                        metrics.precision
                    } else {
                        f64::NEG_INFINITY
                    }
                }
                ThresholdObjective::FixedPrecision(target) => {
                    if metrics.precision >= target {
                        metrics.recall
                    } else {
                        f64::NEG_INFINITY
                    }
                }
                ThresholdObjective::MinCost { fp_cost, fn_cost } => {
                    let cost =
                        fp_cost * cm.false_positives as f64 + fn_cost * cm.false_negatives as f64;
                    -cost // Negate so higher is better
                }
            };

            if score > best.1 {
                best = (threshold, score);
            }
        }

        ThresholdResult {
            threshold: best.0,
            score: if matches!(objective, ThresholdObjective::MinCost { .. }) {
                -best.1 // Un-negate cost
            } else {
                best.1
            },
            objective,
        }
    }
}

/// Objective for threshold optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThresholdObjective {
    /// Maximize F1 score
    MaxF1,
    /// Maximize Youden's J (TPR - FPR)
    MaxYouden,
    /// Maximize Matthews Correlation Coefficient
    MaxMCC,
    /// Maximize balanced accuracy
    MaxBalancedAccuracy,
    /// Maximize precision at fixed recall
    FixedRecall(f64),
    /// Maximize recall at fixed precision
    FixedPrecision(f64),
    /// Minimize total cost with given FP/FN costs
    MinCost { fp_cost: f64, fn_cost: f64 },
}

/// Result of threshold optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdResult {
    pub threshold: f64,
    pub score: f64,
    pub objective: ThresholdObjective,
}

// ============================================================================
// Multi-class MCC
// ============================================================================

/// Compute multi-class Matthews Correlation Coefficient
///
/// Generalization of MCC to K classes (Gorodkin, 2004)
pub fn multiclass_mcc(confusion_matrix: &[Vec<usize>]) -> f64 {
    let k = confusion_matrix.len();
    if k == 0 {
        return 0.0;
    }

    // Total samples
    let n: usize = confusion_matrix.iter().flat_map(|row| row.iter()).sum();
    if n == 0 {
        return 0.0;
    }

    let n = n as f64;

    // Trace (correct predictions)
    let c: f64 = (0..k).map(|i| confusion_matrix[i][i] as f64).sum();

    // Row sums (predictions per class)
    let p: Vec<f64> = (0..k)
        .map(|i| confusion_matrix[i].iter().sum::<usize>() as f64)
        .collect();

    // Column sums (actual per class)
    let t: Vec<f64> = (0..k)
        .map(|j| (0..k).map(|i| confusion_matrix[i][j]).sum::<usize>() as f64)
        .collect();

    // Numerator
    let numerator = c * n - p.iter().zip(t.iter()).map(|(pi, ti)| pi * ti).sum::<f64>();

    // Denominator components
    let sum_p_sq: f64 = p.iter().map(|pi| pi * pi).sum();
    let sum_t_sq: f64 = t.iter().map(|ti| ti * ti).sum();

    let denom_left = (n * n - sum_p_sq).sqrt();
    let denom_right = (n * n - sum_t_sq).sqrt();

    let denominator = denom_left * denom_right;

    if denominator == 0.0 {
        return 0.0;
    }

    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_classification() {
        let predictions = vec![
            (0.9, true),
            (0.8, true),
            (0.7, true),
            (0.2, false),
            (0.1, false),
        ];

        let analyzer = ClassificationAnalyzer::new().with_threshold(0.5);
        let metrics = analyzer.analyze(&predictions);

        assert_eq!(metrics.confusion_matrix.true_positives, 3);
        assert_eq!(metrics.confusion_matrix.true_negatives, 2);
        assert_eq!(metrics.confusion_matrix.false_positives, 0);
        assert_eq!(metrics.confusion_matrix.false_negatives, 0);
        assert!((metrics.accuracy - 1.0).abs() < 0.01);
        assert!((metrics.mcc - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_mcc_random_baseline() {
        // Always predict positive on 50/50 split
        let predictions: Vec<_> = (0..100)
            .map(|i| (0.9, i < 50)) // All positive predictions
            .collect();

        let analyzer = ClassificationAnalyzer::new().with_threshold(0.5);
        let metrics = analyzer.analyze(&predictions);

        // MCC should be ~0 for random-ish predictions
        assert!(metrics.mcc.abs() < 0.3);
    }

    #[test]
    fn test_auroc() {
        let predictions = vec![
            (0.9, true),
            (0.8, true),
            (0.7, true),
            (0.4, false),
            (0.3, false),
            (0.2, false),
        ];

        let analyzer = ClassificationAnalyzer::new();
        let roc = analyzer.roc_curve(&predictions);

        // Perfect separation -> AUC = 1.0
        assert!((roc.auc - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_auroc_random() {
        // Random predictions
        let predictions: Vec<_> = (0..100).map(|i| (i as f64 / 100.0, i % 2 == 0)).collect();

        let analyzer = ClassificationAnalyzer::new();
        let roc = analyzer.roc_curve(&predictions);

        // Random -> AUC ≈ 0.5
        assert!((roc.auc - 0.5).abs() < 0.15);
    }

    #[test]
    fn test_pr_curve() {
        let predictions = vec![(0.9, true), (0.8, true), (0.3, false), (0.2, false)];

        let analyzer = ClassificationAnalyzer::new();
        let pr = analyzer.pr_curve(&predictions);

        // AUPRC should be positive for well-separated predictions
        assert!(pr.auprc > 0.0);
        // Average precision should also be positive
        assert!(pr.average_precision > 0.0);
        // Baseline (positive rate) should be 0.5 for this balanced dataset
        assert!((pr.baseline - 0.5).abs() < 0.01);
        // Should have generated some points
        assert!(!pr.points.is_empty());
    }
}
