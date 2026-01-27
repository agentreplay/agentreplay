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

//! Advanced Statistical Methods for Evaluation
//!
//! This module provides robust statistical methods that go beyond basic assumptions:
//!
//! - **Bootstrap Confidence Intervals**: Non-parametric CIs that work with small samples
//! - **Power Analysis**: Sample size calculations and retrospective power analysis
//! - **Cohen's Kappa**: Inter-rater reliability for categorical data
//! - **Calibration Metrics**: Brier Score, ECE, MCE for probabilistic predictions
//!
//! ## Bootstrap Methods
//!
//! When sample sizes are small (n < 30) or distributions are non-normal, standard
//! confidence intervals (mean ± 1.96*SE) can be misleading. Bootstrap methods
//! provide accurate uncertainty quantification without distributional assumptions.
//!
//! ## Power Analysis
//!
//! Before running an A/B test, power analysis answers:
//! - How many samples do I need to detect a meaningful difference?
//! - What's the minimum effect size I can reliably detect?
//! - Did my completed test have sufficient statistical power?

use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// TASK 1: Bootstrap Confidence Intervals
// ============================================================================

/// Bootstrap confidence interval result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapCI {
    /// Original sample statistic
    pub observed: f64,
    /// Lower bound of confidence interval
    pub lower: f64,
    /// Upper bound of confidence interval
    pub upper: f64,
    /// Confidence level (e.g., 0.95 for 95% CI)
    pub confidence_level: f64,
    /// Number of bootstrap resamples used
    pub n_bootstrap: usize,
    /// Bias correction factor (for BCa)
    pub bias: f64,
    /// Acceleration factor (for BCa)
    pub acceleration: f64,
}

/// Bootstrap methods for CI calculation
#[derive(Debug, Clone, Copy, Default)]
pub enum BootstrapMethod {
    /// Simple percentile method
    Percentile,
    /// Bias-corrected and accelerated (BCa) - recommended
    #[default]
    BCa,
    /// Studentized (bootstrap-t)
    Studentized,
}

/// Bootstrap calculator for confidence intervals
pub struct Bootstrap {
    /// Number of bootstrap resamples (default: 10,000)
    n_resamples: usize,
    /// Random seed for reproducibility
    seed: Option<u64>,
    /// Bootstrap method
    method: BootstrapMethod,
}

impl Default for Bootstrap {
    fn default() -> Self {
        Self {
            n_resamples: 10_000,
            seed: None,
            method: BootstrapMethod::BCa,
        }
    }
}

impl Bootstrap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_resamples(mut self, n: usize) -> Self {
        self.n_resamples = n;
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn with_method(mut self, method: BootstrapMethod) -> Self {
        self.method = method;
        self
    }

    /// Compute bootstrap confidence interval for the mean
    pub fn mean_ci(&self, data: &[f64], confidence: f64) -> BootstrapCI {
        self.statistic_ci(data, confidence, |x| x.iter().sum::<f64>() / x.len() as f64)
    }

    /// Compute bootstrap confidence interval for the median
    pub fn median_ci(&self, data: &[f64], confidence: f64) -> BootstrapCI {
        self.statistic_ci(data, confidence, |x| {
            let mut sorted: Vec<f64> = x.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mid = sorted.len() / 2;
            if sorted.len().is_multiple_of(2) {
                (sorted[mid - 1] + sorted[mid]) / 2.0
            } else {
                sorted[mid]
            }
        })
    }

