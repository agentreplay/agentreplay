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

//! G-Eval: Criteria-based evaluation with LLM-as-judge
//!
//! G-Eval uses LLMs to evaluate generation quality based on custom criteria.
//! Unlike binary classification, it provides nuanced scoring with explanations.

use crate::{
    llm_client::{LLMClient, LLMError},
    EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// G-Eval evaluator using LLM-as-judge with custom criteria
///
/// Evaluates responses based on user-defined criteria like:
/// - Coherence: How well-structured and logical is the response?
/// - Consistency: Is the response consistent with the context?
/// - Fluency: Is the language natural and grammatically correct?
/// - Relevance: How well does it address the input?
///
/// Uses probability normalization (from token logprobs) to reduce variance:
/// Score = Σ(i=1 to 5) i * P(i) / Σ(i=1 to 5) P(i)
/// This reduces standard deviation from ~0.3 to ~0.1 compared to raw scores.
pub struct GEval {
    llm_client: Arc<dyn LLMClient>,
    criteria: Vec<EvalCriterion>,
    threshold: f64,
    /// Enable probability normalization using token logprobs (recommended)
    use_probability_normalization: bool,
}

#[derive(Debug, Clone)]
pub struct EvalCriterion {
    pub name: String,
    pub description: String,
    pub scale: (u8, u8), // e.g., (1, 5) for 1-5 scale
    pub weight: f64,     // Relative importance
}

impl GEval {
    /// Create new G-Eval with default criteria (coherence, consistency, fluency, relevance)
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            criteria: vec![
                EvalCriterion {
                    name: "coherence".to_string(),
                    description: "How well-structured and logical is the response?".to_string(),
                    scale: (1, 5),
                    weight: 1.0,
                },
                EvalCriterion {
                    name: "consistency".to_string(),
                    description: "Is the response consistent with the provided context?"
                        .to_string(),
                    scale: (1, 5),
                    weight: 1.0,
                },
                EvalCriterion {
                    name: "fluency".to_string(),
                    description: "Is the language natural and grammatically correct?".to_string(),
                    scale: (1, 5),
                    weight: 0.8,
                },
                EvalCriterion {
                    name: "relevance".to_string(),
                    description: "How well does the response address the input query?".to_string(),
                    scale: (1, 5),
                    weight: 1.2,
                },
            ],
            threshold: 3.5, // Pass if weighted average >= 3.5 / 5.0
            use_probability_normalization: true, // Enabled by default
        }
    }

    /// Create with custom criteria
    pub fn with_criteria(llm_client: Arc<dyn LLMClient>, criteria: Vec<EvalCriterion>) -> Self {
        Self {
            llm_client,
            criteria,
            threshold: 3.5,
            use_probability_normalization: true,
        }
    }

    /// Set pass/fail threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Enable or disable probability normalization (enabled by default)
    ///
    /// Probability normalization uses token logprobs to weight scores,
    /// reducing variance by ~3x compared to raw scores.
    pub fn with_probability_normalization(mut self, enabled: bool) -> Self {
        self.use_probability_normalization = enabled;
        self
    }

    /// Generate evaluation prompt for all criteria
    fn generate_prompt(&self, input: &str, output: &str, context: &str) -> String {
        let mut prompt = format!(
            r#"You are an expert evaluator assessing the quality of an AI-generated response.

INPUT (user's question/request):
{input}

CONTEXT (available information):
{context}

RESPONSE (AI's output):
{output}

Evaluate the response based on the following criteria:

"#,
            input = input,
            context = context,
            output = output
        );

        for criterion in &self.criteria {
            prompt.push_str(&format!(
                "- **{name}** (scale {min}-{max}): {description}\n",
                name = criterion.name,
                min = criterion.scale.0,
                max = criterion.scale.1,
                description = criterion.description
            ));
        }

        prompt.push_str(
            r#"
For each criterion, provide:
1. A score on the specified scale
2. A brief explanation for your score

Respond in JSON format:
{
  "evaluations": [
    {
      "criterion": "<criterion name>",
      "score": <integer score>,
      "reasoning": "<explanation>"
    },
    ...
  ],
  "overall_quality": <float 0-1>,
  "confidence": <float 0-1>
}
"#,
        );

        prompt
    }

    /// Calculate weighted average score
    fn calculate_weighted_score(&self, scores: &HashMap<String, u8>) -> f64 {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for criterion in &self.criteria {
            if let Some(&score) = scores.get(&criterion.name) {
                let normalized = (score as f64 - criterion.scale.0 as f64)
                    / (criterion.scale.1 as f64 - criterion.scale.0 as f64);
                weighted_sum += normalized * criterion.weight;
                total_weight += criterion.weight;
            }
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight) * 5.0 // Scale back to 1-5
        } else {
            0.0
        }
    }

    /// Calculate probability-weighted score using token logprobs
    ///
    /// Formula: S = Σ(i=1 to 5) i * P(i) / Σ(i=1 to 5) P(i)
    ///
    /// Where P(i) = exp(logprob of token "i")
    ///
    /// This reduces variance from ~0.3 to ~0.1 standard deviation
    fn calculate_probability_weighted_score(
        &self,
        _criterion: &EvalCriterion,
        logprobs: &[f64],
    ) -> (f64, f64) {
        // Calculate weighted sum: Σ(score * probability)
        let mut weighted_sum = 0.0;
        let mut total_prob = 0.0;

        for (i, &prob) in logprobs.iter().enumerate() {
            let score = (i + 1) as f64; // Scores are 1-indexed
            weighted_sum += score * prob;
            total_prob += prob;
        }

        let normalized_score = if total_prob > 0.0 {
            weighted_sum / total_prob
        } else {
            0.0
        };

        // Calculate confidence from probability concentration
        // High confidence = probabilities concentrated on few values
        // Low confidence = probabilities spread evenly
        let confidence = if total_prob > 0.0 {
            logprobs
                .iter()
                .map(|p| {
                    let normalized_p = p / total_prob;
                    normalized_p * normalized_p
                })
                .sum::<f64>()
                .sqrt()
        } else {
            0.0
        };

        (normalized_score, confidence)
    }

    /// Generate actionable feedback for failed evaluations
    fn generate_actionable_feedback(
        &self,
        metrics: &HashMap<String, MetricValue>,
        weighted_score: f64,
    ) -> crate::ActionableFeedback {
        use crate::actionable_feedback::{
            ActionableFeedback, FailureCategory, FailureLocation, FailureMode,
            ImprovementSuggestion, Severity,
        };

        let mut feedback = ActionableFeedback::new();

        // Analyze each criterion for failures
        for criterion in &self.criteria {
            let score_key = format!("{}_score", criterion.name);
            let prob_score_key = format!("{}_score_probability_weighted", criterion.name);
            let reasoning_key = format!("{}_reasoning", criterion.name);

            let score = metrics
                .get(&score_key)
                .or_else(|| metrics.get(&prob_score_key))
                .and_then(|v| match v {
                    MetricValue::Int(i) => Some(*i as f64),
                    MetricValue::Float(f) => Some(*f),
                    _ => None,
                })
                .unwrap_or(0.0);

            let reasoning = metrics
                .get(&reasoning_key)
                .and_then(|v| match v {
                    MetricValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            // Normalize score to 0-1 range
            let normalized = (score - criterion.scale.0 as f64)
                / (criterion.scale.1 as f64 - criterion.scale.0 as f64);

            // Determine if this criterion failed
            if normalized < 0.6 {
                let severity = if normalized < 0.3 {
                    Severity::Critical
                } else if normalized < 0.45 {
                    Severity::Major
                } else {
                    Severity::Minor
                };

                let category = self.criterion_to_category(&criterion.name);

                feedback.add_failure(
                    FailureMode::new(
                        category.clone(),
                        severity,
                        reasoning.clone(),
                        FailureLocation::general("evaluation"),
                    )
                    .with_metric(&criterion.name),
                );

                // Add specific suggestion
                let suggestion = ImprovementSuggestion::from_category(category, severity)
                    .with_confidence(0.7 + normalized * 0.2);
                feedback.add_suggestion(suggestion);
            }
        }

        // Add overall improvement suggestion if score is low
        if weighted_score < 3.0 {
            feedback.add_suggestion(
                ImprovementSuggestion::new(
                    FailureCategory::Custom("overall_quality".to_string()),
                    "Consider restructuring the prompt to be more specific and provide examples",
                    0.15,
                )
                .with_priority(5),
            );
        }

        feedback.calculate_improvement_potential();
        feedback.generate_summary();
        feedback
    }

    /// Map criterion name to failure category
    fn criterion_to_category(
        &self,
        criterion_name: &str,
    ) -> crate::actionable_feedback::FailureCategory {
        use crate::actionable_feedback::FailureCategory;

        match criterion_name.to_lowercase().as_str() {
            "coherence" => FailureCategory::Incoherence,
            "consistency" => FailureCategory::Factualinconsistency,
            "relevance" => FailureCategory::Irrelevance,
            "fluency" => FailureCategory::Custom("fluency".to_string()),
            "faithfulness" => FailureCategory::Hallucination,
            "groundedness" => FailureCategory::Hallucination,
            _ => FailureCategory::Custom(criterion_name.to_string()),
        }
    }
}

#[async_trait]
impl Evaluator for GEval {
    fn id(&self) -> &str {
        "g_eval_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // Extract required fields
        let input = trace
            .input
            .as_ref()
            .ok_or_else(|| EvalError::MissingField("input".to_string()))?;

        let output = trace
            .output
            .as_ref()
            .ok_or_else(|| EvalError::MissingField("output".to_string()))?;

        let context = trace
            .context
            .as_ref()
            .map(|c| c.join("\n\n"))
            .unwrap_or_else(|| "No context provided".to_string());

        // Generate prompt
        let prompt = self.generate_prompt(input, output, &context);

        let mut metrics = HashMap::new();
        let weighted_score: f64;
        let confidence: f64;
        let cost: f64;

        // Use probability normalization if enabled
        if self.use_probability_normalization {
            // Call LLM with logprobs
            let llm_response = self
                .llm_client
                .evaluate_with_logprobs(prompt, 5) // Request top 5 logprobs
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

            // Extract evaluations
            let evaluations = json["evaluations"].as_array().ok_or_else(|| {
                EvalError::LLMClientError("Missing evaluations array".to_string())
            })?;

            // Calculate probability-weighted scores
            let mut criterion_scores = Vec::new();
            let mut criterion_confidences = Vec::new();

            for eval in evaluations.iter() {
                let criterion_name = eval["criterion"]
                    .as_str()
                    .ok_or_else(|| EvalError::LLMClientError("Missing criterion name".to_string()))?
                    .to_string();

                // Find matching criterion
                if let Some(criterion) = self.criteria.iter().find(|c| c.name == criterion_name) {
                    // Extract score probabilities from logprobs
                    let score_probs = llm_response.extract_score_probabilities(criterion.scale.1);

                    // Calculate probability-weighted score
                    let (prob_score, prob_confidence) =
                        self.calculate_probability_weighted_score(criterion, &score_probs);

                    criterion_scores.push(prob_score * criterion.weight);
                    criterion_confidences.push(prob_confidence);

                    // Also store raw score for reference
                    let raw_score = eval["score"].as_u64().unwrap_or(0) as i64;
                    let reasoning = eval["reasoning"].as_str().unwrap_or("").to_string();

                    metrics.insert(
                        format!("{}_score_raw", criterion_name),
                        MetricValue::Int(raw_score),
                    );
                    metrics.insert(
                        format!("{}_score_probability_weighted", criterion_name),
                        MetricValue::Float(prob_score),
                    );
                    metrics.insert(
                        format!("{}_confidence", criterion_name),
                        MetricValue::Float(prob_confidence),
                    );
                    metrics.insert(
                        format!("{}_reasoning", criterion_name),
                        MetricValue::String(reasoning),
                    );
                }
            }

            // Calculate overall weighted score
            let total_weight: f64 = self.criteria.iter().map(|c| c.weight).sum();
            weighted_score = if total_weight > 0.0 {
                criterion_scores.iter().sum::<f64>() / total_weight
            } else {
                0.0
            };

            // Average confidence across criteria
            confidence = if !criterion_confidences.is_empty() {
                criterion_confidences.iter().sum::<f64>() / criterion_confidences.len() as f64
            } else {
                0.85
            };

            // Calculate cost
            let (input_cost, output_cost) = self.llm_client.cost_per_token();
            cost = llm_response.usage.calculate_cost(input_cost, output_cost);

            metrics.insert(
                "probability_normalization_enabled".to_string(),
                MetricValue::Bool(true),
            );
        } else {
            // Fall back to standard evaluation (no logprobs)
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

            // Parse response
            let json = llm_response
                .as_json()
                .map_err(|e| EvalError::LLMClientError(format!("Failed to parse JSON: {}", e)))?;

            // Extract evaluations
            let evaluations = json["evaluations"].as_array().ok_or_else(|| {
                EvalError::LLMClientError("Missing evaluations array".to_string())
            })?;

            let mut scores = HashMap::new();

            for eval in evaluations {
                let criterion = eval["criterion"]
                    .as_str()
                    .ok_or_else(|| EvalError::LLMClientError("Missing criterion name".to_string()))?
                    .to_string();

                let score = eval["score"]
                    .as_u64()
                    .ok_or_else(|| EvalError::LLMClientError("Missing score".to_string()))?
                    as u8;

                let reasoning = eval["reasoning"].as_str().unwrap_or("").to_string();

                scores.insert(criterion.clone(), score);
                metrics.insert(
                    format!("{}_score", criterion),
                    MetricValue::Int(score as i64),
                );
                metrics.insert(
                    format!("{}_reasoning", criterion),
                    MetricValue::String(reasoning),
                );
            }

            // Calculate weighted average (standard method)
            weighted_score = self.calculate_weighted_score(&scores);

            confidence = json["confidence"].as_f64().unwrap_or(0.85);

            // Calculate cost
            let (input_cost, output_cost) = self.llm_client.cost_per_token();
            cost = llm_response.usage.calculate_cost(input_cost, output_cost);

            metrics.insert(
                "probability_normalization_enabled".to_string(),
                MetricValue::Bool(false),
            );
        }

        metrics.insert(
            "weighted_score".to_string(),
            MetricValue::Float(weighted_score),
        );
        metrics.insert(
            "overall_quality".to_string(),
            MetricValue::Float(weighted_score / 5.0),
        );

        let duration_ms = start.elapsed().as_millis() as u64;

        let passed = weighted_score >= self.threshold;

        let explanation = format!(
            "G-Eval weighted score: {:.2}/5.0 (threshold: {:.2}). {}",
            weighted_score,
            self.threshold,
            if self.use_probability_normalization {
                "Using probability normalization for reduced variance."
            } else {
                "Using raw scores."
            }
        );

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("llm_judge".to_string()),
            metrics: metrics.clone(),
            passed,
            explanation: Some(explanation),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence,
            cost: Some(cost),
            duration_ms: Some(duration_ms),
            actionable_feedback: if !passed {
                Some(self.generate_actionable_feedback(&metrics, weighted_score))
            } else {
                None
            },
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        let (input_cost, output_cost) = self.llm_client.cost_per_token();
        // Estimate ~800 tokens input, ~400 tokens output
        let estimated_cost = (800.0 * input_cost) + (400.0 * output_cost);

        EvaluatorMetadata {
            name: "G-Eval".to_string(),
            version: "1.0.0".to_string(),
            description: "Criteria-based evaluation using LLM-as-judge. Evaluates coherence, consistency, fluency, and relevance with detailed scoring.".to_string(),
            cost_per_eval: Some(estimated_cost),
            avg_latency_ms: Some(2000),
            tags: vec![
                "g-eval".to_string(),
                "criteria".to_string(),
                "llm-as-judge".to_string(),
                "quality".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}

// ============================================================================
// Auto-CoT (Chain-of-Thought) Generator for G-Eval
// ============================================================================
//
// Implements structured evaluation step generation from the G-Eval paper.
// Instead of simple prompting, generates detailed evaluation rubrics for
// each criterion, improving consistency and correlation with human judgment.

use dashmap::DashMap;

/// Structured scoring rubric for a criterion
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScoringRubric {
    pub score_1: String,
    pub score_2: String,
    pub score_3: String,
    pub score_4: String,
    pub score_5: String,
}

impl Default for ScoringRubric {
    fn default() -> Self {
        Self {
            score_1: "Completely fails to meet the criterion".to_string(),
            score_2: "Poor performance with major issues".to_string(),
            score_3: "Acceptable, meets basic requirements".to_string(),
            score_4: "Good quality, minor improvements possible".to_string(),
            score_5: "Excellent, fully satisfies the criterion".to_string(),
        }
    }
}

/// A single evaluation step in the Auto-CoT process
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvaluationStep {
    pub step_number: u8,
    pub aspect: String,
    pub question: String,
    pub rubric: ScoringRubric,
}

/// Cached evaluation steps for a criterion
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CriterionEvalSteps {
    pub criterion_name: String,
    pub steps: Vec<EvaluationStep>,
}

/// Auto-CoT generator that creates structured evaluation steps
///
/// Generates detailed, rubric-based evaluation steps for each criterion,
/// improving evaluation consistency by ~15% correlation with human judgment.
pub struct AutoCoTGenerator {
    llm_client: Arc<dyn LLMClient>,
    /// Cache for generated steps (criterion_name -> steps)
    cache: DashMap<String, CriterionEvalSteps>,
}

impl AutoCoTGenerator {
    const COT_GENERATION_PROMPT: &'static str = r#"You are an expert evaluation methodology designer.

For the criterion "{criterion_name}" ({criterion_description}), generate a structured evaluation framework with 3-4 evaluation steps.

Each step should:
1. Focus on a specific, observable aspect
2. Include a yes/no verification question  
3. Include detailed scoring rubric (what 1-5 means for this aspect)

Respond in JSON:
{{
  "steps": [
    {{
      "step_number": 1,
      "aspect": "What to examine",
      "question": "Verification question about this aspect",
      "rubric": {{
        "score_1": "Description for terrible (1)",
        "score_2": "Description for poor (2)",
        "score_3": "Description for acceptable (3)",
        "score_4": "Description for good (4)",
        "score_5": "Description for excellent (5)"
      }}
    }}
  ]
}}
"#;

    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            cache: DashMap::new(),
        }
    }

    /// Get cached steps or generate new ones
    pub async fn get_or_generate_steps(
        &self,
        criterion: &EvalCriterion,
    ) -> Result<CriterionEvalSteps, crate::EvalError> {
        // Check cache first
        if let Some(cached) = self.cache.get(&criterion.name) {
            return Ok(cached.value().clone());
        }

        // Generate new steps
        let steps = self.generate_steps(criterion).await?;

        // Cache for future use
        self.cache.insert(criterion.name.clone(), steps.clone());

        Ok(steps)
    }

    /// Generate evaluation steps for a criterion
    pub async fn generate_steps(
        &self,
        criterion: &EvalCriterion,
    ) -> Result<CriterionEvalSteps, crate::EvalError> {
        let prompt = Self::COT_GENERATION_PROMPT
            .replace("{criterion_name}", &criterion.name)
            .replace("{criterion_description}", &criterion.description);

        let response = self.llm_client.evaluate(prompt).await.map_err(|e| {
            crate::EvalError::LLMClientError(format!("CoT generation failed: {:?}", e))
        })?;

        let json = response.as_json().map_err(|e| {
            crate::EvalError::LLMClientError(format!("Failed to parse CoT steps: {}", e))
        })?;

        let steps: Vec<EvaluationStep> = json["steps"]
            .as_array()
            .ok_or_else(|| crate::EvalError::LLMClientError("Missing steps array".to_string()))?
            .iter()
            .filter_map(|s| {
                Some(EvaluationStep {
                    step_number: s["step_number"].as_u64()? as u8,
                    aspect: s["aspect"].as_str()?.to_string(),
                    question: s["question"].as_str()?.to_string(),
                    rubric: ScoringRubric {
                        score_1: s["rubric"]["score_1"]
                            .as_str()
                            .unwrap_or("Poor")
                            .to_string(),
                        score_2: s["rubric"]["score_2"]
                            .as_str()
                            .unwrap_or("Below average")
                            .to_string(),
                        score_3: s["rubric"]["score_3"]
                            .as_str()
                            .unwrap_or("Average")
                            .to_string(),
                        score_4: s["rubric"]["score_4"]
                            .as_str()
                            .unwrap_or("Good")
                            .to_string(),
                        score_5: s["rubric"]["score_5"]
                            .as_str()
                            .unwrap_or("Excellent")
                            .to_string(),
                    },
                })
            })
            .collect();

        Ok(CriterionEvalSteps {
            criterion_name: criterion.name.clone(),
            steps,
        })
    }

    /// Build an enhanced prompt with CoT steps
    pub fn build_enhanced_prompt(
        &self,
        input: &str,
        output: &str,
        context: &str,
        criterion: &EvalCriterion,
        steps: &CriterionEvalSteps,
    ) -> String {
        let steps_text = steps.steps.iter()
            .map(|s| format!(
                "Step {}: {}\n  Question: {}\n  Rubric:\n    1: {}\n    2: {}\n    3: {}\n    4: {}\n    5: {}",
                s.step_number, s.aspect, s.question,
                s.rubric.score_1, s.rubric.score_2, s.rubric.score_3,
                s.rubric.score_4, s.rubric.score_5
            ))
            .collect::<Vec<_>>()
            .join("\n\n");

        format!(
            r#"Evaluate the following response for "{criterion}" ({description}).

INPUT:
{input}

CONTEXT:
{context}

RESPONSE:
{output}

EVALUATION STEPS:
{steps}

For each step, think through your reasoning, then provide a final score.

Respond in JSON:
{{
  "step_evaluations": [
    {{"step": 1, "reasoning": "...", "sub_score": <1-5>}},
    ...
  ],
  "final_score": <1-5>,
  "confidence": <0-1>,
  "overall_reasoning": "..."
}}
"#,
            criterion = criterion.name,
            description = criterion.description,
            input = input,
            context = context,
            output = output,
            steps = steps_text
        )
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}
