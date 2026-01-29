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

//! Statistical Comparison Module for A/B Testing Eval Runs
//!
//! Provides Welch's t-test and Cohen's d effect size for comparing
//! two evaluation runs with statistical rigor.
//!
//! ## Multi-Run Comparison (N > 2)
//!
//! For comparing more than 2 runs simultaneously, this module provides:
//! - **One-way ANOVA**: For normally distributed data with equal variances
//! - **Kruskal-Wallis H-test**: Non-parametric alternative for any distribution
//! - **Tukey's HSD**: Post-hoc pairwise comparison after significant ANOVA
//! - **Bonferroni correction**: Control family-wise error rate

// Statistical code has inherently complex types and precision requirements
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::excessive_precision)]
#![allow(clippy::needless_range_loop)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of comparing two eval runs on a specific metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricComparison {
    /// Name of the metric being compared
    pub metric_name: String,

    /// Baseline run statistics
    pub baseline: RunStats,

    /// Treatment run statistics  
    pub treatment: RunStats,

    /// Difference (treatment - baseline)
    pub difference: f64,

    /// Percent change from baseline
    pub percent_change: f64,

    /// Welch's t-statistic
    pub t_statistic: f64,

    /// Two-tailed p-value
    pub p_value: f64,

    /// Cohen's d effect size
    pub cohens_d: f64,

    /// Effect size interpretation
    pub effect_size: EffectSize,

    /// 95% confidence interval for the difference
    pub confidence_interval: (f64, f64),

    /// Whether the difference is statistically significant (p < 0.05)
    pub is_significant: bool,

    /// Which run is better for this metric (higher is better assumed)
    pub winner: Option<Winner>,
}

/// Statistics for a single run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStats {
    pub run_id: String,
    pub run_name: String,
    pub mean: f64,
    pub std_dev: f64,
    pub n: usize,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p95: f64,
}

/// Effect size interpretation based on Cohen's conventions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EffectSize {
    Negligible, // |d| < 0.2
    Small,      // 0.2 <= |d| < 0.5
    Medium,     // 0.5 <= |d| < 0.8
    Large,      // |d| >= 0.8
}

impl EffectSize {
    pub fn from_cohens_d(d: f64) -> Self {
        let abs_d = d.abs();
        if abs_d < 0.2 {
            EffectSize::Negligible
        } else if abs_d < 0.5 {
            EffectSize::Small
        } else if abs_d < 0.8 {
            EffectSize::Medium
        } else {
            EffectSize::Large
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EffectSize::Negligible => "negligible",
            EffectSize::Small => "small",
            EffectSize::Medium => "medium",
            EffectSize::Large => "large",
        }
    }
}

/// Which run won the comparison
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Winner {
    Baseline,
    Treatment,
    Tie,
}

// ============================================================================
// Multi-Run Comparison Types (ANOVA / Kruskal-Wallis)
// ============================================================================

/// Result of comparing N eval runs simultaneously using ANOVA or Kruskal-Wallis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRunComparisonResult {
    /// IDs of all compared runs
    pub run_ids: Vec<String>,

    /// Comparison results for each metric
    pub metrics: Vec<MultiRunMetricComparison>,

    /// Overall recommendation
    pub recommendation: MultiRunRecommendation,

    /// Summary statistics
    pub summary: MultiRunSummary,
}

/// Result of multi-run comparison for a single metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRunMetricComparison {
    /// Name of the metric being compared
    pub metric_name: String,

    /// Statistics for each run
    pub run_stats: Vec<RunStats>,

    /// ANOVA F-statistic (if parametric test used)
    pub f_statistic: Option<f64>,

    /// Kruskal-Wallis H-statistic (if non-parametric test used)
    pub h_statistic: Option<f64>,

    /// P-value from the omnibus test
    pub p_value: f64,

    /// Whether there's a significant difference among groups
    pub is_significant: bool,

    /// Post-hoc pairwise comparisons (Tukey's HSD)
    pub pairwise_comparisons: Vec<PairwiseComparison>,

    /// Effect size (eta-squared for ANOVA)
    pub eta_squared: f64,

    /// Effect size interpretation
    pub effect_size: MultiRunEffectSize,

    /// The best performing run for this metric
    pub best_run_id: Option<String>,
}

/// Pairwise comparison result from Tukey's HSD or Dunn's test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairwiseComparison {
    /// First run ID
    pub run_a_id: String,

    /// Second run ID
    pub run_b_id: String,

    /// Mean difference (run_b - run_a)
    pub mean_difference: f64,

    /// Standard error of the difference
    pub std_error: f64,

    /// P-value (Bonferroni corrected)
    pub p_value: f64,

    /// 95% confidence interval for the difference
    pub confidence_interval: (f64, f64),

    /// Whether this specific pair differs significantly
    pub is_significant: bool,
}

/// Effect size for multi-run comparison
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MultiRunEffectSize {
    Negligible, // η² < 0.01
    Small,      // 0.01 <= η² < 0.06
    Medium,     // 0.06 <= η² < 0.14
    Large,      // η² >= 0.14
}

impl MultiRunEffectSize {
    pub fn from_eta_squared(eta_sq: f64) -> Self {
        if eta_sq < 0.01 {
            MultiRunEffectSize::Negligible
        } else if eta_sq < 0.06 {
            MultiRunEffectSize::Small
        } else if eta_sq < 0.14 {
            MultiRunEffectSize::Medium
        } else {
            MultiRunEffectSize::Large
        }
    }
}

