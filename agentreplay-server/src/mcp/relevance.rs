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

//! Multi-Signal Relevance Scoring
//!
//! Implements the composite relevance scoring function:
//!
//! ```text
//! FinalScore(t) = α·S_sem + β·S_time + γ·S_graph
//! ```
//!
//! Where:
//! - `S_sem` = Cosine similarity from HNSW vector search (0.0 - 1.0)
//! - `S_time` = Temporal decay: e^(-λ(t_now - t_trace)) (0.0 - 1.0)
//! - `S_graph` = PageRank/betweenness centrality score (0.0 - 1.0)
//!
//! Default weights: α=0.5, β=0.3, γ=0.2
//!
//! ## Temporal Decay
//!
//! The temporal score uses exponential decay to prioritize recent traces.
//! The decay rate λ controls how quickly old traces lose relevance.
//!
//! - λ = 0.0001 → Very slow decay (traces stay relevant for months)
//! - λ = 0.001 → Medium decay (traces stay relevant for weeks)
//! - λ = 0.01 → Fast decay (traces stay relevant for days)
//!
//! ## Graph Influence (PageRank)
//!
//! The graph score uses a simplified PageRank algorithm on the causal graph
//! to identify "influential" traces that resolved major issues or were
//! frequently referenced.
//!
//! PR(A) = (1-d)/N + d·Σ(PR(Ti)/C(Ti))
//!
//! Where:
//! - d = damping factor (typically 0.85)
//! - N = total nodes
//! - C(Ti) = out-degree of node Ti

use agentreplay_index::CausalIndex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration for relevance scoring
#[derive(Debug, Clone)]
pub struct RelevanceConfig {
    /// Weight for semantic similarity (α)
    pub semantic_weight: f64,
    /// Weight for temporal recency (β)
    pub temporal_weight: f64,
    /// Weight for graph influence (γ)
    pub graph_weight: f64,
    /// Temporal decay rate (λ) - higher = faster decay
    pub temporal_decay_rate: f64,
    /// PageRank damping factor
    pub pagerank_damping: f64,
    /// PageRank iterations
    pub pagerank_iterations: usize,
}

impl Default for RelevanceConfig {
    fn default() -> Self {
        Self {
            semantic_weight: 0.5,
            temporal_weight: 0.3,
            graph_weight: 0.2,
            // Decay rate: ~1.15e-12 per microsecond ≈ traces lose 50% relevance after ~1 week
            // λ = ln(2) / (7 days in microseconds) = 0.693 / 604_800_000_000
            temporal_decay_rate: 1.15e-12,
            pagerank_damping: 0.85,
            pagerank_iterations: 20,
        }
    }
}

impl RelevanceConfig {
    /// Create a config optimized for recent traces
    pub fn recent_focused() -> Self {
        Self {
            semantic_weight: 0.4,
            temporal_weight: 0.5, // Higher weight on recency
            graph_weight: 0.1,
            temporal_decay_rate: 5.0e-12, // Faster decay (~50% after ~1.5 days)
            ..Default::default()
        }
    }

    /// Create a config optimized for influential traces
    pub fn influence_focused() -> Self {
        Self {
            semantic_weight: 0.4,
            temporal_weight: 0.2,
            graph_weight: 0.4,            // Higher weight on graph influence
            temporal_decay_rate: 5.0e-13, // Slower decay (~50% after ~2 weeks)
            ..Default::default()
        }
    }
}

/// Relevance scorer for traces
pub struct RelevanceScorer {
    config: RelevanceConfig,
    /// Cached PageRank scores (edge_id -> score)
    pagerank_cache: HashMap<u128, f64>,
    /// Whether the cache is valid
    cache_valid: bool,
}

impl RelevanceScorer {
    /// Create a new relevance scorer with default config
    pub fn new() -> Self {
        Self {
            config: RelevanceConfig::default(),
            pagerank_cache: HashMap::new(),
            cache_valid: false,
        }
    }

    /// Create a new relevance scorer with custom config
    pub fn with_config(config: RelevanceConfig) -> Self {
        Self {
            config,
            pagerank_cache: HashMap::new(),
            cache_valid: false,
        }
    }

    /// Compute the composite relevance score for a trace
    ///
    /// # Arguments
    /// - `semantic_score`: Cosine similarity from vector search (0.0 - 1.0)
    /// - `timestamp_us`: Trace timestamp in microseconds
    /// - `edge_id`: Edge ID for graph score lookup
    ///
    /// # Returns
    /// Tuple of (final_score, semantic_score, temporal_score, graph_score)
    pub fn score(
        &self,
        semantic_score: f64,
        timestamp_us: u64,
        edge_id: u128,
    ) -> (f64, f64, f64, f64) {
        let temporal_score = self.compute_temporal_score(timestamp_us);
        let graph_score = self.get_graph_score(edge_id);

        let final_score = self.config.semantic_weight * semantic_score
            + self.config.temporal_weight * temporal_score
            + self.config.graph_weight * graph_score;

        (final_score, semantic_score, temporal_score, graph_score)
    }

    /// Compute temporal decay score
    ///
    /// S_time = e^(-λ·(t_now - t_trace))
    fn compute_temporal_score(&self, timestamp_us: u64) -> f64 {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let age_us = now_us.saturating_sub(timestamp_us) as f64;
        let decay = (-self.config.temporal_decay_rate * age_us).exp();

        // Clamp to [0.0, 1.0]
        decay.clamp(0.0, 1.0)
    }

