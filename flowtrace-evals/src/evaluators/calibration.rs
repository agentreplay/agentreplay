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

//! Calibration Metrics for Probabilistic Predictions
//!
//! When evaluators return confidence scores, it's crucial to verify that these
//! scores actually reflect true probabilities. A well-calibrated model with
//! confidence 0.8 should be correct ~80% of the time.
//!
//! ## Metrics
//!
//! - **Brier Score**: Proper scoring rule measuring accuracy of probabilistic predictions
//! - **Expected Calibration Error (ECE)**: Mean absolute calibration gap across bins
//! - **Maximum Calibration Error (MCE)**: Worst-case calibration gap
//! - **Reliability Diagram**: Visual representation of calibration
//!
//! ## Usage
//!
//! ```rust,ignore
//! use flowtrace_evals::evaluators::calibration::{CalibrationAnalyzer, CalibrationMetrics};
//!
//! let predictions = vec![
//!     (0.9, true),   // High confidence, correct
//!     (0.7, true),   // Medium confidence, correct
//!     (0.3, false),  // Low confidence, correct
//!     (0.8, false),  // High confidence, wrong (overconfident!)
//! ];
//!
//! let analyzer = CalibrationAnalyzer::new();
//! let metrics = analyzer.analyze(&predictions);
//! println!("ECE: {:.3}", metrics.ece);
//! ```

// Loop index used to access array is clearer in PAV algorithm context
#![allow(clippy::needless_range_loop)]

use serde::{Deserialize, Serialize};

/// Complete calibration analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationMetrics {
    /// Brier Score (lower is better, 0 = perfect, 0.25 = random)
    pub brier_score: f64,
    /// Expected Calibration Error (lower is better)
    pub ece: f64,
    /// Maximum Calibration Error (lower is better)
    pub mce: f64,
    /// Adaptive ECE (equal-mass bins, less sensitive to binning)
    pub ace: f64,
    /// Overconfidence score (average when confidence > accuracy)
    pub overconfidence: f64,
    /// Underconfidence score (average when confidence < accuracy)
    pub underconfidence: f64,
    /// Reliability diagram data: (bin_center, accuracy, count)
    pub reliability_diagram: Vec<ReliabilityBin>,
    /// Number of predictions analyzed
    pub n_predictions: usize,
    /// Overall accuracy
    pub accuracy: f64,
    /// Mean predicted probability
    pub mean_confidence: f64,
}

/// Single bin in reliability diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityBin {
    /// Center of the confidence bin (e.g., 0.15 for [0.1, 0.2])
    pub bin_center: f64,
    /// Lower bound of bin
    pub bin_lower: f64,
    /// Upper bound of bin
    pub bin_upper: f64,
    /// Observed accuracy in this bin
    pub accuracy: f64,
    /// Mean confidence in this bin
    pub mean_confidence: f64,
    /// Number of predictions in this bin
    pub count: usize,
    /// Calibration gap (accuracy - mean_confidence)
    pub gap: f64,
}

/// Calibration analysis configuration
#[derive(Debug, Clone)]
pub struct CalibrationAnalyzer {
    /// Number of bins for ECE/MCE (default: 10)
    n_bins: usize,
    /// Use equal-mass bins (adaptive) instead of equal-width
    adaptive_binning: bool,
}

impl Default for CalibrationAnalyzer {
    fn default() -> Self {
        Self {
            n_bins: 10,
            adaptive_binning: false,
        }
    }
}