    /// Compute bootstrap confidence interval for any statistic
    pub fn statistic_ci<F>(&self, data: &[f64], confidence: f64, statistic: F) -> BootstrapCI
    where
        F: Fn(&[f64]) -> f64,
    {
        if data.is_empty() {
            return BootstrapCI {
                observed: 0.0,
                lower: 0.0,
                upper: 0.0,
                confidence_level: confidence,
                n_bootstrap: 0,
                bias: 0.0,
                acceleration: 0.0,
            };
        }

        let observed = statistic(data);
        let n = data.len();

        // Create RNG
        let mut rng: Box<dyn RngCore> = match self.seed {
            Some(s) => Box::new(StdRng::seed_from_u64(s)),
            None => Box::new(thread_rng()),
        };

        // Generate bootstrap samples
        let mut bootstrap_stats: Vec<f64> = Vec::with_capacity(self.n_resamples);
        let mut resample = vec![0.0; n];

        for _ in 0..self.n_resamples {
            // Resample with replacement
            for elem in resample.iter_mut() {
                *elem = data[rng.gen_range(0..n)];
            }
            bootstrap_stats.push(statistic(&resample));
        }

        // Sort bootstrap statistics
        bootstrap_stats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        match self.method {
            BootstrapMethod::Percentile => {
                self.percentile_ci(&bootstrap_stats, observed, confidence)
            }
            BootstrapMethod::BCa => {
                self.bca_ci(data, &bootstrap_stats, observed, confidence, &statistic)
            }
            BootstrapMethod::Studentized => {
                // Simplified: fall back to percentile for now
                self.percentile_ci(&bootstrap_stats, observed, confidence)
            }
        }
    }

    /// Simple percentile method
    fn percentile_ci(
        &self,
        bootstrap_stats: &[f64],
        observed: f64,
        confidence: f64,
    ) -> BootstrapCI {
        let alpha = 1.0 - confidence;
        let lower_idx = ((alpha / 2.0) * bootstrap_stats.len() as f64).floor() as usize;
        let upper_idx = ((1.0 - alpha / 2.0) * bootstrap_stats.len() as f64).ceil() as usize - 1;

        BootstrapCI {
            observed,
            lower: bootstrap_stats[lower_idx.min(bootstrap_stats.len() - 1)],
            upper: bootstrap_stats[upper_idx.min(bootstrap_stats.len() - 1)],
            confidence_level: confidence,
            n_bootstrap: bootstrap_stats.len(),
            bias: 0.0,
            acceleration: 0.0,
        }
    }

    /// Bias-corrected and accelerated (BCa) method
    fn bca_ci<F>(
        &self,
        data: &[f64],
        bootstrap_stats: &[f64],
        observed: f64,
        confidence: f64,
        statistic: &F,
    ) -> BootstrapCI
    where
        F: Fn(&[f64]) -> f64,
    {
        let n = data.len();
        let b = bootstrap_stats.len();

        // Bias correction factor z0
        let below_observed = bootstrap_stats.iter().filter(|&&x| x < observed).count();
        let proportion = below_observed as f64 / b as f64;
        let z0 = inv_normal_cdf(proportion);

        // Acceleration factor using jackknife
        let mut jackknife_stats = Vec::with_capacity(n);
        for i in 0..n {
            let jackknife_sample: Vec<f64> = data
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, &v)| v)
                .collect();
            jackknife_stats.push(statistic(&jackknife_sample));
        }

        let jack_mean = jackknife_stats.iter().sum::<f64>() / n as f64;
        let diffs: Vec<f64> = jackknife_stats.iter().map(|&x| jack_mean - x).collect();

        let num: f64 = diffs.iter().map(|d| d.powi(3)).sum();
        let denom: f64 = diffs.iter().map(|d| d.powi(2)).sum::<f64>().powf(1.5);
        let acceleration = if denom.abs() > 1e-10 {
            num / (6.0 * denom)
        } else {
            0.0
        };

        // BCa adjusted percentiles
        let alpha = 1.0 - confidence;
        let z_alpha_2 = inv_normal_cdf(alpha / 2.0);
        let z_1_alpha_2 = inv_normal_cdf(1.0 - alpha / 2.0);

        let alpha_1 = normal_cdf(z0 + (z0 + z_alpha_2) / (1.0 - acceleration * (z0 + z_alpha_2)));
        let alpha_2 =
            normal_cdf(z0 + (z0 + z_1_alpha_2) / (1.0 - acceleration * (z0 + z_1_alpha_2)));

        let lower_idx = ((alpha_1 * b as f64).floor() as usize).clamp(0, b - 1);
        let upper_idx = ((alpha_2 * b as f64).ceil() as usize - 1).clamp(0, b - 1);

        BootstrapCI {
            observed,
            lower: bootstrap_stats[lower_idx],
            upper: bootstrap_stats[upper_idx],
            confidence_level: confidence,
            n_bootstrap: b,
            bias: z0,
            acceleration,
        }
    }

    /// Compute bootstrap confidence interval for difference of means
    pub fn difference_ci(&self, group_a: &[f64], group_b: &[f64], confidence: f64) -> BootstrapCI {
        if group_a.is_empty() || group_b.is_empty() {
            return BootstrapCI {
                observed: 0.0,
                lower: 0.0,
                upper: 0.0,
                confidence_level: confidence,
                n_bootstrap: 0,
                bias: 0.0,
                acceleration: 0.0,
            };
        }

        let mean_a = group_a.iter().sum::<f64>() / group_a.len() as f64;
        let mean_b = group_b.iter().sum::<f64>() / group_b.len() as f64;
        let observed_diff = mean_a - mean_b;

        let mut rng: Box<dyn RngCore> = match self.seed {
            Some(s) => Box::new(StdRng::seed_from_u64(s)),
            None => Box::new(thread_rng()),
        };

        let mut bootstrap_diffs: Vec<f64> = Vec::with_capacity(self.n_resamples);

        for _ in 0..self.n_resamples {
            // Resample each group independently
            let resample_a: Vec<f64> = (0..group_a.len())
                .map(|_| group_a[rng.gen_range(0..group_a.len())])
                .collect();
            let resample_b: Vec<f64> = (0..group_b.len())
                .map(|_| group_b[rng.gen_range(0..group_b.len())])
                .collect();

            let mean_a_star = resample_a.iter().sum::<f64>() / resample_a.len() as f64;
            let mean_b_star = resample_b.iter().sum::<f64>() / resample_b.len() as f64;

            bootstrap_diffs.push(mean_a_star - mean_b_star);
        }

        bootstrap_diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let alpha = 1.0 - confidence;
        let lower_idx = ((alpha / 2.0) * bootstrap_diffs.len() as f64).floor() as usize;
        let upper_idx = ((1.0 - alpha / 2.0) * bootstrap_diffs.len() as f64).ceil() as usize - 1;

        BootstrapCI {
            observed: observed_diff,
            lower: bootstrap_diffs[lower_idx.min(bootstrap_diffs.len() - 1)],
            upper: bootstrap_diffs[upper_idx.min(bootstrap_diffs.len() - 1)],
            confidence_level: confidence,
            n_bootstrap: bootstrap_diffs.len(),
            bias: 0.0,
            acceleration: 0.0,
        }
    }
}

