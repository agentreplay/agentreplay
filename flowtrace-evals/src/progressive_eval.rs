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

//! Progressive Streaming Evaluation Module
//!
//! Implements multi-phase streaming evaluation from tauri.md Task 5:
//! - Phase 1: Heuristic scoring (instant, ~50ms)
//! - Phase 2: Local model scoring (~200ms)
//! - Phase 3: LLM judge scoring (~2000ms)
//!
//! Progressive score refinement:
//! ```text
//! t=0:     w = [1.0, 0, 0]      → S ≈ S_heuristic
//! t=200ms: w = [0.3, 0.7, 0]   → S ≈ 0.3*S_h + 0.7*S_local
//! t=2000ms: w = [0.1, 0.2, 0.7] → S ≈ 0.1*S_h + 0.2*S_local + 0.7*S_llm
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Evaluation phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalPhase {
    /// Fast heuristic scoring (~50ms)
    Heuristic,
    /// Local model scoring (~200ms)
    LocalModel,
    /// Full LLM judge scoring (~2000ms)
    LlmJudge,
}

impl EvalPhase {
    /// Get the weight for this phase's score in final calculation
    pub fn weight(&self) -> f64 {
        match self {
            EvalPhase::Heuristic => 0.1,
            EvalPhase::LocalModel => 0.2,
            EvalPhase::LlmJudge => 0.7,
        }
    }

    /// Get typical latency for this phase in milliseconds
    pub fn typical_latency_ms(&self) -> u64 {
        match self {
            EvalPhase::Heuristic => 50,
            EvalPhase::LocalModel => 200,
            EvalPhase::LlmJudge => 2000,
        }
    }

    /// Get the next phase (if any)
    pub fn next(&self) -> Option<EvalPhase> {
        match self {
            EvalPhase::Heuristic => Some(EvalPhase::LocalModel),
            EvalPhase::LocalModel => Some(EvalPhase::LlmJudge),
            EvalPhase::LlmJudge => None,
        }
    }
}

/// Configuration for progressive evaluation
#[derive(Debug, Clone)]
pub struct ProgressiveEvalConfig {
    /// Enable heuristic phase
    pub enable_heuristic: bool,
    /// Enable local model phase (requires local model)
    pub enable_local_model: bool,
    /// Enable LLM judge phase
    pub enable_llm_judge: bool,
    /// Convergence threshold for early stopping
    pub convergence_threshold: f64,
    /// Minimum confidence to stop early
    pub min_confidence_for_early_stop: f64,
}

impl Default for ProgressiveEvalConfig {
    fn default() -> Self {
        Self {
            enable_heuristic: true,
            enable_local_model: false, // Disabled by default (requires local model setup)
            enable_llm_judge: true,
            convergence_threshold: 0.02,
            min_confidence_for_early_stop: 0.9,
        }
    }
}

/// A single phase result in progressive evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    /// Which phase produced this result
    pub phase: EvalPhase,
    /// Scores for each criterion
    pub scores: HashMap<String, f64>,
    /// Confidence in these scores (0.0 - 1.0)
    pub confidence: f64,
    /// Time taken for this phase in milliseconds
    pub duration_ms: u64,
    /// Whether more refinement is pending
    pub refinement_pending: bool,
}

/// Progressive evaluation update event (for SSE streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressiveEvalUpdate {
    /// Unique evaluation ID
    pub eval_id: String,
    /// Trace being evaluated
    pub trace_id: String,
    /// Current phase
    pub phase: EvalPhase,
    /// Current combined scores (weighted by phase)
    pub current_scores: HashMap<String, f64>,
    /// Overall score
    pub overall_score: f64,
    /// Current confidence
    pub confidence: f64,
    /// Whether evaluation is complete
    pub is_complete: bool,
    /// Whether the trace passed (based on current scores)
    pub passed: bool,
    /// Phases completed so far
    pub phases_completed: Vec<EvalPhase>,
    /// Time elapsed in milliseconds
    pub elapsed_ms: u64,
    /// Score delta from previous update
    pub score_delta: f64,
}

/// Channel for streaming progressive evaluation updates
pub type ProgressiveEvalChannel = mpsc::Sender<ProgressiveEvalUpdate>;

/// Heuristic evaluator for fast initial scoring
pub struct HeuristicEvaluator {
    /// Keywords that indicate quality issues
    quality_keywords: HashMap<String, (f64, bool)>, // (weight, is_positive)
}