impl CalibrationAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bins(mut self, n: usize) -> Self {
        self.n_bins = n.max(2);
        self
    }

    pub fn with_adaptive_binning(mut self, adaptive: bool) -> Self {
        self.adaptive_binning = adaptive;
        self
    }

    /// Analyze calibration from (confidence, actual_outcome) pairs
    ///
    /// # Arguments
    /// * `predictions` - Vec of (predicted_probability, was_correct)
    pub fn analyze(&self, predictions: &[(f64, bool)]) -> CalibrationMetrics {
        if predictions.is_empty() {
            return CalibrationMetrics {
                brier_score: 0.0,
                ece: 0.0,
                mce: 0.0,
                ace: 0.0,
                overconfidence: 0.0,
                underconfidence: 0.0,
                reliability_diagram: Vec::new(),
                n_predictions: 0,
                accuracy: 0.0,
                mean_confidence: 0.0,
            };
        }

        let n = predictions.len() as f64;

        // Brier Score
        let brier_score = predictions
            .iter()
            .map(|(conf, outcome)| {
                let o = if *outcome { 1.0 } else { 0.0 };
                (conf - o).powi(2)
            })
            .sum::<f64>()
            / n;

        // Overall statistics
        let accuracy = predictions.iter().filter(|(_, o)| *o).count() as f64 / n;
        let mean_confidence = predictions.iter().map(|(c, _)| c).sum::<f64>() / n;

        // Create bins
        let bins = if self.adaptive_binning {
            self.create_adaptive_bins(predictions)
        } else {
            self.create_equal_width_bins(predictions)
        };

        // Calculate ECE, MCE
        let mut ece = 0.0;
        let mut mce = 0.0f64;
        let mut ace = 0.0;
        let mut overconfidence = 0.0;
        let mut underconfidence = 0.0;
        let mut over_count = 0;
        let mut under_count = 0;

        for bin in &bins {
            if bin.count > 0 {
                let weight = bin.count as f64 / n;
                let gap = (bin.accuracy - bin.mean_confidence).abs();

                ece += weight * gap;
                mce = mce.max(gap);
                ace += gap; // Will normalize by non-empty bins

                if bin.mean_confidence > bin.accuracy {
                    overconfidence += bin.mean_confidence - bin.accuracy;
                    over_count += 1;
                } else {
                    underconfidence += bin.accuracy - bin.mean_confidence;
                    under_count += 1;
                }
            }
        }

        let non_empty_bins = bins.iter().filter(|b| b.count > 0).count();
        ace = if non_empty_bins > 0 {
            ace / non_empty_bins as f64
        } else {
            0.0
        };

        overconfidence = if over_count > 0 {
            overconfidence / over_count as f64
        } else {
            0.0
        };

        underconfidence = if under_count > 0 {
            underconfidence / under_count as f64
        } else {
            0.0
        };

        CalibrationMetrics {
            brier_score,
            ece,
            mce,
            ace,
            overconfidence,
            underconfidence,
            reliability_diagram: bins,
            n_predictions: predictions.len(),
            accuracy,
            mean_confidence,
        }
    }

    /// Create equal-width bins
    fn create_equal_width_bins(&self, predictions: &[(f64, bool)]) -> Vec<ReliabilityBin> {
        let bin_width = 1.0 / self.n_bins as f64;
        let mut bins = Vec::with_capacity(self.n_bins);

        for i in 0..self.n_bins {
            let lower = i as f64 * bin_width;
            let upper = (i + 1) as f64 * bin_width;
            let center = (lower + upper) / 2.0;

            let in_bin: Vec<_> = predictions
                .iter()
                .filter(|(c, _)| *c >= lower && *c < upper)
                .collect();

            let count = in_bin.len();
            let (accuracy, mean_conf) = if count > 0 {
                let acc = in_bin.iter().filter(|(_, o)| *o).count() as f64 / count as f64;
                let conf = in_bin.iter().map(|(c, _)| c).sum::<f64>() / count as f64;
                (acc, conf)
            } else {
                (0.0, center)
            };

            bins.push(ReliabilityBin {
                bin_center: center,
                bin_lower: lower,
                bin_upper: upper,
                accuracy,
                mean_confidence: mean_conf,
                count,
                gap: accuracy - mean_conf,
            });
        }

        bins
    }

    /// Create adaptive (equal-mass) bins
    fn create_adaptive_bins(&self, predictions: &[(f64, bool)]) -> Vec<ReliabilityBin> {
        let mut sorted: Vec<_> = predictions.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let samples_per_bin = (sorted.len() / self.n_bins).max(1);
        let mut bins = Vec::new();

        for chunk in sorted.chunks(samples_per_bin) {
            if chunk.is_empty() {
                continue;
            }

            let lower = chunk.first().map(|(c, _)| *c).unwrap_or(0.0);
            let upper = chunk.last().map(|(c, _)| *c).unwrap_or(1.0);
            let center = (lower + upper) / 2.0;

            let count = chunk.len();
            let accuracy = chunk.iter().filter(|(_, o)| *o).count() as f64 / count as f64;
            let mean_conf = chunk.iter().map(|(c, _)| c).sum::<f64>() / count as f64;

            bins.push(ReliabilityBin {
                bin_center: center,
                bin_lower: lower,
                bin_upper: upper,
                accuracy,
                mean_confidence: mean_conf,
                count,
                gap: accuracy - mean_conf,
            });
        }

        bins
    }
}