/// Recommendation for multi-run comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRunRecommendation {
    /// Recommended run to deploy
    pub recommended_run_id: Option<String>,

    /// Action to take
    pub action: MultiRunAction,

    /// Confidence level (0-1)
    pub confidence: f64,

    /// Human-readable explanation
    pub explanation: String,

    /// Ranking of runs from best to worst
    pub run_rankings: Vec<RunRanking>,
}

/// Ranking of a single run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRanking {
    pub run_id: String,
    pub rank: usize,
    pub overall_score: f64,
    pub significant_wins: usize,
    pub significant_losses: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MultiRunAction {
    DeployBest,
    NeedMoreData,
    NoSignificantDifference,
    MixedResults,
}

/// Summary of multi-run comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRunSummary {
    pub total_runs: usize,
    pub total_metrics: usize,
    pub metrics_with_significant_difference: usize,
    pub total_pairwise_comparisons: usize,
    pub significant_pairwise_differences: usize,
}

// ============================================================================
// Original Two-Run Comparison Types
// ============================================================================

/// Full comparison result between two eval runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// Baseline run ID
    pub baseline_run_id: String,

    /// Treatment run ID
    pub treatment_run_id: String,

    /// Comparison results for each metric
    pub metrics: Vec<MetricComparison>,

    /// Overall recommendation
    pub recommendation: Recommendation,

    /// Summary statistics
    pub summary: ComparisonSummary,
}

/// Recommendation based on comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Action to take
    pub action: RecommendedAction,

    /// Confidence level (0-1)
    pub confidence: f64,

    /// Human-readable explanation
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedAction {
    DeployTreatment,
    KeepBaseline,
    NeedMoreData,
    Inconclusive,
}

/// Summary of the comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub total_metrics: usize,
    pub significant_improvements: usize,
    pub significant_regressions: usize,
    pub no_significant_change: usize,
}

/// Statistical comparator for eval runs
pub struct Comparator;

impl Comparator {
    // ========================================================================
    // Multi-Run Comparison (ANOVA / Kruskal-Wallis)
    // ========================================================================

    /// Compare N runs simultaneously using one-way ANOVA
    ///
    /// Returns comprehensive statistical analysis including:
    /// - One-way ANOVA F-test for each metric
    /// - Tukey's HSD post-hoc pairwise comparisons
    /// - Effect size (eta-squared)
    /// - Rankings and recommendations
    pub fn compare_multiple_runs(
        runs_data: &[(String, String, HashMap<String, Vec<f64>>)], // (run_id, run_name, metric_values)
        metric_direction: &HashMap<String, bool>,                  // true = higher is better
    ) -> MultiRunComparisonResult {
        if runs_data.len() < 2 {
            return MultiRunComparisonResult {
                run_ids: runs_data.iter().map(|(id, _, _)| id.clone()).collect(),
                metrics: vec![],
                recommendation: MultiRunRecommendation {
                    recommended_run_id: None,
                    action: MultiRunAction::NeedMoreData,
                    confidence: 0.0,
                    explanation: "Need at least 2 runs to compare".to_string(),
                    run_rankings: vec![],
                },
                summary: MultiRunSummary {
                    total_runs: runs_data.len(),
                    total_metrics: 0,
                    metrics_with_significant_difference: 0,
                    total_pairwise_comparisons: 0,
                    significant_pairwise_differences: 0,
                },
            };
        }

        let run_ids: Vec<String> = runs_data.iter().map(|(id, _, _)| id.clone()).collect();
        let run_names: Vec<String> = runs_data.iter().map(|(_, name, _)| name.clone()).collect();

        // Collect all metric names
        let mut all_metrics: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (_, _, metrics) in runs_data {
            for key in metrics.keys() {
                all_metrics.insert(key.clone());
            }
        }

        let mut metric_comparisons = Vec::new();
        let mut total_pairwise = 0;
        let mut significant_pairwise = 0;

        for metric_name in &all_metrics {
            // Collect values for each run
            let mut groups: Vec<Vec<f64>> = Vec::new();
            let mut run_stats_list = Vec::new();

            for (run_id, run_name, metrics) in runs_data.iter() {
                let values = metrics.get(metric_name).cloned().unwrap_or_default();
                if !values.is_empty() {
                    let stats = Self::compute_stats(&values, run_id, run_name);
                    run_stats_list.push(stats);
                    groups.push(values);
                } else {
                    // Empty group - use placeholder stats
                    run_stats_list.push(RunStats {
                        run_id: run_id.clone(),
                        run_name: run_name.clone(),
                        mean: 0.0,
                        std_dev: 0.0,
                        n: 0,
                        min: 0.0,
                        max: 0.0,
                        p50: 0.0,
                        p95: 0.0,
                    });
                    groups.push(vec![]);
                }
            }

            // Skip if not enough data
            let non_empty_groups: Vec<&Vec<f64>> =
                groups.iter().filter(|g| !g.is_empty()).collect();
            if non_empty_groups.len() < 2 {
                continue;
            }

            // Perform one-way ANOVA
            let (f_stat, p_value, eta_squared) = Self::one_way_anova(&groups);
            let is_significant = p_value < 0.05;

            // Perform Tukey's HSD if significant
            let mut pairwise_comparisons = Vec::new();
            if is_significant {
                let pairwise = Self::tukeys_hsd(&groups, &run_ids, &run_names);
                total_pairwise += pairwise.len();
                significant_pairwise += pairwise.iter().filter(|p| p.is_significant).count();
                pairwise_comparisons = pairwise;
            }

            // Find best run for this metric
            let higher_is_better = metric_direction.get(metric_name).copied().unwrap_or(true);
            let best_run_id = run_stats_list
                .iter()
                .filter(|s| s.n > 0)
                .max_by(|a, b| {
                    let ord = a
                        .mean
                        .partial_cmp(&b.mean)
                        .unwrap_or(std::cmp::Ordering::Equal);
                    if higher_is_better {
                        ord
                    } else {
                        ord.reverse()
                    }
                })
                .map(|s| s.run_id.clone());

            metric_comparisons.push(MultiRunMetricComparison {
                metric_name: metric_name.clone(),
                run_stats: run_stats_list,
                f_statistic: Some(f_stat),
                h_statistic: None,
                p_value,
                is_significant,
                pairwise_comparisons,
                eta_squared,
                effect_size: MultiRunEffectSize::from_eta_squared(eta_squared),
                best_run_id,
            });
        }

        let metrics_with_sig = metric_comparisons
            .iter()
            .filter(|m| m.is_significant)
            .count();

        // Generate rankings
        let run_rankings =
            Self::compute_run_rankings(&run_ids, &metric_comparisons, metric_direction);

        // Generate recommendation
        let recommendation =
            Self::generate_multi_run_recommendation(&run_rankings, &metric_comparisons);

        MultiRunComparisonResult {
            run_ids,
            metrics: metric_comparisons,
            recommendation,
            summary: MultiRunSummary {
                total_runs: runs_data.len(),
                total_metrics: all_metrics.len(),
                metrics_with_significant_difference: metrics_with_sig,
                total_pairwise_comparisons: total_pairwise,
                significant_pairwise_differences: significant_pairwise,
            },
        }
    }

