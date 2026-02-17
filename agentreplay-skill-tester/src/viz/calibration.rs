// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the AGPL-3.0-or-later license.

//! Calibration data exporter for reliability diagrams
//!
//! Computes Expected Calibration Error (ECE) and exports bin data
//! for rendering reliability diagrams.
//!
//! ECE = Σ (n_m/N) · |accuracy_m - confidence_m|

use serde::{Deserialize, Serialize};

/// A calibration bin for reliability diagrams
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub bin_start: f64,
    pub bin_end: f64,
    pub bin_center: f64,
    pub sample_count: usize,
    pub accuracy: f64,
    pub average_confidence: f64,
    pub gap: f64, // |accuracy - confidence|
}

/// Calibration result for an evaluator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationResult {
    pub evaluator_id: String,
    pub ece: f64,
    pub bins: Vec<CalibrationBin>,
    pub total_samples: usize,
    pub is_well_calibrated: bool,
    pub calibration_status: String,
}

/// Calibration data exporter
pub struct CalibrationExporter {
    /// Number of bins (default: 10)
    num_bins: usize,
    /// ECE threshold for "well-calibrated" (default: 0.05)
    ece_threshold: f64,
}

impl CalibrationExporter {
    pub fn new() -> Self {
        Self {
            num_bins: 10,
            ece_threshold: 0.05,
        }
    }

    pub fn with_bins(mut self, n: usize) -> Self {
        self.num_bins = n;
        self
    }

    pub fn with_ece_threshold(mut self, threshold: f64) -> Self {
        self.ece_threshold = threshold;
        self
    }

    /// Compute calibration from (confidence, correct) pairs
    ///
    /// # Arguments
    /// * `evaluator_id` - ID of the evaluator being calibrated
    /// * `predictions` - Vec of (confidence, was_correct) pairs
    pub fn compute(
        &self,
        evaluator_id: &str,
        predictions: &[(f64, bool)],
    ) -> CalibrationResult {
        let n = predictions.len();
        if n == 0 {
            return CalibrationResult {
                evaluator_id: evaluator_id.to_string(),
                ece: 0.0,
                bins: Vec::new(),
                total_samples: 0,
                is_well_calibrated: true,
                calibration_status: "No data".to_string(),
            };
        }

        let bin_width = 1.0 / self.num_bins as f64;
        let mut bins = Vec::new();
        let mut ece = 0.0;

        for b in 0..self.num_bins {
            let bin_start = b as f64 * bin_width;
            let bin_end = (b + 1) as f64 * bin_width;
            let bin_center = (bin_start + bin_end) / 2.0;

            let bin_preds: Vec<&(f64, bool)> = predictions.iter()
                .filter(|(conf, _)| *conf >= bin_start && *conf < bin_end)
                .collect();

            let count = bin_preds.len();
            if count == 0 {
                bins.push(CalibrationBin {
                    bin_start,
                    bin_end,
                    bin_center,
                    sample_count: 0,
                    accuracy: 0.0,
                    average_confidence: bin_center,
                    gap: 0.0,
                });
                continue;
            }

            let accuracy = bin_preds.iter().filter(|(_, correct)| *correct).count() as f64 / count as f64;
            let avg_confidence = bin_preds.iter().map(|(conf, _)| conf).sum::<f64>() / count as f64;
            let gap = (accuracy - avg_confidence).abs();

            ece += (count as f64 / n as f64) * gap;

            bins.push(CalibrationBin {
                bin_start,
                bin_end,
                bin_center,
                sample_count: count,
                accuracy,
                average_confidence: avg_confidence,
                gap,
            });
        }

        let is_well_calibrated = ece <= self.ece_threshold;
        let calibration_status = if ece <= 0.02 {
            "Excellent calibration".to_string()
        } else if ece <= 0.05 {
            "Well calibrated".to_string()
        } else if ece <= 0.10 {
            "Moderately calibrated".to_string()
        } else if ece <= 0.20 {
            "Poorly calibrated — overconfident".to_string()
        } else {
            "Severely miscalibrated".to_string()
        };

        CalibrationResult {
            evaluator_id: evaluator_id.to_string(),
            ece,
            bins,
            total_samples: n,
            is_well_calibrated,
            calibration_status,
        }
    }
}

impl Default for CalibrationExporter {
    fn default() -> Self {
        Self::new()
    }
}
