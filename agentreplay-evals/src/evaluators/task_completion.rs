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

//! Task completion evaluation using LLM-as-judge
//!
//! This is the PRIMARY metric for agent evaluation. It evaluates whether
//! an AI agent successfully completed a task given the tools and context available.
//!
//! ## Evaluation Modes
//!
//! 1. **Textual (Default)**: LLM-as-judge analyzes trajectory and output
//! 2. **Stateful**: Verifies actual state changes and side effects
//!
//! ## Composite Scoring (Stateful Mode)
//!
//! $$S_{total} = \alpha \cdot S_{textual} + \beta \cdot S_{state} + \gamma \cdot S_{effects}$$
//!
//! Where:
//! - $S_{textual}$: LLM-as-judge score
//! - $S_{state}$: State diff verification score
//! - $S_{effects}$: Side effect verification score

use crate::{
    llm_client::{LLMClient, LLMError},
    EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use agentreplay_core::{AssertionResult, JudgeVote, SpanType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Weights for composite task completion scoring
#[derive(Debug, Clone)]
pub struct EvalWeights {
    /// Weight for textual/LLM-as-judge score (α)
    pub textual: f64,
    /// Weight for state verification score (β)
    pub state: f64,
    /// Weight for side effect verification score (γ)
    pub effects: f64,
}

impl Default for EvalWeights {
    fn default() -> Self {
        Self {
            textual: 0.5,
            state: 0.3,
            effects: 0.2,
        }
    }
}

impl EvalWeights {
    /// Create weights with validation (must sum to 1.0)
    pub fn new(textual: f64, state: f64, effects: f64) -> Self {
        let sum = textual + state + effects;
        if (sum - 1.0).abs() > 0.001 {
            // Normalize to sum to 1.0
            Self {
                textual: textual / sum,
                state: state / sum,
                effects: effects / sum,
            }
        } else {
            Self {
                textual,
                state,
                effects,
            }
        }
    }
}

/// State snapshot for before/after comparison
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateSnapshot {
    /// Timestamp of snapshot
    pub timestamp: Option<DateTime<Utc>>,
    /// Database state (table -> records)
    pub database_state: Option<HashMap<String, serde_json::Value>>,
    /// File system state (path -> metadata)
    pub file_state: Option<HashMap<String, FileMetadata>>,
    /// API/service state
    pub api_state: Option<HashMap<String, serde_json::Value>>,
    /// Custom state entries
    pub custom: Option<HashMap<String, serde_json::Value>>,
}

/// File metadata for state comparison
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileMetadata {
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
}

/// Side effect that occurred during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffect {
    /// Type of side effect
    pub effect_type: SideEffectType,
    /// Target (URL, table name, file path, etc.)
    pub target: String,
    /// Payload/data associated with the effect
    pub payload: serde_json::Value,
    /// When the effect occurred
    pub timestamp: DateTime<Utc>,
    /// Whether the effect was verified
    pub verified: bool,
}

/// Types of side effects
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SideEffectType {
    DatabaseWrite,
    DatabaseRead,
    ApiCall,
    FileWrite,
    FileRead,
    EmailSent,
    WebhookTriggered,
    CacheUpdate,
    Custom(String),
}

/// Expected state changes for verification
#[derive(Debug, Clone, Default)]
pub struct ExpectedStateChange {
    /// Expected database changes
    pub database_changes: Vec<DatabaseChange>,
    /// Expected file changes
    pub file_changes: Vec<FileChange>,
}

/// Expected database change
#[derive(Debug, Clone)]
pub struct DatabaseChange {
    pub table: String,
    pub operation: ChangeOperation,
    pub expected_count: Option<usize>,
}

/// Expected file change
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub operation: ChangeOperation,
}

/// Type of change operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeOperation {
    Create,
    Update,
    Delete,
    NoChange,
}

/// Expected side effect for verification
#[derive(Debug, Clone)]
pub struct ExpectedSideEffect {
    pub effect_type: SideEffectType,
    /// Regex pattern for target matching
    pub target_pattern: String,
    /// Whether this effect is required
    pub required: bool,
}

/// Complete task expectations for stateful verification
#[derive(Debug, Clone, Default)]
pub struct TaskExpectations {
    pub state_changes: ExpectedStateChange,
    pub side_effects: Vec<ExpectedSideEffect>,
}