    /// Perform one-way ANOVA
    /// Returns (F-statistic, p-value, eta-squared)
    fn one_way_anova(groups: &[Vec<f64>]) -> (f64, f64, f64) {
        let non_empty: Vec<&Vec<f64>> = groups.iter().filter(|g| !g.is_empty()).collect();
        let k = non_empty.len() as f64; // Number of groups

        if k < 2.0 {
            return (0.0, 1.0, 0.0);
        }

        // Calculate grand mean
        let all_values: Vec<f64> = non_empty.iter().flat_map(|g| g.iter().cloned()).collect();
        let n_total = all_values.len() as f64;
        let grand_mean = all_values.iter().sum::<f64>() / n_total;

        // Calculate group means
        let group_means: Vec<f64> = non_empty
            .iter()
            .map(|g| g.iter().sum::<f64>() / g.len() as f64)
            .collect();

        // Between-group sum of squares (SSB)
        let ssb: f64 = non_empty
            .iter()
            .zip(group_means.iter())
            .map(|(g, mean)| g.len() as f64 * (mean - grand_mean).powi(2))
            .sum();

        // Within-group sum of squares (SSW)
        let ssw: f64 = non_empty
            .iter()
            .zip(group_means.iter())
            .map(|(g, mean)| g.iter().map(|x| (x - mean).powi(2)).sum::<f64>())
            .sum();

        // Total sum of squares (SST)
        let sst = ssb + ssw;

        // Degrees of freedom
        let df_between = k - 1.0;
        let df_within = n_total - k;

        if df_within <= 0.0 || ssw == 0.0 {
            return (0.0, 1.0, 0.0);
        }

        // Mean squares
        let msb = ssb / df_between;
        let msw = ssw / df_within;

        // F-statistic
        let f_stat = if msw > 0.0 { msb / msw } else { 0.0 };

        // P-value from F-distribution (approximation)
        let p_value = Self::f_distribution_p_value(f_stat, df_between, df_within);

        // Effect size: eta-squared
        let eta_squared = if sst > 0.0 { ssb / sst } else { 0.0 };

        (f_stat, p_value, eta_squared)
    }

    /// Perform Kruskal-Wallis H-test (non-parametric alternative to ANOVA)
    /// Returns (H-statistic, p-value)
    pub fn kruskal_wallis(groups: &[Vec<f64>]) -> (f64, f64) {
        let non_empty: Vec<&Vec<f64>> = groups.iter().filter(|g| !g.is_empty()).collect();
        let k = non_empty.len();

        if k < 2 {
            return (0.0, 1.0);
        }

        // Combine all values with group labels
        let mut all_with_groups: Vec<(f64, usize)> = Vec::new();
        for (group_idx, group) in non_empty.iter().enumerate() {
            for &val in *group {
                all_with_groups.push((val, group_idx));
            }
        }

        let n_total = all_with_groups.len();
        if n_total < 3 {
            return (0.0, 1.0);
        }

        // Assign ranks (average ranks for ties)
        all_with_groups.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut ranks = vec![0.0; n_total];
        let mut i = 0;
        while i < n_total {
            let mut j = i;
            while j < n_total && (all_with_groups[j].0 - all_with_groups[i].0).abs() < 1e-10 {
                j += 1;
            }
            // Average rank for ties
            let avg_rank = (i + 1..=j).map(|r| r as f64).sum::<f64>() / (j - i) as f64;
            for k in i..j {
                ranks[k] = avg_rank;
            }
            i = j;
        }

        // Calculate rank sums for each group
        let mut rank_sums = vec![0.0; k];
        let mut group_sizes = vec![0usize; k];
        for (idx, &(_, group_idx)) in all_with_groups.iter().enumerate() {
            rank_sums[group_idx] += ranks[idx];
            group_sizes[group_idx] += 1;
        }

        // Calculate H statistic
        let n = n_total as f64;
        let mut h_sum = 0.0;
        for (i, &r_sum) in rank_sums.iter().enumerate() {
            let n_i = group_sizes[i] as f64;
            if n_i > 0.0 {
                h_sum += (r_sum * r_sum) / n_i;
            }
        }
        let h = (12.0 / (n * (n + 1.0))) * h_sum - 3.0 * (n + 1.0);

        // P-value using chi-squared approximation (df = k - 1)
        let df = k as f64 - 1.0;
        let p_value = Self::chi_squared_p_value(h, df);

        (h, p_value)
    }

