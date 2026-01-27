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

pub const EVAL_TRACE_SCHEMA_VERSION_V1: &str = "eval_trace_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalTraceV1 {
    pub schema_version: String,
    pub trace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_ref: Option<TraceRefV1>,
    pub session_id: u64,
    pub spans: Vec<SpanSummaryV1>,
    pub transcript: Vec<TranscriptEventV1>,
    pub outcome: OutcomeV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_v2: Option<OutcomeV2>,
    pub stats: TraceStatsV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceStatsV1 {
    pub total_tokens: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_usd: Option<f64>,
    pub latency_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpanSummaryV1 {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub span_type: String,
    pub timestamp_us: u64,
    pub duration_us: u32,
    pub status: String,
    pub attributes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutcomeV1 {
    pub status: String,
    pub error: Option<String>,
    pub messages: Vec<MessageV1>,
    pub output_text: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutcomeV2 {
    pub status: String,
    pub error: Option<String>,
    pub messages: Vec<MessageV1>,
    pub output_text: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub state_before: Option<EnvironmentStateV2>,
    #[serde(default)]
    pub state_after: Option<EnvironmentStateV2>,
    #[serde(default)]
    pub side_effects: Vec<SideEffectV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvironmentStateV2 {
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub state_hash: Option<String>,
    #[serde(default)]
    pub files: HashMap<String, String>,
    #[serde(default)]
    pub databases: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SideEffectV2 {
    pub effect_type: String,
    pub target: String,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    #[serde(default)]
    pub timestamp_us: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscriptEventV1 {
    Message {
        id: String,
        role: String,
        content: Vec<ContentPartV1>,
        timestamp_us: u64,
        span_id: Option<String>,
        metadata: HashMap<String, serde_json::Value>,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Option<String>,
        timestamp_us: u64,
        span_id: Option<String>,
        metadata: HashMap<String, serde_json::Value>,
    },
    ToolResult {
        id: String,
        tool_call_id: String,
        content: Option<String>,
        timestamp_us: u64,
        span_id: Option<String>,
        metadata: HashMap<String, serde_json::Value>,
    },
    SpanStart {
        span_id: String,
        parent_span_id: Option<String>,
        span_type: String,
        timestamp_us: u64,
    },
    SpanEnd {
        span_id: String,
        timestamp_us: u64,
        duration_us: u32,
        status: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageV1 {
    pub role: String,
    pub content: Vec<ContentPartV1>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPartV1 {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        arguments: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        content: Option<String>,
    },
    Json { value: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceRefV1 {
    pub schema_version: String,
    pub trace_id: String,
    #[serde(default)]
    pub export_uri: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
}

impl EvalTraceV1 {
    pub fn new(trace_id: String, session_id: u64) -> Self {
        Self {
            schema_version: EVAL_TRACE_SCHEMA_VERSION_V1.to_string(),
            trace_id,
            trace_ref: None,
            session_id,
            spans: Vec::new(),
            transcript: Vec::new(),
            outcome: OutcomeV1 {
                status: "completed".to_string(),
                error: None,
                messages: Vec::new(),
                output_text: None,
                metadata: HashMap::new(),
            },
            outcome_v2: None,
            stats: TraceStatsV1 {
                total_tokens: 0,
                input_tokens: 0,
                output_tokens: 0,
                cost_usd: None,
                latency_ms: None,
            },
        }
    }

    pub fn content_hash(&self) -> String {
        let mut clone = self.clone();
        clone.trace_ref = None;
        let bytes = serde_json::to_vec(&clone).unwrap_or_default();
        blake3::hash(&bytes).to_hex().to_string()
    }
}
