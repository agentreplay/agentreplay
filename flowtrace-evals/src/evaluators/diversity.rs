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

//! Diversity Metrics for Generation Evaluation
//!
//! Generation systems can score well on quality metrics while producing
//! repetitive outputs. This module provides metrics to detect and quantify
//! output diversity.
//!
//! ## Metrics
//!
//! - **Distinct-n**: Ratio of unique n-grams to total n-grams
//! - **Self-BLEU**: Inverse similarity between generated samples
//! - **Entropy**: Information-theoretic diversity measure
//! - **Type-Token Ratio**: Vocabulary richness
//!
//! ## Usage
//!
//! ```rust,ignore
//! use flowtrace_evals::evaluators::diversity::{DiversityAnalyzer, DiversityMetrics};
//!
//! let generations = vec![
//!     "The cat sat on the mat.",
//!     "A dog ran in the park.",
//!     "Birds fly over the lake.",
//! ];
//!
//! let analyzer = DiversityAnalyzer::new();
//! let metrics = analyzer.analyze(&generations);
//!
//! // High diversity if generations are varied
//! assert!(metrics.distinct_2 > 0.5);
//! assert!(metrics.self_bleu < 0.3);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Complete diversity analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiversityMetrics {
    /// Distinct-1: Unique unigrams / total unigrams
    pub distinct_1: f64,
    /// Distinct-2: Unique bigrams / total bigrams
    pub distinct_2: f64,
    /// Distinct-3: Unique trigrams / total trigrams
    pub distinct_3: f64,
    /// Self-BLEU: Average BLEU of each sample against all others (lower = more diverse)
    pub self_bleu: f64,
    /// Token entropy: Shannon entropy of token distribution
    pub token_entropy: f64,
    /// Type-token ratio: Unique tokens / total tokens
    pub type_token_ratio: f64,
    /// Mean TTR: Average TTR per sample
    pub mean_ttr: f64,
    /// Hapax ratio: Words appearing only once / total unique words
    pub hapax_ratio: f64,
    /// Mean sentence length (tokens)
    pub mean_length: f64,
    /// Sentence length std dev
    pub length_std_dev: f64,
    /// Number of samples analyzed
    pub n_samples: usize,
    /// Total tokens analyzed
    pub total_tokens: usize,
}

/// Diversity analyzer configuration
#[derive(Debug, Clone)]
pub struct DiversityAnalyzer {
    /// Maximum n for n-gram analysis
    max_n: usize,
    /// Whether to lowercase for analysis
    lowercase: bool,
    /// Maximum samples for self-BLEU (for performance)
    max_self_bleu_samples: usize,
}

impl Default for DiversityAnalyzer {
    fn default() -> Self {
        Self {
            max_n: 3,
            lowercase: true,
            max_self_bleu_samples: 100,
        }
    }
}