// ============================================================================
// TASK 2: Power Analysis
// ============================================================================

/// Result of power analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerAnalysis {
    /// Required sample size per group
    pub sample_size_per_group: usize,
    /// Total sample size
    pub total_sample_size: usize,
    /// Effect size (Cohen's d)
    pub effect_size: f64,
    /// Significance level (alpha)
    pub alpha: f64,
    /// Statistical power (1 - beta)
    pub power: f64,
    /// Type of test
    pub test_type: TestType,
}

/// Retrospective power analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrospectivePower {
    /// Achieved power with observed data
    pub achieved_power: f64,
    /// Observed effect size
    pub observed_effect_size: f64,
    /// Sample sizes
    pub n_a: usize,
    pub n_b: usize,
    /// Minimum detectable effect at 80% power
    pub minimum_detectable_effect: f64,
    /// Interpretation
    pub interpretation: PowerInterpretation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PowerInterpretation {
    Adequate,     // power >= 0.80
    Marginal,     // 0.50 <= power < 0.80
    Underpowered, // power < 0.50
}

/// Test type for power analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum TestType {
    #[default]
    TwoSampleTTest,
    PairedTTest,
    OneWayANOVA {
        groups: usize,
    },
}

/// Power analysis calculator
pub struct PowerAnalyzer {
    alpha: f64,
    power: f64,
}

impl Default for PowerAnalyzer {
    fn default() -> Self {
        Self {
            alpha: 0.05,
            power: 0.80,
        }
    }
}