// ============================================================================
// Temperature Scaling for Recalibration
// ============================================================================

/// Temperature scaling recalibrator
///
/// Learns optimal temperature T such that softmax(logits/T) is well-calibrated.
/// For binary predictions without logits, we transform: p_calibrated = sigmoid(logit(p)/T)
pub struct TemperatureScaler {
    /// Learned temperature (> 1 = reduce confidence, < 1 = increase confidence)
    pub temperature: f64,
    /// Whether the scaler has been fitted
    fitted: bool,
}

impl Default for TemperatureScaler {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            fitted: false,
        }
    }
}

impl TemperatureScaler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fit temperature on validation data
    ///
    /// Uses grid search to find T that minimizes ECE
    pub fn fit(&mut self, predictions: &[(f64, bool)]) {
        if predictions.is_empty() {
            return;
        }

        let analyzer = CalibrationAnalyzer::new();
        let mut best_t = 1.0;
        let mut best_ece = f64::INFINITY;

        // Grid search over temperature values
        for t_int in 1..=50 {
            let t = t_int as f64 * 0.1; // 0.1 to 5.0

            let scaled: Vec<(f64, bool)> = predictions
                .iter()
                .map(|(p, o)| (self.scale_probability(*p, t), *o))
                .collect();

            let metrics = analyzer.analyze(&scaled);

            if metrics.ece < best_ece {
                best_ece = metrics.ece;
                best_t = t;
            }
        }

        self.temperature = best_t;
        self.fitted = true;
    }

    /// Scale a probability using learned temperature
    pub fn scale(&self, p: f64) -> f64 {
        self.scale_probability(p, self.temperature)
    }

    /// Scale probability with given temperature
    fn scale_probability(&self, p: f64, t: f64) -> f64 {
        // Convert probability to logit, scale, convert back
        let epsilon = 1e-10;
        let p_clamped = p.clamp(epsilon, 1.0 - epsilon);
        let logit = (p_clamped / (1.0 - p_clamped)).ln();
        let scaled_logit = logit / t;
        1.0 / (1.0 + (-scaled_logit).exp())
    }

    /// Recalibrate a set of predictions
    pub fn transform(&self, predictions: &[(f64, bool)]) -> Vec<(f64, bool)> {
        predictions
            .iter()
            .map(|(p, o)| (self.scale(*p), *o))
            .collect()
    }
}

// ============================================================================
// Isotonic Regression for Non-parametric Recalibration
// ============================================================================

/// Isotonic regression calibrator
///
/// Learns a monotonic mapping from predicted probabilities to calibrated probabilities.
/// More flexible than temperature scaling but requires more validation data.
#[derive(Default)]
pub struct IsotonicCalibrator {
    /// Learned (input, output) calibration points
    calibration_map: Vec<(f64, f64)>,
    fitted: bool,
}

