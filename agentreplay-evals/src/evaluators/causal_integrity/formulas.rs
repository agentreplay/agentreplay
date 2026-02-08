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

//! CIP (Causal Integrity Protocol) metric formulas
//!
//! Mathematical formulas for computing CIP scores:
//! - Adherence (α): Measures causal sensitivity to critical context changes
//! - Robustness (ρ): Measures stability against null perturbations
//! - CIP Score (Ω): Harmonic mean of α and ρ
//!
//! Reference: van Rijsbergen, C.J. (1979). Information Retrieval.

/// Numerical stability constant
pub const EPSILON: f64 = 1e-10;

/// Default threshold for adherence score (α)
/// Below this, the agent likely generates from parametric memory
pub const DEFAULT_ADHERENCE_THRESHOLD: f64 = 0.5;

/// Default threshold for robustness score (ρ)
/// Below this, the agent is too sensitive to noise
pub const DEFAULT_ROBUSTNESS_THRESHOLD: f64 = 0.8;

/// Default threshold for CIP score (Ω)
/// Derived from harmonic mean of α=0.5, ρ=0.8 → Ω≈0.615
pub const DEFAULT_CIP_THRESHOLD: f64 = 0.6;

/// Compute cosine similarity between two vectors with numerical stability
///
/// # Arguments
/// * `vec_a` - First embedding vector
/// * `vec_b` - Second embedding vector
///
/// # Returns
/// Cosine similarity in range [-1, 1], or 0 if vectors are empty/zero
///
/// # Example
/// ```
/// use agentreplay_evals::evaluators::causal_integrity::formulas::cosine_similarity;
///
/// let a = vec![1.0, 0.0];
/// let b = vec![0.0, 1.0];
/// assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-10);
/// ```
#[inline]
pub fn cosine_similarity(vec_a: &[f64], vec_b: &[f64]) -> f64 {
    if vec_a.len() != vec_b.len() || vec_a.is_empty() {
        return 0.0;
    }

    let dot_product: f64 = vec_a.iter().zip(vec_b.iter()).map(|(a, b)| a * b).sum();
    let norm_a: f64 = vec_a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = vec_b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm_a < EPSILON || norm_b < EPSILON {
        return 0.0;
    }

    (dot_product / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// Adherence Score (α): Measures causal sensitivity to critical changes
///
/// α = 1 - sim(Y_base, Y_crit)
///
/// Properties:
/// - α ∈ [0, 2] theoretically (if sim ∈ [-1, 1])
/// - Clamped to [0, 1] for interpretability
/// - α ≈ 1.0: Agent correctly updated answer (strong causal link)
/// - α ≈ 0.0: Agent ignored context change (hallucination risk)
///
/// # Arguments
/// * `baseline_crit_similarity` - Cosine similarity between baseline and critical outputs
///
/// # Returns
/// Adherence score in range [0, 1]
///
/// # Example
/// ```
/// use agentreplay_evals::evaluators::causal_integrity::formulas::adherence_score;
///
/// // Agent changed response significantly (low similarity = high adherence)
/// assert!((adherence_score(0.2) - 0.8).abs() < 1e-10);
///
/// // Agent didn't change response (high similarity = low adherence)
/// assert!((adherence_score(0.9) - 0.1).abs() < 1e-10);
/// ```
#[inline]
pub fn adherence_score(baseline_crit_similarity: f64) -> f64 {
    (1.0 - baseline_crit_similarity).clamp(0.0, 1.0)
}

/// Robustness Score (ρ): Measures stability against null perturbations
///
/// ρ = sim(Y_base, Y_null)
///
/// Properties:
/// - ρ ∈ [-1, 1] theoretically
/// - Clamped to [0, 1] for interpretability
/// - ρ ≈ 1.0: Agent correctly ignored noise
/// - ρ ≈ 0.0: Agent is brittle/distracted
///
/// # Arguments
/// * `baseline_null_similarity` - Cosine similarity between baseline and null outputs
///
/// # Returns
/// Robustness score in range [0, 1]
///
/// # Example
/// ```
/// use agentreplay_evals::evaluators::causal_integrity::formulas::robustness_score;
///
/// // Agent maintained consistent response (high similarity = high robustness)
/// assert!((robustness_score(0.95) - 0.95).abs() < 1e-10);
///
/// // Agent was distracted by noise (low similarity = low robustness)
/// assert!((robustness_score(0.3) - 0.3).abs() < 1e-10);
/// ```
#[inline]
pub fn robustness_score(baseline_null_similarity: f64) -> f64 {
    baseline_null_similarity.clamp(0.0, 1.0)
}

/// CIP Score (Ω): Harmonic mean of adherence and robustness
///
/// Ω = 2αρ / (α + ρ)
///
/// Properties:
/// - Ω ∈ [0, 1]
/// - Penalizes failure in either dimension
/// - Ω = 0 if either α = 0 or ρ = 0
/// - Conservative: Ω ≤ min(α, ρ) due to harmonic mean property
///
/// The harmonic mean is chosen because:
/// - If α ≈ 0 (agent ignores context changes): Ω ≈ 0
/// - If ρ ≈ 0 (agent is brittle to noise): Ω ≈ 0
/// - Only when **both** are high does Ω approach 1
///
/// This is analogous to F1-score where neither precision nor recall can be sacrificed.
///
/// # Arguments
/// * `alpha` - Adherence score
/// * `rho` - Robustness score
///
/// # Returns
/// CIP score in range [0, 1]
///
/// # Example
/// ```
/// use agentreplay_evals::evaluators::causal_integrity::formulas::cip_score;
///
/// // Both high → high CIP
/// assert!((cip_score(0.9, 0.9) - 0.9).abs() < 1e-10);
///
/// // One low → low CIP (harmonic mean penalizes imbalance)
/// let score = cip_score(0.1, 0.9);
/// assert!(score < 0.2);
///
/// // Both zero → zero CIP
/// assert!((cip_score(0.0, 0.0) - 0.0).abs() < 1e-10);
/// ```
#[inline]
pub fn cip_score(alpha: f64, rho: f64) -> f64 {
    let alpha = alpha.clamp(0.0, 1.0);
    let rho = rho.clamp(0.0, 1.0);

    if alpha + rho < EPSILON {
        return 0.0;
    }

    (2.0 * alpha * rho) / (alpha + rho)
}

/// Compute all CIP metrics from embeddings
///
/// This is the main entry point for CIP evaluation. Given embeddings of the
/// three agent outputs (baseline, critical, null), computes all CIP metrics.
///
/// # Arguments
/// * `baseline_embedding` - Embedding of agent response to original context
/// * `critical_embedding` - Embedding of agent response to critically perturbed context
/// * `null_embedding` - Embedding of agent response to null-perturbed context
///
/// # Returns
/// Tuple of (adherence, robustness, cip_score)
///
/// # Example
/// ```
/// use agentreplay_evals::evaluators::causal_integrity::formulas::compute_cip_metrics;
///
/// let baseline = vec![1.0, 0.0, 0.0];
/// let critical = vec![0.0, 1.0, 0.0];  // Very different (good adherence)
/// let null = vec![0.99, 0.1, 0.0];     // Similar (good robustness)
///
/// let (alpha, rho, omega) = compute_cip_metrics(&baseline, &critical, &null);
/// assert!(alpha > 0.8);  // High adherence (response changed)
/// assert!(rho > 0.9);    // High robustness (response stable)
/// assert!(omega > 0.8);  // High overall CIP score
/// ```
pub fn compute_cip_metrics(
    baseline_embedding: &[f64],
    critical_embedding: &[f64],
    null_embedding: &[f64],
) -> (f64, f64, f64) {
    let sim_base_crit = cosine_similarity(baseline_embedding, critical_embedding);
    let sim_base_null = cosine_similarity(baseline_embedding, null_embedding);

    let alpha = adherence_score(sim_base_crit);
    let rho = robustness_score(sim_base_null);
    let omega = cip_score(alpha, rho);

    (alpha, rho, omega)
}

/// Check if CIP metrics pass the default thresholds
///
/// # Arguments
/// * `alpha` - Adherence score
/// * `rho` - Robustness score
/// * `omega` - CIP score
///
/// # Returns
/// `true` if all metrics meet their respective thresholds
pub fn passes_thresholds(alpha: f64, rho: f64, omega: f64) -> bool {
    alpha >= DEFAULT_ADHERENCE_THRESHOLD
        && rho >= DEFAULT_ROBUSTNESS_THRESHOLD
        && omega >= DEFAULT_CIP_THRESHOLD
}

/// Check if CIP metrics pass custom thresholds
///
/// # Arguments
/// * `alpha` - Adherence score
/// * `rho` - Robustness score
/// * `omega` - CIP score
/// * `alpha_threshold` - Custom adherence threshold
/// * `rho_threshold` - Custom robustness threshold
/// * `omega_threshold` - Custom CIP threshold
///
/// # Returns
/// `true` if all metrics meet their respective thresholds
pub fn passes_custom_thresholds(
    alpha: f64,
    rho: f64,
    omega: f64,
    alpha_threshold: f64,
    rho_threshold: f64,
    omega_threshold: f64,
) -> bool {
    alpha >= alpha_threshold && rho >= rho_threshold && omega >= omega_threshold
}

/// Confidence interval estimation for CIP scores
///
/// Uses bootstrap-style estimation based on similarity variance.
/// Higher variance in similarities → lower confidence.
///
/// # Arguments
/// * `similarities` - Array of similarity measurements
///
/// # Returns
/// Tuple of (mean, std_dev, lower_95, upper_95)
pub fn confidence_interval(similarities: &[f64]) -> (f64, f64, f64, f64) {
    if similarities.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }

    let n = similarities.len() as f64;
    let mean: f64 = similarities.iter().sum::<f64>() / n;

    if n < 2.0 {
        return (mean, 0.0, mean, mean);
    }

    let variance: f64 = similarities.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let std_dev = variance.sqrt();

    // 95% confidence interval (approximately 1.96 standard errors)
    let std_error = std_dev / n.sqrt();
    let margin = 1.96 * std_error;

    let lower = (mean - margin).clamp(0.0, 1.0);
    let upper = (mean + margin).clamp(0.0, 1.0);

    (mean, std_dev, lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < EPSILON);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0];
        let b = vec![0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f64> = vec![];
        let b: Vec<f64> = vec![];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_adherence_score_low_similarity() {
        // Low similarity between base and crit → high adherence
        let alpha = adherence_score(0.1);
        assert!((alpha - 0.9).abs() < EPSILON);
    }

    #[test]
    fn test_adherence_score_high_similarity() {
        // High similarity between base and crit → low adherence (bad)
        let alpha = adherence_score(0.9);
        assert!((alpha - 0.1).abs() < EPSILON);
    }

    #[test]
    fn test_adherence_score_clamping() {
        // Negative similarity should clamp to 1.0
        let alpha = adherence_score(-0.5);
        assert!((alpha - 1.0).abs() < EPSILON);

        // Similarity > 1 should clamp to 0.0
        let alpha = adherence_score(1.5);
        assert!((alpha - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_robustness_score() {
        // High similarity between base and null → high robustness
        let rho = robustness_score(0.95);
        assert!((rho - 0.95).abs() < EPSILON);

        // Low similarity → low robustness
        let rho = robustness_score(0.3);
        assert!((rho - 0.3).abs() < EPSILON);
    }

    #[test]
    fn test_robustness_score_clamping() {
        // Negative similarity should clamp to 0.0
        let rho = robustness_score(-0.5);
        assert!((rho - 0.0).abs() < EPSILON);

        // Similarity > 1 should clamp to 1.0
        let rho = robustness_score(1.5);
        assert!((rho - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_cip_score_balanced() {
        // Equal α and ρ → CIP equals them
        let omega = cip_score(0.8, 0.8);
        assert!((omega - 0.8).abs() < EPSILON);
    }

    #[test]
    fn test_cip_score_imbalanced() {
        // Harmonic mean penalizes imbalance
        let omega = cip_score(0.1, 0.9);
        // 2 * 0.1 * 0.9 / (0.1 + 0.9) = 0.18 / 1.0 = 0.18
        assert!((omega - 0.18).abs() < EPSILON);
    }

    #[test]
    fn test_cip_score_zero() {
        // Either zero → CIP is zero
        assert!((cip_score(0.0, 0.9) - 0.0).abs() < EPSILON);
        assert!((cip_score(0.9, 0.0) - 0.0).abs() < EPSILON);
        assert!((cip_score(0.0, 0.0) - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_cip_score_harmonic_mean_property() {
        // Harmonic mean is always ≤ geometric mean ≤ arithmetic mean
        let alpha = 0.6;
        let rho = 0.9;
        let omega = cip_score(alpha, rho);
        let arithmetic = (alpha + rho) / 2.0;
        let geometric = (alpha * rho).sqrt();

        assert!(omega <= geometric);
        assert!(geometric <= arithmetic);
    }

    #[test]
    fn test_compute_cip_metrics() {
        // Agent that properly uses context (changes on critical, stable on null)
        let baseline = vec![1.0, 0.0, 0.0];
        let critical = vec![0.0, 1.0, 0.0]; // Orthogonal = 0 similarity
        let null = vec![0.99, 0.1, 0.0]; // Very similar

        let (alpha, rho, omega) = compute_cip_metrics(&baseline, &critical, &null);

        // High adherence (low base-crit similarity → 1 - 0 = 1)
        assert!(alpha > 0.9);

        // High robustness (high base-null similarity)
        assert!(rho > 0.9);

        // High CIP score
        assert!(omega > 0.9);
    }

    #[test]
    fn test_passes_thresholds() {
        // All passing
        assert!(passes_thresholds(0.6, 0.85, 0.65));

        // Alpha failing
        assert!(!passes_thresholds(0.4, 0.85, 0.65));

        // Rho failing
        assert!(!passes_thresholds(0.6, 0.7, 0.65));

        // Omega failing
        assert!(!passes_thresholds(0.6, 0.85, 0.5));
    }

    #[test]
    fn test_confidence_interval() {
        let similarities = vec![0.8, 0.82, 0.79, 0.81, 0.83];
        let (mean, std_dev, lower, upper) = confidence_interval(&similarities);

        assert!((mean - 0.81).abs() < 0.01);
        assert!(std_dev > 0.0);
        assert!(lower < mean);
        assert!(upper > mean);
        assert!(lower >= 0.0);
        assert!(upper <= 1.0);
    }

    #[test]
    fn test_confidence_interval_empty() {
        let (mean, std_dev, lower, upper) = confidence_interval(&[]);
        assert_eq!(mean, 0.0);
        assert_eq!(std_dev, 0.0);
        assert_eq!(lower, 0.0);
        assert_eq!(upper, 0.0);
    }

    #[test]
    fn test_confidence_interval_single() {
        let (mean, std_dev, lower, upper) = confidence_interval(&[0.5]);
        assert_eq!(mean, 0.5);
        assert_eq!(std_dev, 0.0);
        assert_eq!(lower, 0.5);
        assert_eq!(upper, 0.5);
    }

    // Test cases from mkl.md equivalence partitioning table
    #[test]
    fn test_hallucinator_agent() {
        // Hallucinator: Ignores all context → similar outputs regardless of perturbation
        // sim(base, crit) ≈ 1.0 → α ≈ 0
        // sim(base, null) ≈ 1.0 → ρ ≈ 1
        let alpha = adherence_score(0.95); // High similarity → low adherence
        let rho = robustness_score(0.95); // High similarity → high robustness
        let omega = cip_score(alpha, rho);

        assert!(alpha < 0.1);
        assert!(rho > 0.9);
        assert!(omega < 0.2); // Low CIP due to low adherence
    }

    #[test]
    fn test_faithful_agent() {
        // Faithful: Uses context correctly
        // sim(base, crit) ≈ 0.0 → α ≈ 1
        // sim(base, null) ≈ 1.0 → ρ ≈ 1
        let alpha = adherence_score(0.1);
        let rho = robustness_score(0.95);
        let omega = cip_score(alpha, rho);

        assert!(alpha > 0.8);
        assert!(rho > 0.9);
        assert!(omega > 0.8);
    }

    #[test]
    fn test_brittle_agent() {
        // Brittle: Sensitive to noise
        // sim(base, crit) ≈ 0.0 → α ≈ 1
        // sim(base, null) ≈ 0.0 → ρ ≈ 0
        let alpha = adherence_score(0.1);
        let rho = robustness_score(0.1);
        let omega = cip_score(alpha, rho);

        assert!(alpha > 0.8);
        assert!(rho < 0.2);
        assert!(omega < 0.3); // Low CIP due to low robustness
    }

    #[test]
    fn test_random_agent() {
        // Random: Unpredictable outputs
        // Expected: α ≈ 0.5, ρ ≈ 0.5, Ω ≈ 0.5
        let alpha = adherence_score(0.5);
        let rho = robustness_score(0.5);
        let omega = cip_score(alpha, rho);

        assert!((alpha - 0.5).abs() < 0.1);
        assert!((rho - 0.5).abs() < 0.1);
        assert!((omega - 0.5).abs() < 0.1);
    }
}