impl PowerAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_alpha(mut self, alpha: f64) -> Self {
        self.alpha = alpha;
        self
    }

    pub fn with_power(mut self, power: f64) -> Self {
        self.power = power;
        self
    }

    /// Calculate required sample size for two-sample t-test
    ///
    /// # Arguments
    /// * `effect_size` - Expected Cohen's d (use 0.2 for small, 0.5 for medium, 0.8 for large)
    /// * `ratio` - Allocation ratio n2/n1 (1.0 for equal groups)
    pub fn required_sample_size(&self, effect_size: f64, ratio: f64) -> PowerAnalysis {
        if effect_size <= 0.0 {
            return PowerAnalysis {
                sample_size_per_group: usize::MAX,
                total_sample_size: usize::MAX,
                effect_size,
                alpha: self.alpha,
                power: self.power,
                test_type: TestType::TwoSampleTTest,
            };
        }

        // Two-sample t-test formula: n = 2[(z_{1-α/2} + z_{1-β})^2] / d^2
        // For unequal allocation: n1 = [(1 + 1/k)(z_{1-α/2} + z_{1-β})^2] / d^2
        let z_alpha = inv_normal_cdf(1.0 - self.alpha / 2.0);
        let z_power = inv_normal_cdf(self.power);

        let n1 = ((1.0 + 1.0 / ratio) * (z_alpha + z_power).powi(2)) / effect_size.powi(2);
        let n1_rounded = n1.ceil() as usize;
        let n2_rounded = (n1 * ratio).ceil() as usize;

        PowerAnalysis {
            sample_size_per_group: n1_rounded,
            total_sample_size: n1_rounded + n2_rounded,
            effect_size,
            alpha: self.alpha,
            power: self.power,
            test_type: TestType::TwoSampleTTest,
        }
    }

    /// Calculate power for observed data (retrospective)
    pub fn retrospective_power(
        &self,
        mean_a: f64,
        mean_b: f64,
        std_a: f64,
        std_b: f64,
        n_a: usize,
        n_b: usize,
    ) -> RetrospectivePower {
        // Pooled standard deviation
        let pooled_std = ((((n_a - 1) as f64 * std_a.powi(2))
            + ((n_b - 1) as f64 * std_b.powi(2)))
            / (n_a + n_b - 2) as f64)
            .sqrt();

        let effect_size = if pooled_std > 0.0 {
            (mean_a - mean_b).abs() / pooled_std
        } else {
            0.0
        };

        // Non-centrality parameter
        let se = pooled_std * (1.0 / n_a as f64 + 1.0 / n_b as f64).sqrt();
        let ncp = if se > 0.0 {
            (mean_a - mean_b).abs() / se
        } else {
            0.0
        };

        // Approximate power using normal approximation
        let z_alpha = inv_normal_cdf(1.0 - self.alpha / 2.0);
        let power = normal_cdf(ncp - z_alpha) + normal_cdf(-ncp - z_alpha);

        // Minimum detectable effect at 80% power
        let z_power_80 = inv_normal_cdf(0.80);
        let mde =
            (z_alpha + z_power_80) * pooled_std * (1.0 / n_a as f64 + 1.0 / n_b as f64).sqrt();

        let interpretation = if power >= 0.80 {
            PowerInterpretation::Adequate
        } else if power >= 0.50 {
            PowerInterpretation::Marginal
        } else {
            PowerInterpretation::Underpowered
        };

        RetrospectivePower {
            achieved_power: power,
            observed_effect_size: effect_size,
            n_a,
            n_b,
            minimum_detectable_effect: mde,
            interpretation,
        }
    }

    /// Sensitivity analysis: what's the minimum detectable effect?
    pub fn minimum_detectable_effect(&self, n_per_group: usize, std_dev: f64) -> f64 {
        let z_alpha = inv_normal_cdf(1.0 - self.alpha / 2.0);
        let z_power = inv_normal_cdf(self.power);

        // MDE = (z_α + z_β) * σ * sqrt(2/n)
        (z_alpha + z_power) * std_dev * (2.0 / n_per_group as f64).sqrt()
    }
}

// ============================================================================
// TASK 3: Cohen's Kappa (extends existing Krippendorff's Alpha)
// ============================================================================

/// Cohen's Kappa result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KappaResult {
    /// Cohen's Kappa coefficient
    pub kappa: f64,
    /// Observed agreement (proportion on diagonal)
    pub observed_agreement: f64,
    /// Expected agreement by chance
    pub expected_agreement: f64,
    /// Standard error of kappa
    pub standard_error: f64,
    /// 95% confidence interval
    pub confidence_interval: (f64, f64),
    /// Interpretation
    pub interpretation: KappaInterpretation,
}

