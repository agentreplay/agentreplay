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

//! Actionable Feedback Module for Evaluation Results
//!
//! Implements structured developer feedback from tauri.md Task 3:
//! - Failure mode identification with prioritized issues
//! - Specific improvement suggestions with expected impact
//! - Similar passing traces for reference

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Severity levels for failure modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Major,
    Minor,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Major => "major",
            Severity::Minor => "minor",
            Severity::Info => "info",
        }
    }

    pub fn weight(&self) -> u8 {
        match self {
            Severity::Critical => 4,
            Severity::Major => 3,
            Severity::Minor => 2,
            Severity::Info => 1,
        }
    }
}

/// Categories of failure modes
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    Hallucination,
    IncompleteReasoning,
    Incoherence,
    Irrelevance,
    Factualinconsistency,
    PromptMisunderstanding,
    ToxicContent,
    InsufficientContext,
    OverlyVerbose,
    TooTerse,
    FormatNonCompliance,
    LogicError,
    OffTopic,
    Custom(String),
}

impl FailureCategory {
    pub fn as_str(&self) -> &str {
        match self {
            FailureCategory::Hallucination => "hallucination",
            FailureCategory::IncompleteReasoning => "incomplete_reasoning",
            FailureCategory::Incoherence => "incoherence",
            FailureCategory::Irrelevance => "irrelevance",
            FailureCategory::Factualinconsistency => "factual_inconsistency",
            FailureCategory::PromptMisunderstanding => "prompt_misunderstanding",
            FailureCategory::ToxicContent => "toxic_content",
            FailureCategory::InsufficientContext => "insufficient_context",
            FailureCategory::OverlyVerbose => "overly_verbose",
            FailureCategory::TooTerse => "too_terse",
            FailureCategory::FormatNonCompliance => "format_non_compliance",
            FailureCategory::LogicError => "logic_error",
            FailureCategory::OffTopic => "off_topic",
            FailureCategory::Custom(s) => s,
        }
    }

    pub fn default_action(&self) -> &'static str {
        match self {
            FailureCategory::Hallucination => {
                "Add source verification step with explicit citations"
            }
            FailureCategory::IncompleteReasoning => "Request step-by-step reasoning in prompt",
            FailureCategory::Incoherence => "Add structure requirements to prompt",
            FailureCategory::Irrelevance => "Clarify the specific question/task in prompt",
            FailureCategory::Factualinconsistency => "Cross-reference with retrieved context",
            FailureCategory::PromptMisunderstanding => "Rephrase prompt with clearer instructions",
            FailureCategory::ToxicContent => "Add content safety guardrails",
            FailureCategory::InsufficientContext => "Provide more context or examples",
            FailureCategory::OverlyVerbose => "Request concise responses with word limits",
            FailureCategory::TooTerse => "Request detailed explanations with minimum length",
            FailureCategory::FormatNonCompliance => "Specify exact output format with examples",
            FailureCategory::LogicError => "Add self-verification step",
            FailureCategory::OffTopic => "Constrain the scope explicitly in prompt",
            FailureCategory::Custom(_) => "Review and address the specific issue",
        }
    }

    pub fn typical_impact(&self) -> f64 {
        match self {
            FailureCategory::Hallucination => 0.15,
            FailureCategory::IncompleteReasoning => 0.12,
            FailureCategory::Incoherence => 0.10,
            FailureCategory::Irrelevance => 0.18,
            FailureCategory::Factualinconsistency => 0.14,
            FailureCategory::PromptMisunderstanding => 0.20,
            FailureCategory::ToxicContent => 0.25,
            FailureCategory::InsufficientContext => 0.10,
            FailureCategory::OverlyVerbose => 0.05,
            FailureCategory::TooTerse => 0.05,
            FailureCategory::FormatNonCompliance => 0.08,
            FailureCategory::LogicError => 0.12,
            FailureCategory::OffTopic => 0.15,
            FailureCategory::Custom(_) => 0.10,
        }
    }
}

/// Location information for where a failure occurred
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureLocation {
    pub start_offset: Option<usize>,
    pub end_offset: Option<usize>,
    pub trace_part: String,
    pub span_id: Option<u64>,
    pub line_number: Option<usize>,
}