impl DiversityAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_n(mut self, n: usize) -> Self {
        self.max_n = n.max(1);
        self
    }

    pub fn with_lowercase(mut self, lc: bool) -> Self {
        self.lowercase = lc;
        self
    }

    /// Analyze diversity of a set of generated texts
    pub fn analyze(&self, texts: &[&str]) -> DiversityMetrics {
        if texts.is_empty() {
            return DiversityMetrics {
                distinct_1: 0.0,
                distinct_2: 0.0,
                distinct_3: 0.0,
                self_bleu: 0.0,
                token_entropy: 0.0,
                type_token_ratio: 0.0,
                mean_ttr: 0.0,
                hapax_ratio: 0.0,
                mean_length: 0.0,
                length_std_dev: 0.0,
                n_samples: 0,
                total_tokens: 0,
            };
        }

        // Tokenize all texts
        let tokenized: Vec<Vec<String>> = texts.iter().map(|t| self.tokenize(t)).collect();

        // Calculate all metrics
        let distinct_1 = self.distinct_n(&tokenized, 1);
        let distinct_2 = self.distinct_n(&tokenized, 2);
        let distinct_3 = self.distinct_n(&tokenized, 3);

        let self_bleu = self.self_bleu(&tokenized);
        let token_entropy = self.token_entropy(&tokenized);
        let (type_token_ratio, mean_ttr) = self.type_token_ratio(&tokenized);
        let hapax_ratio = self.hapax_ratio(&tokenized);

        // Length statistics
        let lengths: Vec<f64> = tokenized.iter().map(|t| t.len() as f64).collect();
        let mean_length = lengths.iter().sum::<f64>() / lengths.len() as f64;
        let length_std_dev = if lengths.len() > 1 {
            let variance = lengths
                .iter()
                .map(|l| (l - mean_length).powi(2))
                .sum::<f64>()
                / (lengths.len() - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };

        let total_tokens: usize = tokenized.iter().map(|t| t.len()).sum();

        DiversityMetrics {
            distinct_1,
            distinct_2,
            distinct_3,
            self_bleu,
            token_entropy,
            type_token_ratio,
            mean_ttr,
            hapax_ratio,
            mean_length,
            length_std_dev,
            n_samples: texts.len(),
            total_tokens,
        }
    }

    /// Tokenize text
    fn tokenize(&self, text: &str) -> Vec<String> {
        let processed = if self.lowercase {
            text.to_lowercase()
        } else {
            text.to_string()
        };

        processed
            .split_whitespace()
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Get n-grams from tokens
    fn get_ngrams(&self, tokens: &[String], n: usize) -> Vec<String> {
        if tokens.len() < n {
            return Vec::new();
        }

        tokens.windows(n).map(|w| w.join(" ")).collect()
    }

    /// Calculate Distinct-n metric
    fn distinct_n(&self, tokenized: &[Vec<String>], n: usize) -> f64 {
        let mut all_ngrams: Vec<String> = Vec::new();
        let mut unique_ngrams: HashSet<String> = HashSet::new();

        for tokens in tokenized {
            let ngrams = self.get_ngrams(tokens, n);
            for ng in &ngrams {
                unique_ngrams.insert(ng.clone());
            }
            all_ngrams.extend(ngrams);
        }

        if all_ngrams.is_empty() {
            return 0.0;
        }

        unique_ngrams.len() as f64 / all_ngrams.len() as f64
    }

    /// Calculate Self-BLEU (average BLEU of each sample against all others)
    fn self_bleu(&self, tokenized: &[Vec<String>]) -> f64 {
        if tokenized.len() < 2 {
            return 0.0;
        }

        let n_samples = tokenized.len().min(self.max_self_bleu_samples);
        let mut total_bleu = 0.0;
        let mut count = 0;

        for i in 0..n_samples {
            // References: all other samples
            let references: Vec<&Vec<String>> = tokenized
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, t)| t)
                .collect();

            if references.is_empty() {
                continue;
            }

            // Calculate BLEU-4 against all references
            let bleu = self.calculate_bleu(&tokenized[i], &references, 4);
            total_bleu += bleu;
            count += 1;
        }

        if count == 0 {
            return 0.0;
        }

        total_bleu / count as f64
    }

    /// Calculate BLEU score
    fn calculate_bleu(
        &self,
        candidate: &[String],
        references: &[&Vec<String>],
        max_n: usize,
    ) -> f64 {
        if candidate.is_empty() {
            return 0.0;
        }

        // Brevity penalty
        let c = candidate.len();
        let r = references
            .iter()
            .map(|ref_| ref_.len())
            .min_by_key(|&len| (len as i32 - c as i32).abs())
            .unwrap_or(c);

        let bp = if c > r {
            1.0
        } else if c == 0 {
            0.0
        } else {
            (1.0 - r as f64 / c as f64).exp()
        };

        // N-gram precisions
        let mut log_sum = 0.0;
        let weight = 1.0 / max_n as f64;

        for n in 1..=max_n {
            let precision = self.ngram_precision(candidate, references, n);

            // Smoothing for zero counts
            let smoothed = if precision == 0.0 {
                1.0 / (candidate.len() + 1) as f64
            } else {
                precision
            };

            log_sum += weight * smoothed.ln();
        }

        bp * log_sum.exp()
    }

    /// Calculate clipped n-gram precision
    fn ngram_precision(&self, candidate: &[String], references: &[&Vec<String>], n: usize) -> f64 {
        let cand_ngrams = self.get_ngrams(candidate, n);
        if cand_ngrams.is_empty() {
            return 0.0;
        }

        // Count candidate n-grams
        let mut cand_counts: HashMap<&String, usize> = HashMap::new();
        for ng in &cand_ngrams {
            *cand_counts.entry(ng).or_insert(0) += 1;
        }

        // Get max count for each n-gram across all references
        let mut max_ref_counts: HashMap<&String, usize> = HashMap::new();
        for ref_ in references {
            let ref_ngrams = self.get_ngrams(ref_, n);
            let mut ref_counts: HashMap<&String, usize> = HashMap::new();
            for ng in &ref_ngrams {
                *ref_counts.entry(ng).or_insert(0) += 1;
            }

            for (ng, &_count) in &cand_counts {
                let ref_count = ref_counts.get(ng).copied().unwrap_or(0);
                max_ref_counts
                    .entry(*ng)
                    .and_modify(|c| *c = (*c).max(ref_count))
                    .or_insert(ref_count);
            }
        }

        // Clipped counts
        let clipped: usize = cand_counts
            .iter()
            .map(|(ng, &count)| count.min(*max_ref_counts.get(ng).unwrap_or(&0)))
            .sum();

        clipped as f64 / cand_ngrams.len() as f64
    }

    /// Calculate token entropy
    fn token_entropy(&self, tokenized: &[Vec<String>]) -> f64 {
        let mut token_counts: HashMap<&String, usize> = HashMap::new();
        let mut total = 0;

        for tokens in tokenized {
            for token in tokens {
                *token_counts.entry(token).or_insert(0) += 1;
                total += 1;
            }
        }

        if total == 0 {
            return 0.0;
        }

        let mut entropy = 0.0;
        for count in token_counts.values() {
            let p = *count as f64 / total as f64;
            if p > 0.0 {
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    /// Calculate type-token ratio
    fn type_token_ratio(&self, tokenized: &[Vec<String>]) -> (f64, f64) {
        // Global TTR
        let mut all_tokens: Vec<&String> = Vec::new();
        let mut unique_tokens: HashSet<&String> = HashSet::new();

        for tokens in tokenized {
            for token in tokens {
                all_tokens.push(token);
                unique_tokens.insert(token);
            }
        }

        let global_ttr = if all_tokens.is_empty() {
            0.0
        } else {
            unique_tokens.len() as f64 / all_tokens.len() as f64
        };

        // Mean TTR per sample
        let sample_ttrs: Vec<f64> = tokenized
            .iter()
            .map(|tokens| {
                if tokens.is_empty() {
                    0.0
                } else {
                    let unique: HashSet<_> = tokens.iter().collect();
                    unique.len() as f64 / tokens.len() as f64
                }
            })
            .collect();

        let mean_ttr = if sample_ttrs.is_empty() {
            0.0
        } else {
            sample_ttrs.iter().sum::<f64>() / sample_ttrs.len() as f64
        };

        (global_ttr, mean_ttr)
    }

    /// Calculate hapax ratio (words appearing exactly once)
    fn hapax_ratio(&self, tokenized: &[Vec<String>]) -> f64 {
        let mut token_counts: HashMap<&String, usize> = HashMap::new();

        for tokens in tokenized {
            for token in tokens {
                *token_counts.entry(token).or_insert(0) += 1;
            }
        }

        if token_counts.is_empty() {
            return 0.0;
        }

        let hapax_count = token_counts.values().filter(|&&c| c == 1).count();
        hapax_count as f64 / token_counts.len() as f64
    }
}

// ============================================================================
// Zipf Analysis
// ============================================================================

/// Zipf's law analysis for vocabulary distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipfAnalysis {
    /// Zipf exponent (α in f(r) = C/r^α)
    pub exponent: f64,
    /// R-squared of the fit
    pub r_squared: f64,
    /// Top-k tokens and their frequencies
    pub top_tokens: Vec<(String, usize)>,
    /// Interpretation
    pub interpretation: ZipfInterpretation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ZipfInterpretation {
    /// α ≈ 1.0: Natural language distribution
    Natural,
    /// α > 1.0: Overly concentrated (potential mode collapse)
    Concentrated,
    /// α < 1.0: Too uniform (potentially incoherent)
    Uniform,
}

/// Analyze Zipf distribution
pub fn analyze_zipf(texts: &[&str], top_k: usize) -> ZipfAnalysis {
    let mut token_counts: HashMap<String, usize> = HashMap::new();

    for text in texts {
        for word in text.to_lowercase().split_whitespace() {
            let clean = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string();
            if !clean.is_empty() {
                *token_counts.entry(clean).or_insert(0) += 1;
            }
        }
    }

    if token_counts.is_empty() {
        return ZipfAnalysis {
            exponent: 1.0,
            r_squared: 0.0,
            top_tokens: Vec::new(),
            interpretation: ZipfInterpretation::Natural,
        };
    }

    // Sort by frequency
    let mut sorted: Vec<_> = token_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    // Top-k tokens
    let top_tokens: Vec<_> = sorted.iter().take(top_k).cloned().collect();

    // Fit Zipf's law using log-log regression
    // log(freq) = log(C) - α * log(rank)
    let log_ranks: Vec<f64> = (1..=sorted.len()).map(|r| (r as f64).ln()).collect();
    let log_freqs: Vec<f64> = sorted.iter().map(|(_, f)| (*f as f64).ln()).collect();

    let n = log_ranks.len() as f64;
    let sum_x: f64 = log_ranks.iter().sum();
    let sum_y: f64 = log_freqs.iter().sum();
    let sum_xy: f64 = log_ranks.iter().zip(&log_freqs).map(|(x, y)| x * y).sum();
    let sum_x2: f64 = log_ranks.iter().map(|x| x * x).sum();

    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
    let exponent = -slope; // Zipf exponent

    // Calculate R-squared
    let mean_y = sum_y / n;
    let ss_tot: f64 = log_freqs.iter().map(|y| (y - mean_y).powi(2)).sum();
    let intercept = (sum_y - slope * sum_x) / n;
    let ss_res: f64 = log_ranks
        .iter()
        .zip(&log_freqs)
        .map(|(x, y)| {
            let predicted = intercept + slope * x;
            (y - predicted).powi(2)
        })
        .sum();
    let r_squared = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };

    let interpretation = if (exponent - 1.0).abs() < 0.2 {
        ZipfInterpretation::Natural
    } else if exponent > 1.0 {
        ZipfInterpretation::Concentrated
    } else {
        ZipfInterpretation::Uniform
    };

    ZipfAnalysis {
        exponent,
        r_squared,
        top_tokens,
        interpretation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distinct_n() {
        let texts = vec![
            "the cat sat on the mat",
            "the dog ran in the park",
            "the bird flew over the lake",
        ];

        let analyzer = DiversityAnalyzer::new();
        let metrics = analyzer.analyze(&texts);

        // Should have some diversity
        assert!(metrics.distinct_1 > 0.0);
        assert!(metrics.distinct_2 > 0.0);
    }

    #[test]
    fn test_low_diversity() {
        // Repetitive outputs
        let texts = vec![
            "the cat sat on the mat",
            "the cat sat on the mat",
            "the cat sat on the mat",
        ];

        let analyzer = DiversityAnalyzer::new();
        let metrics = analyzer.analyze(&texts);

        // Self-BLEU should be high (low diversity)
        assert!(metrics.self_bleu > 0.9);
    }

    #[test]
    fn test_high_diversity() {
        let texts = vec![
            "quantum mechanics explains subatomic behavior",
            "impressionist paintings capture light beautifully",
            "recursive algorithms solve problems elegantly",
        ];

        let analyzer = DiversityAnalyzer::new();
        let metrics = analyzer.analyze(&texts);

        // Self-BLEU should be low (high diversity)
        assert!(metrics.self_bleu < 0.2);
        // High distinct-n scores
        assert!(metrics.distinct_1 > 0.5);
    }

    #[test]
    fn test_zipf_analysis() {
        let texts = vec![
            "the cat sat on the mat and the cat was happy",
            "the dog ran in the park where the birds sang",
        ];

        let analysis = analyze_zipf(&texts, 5);

        assert!(analysis.exponent > 0.0);
        assert!(!analysis.top_tokens.is_empty());
        assert_eq!(analysis.top_tokens[0].0, "the"); // "the" is most frequent
    }
}