    /// Tukey's Honestly Significant Difference (HSD) post-hoc test
    fn tukeys_hsd(
        groups: &[Vec<f64>],
        run_ids: &[String],
        _run_names: &[String],
    ) -> Vec<PairwiseComparison> {
        let mut comparisons = Vec::new();
        let non_empty_indices: Vec<usize> = groups
            .iter()
            .enumerate()
            .filter(|(_, g)| !g.is_empty())
            .map(|(i, _)| i)
            .collect();

        if non_empty_indices.len() < 2 {
            return comparisons;
        }

        // Calculate pooled variance (MSW from ANOVA)
        let group_means: Vec<f64> = groups
            .iter()
            .map(|g| {
                if g.is_empty() {
                    0.0
                } else {
                    g.iter().sum::<f64>() / g.len() as f64
                }
            })
            .collect();

        let ssw: f64 = groups
            .iter()
            .zip(group_means.iter())
            .filter(|(g, _)| !g.is_empty())
            .map(|(g, mean)| g.iter().map(|x| (x - mean).powi(2)).sum::<f64>())
            .sum();

        let n_total: usize = groups.iter().map(|g| g.len()).sum();
        let k = non_empty_indices.len();
        let df_within = n_total - k;

        if df_within == 0 {
            return comparisons;
        }

        let msw = ssw / df_within as f64;

        // Number of comparisons for Bonferroni correction
        let num_comparisons = (k * (k - 1)) / 2;

        // Pairwise comparisons
        for i in 0..non_empty_indices.len() {
            for j in (i + 1)..non_empty_indices.len() {
                let idx_a = non_empty_indices[i];
                let idx_b = non_empty_indices[j];

                let n_a = groups[idx_a].len() as f64;
                let n_b = groups[idx_b].len() as f64;
                let mean_a = group_means[idx_a];
                let mean_b = group_means[idx_b];

                let mean_diff = mean_b - mean_a;
                let se = (msw * (1.0 / n_a + 1.0 / n_b)).sqrt();

                // T-statistic for this pair
                let t = if se > 0.0 { mean_diff.abs() / se } else { 0.0 };

                // Bonferroni-corrected p-value
                let raw_p = Self::t_distribution_p_value(t, df_within as f64);
                let corrected_p = (raw_p * num_comparisons as f64).min(1.0);

                // Confidence interval (Bonferroni-corrected)
                let alpha_corrected = 0.05 / num_comparisons as f64;
                let t_crit = Self::t_critical(1.0 - alpha_corrected / 2.0, df_within as f64);
                let margin = t_crit * se;

                comparisons.push(PairwiseComparison {
                    run_a_id: run_ids[idx_a].clone(),
                    run_b_id: run_ids[idx_b].clone(),
                    mean_difference: mean_diff,
                    std_error: se,
                    p_value: corrected_p,
                    confidence_interval: (mean_diff - margin, mean_diff + margin),
                    is_significant: corrected_p < 0.05,
                });
            }
        }

        comparisons
    }

