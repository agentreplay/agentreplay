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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{ActionableFeedback, AssertionResult, JudgeVote};

/// Type-safe metric values for evaluation outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValueV1 {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
    Array(Vec<MetricValueV1>),
    Object(HashMap<String, MetricValueV1>),
    Json(serde_json::Value),
}

/// Versioned evaluation result contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResultV1 {
    /// ID of the evaluator that produced this result
    pub evaluator_id: String,

    /// Optional evaluator type/category (rule-based, llm, hybrid)
    #[serde(default)]
    pub evaluator_type: Option<String>,

    /// Metrics computed by the evaluator
    pub metrics: HashMap<String, MetricValueV1>,

    /// Whether the trace passed evaluation
    pub passed: bool,

    /// Human-readable explanation of the result
    pub explanation: Option<String>,

    /// Detailed assertion results (if applicable)
    #[serde(default)]
    pub assertions: Vec<AssertionResult>,

    /// Multi-judge votes (if applicable)
    #[serde(default)]
    pub judge_votes: Vec<JudgeVote>,

    /// Additional evidence references (trace spans, messages, etc.)
    #[serde(default)]
    pub evidence_refs: Vec<String>,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,

    /// Cost incurred for this evaluation in USD
    pub cost: Option<f64>,

    /// Duration of evaluation in milliseconds
    pub duration_ms: Option<u64>,

    /// Actionable feedback for failed evaluations
    /// Contains failure modes, improvement suggestions, and similar passing traces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actionable_feedback: Option<ActionableFeedback>,
}