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

//! Offline CIP Evaluation for Multi-Turn Conversations
//!
//! Gap 3: Offline CIP for Agentic Applications
//!
//! For agentic applications spanning multiple turns, CIP needs to evaluate
//! coherence across the full trajectory, not just single query-response pairs.
//!
//! ## Multi-Turn CIP Approach
//!
//! 1. **Trajectory-Level Evaluation**: Analyze coherence across conversation turns
//! 2. **Cumulative Context Tracking**: Track how context accumulates
//! 3. **Turn-by-Turn Degradation**: Detect when agents lose context fidelity
//! 4. **Cross-Turn Consistency**: Ensure responses remain coherent
//!
//! ## Key Metrics
//!
//! - **Trajectory Coherence (τ)**: Measures context retention across turns
//! - **Context Degradation (δ)**: Measures how quickly context fidelity drops
//! - **Cross-Turn Consistency (κ)**: Measures contradictions between turns
//! - **Multi-Turn CIP (Ω_mt)**: Composite score for multi-turn evaluation

// Multi-turn CIP evaluation requires complex function signatures
#![allow(clippy::too_many_arguments)]
// .clamp() suggestion is less clear for this use case
#![allow(clippy::manual_clamp)]

use super::{cip_score, cosine_similarity, CIPConfig};
use crate::llm_client::EmbeddingClient;
use crate::{
    ActionableFeedback, EvalError, EvalResult, Evaluator, EvaluatorMetadata, ImprovementSuggestion,
    MetricValue, TraceContext,
};
use async_trait::async_trait;
use agentreplay_core::SpanType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

// ============================================================================
// Trajectory Types for Multi-Turn Analysis
// ============================================================================

/// A single turn in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    /// Turn index (0-based)
    pub turn_index: usize,
    /// User query for this turn
    pub query: String,
    /// Context available at this turn (cumulative or per-turn)
    pub context: Vec<String>,
    /// Agent response
    pub response: String,
    /// Optional ground truth for this turn
    pub ground_truth: Option<String>,
    /// Timestamp (epoch ms)
    pub timestamp_ms: Option<u64>,
}

/// Complete conversation trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTrajectory {
    /// Unique trajectory ID
    pub trajectory_id: String,
    /// All turns in order
    pub turns: Vec<ConversationTurn>,
    /// Initial system context (persistent across turns)
    pub system_context: Option<String>,
    /// Final expected outcome (if known)
    pub expected_outcome: Option<String>,
}

/// Per-turn CIP metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnMetrics {
    /// Turn index
    pub turn_index: usize,
    /// Adherence at this turn
    pub adherence: f64,
    /// Robustness at this turn
    pub robustness: f64,
    /// CIP score at this turn
    pub cip_score: f64,
    /// Similarity with previous turn's response
    pub prev_turn_similarity: Option<f64>,
    /// Context retention from previous turns
    pub context_retention: f64,
}

/// Multi-turn CIP evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTurnCIPResult {
    /// Per-turn metrics
    pub turn_metrics: Vec<TurnMetrics>,
    /// Trajectory coherence score (τ)
    pub trajectory_coherence: f64,
    /// Context degradation rate (δ)
    pub context_degradation: f64,
    /// Cross-turn consistency (κ)
    pub cross_turn_consistency: f64,
    /// Multi-turn CIP score (Ω_mt)
    pub multi_turn_cip_score: f64,
    /// Overall pass/fail
    pub passed: bool,
    /// Explanation
    pub explanation: String,
    /// Turns that failed individual CIP checks
    pub failed_turns: Vec<usize>,
    /// Detected contradictions between turns
    pub contradictions: Vec<Contradiction>,
    /// Total cost
    pub total_cost_usd: f64,
    /// Duration
    pub duration_ms: u64,
}

/// Contradiction between two turns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    /// First turn index
    pub turn_a: usize,
    /// Second turn index
    pub turn_b: usize,
    /// Contradiction score (higher = more contradictory)
    pub score: f64,
    /// Brief description
    pub description: String,
}

