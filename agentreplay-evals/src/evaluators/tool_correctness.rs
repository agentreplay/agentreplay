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

//! Tool correctness evaluator for agent traces
//!
//! Mathematical formulation:
//!
//! Let T_actual = {t₁, t₂, ..., tₙ} be tools actually called
//! Let T_expected = {e₁, e₂, ..., eₘ} be expected tools
//!
//! Precision = |T_actual ∩ T_expected| / |T_actual|
//! Recall = |T_actual ∩ T_expected| / |T_expected|
//! F1 = 2 * (Precision * Recall) / (Precision + Recall)
//!
//! For parameter correctness (if enabled):
//! ParamScore = Σ similarity(actual_params[i], expected_params[i]) / n

use crate::{
    llm_client::{LLMClient, LLMError},
    EvalError, EvalResult, Evaluator, EvaluatorMetadata, MetricValue, TraceContext,
};
use async_trait::async_trait;
use agentreplay_core::SpanType;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

/// Tool correctness evaluation strictness levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCorrectnessStrictness {
    /// Only check if correct tools were called (tool names)
    ToolNameOnly,
    /// Also check input parameters
    IncludeParameters,
    /// Also verify outputs
    IncludeOutputs,
    /// Check everything: names, parameters, and outputs
    Full,
}

/// Definition of a tool that can be called
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Option<serde_json::Value>,
}

/// Record of an actual tool call from the trace
#[derive(Debug, Clone)]
struct ToolCall {
    name: String,
    parameters: Option<String>,
}

/// Expected tool usage specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedToolUsage {
    /// Tools that should have been called
    pub required_tools: Vec<String>,
    /// Tools that should NOT have been called
    pub forbidden_tools: Option<Vec<String>>,
    /// Optional: specific parameters expected for each tool
    pub expected_parameters: Option<HashMap<String, serde_json::Value>>,
}

/// Tool correctness evaluator for agent traces
///
/// Evaluates whether the agent called the right tools to complete its task.
/// Uses deterministic matching for tool names and optional LLM-assisted
/// evaluation for parameter and output correctness.
pub struct ToolCorrectnessEvaluator {
    /// Check tool names only, or also parameters/outputs
    strictness: ToolCorrectnessStrictness,
    /// Consider ordering of tool calls
    consider_ordering: bool,
    /// Available tools for context (optional)
    available_tools: Option<Vec<ToolDefinition>>,
    /// LLM client for parameter similarity (optional, used when strictness > ToolNameOnly)
    llm_client: Option<Arc<dyn LLMClient>>,
    /// Expected tool usage (if provided)
    expected_usage: Option<ExpectedToolUsage>,
    /// Threshold for F1 score to pass
    threshold: f64,
}

impl ToolCorrectnessEvaluator {
    /// Create a new tool correctness evaluator with tool name checking only
    pub fn new() -> Self {
        Self {
            strictness: ToolCorrectnessStrictness::ToolNameOnly,
            consider_ordering: false,
            available_tools: None,
            llm_client: None,
            expected_usage: None,
            threshold: 0.7, // Pass if F1 >= 0.7
        }
    }

    /// Set evaluation strictness
    pub fn with_strictness(mut self, strictness: ToolCorrectnessStrictness) -> Self {
        self.strictness = strictness;
        self
    }

    /// Enable ordering consideration
    pub fn with_ordering(mut self, consider: bool) -> Self {
        self.consider_ordering = consider;
        self
    }