impl IsotonicCalibrator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fit isotonic regression on validation data
    ///
    /// Uses Pool Adjacent Violators (PAV) algorithm
    pub fn fit(&mut self, predictions: &[(f64, bool)]) {
        if predictions.is_empty() {
            return;
        }

        // Sort by predicted probability
        let mut sorted: Vec<_> = predictions.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Convert outcomes to f64
        let outcomes: Vec<f64> = sorted
            .iter()
            .map(|(_, o)| if *o { 1.0 } else { 0.0 })
            .collect();

        // PAV algorithm
        let calibrated = self.pav(&outcomes);

        // Create calibration map (dedup similar values)
        self.calibration_map.clear();
        for (i, (pred, _)) in sorted.iter().enumerate() {
            self.calibration_map.push((*pred, calibrated[i]));
        }

        // Simplify by keeping only distinct calibration points
        self.calibration_map
            .dedup_by(|a, b| (a.1 - b.1).abs() < 0.001);

        self.fitted = true;
    }

    /// Pool Adjacent Violators algorithm
    fn pav(&self, y: &[f64]) -> Vec<f64> {
        let n = y.len();
        if n == 0 {
            return Vec::new();
        }

        let mut result: Vec<f64> = y.to_vec();
        let mut weights: Vec<usize> = vec![1; n];
        let mut pool: Vec<usize> = vec![0];

        for i in 1..n {
            pool.push(i);

            // Pool adjacent violators
            while pool.len() > 1 {
                let last = pool.len() - 1;
                let prev = pool.len() - 2;

                let idx_last = pool[last];
                let idx_prev = pool[prev];

                if result[idx_prev] <= result[idx_last] {
                    break;
                }

                // Merge pools
                let combined_weight = weights[idx_prev] + weights[idx_last];
                let combined_value = (result[idx_prev] * weights[idx_prev] as f64
                    + result[idx_last] * weights[idx_last] as f64)
                    / combined_weight as f64;

                result[idx_prev] = combined_value;
                weights[idx_prev] = combined_weight;
                pool.pop();
            }
        }

        // Expand pools back to original indices
        let mut expanded = vec![0.0; n];
        let _current_pool_idx = 0;
        let _current_pool_start = 0;

        for (i, &pool_start) in pool.iter().enumerate() {
            let pool_end = if i + 1 < pool.len() { pool[i + 1] } else { n };

            let value = result[pool_start];
            for j in pool_start..pool_end {
                expanded[j] = value;
            }
        }

        // Simple fallback: just use running mean if PAV produces issues
        if expanded.iter().any(|x| x.is_nan()) {
            let mut running = Vec::with_capacity(n);
            let mut sum = 0.0;
            for (i, &val) in y.iter().enumerate() {
                sum += val;
                running.push(sum / (i + 1) as f64);
            }
            return running;
        }

        expanded
    }

    /// Calibrate a probability using learned mapping
    pub fn calibrate(&self, p: f64) -> f64 {
        if self.calibration_map.is_empty() {
            return p;
        }

        // Binary search for interpolation
        let pos = self
            .calibration_map
            .binary_search_by(|(x, _)| x.partial_cmp(&p).unwrap())
            .unwrap_or_else(|x| x);

        if pos == 0 {
            return self.calibration_map[0].1;
        }
        if pos >= self.calibration_map.len() {
            return self.calibration_map.last().unwrap().1;
        }

        // Linear interpolation
        let (x0, y0) = self.calibration_map[pos - 1];
        let (x1, y1) = self.calibration_map[pos];

        if (x1 - x0).abs() < 1e-10 {
            return y0;
        }

        let t = (p - x0) / (x1 - x0);
        y0 + t * (y1 - y0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_calibration() {
        // Perfect calibration: 90% confidence, 90% correct
        let predictions = vec![
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, false),
        ];

        let analyzer = CalibrationAnalyzer::new();
        let metrics = analyzer.analyze(&predictions);

        // Should have low ECE since predictions match outcomes
        assert!(metrics.ece < 0.1);
        assert!((metrics.accuracy - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_overconfident_model() {
        // Always predicts 0.95 but only 50% correct
        let predictions: Vec<_> = (0..100).map(|i| (0.95, i < 50)).collect();

        let analyzer = CalibrationAnalyzer::new();
        let metrics = analyzer.analyze(&predictions);

        // Should show overconfidence
        assert!(metrics.overconfidence > 0.3);
        assert!(metrics.ece > 0.3);
    }

    #[test]
    fn test_brier_score() {
        // Random predictions should have Brier ≈ 0.25
        let predictions: Vec<_> = (0..100).map(|i| (0.5, i % 2 == 0)).collect();

        let analyzer = CalibrationAnalyzer::new();
        let metrics = analyzer.analyze(&predictions);

        assert!((metrics.brier_score - 0.25).abs() < 0.05);
    }

    #[test]
    fn test_temperature_scaling() {
        let mut scaler = TemperatureScaler::new();

        // Overconfident predictions
        let predictions: Vec<_> = (0..100)
            .map(|i| (0.9, i < 50)) // 90% confident but only 50% correct
            .collect();

        scaler.fit(&predictions);

        // Temperature should be > 1 to reduce confidence
        assert!(scaler.temperature > 1.0);
    }
}