// ============================================================================
// Offline CIP Evaluator
// ============================================================================

/// Configuration for Offline CIP evaluation
#[derive(Debug, Clone)]
pub struct OfflineCIPConfig {
    /// Base CIP config
    pub base_config: CIPConfig,
    /// Minimum trajectory coherence
    pub trajectory_coherence_threshold: f64,
    /// Maximum acceptable context degradation rate
    pub max_context_degradation: f64,
    /// Minimum cross-turn consistency
    pub cross_turn_consistency_threshold: f64,
    /// Window size for rolling metrics
    pub rolling_window_size: usize,
    /// Whether to detect contradictions
    pub detect_contradictions: bool,
    /// Contradiction detection threshold (cosine dissimilarity)
    pub contradiction_threshold: f64,
}

impl Default for OfflineCIPConfig {
    fn default() -> Self {
        Self {
            base_config: CIPConfig::default(),
            trajectory_coherence_threshold: 0.7,
            max_context_degradation: 0.2,
            cross_turn_consistency_threshold: 0.8,
            rolling_window_size: 3,
            detect_contradictions: true,
            contradiction_threshold: 0.6,
        }
    }
}

/// Offline CIP Evaluator for multi-turn conversations
///
/// Unlike the live CIP evaluator which requires agent invocation,
/// this evaluator works on recorded conversation trajectories.
///
/// ## Evaluation Process
///
/// 1. Parse trajectory into turns
/// 2. For each turn, compute embeddings of query, context, and response
/// 3. Measure context utilization per turn
/// 4. Track context retention across turns
/// 5. Detect contradictions between turns
/// 6. Compute trajectory-level metrics
pub struct OfflineCIPEvaluator {
    /// Embedding client for similarity computation
    embedding_client: Arc<dyn EmbeddingClient>,
    /// Configuration
    config: OfflineCIPConfig,
}

impl OfflineCIPEvaluator {
    /// Create a new offline CIP evaluator
    pub fn new(embedding_client: Arc<dyn EmbeddingClient>) -> Self {
        Self {
            embedding_client,
            config: OfflineCIPConfig::default(),
        }
    }

    /// Set custom configuration
    pub fn with_config(mut self, config: OfflineCIPConfig) -> Self {
        self.config = config;
        self
    }

    /// Evaluate a complete conversation trajectory
    pub async fn evaluate_trajectory(
        &self,
        trajectory: &ConversationTrajectory,
    ) -> Result<MultiTurnCIPResult, OfflineCIPError> {
        let start = Instant::now();

        if trajectory.turns.is_empty() {
            return Err(OfflineCIPError::InvalidTrajectory(
                "Trajectory has no turns".to_string(),
            ));
        }

        info!(
            "Evaluating trajectory {} with {} turns",
            trajectory.trajectory_id,
            trajectory.turns.len()
        );

        // Step 1: Compute embeddings for all turns
        let turn_embeddings = self.embed_turns(&trajectory.turns).await?;

        // Step 2: Compute per-turn metrics
        let turn_metrics = self.compute_turn_metrics(&trajectory.turns, &turn_embeddings)?;

        // Step 3: Compute trajectory-level metrics
        let trajectory_coherence = self.compute_trajectory_coherence(&turn_embeddings);
        let context_degradation = self.compute_context_degradation(&turn_metrics);
        let cross_turn_consistency = self.compute_cross_turn_consistency(&turn_embeddings);

        // Step 4: Detect contradictions
        let contradictions = if self.config.detect_contradictions {
            self.detect_contradictions(&trajectory.turns, &turn_embeddings)?
        } else {
            vec![]
        };

        // Step 5: Compute multi-turn CIP score
        // Ω_mt = τ * (1 - δ) * κ * avg(Ω_i)
        let avg_turn_cip: f64 =
            turn_metrics.iter().map(|m| m.cip_score).sum::<f64>() / turn_metrics.len() as f64;

        let multi_turn_cip_score = trajectory_coherence
            * (1.0 - context_degradation)
            * cross_turn_consistency
            * avg_turn_cip;

        // Step 6: Identify failed turns
        let failed_turns: Vec<usize> = turn_metrics
            .iter()
            .filter(|m| m.cip_score < self.config.base_config.cip_threshold)
            .map(|m| m.turn_index)
            .collect();

        // Step 7: Determine pass/fail
        let passed = multi_turn_cip_score >= self.config.base_config.cip_threshold
            && trajectory_coherence >= self.config.trajectory_coherence_threshold
            && context_degradation <= self.config.max_context_degradation
            && cross_turn_consistency >= self.config.cross_turn_consistency_threshold
            && contradictions.is_empty();

        let duration_ms = start.elapsed().as_millis() as u64;

        let explanation = self.generate_explanation(
            multi_turn_cip_score,
            trajectory_coherence,
            context_degradation,
            cross_turn_consistency,
            &failed_turns,
            &contradictions,
            passed,
        );

        Ok(MultiTurnCIPResult {
            turn_metrics,
            trajectory_coherence,
            context_degradation,
            cross_turn_consistency,
            multi_turn_cip_score,
            passed,
            explanation,
            failed_turns,
            contradictions,
            total_cost_usd: 0.0, // Offline evaluation has no LLM cost
            duration_ms,
        })
    }