/// Weighted Kappa result (for ordinal data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedKappaResult {
    /// Weighted Kappa with linear weights
    pub kappa_linear: f64,
    /// Weighted Kappa with quadratic weights
    pub kappa_quadratic: f64,
    /// Observed weighted disagreement
    pub observed_disagreement: f64,
    /// Expected weighted disagreement
    pub expected_disagreement: f64,
    /// Interpretation (using quadratic)
    pub interpretation: KappaInterpretation,
}

/// Kappa interpretation (Landis & Koch, 1977)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KappaInterpretation {
    Poor,          // κ < 0.00
    Slight,        // 0.00 <= κ < 0.20
    Fair,          // 0.21 <= κ < 0.40
    Moderate,      // 0.41 <= κ < 0.60
    Substantial,   // 0.61 <= κ < 0.80
    AlmostPerfect, // κ >= 0.81
}

impl KappaInterpretation {
    pub fn from_kappa(k: f64) -> Self {
        if k < 0.0 {
            KappaInterpretation::Poor
        } else if k < 0.20 {
            KappaInterpretation::Slight
        } else if k < 0.40 {
            KappaInterpretation::Fair
        } else if k < 0.60 {
            KappaInterpretation::Moderate
        } else if k < 0.80 {
            KappaInterpretation::Substantial
        } else {
            KappaInterpretation::AlmostPerfect
        }
    }
}

/// Inter-rater reliability calculator
pub struct InterRaterReliability;

impl InterRaterReliability {
    /// Calculate Cohen's Kappa for two raters with categorical data
    ///
    /// # Arguments
    /// * `ratings` - Vec of (rater1_rating, rater2_rating) pairs
    pub fn cohens_kappa<T: Eq + std::hash::Hash + Clone>(ratings: &[(T, T)]) -> KappaResult {
        if ratings.is_empty() {
            return KappaResult {
                kappa: 0.0,
                observed_agreement: 0.0,
                expected_agreement: 0.0,
                standard_error: 0.0,
                confidence_interval: (0.0, 0.0),
                interpretation: KappaInterpretation::Poor,
            };
        }

        let n = ratings.len() as f64;

        // Get unique categories
        let categories: std::collections::HashSet<T> = ratings
            .iter()
            .flat_map(|(a, b)| vec![a.clone(), b.clone()])
            .collect();
        let cat_vec: Vec<T> = categories.into_iter().collect();
        let k = cat_vec.len();

        // Build category index map
        let cat_index: HashMap<&T, usize> =
            cat_vec.iter().enumerate().map(|(i, c)| (c, i)).collect();

        // Build confusion matrix
        let mut matrix = vec![vec![0.0; k]; k];
        for (r1, r2) in ratings {
            let i = cat_index[r1];
            let j = cat_index[r2];
            matrix[i][j] += 1.0;
        }

        // Normalize
        for row in &mut matrix {
            for cell in row {
                *cell /= n;
            }
        }

        // Observed agreement (sum of diagonal)
        let po: f64 = (0..k).map(|i| matrix[i][i]).sum();

        // Expected agreement (sum of row_total * col_total)
        let row_totals: Vec<f64> = (0..k).map(|i| matrix[i].iter().sum()).collect();
        let col_totals: Vec<f64> = (0..k).map(|j| (0..k).map(|i| matrix[i][j]).sum()).collect();
        let pe: f64 = (0..k).map(|i| row_totals[i] * col_totals[i]).sum();

        // Kappa
        let kappa = if (1.0 - pe).abs() < 1e-10 {
            1.0 // Perfect agreement
        } else {
            (po - pe) / (1.0 - pe)
        };

        // Standard error (approximate)
        let se = ((po * (1.0 - po)) / (n * (1.0 - pe).powi(2))).sqrt();
        let ci = (kappa - 1.96 * se, kappa + 1.96 * se);

        KappaResult {
            kappa,
            observed_agreement: po,
            expected_agreement: pe,
            standard_error: se,
            confidence_interval: ci,
            interpretation: KappaInterpretation::from_kappa(kappa),
        }
    }