impl FailureLocation {
    pub fn in_output(start: usize, end: usize) -> Self {
        Self {
            start_offset: Some(start),
            end_offset: Some(end),
            trace_part: "output".to_string(),
            span_id: None,
            line_number: None,
        }
    }

    pub fn in_span(span_id: u64) -> Self {
        Self {
            start_offset: None,
            end_offset: None,
            trace_part: "span".to_string(),
            span_id: Some(span_id),
            line_number: None,
        }
    }

    pub fn general(trace_part: &str) -> Self {
        Self {
            start_offset: None,
            end_offset: None,
            trace_part: trace_part.to_string(),
            span_id: None,
            line_number: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureMode {
    pub category: FailureCategory,
    pub severity: Severity,
    pub evidence: String,
    pub location: Option<FailureLocation>,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
}

impl FailureMode {
    pub fn new(
        category: FailureCategory,
        severity: Severity,
        evidence: impl Into<String>,
        location: FailureLocation,
    ) -> Self {
        Self {
            category,
            severity,
            evidence: evidence.into(),
            location: Some(location),
            metric: None,
            details: None,
        }
    }

    pub fn with_metric(mut self, metric: impl Into<String>) -> Self {
        self.metric = Some(metric.into());
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementSuggestion {
    pub category: FailureCategory,
    pub action: String,
    pub expected_impact: f64,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub confidence: Option<f64>,
}

impl ImprovementSuggestion {
    pub fn new(
        category: FailureCategory,
        action: impl Into<String>,
        expected_impact: f64,
    ) -> Self {
        Self {
            category,
            action: action.into(),
            expected_impact,
            priority: 1,
            confidence: None,
        }
    }

    pub fn from_category(category: FailureCategory, severity: Severity) -> Self {
        let base = category.typical_impact();
        let boost = (severity.weight() as f64 - 1.0) * 0.03;
        let expected_impact = (base + boost).min(1.0);
        Self {
            action: category.default_action().to_string(),
            category,
            expected_impact,
            priority: severity.weight(),
            confidence: None,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarPassingTrace {
    pub trace_id: String,
    pub similarity: f64,
    pub key_differences: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionableFeedback {
    pub failure_modes: Vec<FailureMode>,
    pub improvement_suggestions: Vec<ImprovementSuggestion>,
    pub similar_passing_traces: Vec<SimilarPassingTrace>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub improvement_potential: f64,
    #[serde(default)]
    pub confidence: f64,
}

impl ActionableFeedback {
    pub fn new() -> Self {
        Self {
            failure_modes: Vec::new(),
            improvement_suggestions: Vec::new(),
            similar_passing_traces: Vec::new(),
            summary: String::new(),
            improvement_potential: 0.0,
            confidence: 0.0,
        }
    }

    pub fn add_failure_mode(&mut self, mode: FailureMode) {
        self.failure_modes.push(mode);
    }

    pub fn add_failure(&mut self, mode: FailureMode) {
        self.add_failure_mode(mode);
    }

    pub fn add_suggestion(&mut self, suggestion: ImprovementSuggestion) {
        self.improvement_suggestions.push(suggestion);
    }

    pub fn add_similar_trace(&mut self, trace: SimilarPassingTrace) {
        self.similar_passing_traces.push(trace);
    }

    pub fn sort_by_severity(&mut self) {
        self.failure_modes.sort_by(|a, b| b.severity.weight().cmp(&a.severity.weight()));
    }

    pub fn calculate_improvement_potential(&mut self) {
        if self.improvement_suggestions.is_empty() {
            self.improvement_potential = 0.0;
            return;
        }

        let mut remaining = 1.0;
        for suggestion in &self.improvement_suggestions {
            remaining *= 1.0 - suggestion.expected_impact.clamp(0.0, 1.0);
        }
        self.improvement_potential = (1.0 - remaining).clamp(0.0, 1.0);
    }

    pub fn generate_summary(&mut self) {
        if self.failure_modes.is_empty() {
            self.summary = "No significant issues detected".to_string();
            return;
        }

        self.sort_by_severity();
        let top = &self.failure_modes[0];
        self.summary = format!(
            "Top issue: {} ({})",
            top.category.as_str(),
            top.severity.as_str()
        );
    }

    pub fn has_critical_issues(&self) -> bool {
        self.failure_modes
            .iter()
            .any(|mode| matches!(mode.severity, Severity::Critical))
    }

    pub fn is_empty(&self) -> bool {
        self.failure_modes.is_empty()
            && self.improvement_suggestions.is_empty()
            && self.similar_passing_traces.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackBuilder {
    feedback: ActionableFeedback,
}

impl FeedbackBuilder {
    pub fn new() -> Self {
        Self {
            feedback: ActionableFeedback::new(),
        }
    }

    pub fn add_failure(
        mut self,
        category: FailureCategory,
        severity: Severity,
        evidence: &str,
        location: Option<FailureLocation>,
    ) -> Self {
        self.feedback.failure_modes.push(FailureMode {
            category,
            severity,
            evidence: evidence.to_string(),
            location,
            metric: None,
            details: None,
        });
        self
    }

    pub fn add_suggestion(
        mut self,
        category: FailureCategory,
        action: &str,
        expected_impact: f64,
    ) -> Self {
        self.feedback.improvement_suggestions.push(ImprovementSuggestion {
            category,
            action: action.to_string(),
            expected_impact,
            priority: 1,
            confidence: None,
        });
        self
    }

    pub fn add_similar_trace(
        mut self,
        trace_id: &str,
        similarity: f64,
        key_differences: Vec<String>,
    ) -> Self {
        self.feedback.similar_passing_traces.push(SimilarPassingTrace {
            trace_id: trace_id.to_string(),
            similarity,
            key_differences,
        });
        self
    }

    pub fn build(mut self) -> ActionableFeedback {
        self.feedback.sort_by_severity();
        self.feedback
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureLocationType {
    Input,
    Output,
    Context,
    Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureCategoryGroup {
    Hallucination,
    Reasoning,
    Alignment,
    Format,
    Safety,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureCategoryMapping {
    pub category: FailureCategory,
    pub group: FailureCategoryGroup,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionableFeedbackSummary {
    pub dominant_group: FailureCategoryGroup,
    pub total_issues: usize,
    pub weighted_score: f64,
    pub top_issue: Option<FailureMode>,
}

impl ActionableFeedbackSummary {
    pub fn from_feedback(feedback: &ActionableFeedback) -> Self {
        if feedback.failure_modes.is_empty() {
            return Self {
                dominant_group: FailureCategoryGroup::Other,
                total_issues: 0,
                weighted_score: 0.0,
                top_issue: None,
            };
        }

        let mut scores: HashMap<FailureCategoryGroup, f64> = HashMap::new();
        for mode in &feedback.failure_modes {
            let group = categorize_failure(&mode.category);
            let entry = scores.entry(group).or_insert(0.0);
            *entry += mode.severity.weight() as f64;
        }

        let dominant_group = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, _)| k.clone())
            .unwrap_or(FailureCategoryGroup::Other);

        let weighted_score = scores.values().sum::<f64>()
            / feedback.failure_modes.len().max(1) as f64;

        Self {
            dominant_group,
            total_issues: feedback.failure_modes.len(),
            weighted_score,
            top_issue: feedback.failure_modes.first().cloned(),
        }
    }
}

fn categorize_failure(category: &FailureCategory) -> FailureCategoryGroup {
    match category {
        FailureCategory::Hallucination | FailureCategory::Factualinconsistency => {
            FailureCategoryGroup::Hallucination
        }
        FailureCategory::IncompleteReasoning | FailureCategory::LogicError => {
            FailureCategoryGroup::Reasoning
        }
        FailureCategory::Irrelevance | FailureCategory::PromptMisunderstanding => {
            FailureCategoryGroup::Alignment
        }
        FailureCategory::FormatNonCompliance => FailureCategoryGroup::Format,
        FailureCategory::ToxicContent => FailureCategoryGroup::Safety,
        _ => FailureCategoryGroup::Other,
    }
}