    /// Embed all turns (query, context, response)
    async fn embed_turns(
        &self,
        turns: &[ConversationTurn],
    ) -> Result<Vec<TurnEmbeddings>, OfflineCIPError> {
        let mut all_texts = Vec::new();
        let mut indices = Vec::new(); // (turn_idx, type: 0=query, 1=context, 2=response)

        for (i, turn) in turns.iter().enumerate() {
            all_texts.push(turn.query.clone());
            indices.push((i, 0));

            let context_text = turn.context.join(" ");
            all_texts.push(context_text);
            indices.push((i, 1));

            all_texts.push(turn.response.clone());
            indices.push((i, 2));
        }

        let embeddings = self
            .embedding_client
            .embed_batch(&all_texts)
            .await
            .map_err(|e| OfflineCIPError::EmbeddingError(format!("{:?}", e)))?;

        // Organize embeddings by turn
        let mut turn_embeddings = vec![TurnEmbeddings::default(); turns.len()];
        for (idx, (turn_idx, text_type)) in indices.into_iter().enumerate() {
            match text_type {
                0 => turn_embeddings[turn_idx].query = embeddings[idx].clone(),
                1 => turn_embeddings[turn_idx].context = embeddings[idx].clone(),
                2 => turn_embeddings[turn_idx].response = embeddings[idx].clone(),
                _ => unreachable!(),
            }
        }

        Ok(turn_embeddings)
    }

    /// Compute per-turn CIP metrics
    fn compute_turn_metrics(
        &self,
        _turns: &[ConversationTurn],
        embeddings: &[TurnEmbeddings],
    ) -> Result<Vec<TurnMetrics>, OfflineCIPError> {
        let mut metrics = Vec::with_capacity(embeddings.len());

        for (i, emb) in embeddings.iter().enumerate() {
            // Context-response similarity indicates context utilization
            let context_response_sim = cosine_similarity(&emb.context, &emb.response);

            // Query-response alignment
            let query_response_sim = cosine_similarity(&emb.query, &emb.response);

            // Adherence: Does response reflect context? (simplified offline version)
            // High context-response similarity suggests using context
            let adherence = context_response_sim.max(0.0).min(1.0);

            // Robustness: Is response aligned with query?
            let robustness = query_response_sim.max(0.0).min(1.0);

            // CIP score
            let cip = cip_score(adherence, robustness);

            // Previous turn similarity
            let prev_turn_similarity = if i > 0 {
                Some(cosine_similarity(
                    &embeddings[i - 1].response,
                    &emb.response,
                ))
            } else {
                None
            };

            // Context retention: How much of previous context is retained?
            let context_retention = if i > 0 {
                cosine_similarity(&embeddings[i - 1].context, &emb.context)
            } else {
                1.0 // First turn has full retention by definition
            };

            metrics.push(TurnMetrics {
                turn_index: i,
                adherence,
                robustness,
                cip_score: cip,
                prev_turn_similarity,
                context_retention,
            });
        }

        Ok(metrics)
    }