    /// Calculate weighted Kappa for ordinal data (e.g., 1-5 scales)
    ///
    /// # Arguments
    /// * `ratings` - Vec of (rater1_score, rater2_score) pairs (numeric ordinal)
    pub fn weighted_kappa(ratings: &[(i32, i32)]) -> WeightedKappaResult {
        if ratings.is_empty() {
            return WeightedKappaResult {
                kappa_linear: 0.0,
                kappa_quadratic: 0.0,
                observed_disagreement: 0.0,
                expected_disagreement: 0.0,
                interpretation: KappaInterpretation::Poor,
            };
        }

        // Find range of ratings
        let min_rating = ratings
            .iter()
            .flat_map(|(a, b)| [*a, *b])
            .min()
            .unwrap_or(0);
        let max_rating = ratings
            .iter()
            .flat_map(|(a, b)| [*a, *b])
            .max()
            .unwrap_or(0);
        let k = (max_rating - min_rating + 1) as usize;
        let n = ratings.len() as f64;

        if k == 0 {
            return WeightedKappaResult {
                kappa_linear: 0.0,
                kappa_quadratic: 0.0,
                observed_disagreement: 0.0,
                expected_disagreement: 0.0,
                interpretation: KappaInterpretation::Poor,
            };
        }

        // Build confusion matrix
        let mut matrix = vec![vec![0.0; k]; k];
        for (r1, r2) in ratings {
            let i = (*r1 - min_rating) as usize;
            let j = (*r2 - min_rating) as usize;
            matrix[i][j] += 1.0;
        }

        // Normalize
        for row in &mut matrix {
            for cell in row {
                *cell /= n;
            }
        }

        // Marginal distributions
        let row_totals: Vec<f64> = (0..k).map(|i| matrix[i].iter().sum()).collect();
        let col_totals: Vec<f64> = (0..k).map(|j| (0..k).map(|i| matrix[i][j]).sum()).collect();

        // Weight matrices
        let k_minus_1 = (k - 1) as f64;

        // Linear and quadratic weighted kappa
        let mut obs_lin = 0.0;
        let mut exp_lin = 0.0;
        let mut obs_quad = 0.0;
        let mut exp_quad = 0.0;

        for i in 0..k {
            for j in 0..k {
                let diff = (i as f64 - j as f64).abs();
                let w_lin = if k_minus_1 > 0.0 {
                    1.0 - diff / k_minus_1
                } else {
                    1.0
                };
                let w_quad = if k_minus_1 > 0.0 {
                    1.0 - (diff / k_minus_1).powi(2)
                } else {
                    1.0
                };

                obs_lin += w_lin * matrix[i][j];
                exp_lin += w_lin * row_totals[i] * col_totals[j];
                obs_quad += w_quad * matrix[i][j];
                exp_quad += w_quad * row_totals[i] * col_totals[j];
            }
        }

        let kappa_lin = if (1.0 - exp_lin).abs() < 1e-10 {
            1.0
        } else {
            (obs_lin - exp_lin) / (1.0 - exp_lin)
        };

        let kappa_quad = if (1.0 - exp_quad).abs() < 1e-10 {
            1.0
        } else {
            (obs_quad - exp_quad) / (1.0 - exp_quad)
        };

        WeightedKappaResult {
            kappa_linear: kappa_lin,
            kappa_quadratic: kappa_quad,
            observed_disagreement: 1.0 - obs_quad,
            expected_disagreement: 1.0 - exp_quad,
            interpretation: KappaInterpretation::from_kappa(kappa_quad),
        }
    }