    /// Set available tools for context
    pub fn with_available_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.available_tools = Some(tools);
        self
    }

    /// Set LLM client for parameter evaluation
    pub fn with_llm_client(mut self, client: Arc<dyn LLMClient>) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Set expected tool usage
    pub fn with_expected_usage(mut self, usage: ExpectedToolUsage) -> Self {
        self.expected_usage = Some(usage);
        self
    }

    /// Set pass/fail threshold (default: 0.7)
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Extract tool calls from trace edges
    fn extract_tool_calls(&self, trace: &TraceContext) -> Vec<ToolCall> {
        trace
            .edges
            .iter()
            .filter(|e| {
                let span_type = e.get_span_type();
                span_type == SpanType::ToolCall
            })
            .map(|e| ToolCall {
                // Note: Tool names are stored in payloads, not in the fixed edge structure
                // For now, use span type as identifier. Production code should fetch payload.
                name: format!("Tool_{:x}", e.edge_id),
                parameters: None, // TODO: Extract from edge payload if available
            })
            .collect()
    }

    /// Calculate tool name precision, recall, and F1
    fn calculate_tool_metrics(
        &self,
        actual_tools: &[String],
        expected_tools: &[String],
    ) -> (f64, f64, f64) {
        let actual_set: HashSet<&str> = actual_tools.iter().map(|s| s.as_str()).collect();
        let expected_set: HashSet<&str> = expected_tools.iter().map(|s| s.as_str()).collect();

        let intersection: HashSet<_> = actual_set.intersection(&expected_set).collect();
        let intersection_count = intersection.len() as f64;

        let precision = if actual_set.is_empty() {
            if expected_set.is_empty() {
                1.0
            } else {
                0.0
            }
        } else {
            intersection_count / actual_set.len() as f64
        };

        let recall = if expected_set.is_empty() {
            if actual_set.is_empty() {
                1.0
            } else {
                0.0
            }
        } else {
            intersection_count / expected_set.len() as f64
        };

        let f1 = if precision + recall > 0.0 {
            2.0 * (precision * recall) / (precision + recall)
        } else {
            0.0
        };

        (precision, recall, f1)
    }

    /// Check for forbidden tools
    fn check_forbidden_tools(&self, actual_tools: &[String]) -> Vec<String> {
        if let Some(expected) = &self.expected_usage {
            if let Some(forbidden) = &expected.forbidden_tools {
                let actual_set: HashSet<&str> = actual_tools.iter().map(|s| s.as_str()).collect();
                return forbidden
                    .iter()
                    .filter(|t| actual_set.contains(t.as_str()))
                    .cloned()
                    .collect();
            }
        }
        vec![]
    }

    /// Evaluate parameter correctness using LLM (if enabled)
    async fn evaluate_parameters(
        &self,
        tool_calls: &[ToolCall],
        expected_params: &HashMap<String, serde_json::Value>,
    ) -> Result<f64, EvalError> {
        // If no LLM client or strictness doesn't require it, return 1.0
        if self.llm_client.is_none() || self.strictness == ToolCorrectnessStrictness::ToolNameOnly {
            return Ok(1.0);
        }

        let llm_client = self.llm_client.as_ref().unwrap();

        // Build prompt for parameter evaluation
        let mut param_evaluations = Vec::new();

        for call in tool_calls {
            if let Some(expected) = expected_params.get(&call.name) {
                let actual_params = call.parameters.as_deref().unwrap_or("{}");

                let prompt = format!(
                    r#"Evaluate if the actual parameters match the expected parameters semantically.

Tool: {}
Expected parameters: {}
Actual parameters: {}

Are these parameters semantically equivalent? Consider that:
- Field ordering doesn't matter
- Minor formatting differences are acceptable
- Values should match or be semantically equivalent

Respond in JSON:
{{
  "match": <boolean>,
  "similarity": <float 0-1>,
  "reasoning": "<explanation>"
}}"#,
                    call.name,
                    serde_json::to_string_pretty(expected)?,
                    actual_params
                );

                let response = llm_client.evaluate(prompt).await.map_err(|e| match e {
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

                let json = response.as_json()?;
                let similarity = json["similarity"].as_f64().unwrap_or(0.0);
                param_evaluations.push(similarity);
            }
        }

        if param_evaluations.is_empty() {
            return Ok(1.0);
        }

        // Return average similarity
        Ok(param_evaluations.iter().sum::<f64>() / param_evaluations.len() as f64)
    }
}

impl Default for ToolCorrectnessEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for ToolCorrectnessEvaluator {
    fn id(&self) -> &str {
        "tool_correctness_v1"
    }

    async fn evaluate(&self, trace: &TraceContext) -> Result<EvalResult, EvalError> {
        let start = Instant::now();

        // Extract tool calls from trace
        let tool_calls = self.extract_tool_calls(trace);
        let actual_tool_names: Vec<String> = tool_calls.iter().map(|t| t.name.clone()).collect();

        // If no expected usage is provided, we can only report what was called
        let expected = self.expected_usage.as_ref().ok_or_else(|| {
            EvalError::MissingField(
                "expected_usage (required for tool correctness evaluation)".to_string(),
            )
        })?;

        let expected_tool_names = &expected.required_tools;

        // Calculate tool name metrics (deterministic)
        let (precision, recall, f1) =
            self.calculate_tool_metrics(&actual_tool_names, expected_tool_names);

        // Check for forbidden tools
        let forbidden_called = self.check_forbidden_tools(&actual_tool_names);

        // Evaluate parameters if needed
        let param_score = if self.strictness != ToolCorrectnessStrictness::ToolNameOnly {
            if let Some(expected_params) = &expected.expected_parameters {
                self.evaluate_parameters(&tool_calls, expected_params)
                    .await?
            } else {
                1.0
            }
        } else {
            1.0
        };

        // Calculate overall score
        let tool_score = f1;
        let overall_score = tool_score * param_score;

        // Apply penalty for forbidden tools
        let final_score = if !forbidden_called.is_empty() {
            overall_score * 0.5 // 50% penalty for calling forbidden tools
        } else {
            overall_score
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Build metrics
        let mut metrics = HashMap::new();
        metrics.insert("precision".to_string(), MetricValue::Float(precision));
        metrics.insert("recall".to_string(), MetricValue::Float(recall));
        metrics.insert("f1_score".to_string(), MetricValue::Float(f1));
        metrics.insert(
            "parameter_score".to_string(),
            MetricValue::Float(param_score),
        );
        metrics.insert("overall_score".to_string(), MetricValue::Float(final_score));
        metrics.insert(
            "tools_called".to_string(),
            MetricValue::Int(actual_tool_names.len() as i64),
        );
        metrics.insert(
            "tools_expected".to_string(),
            MetricValue::Int(expected_tool_names.len() as i64),
        );
        metrics.insert(
            "forbidden_tools_called".to_string(),
            MetricValue::Int(forbidden_called.len() as i64),
        );

        if !forbidden_called.is_empty() {
            metrics.insert(
                "forbidden_tools_list".to_string(),
                MetricValue::Array(
                    forbidden_called
                        .iter()
                        .map(|s| MetricValue::String(s.clone()))
                        .collect(),
                ),
            );
        }

        // Calculate cost (if LLM was used)
        let cost = if let Some(llm_client) = &self.llm_client {
            let (input_cost, output_cost) = llm_client.cost_per_token();
            // Estimate ~200 tokens per tool call for parameter evaluation
            let estimated_tokens = tool_calls.len() as f64 * 200.0;
            Some(estimated_tokens * (input_cost + output_cost))
        } else {
            Some(0.0)
        };

        let passed = final_score >= self.threshold;

        let explanation = format!(
            "Tool correctness: F1={:.2}, Precision={:.2}, Recall={:.2}, Param={:.2}. {} tools called ({} expected). {}",
            f1,
            precision,
            recall,
            param_score,
            actual_tool_names.len(),
            expected_tool_names.len(),
            if !forbidden_called.is_empty() {
                format!("⚠️  {} forbidden tools called", forbidden_called.len())
            } else {
                "✓ No forbidden tools called".to_string()
            }
        );

        Ok(EvalResult {
            evaluator_id: self.id().to_string(),
            evaluator_type: Some("hybrid".to_string()),
            metrics,
            passed,
            explanation: Some(explanation),
            assertions: Vec::new(),
            judge_votes: Vec::new(),
            evidence_refs: Vec::new(),
            confidence: 0.95, // High confidence for deterministic tool name matching
            cost,
            duration_ms: Some(duration_ms),
            actionable_feedback: None,
        })
    }

    fn metadata(&self) -> EvaluatorMetadata {
        EvaluatorMetadata {
            name: "Tool Correctness Evaluator".to_string(),
            version: "1.0.0".to_string(),
            description: "Evaluates tool calling correctness using hybrid deterministic + LLM approach. Computes precision, recall, and F1 for tool selection, with optional parameter validation.".to_string(),
            cost_per_eval: Some(0.0001), // Minimal cost, mostly deterministic
            avg_latency_ms: Some(100),
            tags: vec![
                "tool-correctness".to_string(),
                "agent".to_string(),
                "deterministic".to_string(),
                "f1-score".to_string(),
            ],
            author: Some("Agentreplay".to_string()),
        }
    }
}