    /// Compute trajectory coherence (τ)
    /// Average pairwise response similarity across turns
    fn compute_trajectory_coherence(&self, embeddings: &[TurnEmbeddings]) -> f64 {
        if embeddings.len() < 2 {
            return 1.0;
        }

        let mut total_sim = 0.0;
        let mut count = 0;

        // Compare each turn with its successor
        for window in embeddings.windows(2) {
            let sim = cosine_similarity(&window[0].response, &window[1].response);
            // Coherence is positive similarity (topics should be related)
            total_sim += sim.max(0.0);
            count += 1;
        }

        if count == 0 {
            1.0
        } else {
            total_sim / count as f64
        }
    }

    /// Compute context degradation rate (δ)
    /// How quickly context retention drops across turns
    fn compute_context_degradation(&self, turn_metrics: &[TurnMetrics]) -> f64 {
        if turn_metrics.len() < 2 {
            return 0.0;
        }

        // Fit a simple linear regression on context_retention over turns
        let n = turn_metrics.len() as f64;
        let sum_x: f64 = (0..turn_metrics.len()).map(|i| i as f64).sum();
        let sum_y: f64 = turn_metrics.iter().map(|m| m.context_retention).sum();
        let sum_xy: f64 = turn_metrics
            .iter()
            .enumerate()
            .map(|(i, m)| i as f64 * m.context_retention)
            .sum();
        let sum_x2: f64 = (0..turn_metrics.len()).map(|i| (i * i) as f64).sum();

        let denom = n * sum_x2 - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            return 0.0;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denom;

        // Negative slope means degradation; clamp to [0, 1]
        (-slope).max(0.0).min(1.0)
    }

    /// Compute cross-turn consistency (κ)
    /// Measures logical consistency between non-adjacent turns
    fn compute_cross_turn_consistency(&self, embeddings: &[TurnEmbeddings]) -> f64 {
        if embeddings.len() < 3 {
            return 1.0;
        }

        let mut consistent_pairs = 0;
        let mut total_pairs = 0;

        // Check consistency between turns that are 2+ apart
        for i in 0..embeddings.len() {
            for j in (i + 2)..embeddings.len() {
                let sim = cosine_similarity(&embeddings[i].response, &embeddings[j].response);
                total_pairs += 1;

                // Consider consistent if similarity is not negative (no contradiction)
                if sim > -self.config.contradiction_threshold {
                    consistent_pairs += 1;
                }
            }
        }

        if total_pairs == 0 {
            1.0
        } else {
            consistent_pairs as f64 / total_pairs as f64
        }
    }

    /// Detect contradictions between turns
    fn detect_contradictions(
        &self,
        _turns: &[ConversationTurn],
        embeddings: &[TurnEmbeddings],
    ) -> Result<Vec<Contradiction>, OfflineCIPError> {
        let mut contradictions = Vec::new();

        // Compare non-adjacent turn pairs
        for i in 0..embeddings.len() {
            for j in (i + 1)..embeddings.len() {
                let sim = cosine_similarity(&embeddings[i].response, &embeddings[j].response);

                // Negative similarity or very low similarity with high confidence
                // might indicate contradiction
                if sim < -self.config.contradiction_threshold {
                    let description = format!(
                        "Potential contradiction: Turn {} response conflicts with Turn {} (similarity: {:.3})",
                        i, j, sim
                    );

                    contradictions.push(Contradiction {
                        turn_a: i,
                        turn_b: j,
                        score: -sim,
                        description,
                    });
                }
            }
        }

        Ok(contradictions)
    }