impl HeuristicEvaluator {
    pub fn new() -> Self {
        let mut keywords = HashMap::new();

        // Negative quality indicators
        keywords.insert("i don't know".to_string(), (-0.2, false));
        keywords.insert("i'm not sure".to_string(), (-0.15, false));
        keywords.insert("error".to_string(), (-0.1, false));
        keywords.insert("sorry".to_string(), (-0.05, false));
        keywords.insert("unable to".to_string(), (-0.15, false));
        keywords.insert("cannot".to_string(), (-0.1, false));
        keywords.insert("unfortunately".to_string(), (-0.05, false));

        // Positive quality indicators
        keywords.insert("because".to_string(), (0.1, true));
        keywords.insert("therefore".to_string(), (0.1, true));
        keywords.insert("specifically".to_string(), (0.05, true));
        keywords.insert("for example".to_string(), (0.1, true));
        keywords.insert("according to".to_string(), (0.1, true));
        keywords.insert("the reason".to_string(), (0.1, true));
        keywords.insert("in summary".to_string(), (0.05, true));

        Self {
            quality_keywords: keywords,
        }
    }

    /// Evaluate using fast heuristics
    pub fn evaluate(&self, input: &str, output: &str, _context: &str) -> PhaseResult {
        let start = std::time::Instant::now();

        let output_lower = output.to_lowercase();
        let mut scores = HashMap::new();

        // Coherence: structure and length analysis
        let coherence = self.score_coherence(output);
        scores.insert("coherence".to_string(), coherence);

        // Relevance: keyword overlap between input and output
        let relevance = self.score_relevance(input, output);
        scores.insert("relevance".to_string(), relevance);

        // Fluency: grammar/structure heuristics
        let fluency = self.score_fluency(output);
        scores.insert("fluency".to_string(), fluency);

        // Quality: keyword-based quality assessment
        let quality = self.score_quality(&output_lower);
        scores.insert("quality".to_string(), quality);

        let duration_ms = start.elapsed().as_millis() as u64;

        PhaseResult {
            phase: EvalPhase::Heuristic,
            scores,
            confidence: 0.4, // Low confidence for heuristics
            duration_ms,
            refinement_pending: true,
        }
    }

    fn score_coherence(&self, output: &str) -> f64 {
        // Check for proper sentence structure
        let sentences: Vec<&str> = output
            .split(['.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .collect();

        let sentence_count = sentences.len();

        // Penalize very short or very long responses
        let length_score: f64 = if sentence_count == 0 {
            0.2
        } else if sentence_count < 2 {
            0.5
        } else if sentence_count > 20 {
            0.7
        } else {
            0.8
        };

        // Check for proper paragraph structure
        let has_structure = output.contains("\n") || output.len() > 200;
        let structure_bonus: f64 = if has_structure { 0.1 } else { 0.0 };

        (length_score + structure_bonus).min(1.0)
    }

    fn score_relevance(&self, input: &str, output: &str) -> f64 {
        // Simple word overlap
        let input_lower = input.to_lowercase();
        let output_lower = output.to_lowercase();

        let input_words: std::collections::HashSet<_> = input_lower
            .split_whitespace()
            .filter(|w| w.len() > 3) // Ignore short words
            .collect();

        let output_words: std::collections::HashSet<_> = output_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        if input_words.is_empty() || output_words.is_empty() {
            return 0.5;
        }

        let overlap = input_words.intersection(&output_words).count();
        let max_possible = input_words.len().min(output_words.len());

        let overlap_ratio = overlap as f64 / max_possible as f64;

        // Scale to reasonable range
        (0.4 + overlap_ratio * 0.5).min(1.0)
    }

    fn score_fluency(&self, output: &str) -> f64 {
        // Check for basic grammar/structure indicators
        let mut score: f64 = 0.7;

        // Starts with capital letter
        if output
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            score += 0.05;
        }

        // Ends with punctuation
        if output
            .chars()
            .last()
            .map(|c| ".!?".contains(c))
            .unwrap_or(false)
        {
            score += 0.05;
        }

        // Has reasonable word lengths (not all very short or very long words)
        let words: Vec<&str> = output.split_whitespace().collect();
        if !words.is_empty() {
            let avg_word_len: f64 =
                words.iter().map(|w| w.len() as f64).sum::<f64>() / words.len() as f64;
            if avg_word_len > 3.0 && avg_word_len < 12.0 {
                score += 0.1;
            }
        }

        // Check for repeated words (sign of stuttering/errors)
        let word_count = words.len();
        let unique_words: std::collections::HashSet<_> =
            words.iter().map(|w| w.to_lowercase()).collect();
        if word_count > 0 {
            let uniqueness = unique_words.len() as f64 / word_count as f64;
            if uniqueness < 0.5 {
                score -= 0.2; // Too much repetition
            }
        }

        score.clamp(0.0, 1.0)
    }

    fn score_quality(&self, output_lower: &str) -> f64 {
        let mut score = 0.6; // Baseline

        for (keyword, (weight, _is_positive)) in &self.quality_keywords {
            if output_lower.contains(keyword) {
                score += weight;
            }
        }

        score.clamp(0.0, 1.0)
    }
}

impl Default for HeuristicEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Progressive evaluator that runs multiple phases and streams results
pub struct ProgressiveEvaluator {
    config: ProgressiveEvalConfig,
    heuristic_evaluator: HeuristicEvaluator,
}

impl ProgressiveEvaluator {
    pub fn new(config: ProgressiveEvalConfig) -> Self {
        Self {
            config,
            heuristic_evaluator: HeuristicEvaluator::new(),
        }
    }