/// Task completion evaluator using LLM-as-judge
///
/// Unlike component-level metrics, this evaluates the ENTIRE trace trajectory
/// to determine if the agent successfully completed the user's request.
pub struct TaskCompletionEvaluator {
    llm_client: Arc<dyn LLMClient>,
    /// Custom success criteria (optional)
    success_criteria: Option<String>,
    /// Whether to require explicit goal statement
    require_explicit_goal: bool,
    /// Minimum score to consider task completed
    threshold: f64,
}

impl TaskCompletionEvaluator {
    /// Create a new task completion evaluator
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            success_criteria: None,
            require_explicit_goal: false,
            threshold: 0.7, // Task must score >= 0.7 to be considered successful
        }
    }

    /// Set custom success criteria for evaluation
    pub fn with_success_criteria(mut self, criteria: String) -> Self {
        self.success_criteria = Some(criteria);
        self
    }

    /// Set whether to require explicit goal statement
    pub fn with_explicit_goal_required(mut self, required: bool) -> Self {
        self.require_explicit_goal = required;
        self
    }

    /// Set pass/fail threshold (default: 0.7)
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Format the agent's execution trajectory as a human-readable string
    fn format_trajectory(&self, trace: &TraceContext) -> String {
        let mut trajectory = String::new();

        // Filter out internal spans and format visible actions
        let visible_edges: Vec<_> = trace
            .edges
            .iter()
            .filter(|e| {
                // Show all spans except custom/internal ones
                let span_type = e.get_span_type();
                span_type != SpanType::Custom // Simplified check
            })
            .collect();

        if visible_edges.is_empty() {
            return "No visible actions recorded.".to_string();
        }

        for (i, edge) in visible_edges.iter().enumerate() {
            let span_type = edge.get_span_type();
            let duration_ms = edge.duration_us / 1000;

            // Note: AgentFlowEdge doesn't store names in the fixed structure
            // Names are stored separately in the payload. For now, use span type.
            trajectory.push_str(&format!(
                "Step {}: [{:?}] ({}ms)\n",
                i + 1,
                span_type,
                duration_ms
            ));
        }

        trajectory
    }

    /// Generate evaluation prompt
    fn generate_prompt(&self, trace: &TraceContext) -> String {
        let input = trace.input.as_deref().unwrap_or("");
        let output = trace.output.as_deref().unwrap_or("");
        let trajectory = self.format_trajectory(trace);

        let success_criteria_section = if let Some(criteria) = &self.success_criteria {
            format!("\nSUCCESS CRITERIA:\n{}\n", criteria)
        } else {
            String::new()
        };

        format!(
            r#"You are evaluating whether an AI agent successfully completed a task.

USER REQUEST:
{input}

AGENT TRAJECTORY (sequence of actions):
{trajectory}

FINAL OUTPUT:
{output}
{success_criteria_section}
Evaluate task completion by analyzing:
1. Did the agent understand the user's intent correctly?
2. Did the agent take appropriate actions to fulfill the request?
3. Was the final output correct and complete?
4. Were there any unnecessary or incorrect steps?
5. Did the agent achieve the stated goal?

Respond in JSON format:
{{
  "task_understood": <boolean>,
  "actions_appropriate": <boolean>,
  "output_correct": <boolean>,
  "output_complete": <boolean>,
  "goal_achieved": <boolean>,
  "unnecessary_steps": <integer>,
  "completion_score": <float 0-1>,
  "confidence": <float 0-1>,
  "reasoning": "<detailed explanation of your evaluation>",
  "strengths": [<list of things the agent did well>],
  "weaknesses": [<list of issues or improvements needed>]
}}"#,
            input = input,
            trajectory = trajectory,
            output = output,
            success_criteria_section = success_criteria_section
        )
    }
}