    /// Generate human-readable explanation
    fn generate_explanation(
        &self,
        multi_turn_cip: f64,
        trajectory_coherence: f64,
        context_degradation: f64,
        cross_turn_consistency: f64,
        failed_turns: &[usize],
        contradictions: &[Contradiction],
        passed: bool,
    ) -> String {
        let mut parts = vec![format!(
            "Multi-Turn CIP Score: {:.1}% (τ={:.1}%, δ={:.1}%, κ={:.1}%)",
            multi_turn_cip * 100.0,
            trajectory_coherence * 100.0,
            context_degradation * 100.0,
            cross_turn_consistency * 100.0
        )];

        if passed {
            parts.push("✓ Agent maintains context fidelity across the conversation.".to_string());
        } else {
            if trajectory_coherence < self.config.trajectory_coherence_threshold {
                parts.push(
                    "✗ Low trajectory coherence: responses are not thematically connected."
                        .to_string(),
                );
            }
            if context_degradation > self.config.max_context_degradation {
                parts
                    .push("✗ High context degradation: agent loses context over time.".to_string());
            }
            if cross_turn_consistency < self.config.cross_turn_consistency_threshold {
                parts.push(
                    "✗ Low cross-turn consistency: later turns conflict with earlier ones."
                        .to_string(),
                );
            }
            if !failed_turns.is_empty() {
                parts.push(format!(
                    "✗ {} turns failed individual CIP checks: {:?}",
                    failed_turns.len(),
                    failed_turns
                ));
            }
            if !contradictions.is_empty() {
                parts.push(format!(
                    "✗ {} contradictions detected between turns.",
                    contradictions.len()
                ));
            }
        }

        parts.join(" ")
    }

    /// Evaluate from a TraceContext containing multi-turn data
    pub async fn evaluate_from_trace(
        &self,
        trace: &TraceContext,
    ) -> Result<MultiTurnCIPResult, OfflineCIPError> {
        // Extract trajectory from trace edges (spans)
        let trajectory = self.extract_trajectory_from_trace(trace)?;
        self.evaluate_trajectory(&trajectory).await
    }

    /// Extract conversation trajectory from trace
    fn extract_trajectory_from_trace(
        &self,
        trace: &TraceContext,
    ) -> Result<ConversationTrajectory, OfflineCIPError> {
        let mut turns = Vec::new();

        // Filter to LLM/chat/generation spans which represent turns
        for edge in trace.edges.iter() {
            let span_type = edge.get_span_type();

            // Include spans that represent LLM interactions
            if matches!(
                span_type,
                SpanType::Generation | SpanType::Response | SpanType::Synthesis
            ) {
                // For offline analysis, we use trace-level input/output
                // In a real implementation, payloads would be fetched per edge
                let query = trace.input.clone().unwrap_or_default();
                let response = trace.output.clone().unwrap_or_default();
                let context = trace.context.clone().unwrap_or_default();

                turns.push(ConversationTurn {
                    turn_index: turns.len(),
                    query,
                    context,
                    response,
                    ground_truth: None, // Expected output not in TraceContext
                    timestamp_ms: Some(edge.timestamp_us / 1000),
                });
            }
        }

        // If no LLM spans found, treat the entire trace as a single turn
        if turns.is_empty() && trace.input.is_some() {
            turns.push(ConversationTurn {
                turn_index: 0,
                query: trace.input.clone().unwrap_or_default(),
                context: trace.context.clone().unwrap_or_default(),
                response: trace.output.clone().unwrap_or_default(),
                ground_truth: None,
                timestamp_ms: Some(trace.timestamp_us / 1000),
            });
        }

        if turns.is_empty() {
            return Err(OfflineCIPError::InvalidTrajectory(
                "No conversation turns found in trace".to_string(),
            ));
        }

        Ok(ConversationTrajectory {
            trajectory_id: trace.trace_id.to_string(),
            turns,
            system_context: trace.context.as_ref().map(|c| c.join("\n")),
            expected_outcome: None,
        })
    }
}