    /// Get current progressive weights based on completed phases
    pub fn get_weights(phases_completed: &[EvalPhase]) -> (f64, f64, f64) {
        match phases_completed.len() {
            0 => (1.0, 0.0, 0.0), // Before any phase
            1 => (0.3, 0.7, 0.0), // After heuristic
            2 => (0.1, 0.2, 0.7), // After local model
            _ => (0.1, 0.2, 0.7), // After LLM (final)
        }
    }

    /// Combine scores from multiple phases using progressive weights
    pub fn combine_scores(
        heuristic_scores: Option<&HashMap<String, f64>>,
        local_scores: Option<&HashMap<String, f64>>,
        llm_scores: Option<&HashMap<String, f64>>,
    ) -> HashMap<String, f64> {
        let mut combined = HashMap::new();

        // Collect all criterion names
        let mut all_criteria: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Some(scores) = heuristic_scores {
            all_criteria.extend(scores.keys().cloned());
        }
        if let Some(scores) = local_scores {
            all_criteria.extend(scores.keys().cloned());
        }
        if let Some(scores) = llm_scores {
            all_criteria.extend(scores.keys().cloned());
        }

        // Calculate weights based on available phases
        let has_heuristic = heuristic_scores.is_some();
        let has_local = local_scores.is_some();
        let has_llm = llm_scores.is_some();

        let (w_h, w_l, w_llm) = match (has_heuristic, has_local, has_llm) {
            (true, false, false) => (1.0, 0.0, 0.0),
            (true, true, false) => (0.3, 0.7, 0.0),
            (true, false, true) => (0.2, 0.0, 0.8),
            (true, true, true) => (0.1, 0.2, 0.7),
            (false, true, false) => (0.0, 1.0, 0.0),
            (false, true, true) => (0.0, 0.3, 0.7),
            (false, false, true) => (0.0, 0.0, 1.0),
            (false, false, false) => (0.0, 0.0, 0.0),
        };

        for criterion in all_criteria {
            let h_score = heuristic_scores
                .and_then(|s| s.get(&criterion))
                .copied()
                .unwrap_or(0.0);
            let l_score = local_scores
                .and_then(|s| s.get(&criterion))
                .copied()
                .unwrap_or(0.0);
            let llm_score = llm_scores
                .and_then(|s| s.get(&criterion))
                .copied()
                .unwrap_or(0.0);

            let combined_score = h_score * w_h + l_score * w_l + llm_score * w_llm;
            combined.insert(criterion, combined_score);
        }

        combined
    }

    /// Run heuristic evaluation phase
    pub fn run_heuristic(&self, input: &str, output: &str, context: &str) -> PhaseResult {
        self.heuristic_evaluator.evaluate(input, output, context)
    }

    /// Check if we should stop early based on convergence
    pub fn should_stop_early(
        &self,
        previous_score: f64,
        current_score: f64,
        current_confidence: f64,
    ) -> bool {
        let delta = (current_score - previous_score).abs();

        delta < self.config.convergence_threshold
            && current_confidence >= self.config.min_confidence_for_early_stop
    }