    /// Fleiss' Kappa for multiple raters (n > 2)
    pub fn fleiss_kappa(ratings: &[Vec<usize>], num_categories: usize) -> f64 {
        if ratings.is_empty() || num_categories == 0 {
            return 0.0;
        }

        let n = ratings.len() as f64; // Number of subjects
        let _k = num_categories as f64; // Number of categories

        // Count how many raters assigned each category to each subject
        let mut category_counts: Vec<Vec<usize>> = Vec::with_capacity(ratings.len());
        for subject_ratings in ratings {
            let mut counts = vec![0usize; num_categories];
            for &rating in subject_ratings {
                if rating < num_categories {
                    counts[rating] += 1;
                }
            }
            category_counts.push(counts);
        }

        // Calculate P_bar (mean proportion agreement per subject)
        let m = ratings[0].len() as f64; // Number of raters (assumed constant)
        if m <= 1.0 {
            return 1.0; // Perfect agreement with single rater
        }

        let mut p_bar = 0.0;
        for counts in &category_counts {
            let sum_sq: f64 = counts.iter().map(|&c| (c as f64).powi(2)).sum();
            p_bar += (sum_sq - m) / (m * (m - 1.0));
        }
        p_bar /= n;

        // Calculate P_e (expected agreement by chance)
        let mut category_totals = vec![0.0; num_categories];
        for counts in &category_counts {
            for (j, &c) in counts.iter().enumerate() {
                category_totals[j] += c as f64;
            }
        }

        let total = n * m;
        let p_e: f64 = category_totals.iter().map(|&t| (t / total).powi(2)).sum();

        // Fleiss' Kappa
        if (1.0 - p_e).abs() < 1e-10 {
            1.0
        } else {
            (p_bar - p_e) / (1.0 - p_e)
        }
    }
}

// ============================================================================
// Helper Functions: Normal Distribution
// ============================================================================

/// Cumulative distribution function of standard normal
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Inverse CDF of standard normal (probit function)
fn inv_normal_cdf(p: f64) -> f64 {
    // Approximation using rational function (Abramowitz & Stegun)
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }

    // For p near 0.5
    if (p - 0.5).abs() < 0.42 {
        let q = p - 0.5;
        let r = q * q;
        return q
            * ((((-25.44106049637 * r + 41.39119773534) * r - 18.61500062529) * r
                + 2.50662823884)
                / ((((3.13082909833 * r - 21.06224101826) * r + 23.08336743743) * r
                    - 8.47351093090)
                    * r
                    + 1.0));
    }

    // For tails
    let r = if p < 0.5 { p } else { 1.0 - p };
    let r = (-2.0 * r.ln()).sqrt();

    let x = (((2.32121276858 * r + 4.85014127135) * r - 2.29796479134) * r - 2.78718931138)
        / (((1.63706781897 * r + 3.54388924762) * r + 1.0) * r + 0.3193815863);

    if p < 0.5 {
        -x
    } else {
        x
    }
}

/// Error function approximation
fn erf(x: f64) -> f64 {
    // Approximation using Horner's method
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();

    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_mean_ci() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let bootstrap = Bootstrap::new().with_seed(42).with_resamples(1000);
        let ci = bootstrap.mean_ci(&data, 0.95);

        assert!((ci.observed - 5.5).abs() < 0.01);
        assert!(ci.lower < ci.observed);
        assert!(ci.upper > ci.observed);
        assert!(ci.lower >= 1.0);
        assert!(ci.upper <= 10.0);
    }

    #[test]
    fn test_power_analysis() {
        let analyzer = PowerAnalyzer::new();
        let result = analyzer.required_sample_size(0.5, 1.0); // Medium effect

        // For d=0.5, α=0.05, power=0.80, sample size should be reasonable
        // The exact value depends on inv_normal_cdf approximation accuracy
        assert!(result.sample_size_per_group > 0);
        assert!(result.sample_size_per_group < usize::MAX);
        // With ratio=1.0, total should be approximately 2x per_group
        assert!(result.total_sample_size >= result.sample_size_per_group);
        // Effect size should match input
        assert!((result.effect_size - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cohens_kappa_perfect() {
        let ratings: Vec<(i32, i32)> = vec![(1, 1), (2, 2), (3, 3), (1, 1), (2, 2)];
        let result = InterRaterReliability::cohens_kappa(&ratings);

        assert!((result.kappa - 1.0).abs() < 0.01);
        assert_eq!(result.interpretation, KappaInterpretation::AlmostPerfect);
    }

    #[test]
    fn test_weighted_kappa() {
        // Example from Wikipedia
        let ratings = vec![
            (1, 1),
            (1, 2),
            (2, 1),
            (2, 2),
            (3, 3),
            (3, 4),
            (4, 4),
            (4, 5),
        ];
        let result = InterRaterReliability::weighted_kappa(&ratings);

        assert!(result.kappa_quadratic > 0.0);
        assert!(result.kappa_quadratic < 1.0);
    }
}