/// Embeddings for a single turn
#[derive(Debug, Clone, Default)]
struct TurnEmbeddings {
    query: Vec<f64>,
    context: Vec<f64>,
    response: Vec<f64>,
}

/// Errors for offline CIP evaluation
#[derive(Debug, thiserror::Error)]
pub enum OfflineCIPError {
    #[error("Invalid trajectory: {0}")]
    InvalidTrajectory(String),

    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    #[error("Missing data: {0}")]
    MissingData(String),
}

// ============================================================================
// Evaluator Trait Implementation
// ============================================================================

/// Wrapper to integrate Offline CIP with the standard Evaluator trait
pub struct OfflineCIPTraceEvaluator {
    evaluator: OfflineCIPEvaluator,
}

impl OfflineCIPTraceEvaluator {
    /// Create a new offline CIP trace evaluator
    pub fn new(embedding_client: Arc<dyn EmbeddingClient>) -> Self {
        Self {
            evaluator: OfflineCIPEvaluator::new(embedding_client),
        }
    }

    /// Set custom configuration
    pub fn with_config(mut self, config: OfflineCIPConfig) -> Self {
        self.evaluator = self.evaluator.with_config(config);
        self
    }
}

#[async_trait]
impl Evaluator for OfflineCIPTraceEvaluator {
    fn id(&self) -> &str {
        "offline_cip_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let result = self
            .evaluator
            .evaluate_from_trace(trace)
            .await
            .map_err(|e| EvalError::Internal(e.to_string()))?;

        let mut metrics = HashMap::new();
        metrics.insert(
            "multi_turn_cip_score".to_string(),
            MetricValue::Float(result.multi_turn_cip_score),
        );
        metrics.insert(
            "trajectory_coherence".to_string(),
            MetricValue::Float(result.trajectory_coherence),
        );
        metrics.insert(
            "context_degradation".to_string(),
            MetricValue::Float(result.context_degradation),
        );
        metrics.insert(
            "cross_turn_consistency".to_string(),
            MetricValue::Float(result.cross_turn_consistency),
        );
        metrics.insert(
            "num_turns".to_string(),
            MetricValue::Int(result.turn_metrics.len() as i64),
        );
        metrics.insert(
            "num_failed_turns".to_string(),
            MetricValue::Int(result.failed_turns.len() as i64),
        );
        metrics.insert(
            "num_contradictions".to_string(),
            MetricValue::Int(result.contradictions.len() as i64),
        );

        // Include per-turn scores as array
        let turn_scores: Vec<MetricValue> = result
            .turn_metrics
            .iter()
            .map(|m| MetricValue::Float(m.cip_score))
            .collect();
        metrics.insert(
            "turn_cip_scores".to_string(),
            MetricValue::Array(turn_scores),
        );

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("hybrid".to_string()),
            metrics,
            passed: result.passed,
            explanation: Some(result.explanation.clone()),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 0.85, // Slightly lower than live CIP due to offline nature
            cost: Some(result.total_cost_usd),
            duration_ms: Some(result.duration_ms),
            actionable_feedback: if !result.passed {
                Some(self.generate_feedback(&result))
            } else {
                None
            },
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Offline Multi-Turn CIP".to_string(),
            version: "1.0.0".to_string(),
            description: "Evaluates multi-turn conversations for context retention, \
                         trajectory coherence, and cross-turn consistency using \
                         pre-recorded traces."
                .to_string(),
            cost_per_eval: Some(0.001), // Only embedding costs
            avg_latency_ms: Some(500),  // Fast since no LLM calls
            tags: vec![
                "cip".to_string(),
                "offline".to_string(),
                "multi-turn".to_string(),
                "trajectory".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}

impl OfflineCIPTraceEvaluator {
    /// Generate actionable feedback for failed evaluations
    fn generate_feedback(&self, result: &MultiTurnCIPResult) -> ActionableFeedback {
        use crate::FailureCategory;

        let mut feedback = ActionableFeedback::new();

        if result.context_degradation > 0.2 {
            feedback.add_suggestion(
                ImprovementSuggestion::new(
                    FailureCategory::InsufficientContext,
                    "Consider implementing explicit context summarization or memory mechanisms \
                to maintain context fidelity across longer conversations.",
                    0.3,
                )
                .with_priority(2),
            );
        }

        if result.trajectory_coherence < 0.7 {
            feedback.add_suggestion(
                ImprovementSuggestion::new(
                    FailureCategory::Incoherence,
                    "Responses lack thematic continuity. Consider adding conversation history \
                or topic tracking to maintain coherent multi-turn dialogues.",
                    0.4,
                )
                .with_priority(1),
            );
        }

        if !result.contradictions.is_empty() {
            feedback.add_suggestion(
                ImprovementSuggestion::new(
                    FailureCategory::Factualinconsistency,
                    "Detected logical contradictions between turns. Implement consistency \
                checks or use a knowledge graph to track stated facts.",
                    0.5,
                )
                .with_priority(1),
            );
        }

        if !result.failed_turns.is_empty() {
            feedback.add_suggestion(
                ImprovementSuggestion::new(
                    FailureCategory::InsufficientContext,
                    format!(
                        "Turns {:?} showed poor context utilization. Review the prompts and \
                    context injection at these turns.",
                        result.failed_turns
                    ),
                    0.3,
                )
                .with_priority(2),
            );
        }

        feedback.summary = format!(
            "Multi-turn evaluation failed: coherence={:.0}%, degradation={:.0}%, contradictions={}",
            result.trajectory_coherence * 100.0,
            result.context_degradation * 100.0,
            result.contradictions.len()
        );

        feedback.improvement_potential = 0.4; // Moderate improvement potential
        feedback.confidence = 0.8;

        feedback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_cip_config_defaults() {
        let config = OfflineCIPConfig::default();
        assert_eq!(config.trajectory_coherence_threshold, 0.7);
        assert_eq!(config.max_context_degradation, 0.2);
        assert!(config.detect_contradictions);
    }

    #[test]
    fn test_conversation_turn_serialization() {
        let turn = ConversationTurn {
            turn_index: 0,
            query: "What is the capital?".to_string(),
            context: vec!["France is a country in Europe.".to_string()],
            response: "Paris is the capital of France.".to_string(),
            ground_truth: Some("Paris".to_string()),
            timestamp_ms: Some(1234567890),
        };

        let json = serde_json::to_string(&turn).unwrap();
        assert!(json.contains("turn_index"));
        assert!(json.contains("Paris"));
    }

    #[test]
    fn test_trajectory_serialization() {
        let trajectory = ConversationTrajectory {
            trajectory_id: "test-123".to_string(),
            turns: vec![ConversationTurn {
                turn_index: 0,
                query: "Hello".to_string(),
                context: vec![],
                response: "Hi there!".to_string(),
                ground_truth: None,
                timestamp_ms: None,
            }],
            system_context: Some("You are a helpful assistant.".to_string()),
            expected_outcome: None,
        };

        let json = serde_json::to_string(&trajectory).unwrap();
        assert!(json.contains("test-123"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_contradiction_struct() {
        let contradiction = Contradiction {
            turn_a: 0,
            turn_b: 3,
            score: 0.75,
            description: "Conflicting statements about capital.".to_string(),
        };

        assert_eq!(contradiction.turn_a, 0);
        assert_eq!(contradiction.turn_b, 3);
    }
}