    /// Get cached graph score or return default
    fn get_graph_score(&self, edge_id: u128) -> f64 {
        self.pagerank_cache.get(&edge_id).copied().unwrap_or(0.5)
    }

    /// Recompute PageRank scores from the causal index
    ///
    /// This should be called periodically (e.g., every hour) to update
    /// graph influence scores.
    pub fn recompute_pagerank(&mut self, causal_index: &CausalIndex) {
        let stats = causal_index.stats();
        if stats.num_nodes == 0 {
            self.cache_valid = true;
            return;
        }

        // PageRank parameters (used in future full implementation)
        let _n = stats.num_nodes as f64;
        let _d = self.config.pagerank_damping;

        // TODO: Implement full PageRank once we expose node iteration on CausalIndex
        // For MVP, we use a simplified scoring based on in-degree via compute_influence_score
        //
        // Future implementation outline:
        // 1. Collect all node IDs by iterating the causal index
        // 2. Initialize ranks with 1/N
        // 3. Iterate: PR(n) = (1-d)/N + d * Σ(PR(m)/C(m)) for m linking to n
        // 4. Repeat until convergence
        self.cache_valid = true;
    }

    /// Compute simplified influence score based on in-degree
    ///
    /// Traces with more children (influenced more downstream traces) get higher scores.
    pub fn compute_influence_score(&self, causal_index: &CausalIndex, edge_id: u128) -> f64 {
        let children = causal_index.get_children(edge_id);
        let descendants = causal_index.get_descendants(edge_id);

        // Score based on direct children + descendants with decay
        let direct_influence = (children.len() as f64).ln_1p() / 10.0;
        let total_influence = (descendants.len() as f64).ln_1p() / 20.0;

        (direct_influence + total_influence).clamp(0.0, 1.0)
    }

    /// Invalidate the PageRank cache (call when graph structure changes)
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
        self.pagerank_cache.clear();
    }
}

impl Default for RelevanceScorer {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch scorer for efficiently scoring multiple traces
pub struct BatchRelevanceScorer {
    scorer: RelevanceScorer,
    causal_index: Arc<CausalIndex>,
}

impl BatchRelevanceScorer {
    pub fn new(causal_index: Arc<CausalIndex>) -> Self {
        Self {
            scorer: RelevanceScorer::new(),
            causal_index,
        }
    }

    pub fn with_config(causal_index: Arc<CausalIndex>, config: RelevanceConfig) -> Self {
        Self {
            scorer: RelevanceScorer::with_config(config),
            causal_index,
        }
    }

    /// Score multiple traces and return sorted by final score (descending)
    ///
    /// # Arguments
    /// - `traces`: Vec of (edge_id, semantic_score, timestamp_us)
    ///
    /// # Returns
    /// Vec of (edge_id, final_score, semantic_score, temporal_score, graph_score)
    /// sorted by final_score descending
    pub fn score_batch(&self, traces: Vec<(u128, f64, u64)>) -> Vec<(u128, f64, f64, f64, f64)> {
        let mut scored: Vec<(u128, f64, f64, f64, f64)> = traces
            .into_iter()
            .map(|(edge_id, semantic_score, timestamp_us)| {
                // Compute graph score dynamically
                let graph_score = self
                    .scorer
                    .compute_influence_score(&self.causal_index, edge_id);

                let temporal_score = self.scorer.compute_temporal_score(timestamp_us);

                let final_score = self.scorer.config.semantic_weight * semantic_score
                    + self.scorer.config.temporal_weight * temporal_score
                    + self.scorer.config.graph_weight * graph_score;

                (
                    edge_id,
                    final_score,
                    semantic_score,
                    temporal_score,
                    graph_score,
                )
            })
            .collect();

        // Sort by final score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_decay() {
        let scorer = RelevanceScorer::new();

        // Recent timestamp should have high score
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let recent_score = scorer.compute_temporal_score(now);
        assert!(recent_score > 0.99, "Recent trace should have score ~1.0");

        // 1 day old should have lower score
        let one_day_ago = now.saturating_sub(86_400_000_000); // 24 hours in microseconds
        let day_old_score = scorer.compute_temporal_score(one_day_ago);
        assert!(
            day_old_score < recent_score,
            "Day-old trace should have lower score"
        );

        // 1 week old should have even lower score
        let one_week_ago = now.saturating_sub(7 * 86_400_000_000);
        let week_old_score = scorer.compute_temporal_score(one_week_ago);
        assert!(
            week_old_score < day_old_score,
            "Week-old trace should have lower score"
        );
    }

    #[test]
    fn test_composite_score() {
        let scorer = RelevanceScorer::new();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // High semantic score, recent timestamp
        let (final_score, sem, temp, graph) = scorer.score(0.9, now, 1);

        assert!(final_score > 0.0);
        assert!((sem - 0.9).abs() < 0.001);
        assert!(temp > 0.99);
        assert!((graph - 0.5).abs() < 0.001); // Default graph score
    }

    #[test]
    fn test_config_weights() {
        let config = RelevanceConfig {
            semantic_weight: 1.0,
            temporal_weight: 0.0,
            graph_weight: 0.0,
            ..Default::default()
        };

        let scorer = RelevanceScorer::with_config(config);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // With only semantic weight, final score should equal semantic score
        let (final_score, sem, _, _) = scorer.score(0.75, now, 1);
        assert!((final_score - sem).abs() < 0.001);
    }
}