    /// Compute rankings for each run based on pairwise comparisons
    fn compute_run_rankings(
        run_ids: &[String],
        metric_comparisons: &[MultiRunMetricComparison],
        metric_direction: &HashMap<String, bool>,
    ) -> Vec<RunRanking> {
        let mut scores: HashMap<String, (f64, usize, usize)> = HashMap::new(); // (score, wins, losses)

        for run_id in run_ids {
            scores.insert(run_id.clone(), (0.0, 0, 0));
        }

        // Accumulate scores from pairwise comparisons
        for comparison in metric_comparisons {
            let higher_is_better = metric_direction
                .get(&comparison.metric_name)
                .copied()
                .unwrap_or(true);

            for pairwise in &comparison.pairwise_comparisons {
                if pairwise.is_significant {
                    let winner = if (pairwise.mean_difference > 0.0) == higher_is_better {
                        &pairwise.run_b_id
                    } else {
                        &pairwise.run_a_id
                    };
                    let loser = if winner == &pairwise.run_a_id {
                        &pairwise.run_b_id
                    } else {
                        &pairwise.run_a_id
                    };

                    if let Some((score, wins, _)) = scores.get_mut(winner) {
                        *score += 1.0;
                        *wins += 1;
                    }
                    if let Some((score, _, losses)) = scores.get_mut(loser) {
                        *score -= 0.5;
                        *losses += 1;
                    }
                }
            }
        }

        let mut rankings: Vec<RunRanking> = scores
            .into_iter()
            .map(|(run_id, (score, wins, losses))| RunRanking {
                run_id,
                rank: 0,
                overall_score: score,
                significant_wins: wins,
                significant_losses: losses,
            })
            .collect();

        // Sort by score (descending)
        rankings.sort_by(|a, b| {
            b.overall_score
                .partial_cmp(&a.overall_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Assign ranks
        for (i, ranking) in rankings.iter_mut().enumerate() {
            ranking.rank = i + 1;
        }

        rankings
    }

    /// Generate recommendation for multi-run comparison
    fn generate_multi_run_recommendation(
        rankings: &[RunRanking],
        comparisons: &[MultiRunMetricComparison],
    ) -> MultiRunRecommendation {
        let significant_metrics = comparisons.iter().filter(|c| c.is_significant).count();

        if rankings.is_empty() {
            return MultiRunRecommendation {
                recommended_run_id: None,
                action: MultiRunAction::NeedMoreData,
                confidence: 0.0,
                explanation: "No runs to compare".to_string(),
                run_rankings: vec![],
            };
        }

        if significant_metrics == 0 {
            return MultiRunRecommendation {
                recommended_run_id: None,
                action: MultiRunAction::NoSignificantDifference,
                confidence: 0.5,
                explanation: "No statistically significant differences found between runs. Consider collecting more data or using different metrics.".to_string(),
                run_rankings: rankings.to_vec(),
            };
        }

        let best = &rankings[0];

        if best.significant_wins == 0 && best.significant_losses == 0 {
            return MultiRunRecommendation {
                recommended_run_id: None,
                action: MultiRunAction::MixedResults,
                confidence: 0.4,
                explanation: "Mixed results across metrics. Manual review recommended.".to_string(),
                run_rankings: rankings.to_vec(),
            };
        }

        let win_ratio = best.significant_wins as f64
            / (best.significant_wins + best.significant_losses).max(1) as f64;
        let confidence = (0.5 + win_ratio * 0.5).min(0.95);

        if best.significant_wins > best.significant_losses {
            MultiRunRecommendation {
                recommended_run_id: Some(best.run_id.clone()),
                action: MultiRunAction::DeployBest,
                confidence,
                explanation: format!(
                    "Run '{}' is the best performer with {} significant wins and {} losses across {} metrics with significant differences.",
                    best.run_id, best.significant_wins, best.significant_losses, significant_metrics
                ),
                run_rankings: rankings.to_vec(),
            }
        } else {
            MultiRunRecommendation {
                recommended_run_id: Some(best.run_id.clone()),
                action: MultiRunAction::MixedResults,
                confidence: confidence * 0.7,
                explanation: format!(
                    "Run '{}' has the highest overall score but results are mixed. Review individual metrics before deploying.",
                    best.run_id
                ),
                run_rankings: rankings.to_vec(),
            }
        }
    }

    /// Approximate p-value from F-distribution
    fn f_distribution_p_value(f: f64, df1: f64, df2: f64) -> f64 {
        if f <= 0.0 || df1 <= 0.0 || df2 <= 0.0 {
            return 1.0;
        }

        // Use incomplete beta function: P(F > f) = I_{df2/(df2+df1*f)}(df2/2, df1/2)
        let x = df2 / (df2 + df1 * f);
        Self::incomplete_beta(df2 / 2.0, df1 / 2.0, x)
    }

    /// Approximate p-value from chi-squared distribution
    fn chi_squared_p_value(chi2: f64, df: f64) -> f64 {
        if chi2 <= 0.0 || df <= 0.0 {
            return 1.0;
        }

        // Use incomplete gamma function approximation
        // P(X > chi2) = 1 - gamma(df/2, chi2/2) / Gamma(df/2)
        let a = df / 2.0;
        let x = chi2 / 2.0;

        // Simple approximation using normal distribution for large df
        if df > 100.0 {
            let z = ((chi2 / df).powf(1.0 / 3.0) - (1.0 - 2.0 / (9.0 * df)))
                / (2.0 / (9.0 * df)).sqrt();
            return 1.0 - Self::normal_cdf(z);
        }

        // For smaller df, use regularized incomplete gamma
        1.0 - Self::regularized_gamma(a, x)
    }

    /// Regularized incomplete gamma function (approximation)
    fn regularized_gamma(a: f64, x: f64) -> f64 {
        if x < 0.0 || a <= 0.0 {
            return 0.0;
        }

        if x < a + 1.0 {
            // Series expansion
            let mut sum = 1.0 / a;
            let mut term = sum;
            for n in 1..100 {
                term *= x / (a + n as f64);
                sum += term;
                if term.abs() < 1e-10 {
                    break;
                }
            }
            sum * (-x + a * x.ln() - Self::ln_gamma(a)).exp()
        } else {
            // Continued fraction
            1.0 - Self::incomplete_gamma_cf(a, x)
        }
    }

    /// Continued fraction for incomplete gamma
    fn incomplete_gamma_cf(a: f64, x: f64) -> f64 {
        let mut f = 1.0;
        let mut c = 1.0;
        let mut d = 1.0 / x;

        for n in 1..100 {
            let an = if n % 2 == 1 {
                ((n as f64 + 1.0) / 2.0) - a
            } else {
                n as f64 / 2.0
            };
            let bn = x + n as f64 + 1.0 - a;

            d = bn + an * d;
            if d.abs() < 1e-30 {
                d = 1e-30;
            }
            c = bn + an / c;
            if c.abs() < 1e-30 {
                c = 1e-30;
            }
            d = 1.0 / d;
            let delta = c * d;
            f *= delta;
            if (delta - 1.0).abs() < 1e-10 {
                break;
            }
        }

        ((-x + a * x.ln() - Self::ln_gamma(a)).exp() / x) * f
    }

    /// Log gamma function (Lanczos approximation)
    fn ln_gamma(z: f64) -> f64 {
        Self::gamma(z).ln()
    }

    // ========================================================================
    // Two-Run Comparison (Original)
    // ========================================================================

    /// Compare two sets of metric values using Welch's t-test
    pub fn compare_metrics(
        baseline_values: &[f64],
        treatment_values: &[f64],
        metric_name: &str,
        baseline_run_id: &str,
        baseline_run_name: &str,
        treatment_run_id: &str,
        treatment_run_name: &str,
        higher_is_better: bool,
    ) -> Option<MetricComparison> {
        if baseline_values.is_empty() || treatment_values.is_empty() {
            return None;
        }

        let baseline_stats =
            Self::compute_stats(baseline_values, baseline_run_id, baseline_run_name);
        let treatment_stats =
            Self::compute_stats(treatment_values, treatment_run_id, treatment_run_name);

        let n1 = baseline_stats.n as f64;
        let n2 = treatment_stats.n as f64;
        let m1 = baseline_stats.mean;
        let m2 = treatment_stats.mean;
        let s1 = baseline_stats.std_dev;
        let s2 = treatment_stats.std_dev;

        // Welch's t-test
        let se = ((s1 * s1 / n1) + (s2 * s2 / n2)).sqrt();
        let t_statistic = if se > 0.0 { (m2 - m1) / se } else { 0.0 };

        // Welch-Satterthwaite degrees of freedom
        let v1 = s1 * s1 / n1;
        let v2 = s2 * s2 / n2;
        let df = if v1 + v2 > 0.0 {
            ((v1 + v2).powi(2)) / ((v1 * v1 / (n1 - 1.0)) + (v2 * v2 / (n2 - 1.0)))
        } else {
            n1 + n2 - 2.0
        };

        // Approximate p-value using Student's t-distribution
        let p_value = Self::t_distribution_p_value(t_statistic.abs(), df);

        // Cohen's d (pooled standard deviation)
        let pooled_std = (((n1 - 1.0) * s1 * s1 + (n2 - 1.0) * s2 * s2) / (n1 + n2 - 2.0)).sqrt();
        let cohens_d = if pooled_std > 0.0 {
            (m2 - m1) / pooled_std
        } else {
            0.0
        };

        // 95% confidence interval
        let t_crit = Self::t_critical(0.975, df); // Two-tailed 95% CI
        let margin = t_crit * se;
        let diff = m2 - m1;
        let confidence_interval = (diff - margin, diff + margin);

        let is_significant = p_value < 0.05;

        // Determine winner based on significance and direction
        let winner = if !is_significant {
            Some(Winner::Tie)
        } else if higher_is_better {
            if diff > 0.0 {
                Some(Winner::Treatment)
            } else {
                Some(Winner::Baseline)
            }
        } else {
            // For metrics where lower is better (e.g., hallucination, latency)
            if diff < 0.0 {
                Some(Winner::Treatment)
            } else {
                Some(Winner::Baseline)
            }
        };

        let percent_change = if m1 != 0.0 { (diff / m1) * 100.0 } else { 0.0 };

        Some(MetricComparison {
            metric_name: metric_name.to_string(),
            baseline: baseline_stats,
            treatment: treatment_stats,
            difference: diff,
            percent_change,
            t_statistic,
            p_value,
            cohens_d,
            effect_size: EffectSize::from_cohens_d(cohens_d),
            confidence_interval,
            is_significant,
            winner,
        })
    }

    /// Compute statistics for a set of values
    fn compute_stats(values: &[f64], run_id: &str, run_name: &str) -> RunStats {
        let n = values.len();
        if n == 0 {
            return RunStats {
                run_id: run_id.to_string(),
                run_name: run_name.to_string(),
                mean: 0.0,
                std_dev: 0.0,
                n: 0,
                min: 0.0,
                max: 0.0,
                p50: 0.0,
                p95: 0.0,
            };
        }

        let mean = values.iter().sum::<f64>() / n as f64;
        let variance =
            values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1).max(1) as f64;
        let std_dev = variance.sqrt();

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min = sorted.first().copied().unwrap_or(0.0);
        let max = sorted.last().copied().unwrap_or(0.0);
        let p50 = Self::percentile(&sorted, 0.50);
        let p95 = Self::percentile(&sorted, 0.95);

        RunStats {
            run_id: run_id.to_string(),
            run_name: run_name.to_string(),
            mean,
            std_dev,
            n,
            min,
            max,
            p50,
            p95,
        }
    }

    fn percentile(sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let idx = (p * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Approximate p-value for t-distribution (two-tailed)
    fn t_distribution_p_value(t: f64, df: f64) -> f64 {
        // Use approximation for large df
        if df > 100.0 {
            // Normal approximation
            return 2.0 * Self::normal_cdf(-t.abs());
        }

        // Use regularized incomplete beta function approximation
        let x = df / (df + t * t);
        2.0 * Self::incomplete_beta(df / 2.0, 0.5, x) / 2.0
    }

    /// Normal CDF approximation
    fn normal_cdf(x: f64) -> f64 {
        0.5 * (1.0 + Self::erf(x / std::f64::consts::SQRT_2))
    }

    /// Error function approximation (Abramowitz and Stegun)
    fn erf(x: f64) -> f64 {
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();
        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
        sign * y
    }

    /// Incomplete beta function approximation
    fn incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
        // Simple approximation using continued fraction
        if x == 0.0 {
            return 0.0;
        }
        if x == 1.0 {
            return 1.0;
        }

        // Use Lentz's algorithm for continued fraction
        let mut f = 1.0;
        let mut c = 1.0;
        let mut d = 0.0;

        for m in 1..100 {
            let m = m as f64;

            // Even step
            let numerator = m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m));
            d = 1.0 + numerator * d;
            if d.abs() < 1e-30 {
                d = 1e-30;
            }
            c = 1.0 + numerator / c;
            if c.abs() < 1e-30 {
                c = 1e-30;
            }
            d = 1.0 / d;
            f *= c * d;

            // Odd step
            let numerator = -(a + m) * (a + b + m) * x / ((a + 2.0 * m) * (a + 2.0 * m + 1.0));
            d = 1.0 + numerator * d;
            if d.abs() < 1e-30 {
                d = 1e-30;
            }
            c = 1.0 + numerator / c;
            if c.abs() < 1e-30 {
                c = 1e-30;
            }
            d = 1.0 / d;
            let delta = c * d;
            f *= delta;

            if (delta - 1.0).abs() < 1e-10 {
                break;
            }
        }

        // Return incomplete beta
        let prefix = x.powf(a) * (1.0 - x).powf(b) / (a * Self::beta(a, b));
        prefix * f
    }

    /// Beta function approximation using gamma
    fn beta(a: f64, b: f64) -> f64 {
        Self::gamma(a) * Self::gamma(b) / Self::gamma(a + b)
    }

    /// Lanczos approximation of gamma function
    fn gamma(z: f64) -> f64 {
        let g = 7;
        let c = [
            0.99999999999980993,
            676.5203681218851,
            -1259.1392167224028,
            771.32342877765313,
            -176.61502916214059,
            12.507343278686905,
            -0.13857109526572012,
            9.9843695780195716e-6,
            1.5056327351493116e-7,
        ];

        if z < 0.5 {
            std::f64::consts::PI / ((std::f64::consts::PI * z).sin() * Self::gamma(1.0 - z))
        } else {
            let z = z - 1.0;
            let mut x = c[0];
            for i in 1..g + 2 {
                x += c[i] / (z + i as f64);
            }
            let t = z + g as f64 + 0.5;
            (2.0 * std::f64::consts::PI).sqrt() * t.powf(z + 0.5) * (-t).exp() * x
        }
    }

    /// Critical t-value for given probability and degrees of freedom
    fn t_critical(p: f64, df: f64) -> f64 {
        // Approximation for t-critical value
        // For df > 30, use normal approximation
        if df > 30.0 {
            return Self::normal_inv(p);
        }

        // Simple lookup/approximation for common values
        // This is a rough approximation; real implementation would use more accurate method
        let z = Self::normal_inv(p);
        let g1 = (z.powi(3) + z) / 4.0;
        let g2 = (5.0 * z.powi(5) + 16.0 * z.powi(3) + 3.0 * z) / 96.0;
        z + g1 / df + g2 / (df * df)
    }

    /// Inverse normal CDF (probit function) approximation
    fn normal_inv(p: f64) -> f64 {
        // Rational approximation
        let a = [
            -3.969683028665376e+01,
            2.209460984245205e+02,
            -2.759285104469687e+02,
            1.383577518672690e+02,
            -3.066479806614716e+01,
            2.506628277459239e+00,
        ];
        let b = [
            -5.447609879822406e+01,
            1.615858368580409e+02,
            -1.556989798598866e+02,
            6.680131188771972e+01,
            -1.328068155288572e+01,
        ];
        let c = [
            -7.784894002430293e-03,
            -3.223964580411365e-01,
            -2.400758277161838e+00,
            -2.549732539343734e+00,
            4.374664141464968e+00,
            2.938163982698783e+00,
        ];
        let d = [
            7.784695709041462e-03,
            3.224671290700398e-01,
            2.445134137142996e+00,
            3.754408661907416e+00,
        ];

        let p_low = 0.02425;
        let p_high = 1.0 - p_low;

        if p < p_low {
            let q = (-2.0 * p.ln()).sqrt();
            (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
                / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
        } else if p <= p_high {
            let q = p - 0.5;
            let r = q * q;
            (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
                / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
        } else {
            let q = (-2.0 * (1.0 - p).ln()).sqrt();
            -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
                / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
        }
    }

    /// Compare two eval runs across all common metrics
    pub fn compare_runs(
        baseline_run_id: &str,
        baseline_run_name: &str,
        baseline_results: &HashMap<String, Vec<f64>>,
        treatment_run_id: &str,
        treatment_run_name: &str,
        treatment_results: &HashMap<String, Vec<f64>>,
        metric_direction: &HashMap<String, bool>, // true = higher is better
    ) -> ComparisonResult {
        let mut metrics = Vec::new();
        let mut significant_improvements = 0;
        let mut significant_regressions = 0;
        let mut no_significant_change = 0;

        // Compare all common metrics
        for (metric_name, baseline_values) in baseline_results {
            if let Some(treatment_values) = treatment_results.get(metric_name) {
                let higher_is_better = metric_direction.get(metric_name).copied().unwrap_or(true);

                if let Some(comparison) = Self::compare_metrics(
                    baseline_values,
                    treatment_values,
                    metric_name,
                    baseline_run_id,
                    baseline_run_name,
                    treatment_run_id,
                    treatment_run_name,
                    higher_is_better,
                ) {
                    if comparison.is_significant {
                        if comparison.winner == Some(Winner::Treatment) {
                            significant_improvements += 1;
                        } else if comparison.winner == Some(Winner::Baseline) {
                            significant_regressions += 1;
                        }
                    } else {
                        no_significant_change += 1;
                    }
                    metrics.push(comparison);
                }
            }
        }

        let total_metrics = metrics.len();
        let summary = ComparisonSummary {
            total_metrics,
            significant_improvements,
            significant_regressions,
            no_significant_change,
        };

        // Generate recommendation
        let recommendation = Self::generate_recommendation(&summary, &metrics);

        ComparisonResult {
            baseline_run_id: baseline_run_id.to_string(),
            treatment_run_id: treatment_run_id.to_string(),
            metrics,
            recommendation,
            summary,
        }
    }

    fn generate_recommendation(
        summary: &ComparisonSummary,
        _metrics: &[MetricComparison],
    ) -> Recommendation {
        // Need at least some metrics to make a recommendation
        if summary.total_metrics == 0 {
            return Recommendation {
                action: RecommendedAction::NeedMoreData,
                confidence: 0.0,
                explanation: "No metrics available for comparison.".to_string(),
            };
        }

        // Check for regressions first
        if summary.significant_regressions > 0 && summary.significant_improvements == 0 {
            return Recommendation {
                action: RecommendedAction::KeepBaseline,
                confidence: 0.8,
                explanation: format!(
                    "Treatment shows {} significant regression(s) with no improvements. Keep baseline.",
                    summary.significant_regressions
                ),
            };
        }

        // Pure improvements
        if summary.significant_improvements > 0 && summary.significant_regressions == 0 {
            let confidence = if summary.significant_improvements >= 3 {
                0.95
            } else if summary.significant_improvements >= 2 {
                0.85
            } else {
                0.75
            };

            return Recommendation {
                action: RecommendedAction::DeployTreatment,
                confidence,
                explanation: format!(
                    "Treatment shows {} significant improvement(s) with no regressions. Deploy recommended.",
                    summary.significant_improvements
                ),
            };
        }

        // Mixed results
        if summary.significant_improvements > 0 && summary.significant_regressions > 0 {
            if summary.significant_improvements > summary.significant_regressions {
                return Recommendation {
                    action: RecommendedAction::DeployTreatment,
                    confidence: 0.6,
                    explanation: format!(
                        "Treatment shows {} improvements but also {} regressions. Review trade-offs before deploying.",
                        summary.significant_improvements, summary.significant_regressions
                    ),
                };
            } else {
                return Recommendation {
                    action: RecommendedAction::KeepBaseline,
                    confidence: 0.6,
                    explanation: format!(
                        "Treatment has {} regressions vs {} improvements. Keep baseline unless improvements are critical.",
                        summary.significant_regressions, summary.significant_improvements
                    ),
                };
            }
        }

        // No significant changes
        Recommendation {
            action: RecommendedAction::Inconclusive,
            confidence: 0.5,
            explanation: "No statistically significant differences detected. Consider running with more samples.".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_size_classification() {
        assert_eq!(EffectSize::from_cohens_d(0.1), EffectSize::Negligible);
        assert_eq!(EffectSize::from_cohens_d(0.3), EffectSize::Small);
        assert_eq!(EffectSize::from_cohens_d(0.6), EffectSize::Medium);
        assert_eq!(EffectSize::from_cohens_d(1.0), EffectSize::Large);
    }

    #[test]
    fn test_basic_comparison() {
        let baseline = vec![0.80, 0.82, 0.78, 0.81, 0.79, 0.83, 0.80, 0.77, 0.82, 0.81];
        let treatment = vec![0.85, 0.87, 0.84, 0.86, 0.88, 0.85, 0.89, 0.86, 0.84, 0.87];

        let result = Comparator::compare_metrics(
            &baseline,
            &treatment,
            "accuracy",
            "run-1",
            "Baseline",
            "run-2",
            "Treatment",
            true,
        );

        assert!(result.is_some());
        let comparison = result.unwrap();

        assert!(comparison.difference > 0.0); // Treatment is better
        assert!(comparison.p_value < 0.05); // Statistically significant
        assert_eq!(comparison.winner, Some(Winner::Treatment));
    }

    #[test]
    fn test_no_significant_difference() {
        // Use nearly identical values to ensure no statistical significance
        let baseline = vec![0.80, 0.80, 0.80, 0.80, 0.80];
        let treatment = vec![0.80, 0.80, 0.80, 0.80, 0.80];

        let result = Comparator::compare_metrics(
            &baseline,
            &treatment,
            "accuracy",
            "run-1",
            "Baseline",
            "run-2",
            "Treatment",
            true,
        );

        assert!(result.is_some());
        let comparison = result.unwrap();

        // With identical values, should not be significant
        assert!(!comparison.is_significant || comparison.winner == Some(Winner::Tie));
    }
}