    /// Calculate overall score from criterion scores
    pub fn calculate_overall_score(scores: &HashMap<String, f64>) -> f64 {
        if scores.is_empty() {
            return 0.0;
        }
        scores.values().sum::<f64>() / scores.len() as f64
    }
}

impl Default for ProgressiveEvaluator {
    fn default() -> Self {
        Self::new(ProgressiveEvalConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_evaluator() {
        let evaluator = HeuristicEvaluator::new();

        let input = "What is the capital of France?";
        let output = "Paris is the capital of France. It has been the capital since the 10th century and is known for landmarks like the Eiffel Tower.";
        let context = "";

        let result = evaluator.evaluate(input, output, context);

        assert_eq!(result.phase, EvalPhase::Heuristic);
        assert!(result.scores.contains_key("coherence"));
        assert!(result.scores.contains_key("relevance"));
        assert!(result.scores.contains_key("fluency"));
        assert!(result.confidence < 0.5); // Heuristics have low confidence
        assert!(result.refinement_pending);
    }

    #[test]
    fn test_heuristic_detects_low_quality() {
        let evaluator = HeuristicEvaluator::new();

        let input = "Explain quantum computing";
        let output = "I don't know. Sorry, I'm not sure about that.";
        let context = "";

        let result = evaluator.evaluate(input, output, context);

        let quality = result.scores.get("quality").unwrap();
        assert!(*quality < 0.5, "Low quality response should score low");
    }

    #[test]
    fn test_heuristic_detects_high_quality() {
        let evaluator = HeuristicEvaluator::new();

        let input = "Explain photosynthesis";
        let output = "Photosynthesis is the process by which plants convert sunlight into energy. Specifically, it occurs in the chloroplasts. For example, plants absorb carbon dioxide and water, and with the help of sunlight, produce glucose and oxygen. Therefore, photosynthesis is essential for life on Earth.";
        let context = "";

        let result = evaluator.evaluate(input, output, context);

        let quality = result.scores.get("quality").unwrap();
        assert!(*quality > 0.5, "High quality response should score higher");
    }

    #[test]
    fn test_progressive_weights() {
        // After heuristic only
        let (w_h, w_l, w_llm) = ProgressiveEvaluator::get_weights(&[EvalPhase::Heuristic]);
        assert_eq!((w_h, w_l, w_llm), (0.3, 0.7, 0.0));

        // After all phases
        let (w_h, w_l, w_llm) = ProgressiveEvaluator::get_weights(&[
            EvalPhase::Heuristic,
            EvalPhase::LocalModel,
            EvalPhase::LlmJudge,
        ]);
        assert_eq!((w_h, w_l, w_llm), (0.1, 0.2, 0.7));
    }

    #[test]
    fn test_combine_scores() {
        let mut heuristic = HashMap::new();
        heuristic.insert("coherence".to_string(), 0.7);
        heuristic.insert("relevance".to_string(), 0.6);

        let mut llm = HashMap::new();
        llm.insert("coherence".to_string(), 0.9);
        llm.insert("relevance".to_string(), 0.85);

        let combined = ProgressiveEvaluator::combine_scores(Some(&heuristic), None, Some(&llm));

        // With heuristic (0.2) and LLM (0.8)
        let coherence = combined.get("coherence").unwrap();
        let expected = 0.7 * 0.2 + 0.9 * 0.8;
        assert!((coherence - expected).abs() < 0.01);
    }

    #[test]
    fn test_should_stop_early() {
        let config = ProgressiveEvalConfig {
            convergence_threshold: 0.02,
            min_confidence_for_early_stop: 0.9,
            ..Default::default()
        };
        let evaluator = ProgressiveEvaluator::new(config);

        // Score barely changed, high confidence -> stop
        assert!(evaluator.should_stop_early(0.85, 0.86, 0.95));

        // Score changed a lot -> don't stop
        assert!(!evaluator.should_stop_early(0.5, 0.8, 0.95));

        // Low confidence -> don't stop
        assert!(!evaluator.should_stop_early(0.85, 0.86, 0.5));
    }

    #[test]
    fn test_eval_phase_progression() {
        assert_eq!(EvalPhase::Heuristic.next(), Some(EvalPhase::LocalModel));
        assert_eq!(EvalPhase::LocalModel.next(), Some(EvalPhase::LlmJudge));
        assert_eq!(EvalPhase::LlmJudge.next(), None);
    }

    #[test]
    fn test_phase_weights() {
        assert_eq!(EvalPhase::Heuristic.weight(), 0.1);
        assert_eq!(EvalPhase::LocalModel.weight(), 0.2);
        assert_eq!(EvalPhase::LlmJudge.weight(), 0.7);

        // Weights should sum to 1.0
        let total = EvalPhase::Heuristic.weight()
            + EvalPhase::LocalModel.weight()
            + EvalPhase::LlmJudge.weight();
        assert!((total - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_overall_score() {
        let mut scores = HashMap::new();
        scores.insert("coherence".to_string(), 0.8);
        scores.insert("relevance".to_string(), 0.9);
        scores.insert("fluency".to_string(), 0.7);

        let overall = ProgressiveEvaluator::calculate_overall_score(&scores);
        let expected = (0.8 + 0.9 + 0.7) / 3.0;
        assert!((overall - expected).abs() < 0.001);
    }

    #[test]
    fn test_empty_scores() {
        let scores = HashMap::new();
        let overall = ProgressiveEvaluator::calculate_overall_score(&scores);
        assert_eq!(overall, 0.0);
    }
}
