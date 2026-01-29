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

//! Core types for Agentreplay plugins
//!
//! These types match the WIT interface definition and are used for
//! data exchange between plugins and the host.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for traces and spans (128-bit UUID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId {
    pub high: u64,
    pub low: u64,
}

impl TraceId {
    pub fn new(high: u64, low: u64) -> Self {
        Self { high, low }
    }

    pub fn from_uuid(uuid: &str) -> Option<Self> {
        let uuid = uuid.replace("-", "");
        if uuid.len() != 32 {
            return None;
        }
        let high = u64::from_str_radix(&uuid[..16], 16).ok()?;
        let low = u64::from_str_radix(&uuid[16..], 16).ok()?;
        Some(Self { high, low })
    }

    pub fn to_uuid(&self) -> String {
        format!("{:016x}{:016x}", self.high, self.low)
    }
}

/// Span types in a trace
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    LlmCall,
    ToolCall,
    Retrieval,
    AgentStep,
    Embedding,
    #[default]
    Custom,
}

/// A single span/edge in a trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub id: TraceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<TraceId>,
    #[serde(default)]
    pub span_type: SpanType,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub timestamp_us: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_us: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Complete trace context for evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub spans: Vec<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl TraceContext {
    /// Get the root span (first span without a parent)
    pub fn root_span(&self) -> Option<&Span> {
        self.spans.iter().find(|s| s.parent_id.is_none())
    }

    /// Get all LLM call spans
    pub fn llm_spans(&self) -> impl Iterator<Item = &Span> {
        self.spans
            .iter()
            .filter(|s| s.span_type == SpanType::LlmCall)
    }

    /// Get all tool call spans
    pub fn tool_spans(&self) -> impl Iterator<Item = &Span> {
        self.spans
            .iter()
            .filter(|s| s.span_type == SpanType::ToolCall)
    }

    /// Calculate total duration
    pub fn total_duration_us(&self) -> u64 {
        self.spans.iter().filter_map(|s| s.duration_us).sum()
    }

    /// Calculate total tokens
    pub fn total_tokens(&self) -> u32 {
        self.spans.iter().filter_map(|s| s.token_count).sum()
    }

    /// Calculate total cost
    pub fn total_cost(&self) -> f64 {
        self.spans.iter().filter_map(|s| s.cost_usd).sum()
    }
}

/// Metric value (union type)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
}

impl From<f64> for MetricValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<i64> for MetricValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<bool> for MetricValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<String> for MetricValue {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for MetricValue {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

/// Evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub evaluator_id: String,
    pub passed: bool,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    #[serde(default)]
    pub metrics: HashMap<String, MetricValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
}

impl Default for EvalResult {
    fn default() -> Self {
        Self {
            evaluator_id: String::new(),
            passed: false,
            confidence: 0.0,
            explanation: None,
            metrics: HashMap::new(),
            cost_usd: None,
            duration_ms: None,
        }
    }
}

impl EvalResult {
    /// Create a passing result
    pub fn pass(evaluator_id: impl Into<String>, confidence: f64) -> Self {
        Self {
            evaluator_id: evaluator_id.into(),
            passed: true,
            confidence,
            ..Default::default()
        }
    }

    /// Create a failing result
    pub fn fail(evaluator_id: impl Into<String>, confidence: f64) -> Self {
        Self {
            evaluator_id: evaluator_id.into(),
            passed: false,
            confidence,
            ..Default::default()
        }
    }

    /// Add an explanation
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    /// Add a metric
    pub fn with_metric(mut self, key: impl Into<String>, value: impl Into<MetricValue>) -> Self {
        self.metrics.insert(key.into(), value.into());
        self
    }
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_eval: Option<f64>,
}

impl Default for PluginMetadata {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "0.1.0".into(),
            description: String::new(),
            author: None,
            tags: vec![],
            cost_per_eval: None,
        }
    }
}

/// Embedding vector type
pub type Embedding = Vec<f32>;

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// HTTP response from host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Get body as string
    pub fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.clone())
    }

    /// Parse body as JSON
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }

    /// Check if response is successful (2xx)
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id() {
        let id = TraceId::new(0x0123456789abcdef, 0xfedcba9876543210);
        assert_eq!(id.to_uuid(), "0123456789abcdeffedcba9876543210");
    }

    #[test]
    fn test_eval_result() {
        let result = EvalResult::pass("test", 0.95)
            .with_explanation("All checks passed")
            .with_metric("score", 0.95)
            .with_metric("passed_checks", 10i64);

        assert!(result.passed);
        assert_eq!(result.confidence, 0.95);
    }
}