#[async_trait]
impl Evaluator for TaskCompletionEvaluator {
    fn id(&self) -> &str {
        "task_completion_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // Validate required fields
        if trace.input.is_none() {
            return Err(EvalError::MissingField("input".to_string()));
        }
        if trace.output.is_none() {
            return Err(EvalError::MissingField("output".to_string()));
        }

        // Generate prompt
        let prompt = self.generate_prompt(trace);

        // Call LLM for evaluation
        let llm_response = self
            .llm_client
            .evaluate(prompt)
            .await
            .map_err(|e| match e {
                LLMError::ApiError(msg) => EvalError::LLMClientError(msg),
                LLMError::RateLimitExceeded => {
                    EvalError::LLMClientError("Rate limit exceeded".to_string())
                }
                LLMError::InvalidResponse(msg) => {
                    EvalError::LLMClientError(format!("Invalid response: {}", msg))
                }
                LLMError::Http(e) => EvalError::LLMClientError(format!("HTTP error: {}", e)),
                LLMError::Json(e) => EvalError::LLMClientError(format!("JSON error: {}", e)),
            })?;

        // Parse JSON response
        let json = llm_response
            .as_json()
            .map_err(|e| EvalError::LLMClientError(format!("Failed to parse JSON: {}", e)))?;

        // Extract metrics
        let task_understood = json["task_understood"].as_bool().unwrap_or(false);
        let actions_appropriate = json["actions_appropriate"].as_bool().unwrap_or(false);
        let output_correct = json["output_correct"].as_bool().unwrap_or(false);
        let output_complete = json["output_complete"].as_bool().unwrap_or(false);
        let goal_achieved = json["goal_achieved"].as_bool().unwrap_or(false);
        let unnecessary_steps = json["unnecessary_steps"].as_i64().unwrap_or(0);
        let completion_score = json["completion_score"].as_f64().ok_or_else(|| {
            EvalError::LLMClientError("Missing completion_score in response".to_string())
        })?;
        let confidence = json["confidence"].as_f64().unwrap_or(0.85);
        let reasoning = json["reasoning"].as_str().unwrap_or("").to_string();

        let strengths = json["strengths"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let weaknesses = json["weaknesses"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Calculate cost
        let (input_cost, output_cost) = self.llm_client.cost_per_token();
        let cost = llm_response.usage.calculate_cost(input_cost, output_cost);

        let duration_ms = start.elapsed().as_millis() as u64;

        // Build metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "completion_score".to_string(),
            MetricValue::Float(completion_score),
        );
        metrics.insert(
            "task_understood".to_string(),
            MetricValue::Bool(task_understood),
        );
        metrics.insert(
            "actions_appropriate".to_string(),
            MetricValue::Bool(actions_appropriate),
        );
        metrics.insert(
            "output_correct".to_string(),
            MetricValue::Bool(output_correct),
        );
        metrics.insert(
            "output_complete".to_string(),
            MetricValue::Bool(output_complete),
        );
        metrics.insert(
            "goal_achieved".to_string(),
            MetricValue::Bool(goal_achieved),
        );
        metrics.insert(
            "unnecessary_steps".to_string(),
            MetricValue::Int(unnecessary_steps),
        );

        if !strengths.is_empty() {
            metrics.insert(
                "strengths".to_string(),
                MetricValue::Array(
                    strengths
                        .iter()
                        .map(|s| MetricValue::String(s.clone()))
                        .collect(),
                ),
            );
        }

        if !weaknesses.is_empty() {
            metrics.insert(
                "weaknesses".to_string(),
                MetricValue::Array(
                    weaknesses
                        .iter()
                        .map(|s| MetricValue::String(s.clone()))
                        .collect(),
                ),
            );
        }

        // Pass if completion score >= threshold
        let passed = completion_score >= self.threshold;

        let explanation = format!(
            "Task completion: {:.1}% (threshold: {:.1}%). {}",
            completion_score * 100.0,
            self.threshold * 100.0,
            reasoning
        );

        let assertions = vec![
            AssertionResult {
                id: "output_complete".to_string(),
                passed: output_complete,
                evidence_refs: Vec::new(),
                message: Some("Output completeness check".to_string()),
            },
            AssertionResult {
                id: "goal_achieved".to_string(),
                passed: goal_achieved,
                evidence_refs: Vec::new(),
                message: Some("Goal achievement check".to_string()),
            },
            AssertionResult {
                id: "unnecessary_steps".to_string(),
                passed: unnecessary_steps <= 1,
                evidence_refs: Vec::new(),
                message: Some("Unnecessary steps threshold".to_string()),
            },
        ];

        let judge_votes = vec![JudgeVote {
            judge_id: self.id().to_string(),
            passed,
            score: Some(completion_score),
            rationale: Some(reasoning.clone()),
        }];

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("llm_judge".to_string()),
            metrics,
            passed,
            explanation: Some(explanation),
            assertions,
            judge_votes,
            evidence_refs: Vec::new(),
            confidence,
            cost: Some(cost),
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        let (input_cost, output_cost) = self.llm_client.cost_per_token();
        // Estimate ~1000 tokens input (trajectory can be long), ~300 tokens output
        let estimated_cost = (1000.0 * input_cost) + (300.0 * output_cost);

        EvaluatorMetadata {
            name: "Task Completion Evaluator".to_string(),
            version: "1.0.0".to_string(),
            description: "PRIMARY agent metric: Evaluates end-to-end task completion success using LLM-as-judge. Analyzes the full trajectory to determine if the agent achieved its goal.".to_string(),
            cost_per_eval: Some(estimated_cost),
            avg_latency_ms: Some(2500),
            tags: vec![
                "task-completion".to_string(),
                "agent".to_string(),
                "llm-as-judge".to_string(),
                "end-to-end".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}

// ============================================================================
// Stateful Task Completion Evaluator
// ============================================================================

/// Stateful task completion evaluator with state and side effect verification
///
/// Extends the basic task completion evaluator with:
/// - State snapshot comparison (before/after)
/// - Side effect verification
/// - Composite scoring (textual + state + effects)
///
/// ## Composite Score Calculation
///
/// $$S_{total} = \alpha \cdot S_{textual} + \beta \cdot S_{state} + \gamma \cdot S_{effects}$$
pub struct StatefulTaskCompletionEvaluator {
    /// Base evaluator for textual analysis
    base_evaluator: TaskCompletionEvaluator,
    /// Weights for composite scoring
    weights: EvalWeights,
    /// Pass/fail threshold
    threshold: f64,
}

impl StatefulTaskCompletionEvaluator {
    /// Create a new stateful evaluator
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            base_evaluator: TaskCompletionEvaluator::new(llm_client),
            weights: EvalWeights::default(),
            threshold: 0.7,
        }
    }

    /// Set composite scoring weights
    pub fn with_weights(mut self, weights: EvalWeights) -> Self {
        self.weights = weights;
        self
    }

    /// Set pass/fail threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self.base_evaluator = self.base_evaluator.with_threshold(threshold);
        self
    }

    /// Evaluate with state snapshots and side effects
    pub async fn evaluate_with_state(
        &self,
        trace: &TraceContext,
        state_before: Option<&StateSnapshot>,
        state_after: Option<&StateSnapshot>,
        side_effects: Option<&[SideEffect]>,
        expectations: &TaskExpectations,
    ) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // 1. Get textual score from base evaluator
        let textual_result = self.base_evaluator.evaluate(trace).await?;
        let textual_score = textual_result
            .metrics
            .get("completion_score")
            .and_then(|v| match v {
                MetricValue::Float(f) => Some(*f),
                _ => None,
            })
            .unwrap_or(0.0);

        // 2. Compute state verification score
        let state_score = if let (Some(before), Some(after)) = (state_before, state_after) {
            self.verify_state_changes(before, after, &expectations.state_changes)
        } else {
            1.0 // No state verification requested
        };

        // 3. Compute side effect verification score
        let effect_score = if let Some(effects) = side_effects {
            self.verify_side_effects(effects, &expectations.side_effects)
        } else {
            1.0 // No effect verification requested
        };

        // 4. Compute composite score
        let total_score = self.weights.textual * textual_score
            + self.weights.state * state_score
            + self.weights.effects * effect_score;

        let passed = total_score >= self.threshold;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Build comprehensive metrics
        let mut metrics = textual_result.metrics.clone();
        metrics.insert("state_score".to_string(), MetricValue::Float(state_score));
        metrics.insert("effect_score".to_string(), MetricValue::Float(effect_score));
        metrics.insert(
            "composite_score".to_string(),
            MetricValue::Float(total_score),
        );
        metrics.insert(
            "weights_textual".to_string(),
            MetricValue::Float(self.weights.textual),
        );
        metrics.insert(
            "weights_state".to_string(),
            MetricValue::Float(self.weights.state),
        );
        metrics.insert(
            "weights_effects".to_string(),
            MetricValue::Float(self.weights.effects),
        );

        let explanation = format!(
            "Task completion: {:.1}% (textual: {:.1}%, state: {:.1}%, effects: {:.1}%)",
            total_score * 100.0,
            textual_score * 100.0,
            state_score * 100.0,
            effect_score * 100.0
        );

        // Add state confidence boost
        let confidence = textual_result.confidence * 0.7 + 0.3; // State adds confidence

        let mut assertions = textual_result.assertions.clone();
        assertions.push(AssertionResult {
            id: "state_verification".to_string(),
            passed: state_score >= self.threshold,
            evidence_refs: Vec::new(),
            message: Some("State verification score".to_string()),
        });
        assertions.push(AssertionResult {
            id: "effects_verification".to_string(),
            passed: effect_score >= self.threshold,
            evidence_refs: Vec::new(),
            message: Some("Side effect verification score".to_string()),
        });

        let judge_votes = vec![JudgeVote {
            judge_id: "task_completion_stateful_v1".to_string(),
            passed,
            score: Some(total_score),
            rationale: Some(explanation.clone()),
        }];

        Ok(EvalResult {
            evaluator_id: "task_completion_stateful_v1".to_string(),
            evaluator_type: Some("hybrid".to_string()),
            metrics,
            passed,
            explanation: Some(explanation),
            assertions,
            judge_votes,
            evidence_refs: Vec::new(),
            confidence,
            cost: textual_result.cost,
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }

    /// Verify state changes between snapshots
    fn verify_state_changes(
        &self,
        before: &StateSnapshot,
        after: &StateSnapshot,
        expected: &ExpectedStateChange,
    ) -> f64 {
        let mut verified_count = 0;
        let total_expected = expected.database_changes.len() + expected.file_changes.len();

        if total_expected == 0 {
            return 1.0; // No expectations, automatically pass
        }

        // Verify database changes
        for db_change in &expected.database_changes {
            let before_data = before
                .database_state
                .as_ref()
                .and_then(|db| db.get(&db_change.table));
            let after_data = after
                .database_state
                .as_ref()
                .and_then(|db| db.get(&db_change.table));

            let verified = match db_change.operation {
                ChangeOperation::Create => before_data.is_none() && after_data.is_some(),
                ChangeOperation::Update => before_data != after_data && after_data.is_some(),
                ChangeOperation::Delete => before_data.is_some() && after_data.is_none(),
                ChangeOperation::NoChange => before_data == after_data,
            };

            if verified {
                verified_count += 1;
            }
        }

        // Verify file changes
        for file_change in &expected.file_changes {
            let before_file = before
                .file_state
                .as_ref()
                .and_then(|fs| fs.get(&file_change.path));
            let after_file = after
                .file_state
                .as_ref()
                .and_then(|fs| fs.get(&file_change.path));

            let verified = match file_change.operation {
                ChangeOperation::Create => {
                    before_file.map(|f| !f.exists).unwrap_or(true)
                        && after_file.map(|f| f.exists).unwrap_or(false)
                }
                ChangeOperation::Update => {
                    before_file.is_some()
                        && after_file.is_some()
                        && before_file.map(|f| &f.content_hash)
                            != after_file.map(|f| &f.content_hash)
                }
                ChangeOperation::Delete => {
                    before_file.map(|f| f.exists).unwrap_or(false)
                        && after_file.map(|f| !f.exists).unwrap_or(true)
                }
                ChangeOperation::NoChange => before_file == after_file,
            };

            if verified {
                verified_count += 1;
            }
        }

        verified_count as f64 / total_expected as f64
    }

    /// Verify side effects occurred as expected
    fn verify_side_effects(&self, actual: &[SideEffect], expected: &[ExpectedSideEffect]) -> f64 {
        if expected.is_empty() {
            return 1.0; // No expectations
        }

        let required_count = expected.iter().filter(|e| e.required).count();
        if required_count == 0 {
            return 1.0; // No required effects
        }

        let mut matched_required = 0;

        for exp in expected.iter().filter(|e| e.required) {
            let pattern = regex::Regex::new(&exp.target_pattern).ok();

            let matched = actual.iter().any(|a| {
                a.effect_type == exp.effect_type
                    && pattern
                        .as_ref()
                        .map(|p| p.is_match(&a.target))
                        .unwrap_or(a.target.contains(&exp.target_pattern))
            });

            if matched {
                matched_required += 1;
            }
        }

        matched_required as f64 / required_count as f64
    }
}
