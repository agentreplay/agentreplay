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

use flowtrace_core::{AgentFlowEdge, SpanEvent, SpanType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Invalid span_id format")]
    InvalidSpanId,
    #[error("Invalid trace_id format")]
    InvalidTraceId,
    #[error("Missing required attribute: agent_id")]
    MissingAgentId,
    #[error("Missing required attribute: {0}")]
    MissingAttribute(String),
    #[error("Invalid attribute value: {0}")]
    InvalidAttribute(String),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OtelSpan {
    pub span_id: String,
    pub trace_id: String,
    #[serde(default)]
    pub parent_span_id: Option<String>,
    pub name: String,
    pub start_time: u64,
    #[serde(default)]
    pub end_time: Option<u64>,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub events: Vec<SpanEvent>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OtelSpanBatch {
    pub spans: Vec<OtelSpan>,
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub accepted: usize,
    pub rejected: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// Convert OpenTelemetry span to AgentFlowEdge
pub fn convert_otel_span_to_edge(
    span: &OtelSpan,
    tenant_id: u64,
    project_id: u16,
) -> Result<AgentFlowEdge, ConversionError> {
    // Parse span_id as hex → u128
    let span_id_str = span.span_id.trim_start_matches("0x");
    let edge_id = u128::from_str_radix(span_id_str, 16)
        .or_else(|_| span_id_str.parse::<u128>())
        .map_err(|_| ConversionError::InvalidSpanId)?;

    // Extract agent_id from attributes
    let agent_id = extract_u64_attr(&span.attributes, "agent_id")
        .or_else(|| extract_u64_attr(&span.attributes, "agent.id"))
        .unwrap_or(1); // Default agent_id if not specified

    // Extract session_id from trace_id or attributes
    let session_id = extract_u64_attr(&span.attributes, "session_id")
        .or_else(|| extract_u64_attr(&span.attributes, "session.id"))
        .or_else(|| span.trace_id.parse::<u64>().ok())
        .unwrap_or_else(|| {
            // Use trace_id hash as fallback
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            span.trace_id.hash(&mut hasher);
            hasher.finish()
        });

    // Parse parent span_id
    let causal_parent = match &span.parent_span_id {
        Some(p) if !p.is_empty() && p != "0" => {
            let parent_str = p.trim_start_matches("0x");
            u128::from_str_radix(parent_str, 16)
                .or_else(|_| parent_str.parse::<u128>())
                .unwrap_or(0)
        }
        _ => 0,
    };

    // Calculate duration
    let duration_us = match span.end_time {
        Some(end) if end > span.start_time => (end - span.start_time) as u32,
        _ => 0, // Still in progress or invalid
    };

    // Map span name to SpanType
    let span_type = match span.name.to_lowercase().as_str() {
        // LangChain/LangGraph specific patterns
        name if name.starts_with("chain.") => SpanType::Planning,
        name if name.starts_with("tool.") => SpanType::ToolCall,
        name if name.starts_with("llm.") => SpanType::Reasoning,
        name if name.starts_with("retriever.") => SpanType::ToolCall,

        // Generic patterns
        name if name.contains("agent") || name.contains("planning") => SpanType::Planning,
        name if name.contains("llm") || name.contains("chat") || name.contains("completion") => {
            SpanType::Reasoning
        }
        name if name.contains("tool") || name.contains("function") => SpanType::ToolCall,
        name if name.contains("retriev") || name.contains("search") || name.contains("query") => {
            SpanType::ToolCall
        }
        name if name.contains("chain") || name.contains("sequence") => SpanType::Synthesis,
        name if name.contains("embed") => SpanType::ToolCall,
        name if name.contains("error") => SpanType::Error,
        name if name.contains("response") => SpanType::Response,
        _ => SpanType::Root,
    };

    // Extract token count
    let token_count = extract_u32_attr(&span.attributes, "tokens")
        .or_else(|| extract_u32_attr(&span.attributes, "token_count"))
        .or_else(|| extract_u32_attr(&span.attributes, "llm.token_count.total"))
        .unwrap_or(0);

    // Determine flags (error status)
    let flags = if span.status.as_deref() == Some("error")
        || span
            .attributes
            .get("error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        1 // Error flag
    } else {
        0
    };

    // Create edge using the new() constructor
    let mut edge = AgentFlowEdge::new(
        tenant_id,
        project_id,
        agent_id,
        session_id,
        span_type,
        causal_parent,
    );

    // Override fields from OTel span
    edge.edge_id = edge_id;
    edge.timestamp_us = span.start_time;
    edge.duration_us = duration_us;
    edge.token_count = token_count;
    edge.flags = flags;
    edge.has_payload = 1; // We store attributes as payload

    // Recompute checksum after modifications
    edge.checksum = edge.compute_checksum();

    Ok(edge)
}

/// Extract u64 attribute from span attributes
fn extract_u64_attr(attrs: &HashMap<String, serde_json::Value>, key: &str) -> Option<u64> {
    attrs.get(key).and_then(|v| match v {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

/// Extract u32 attribute from span attributes
fn extract_u32_attr(attrs: &HashMap<String, serde_json::Value>, key: &str) -> Option<u32> {
    attrs.get(key).and_then(|v| match v {
        serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_basic_span() {
        let mut attrs = HashMap::new();
        attrs.insert("agent_id".to_string(), serde_json::json!(42));
        attrs.insert("tokens".to_string(), serde_json::json!(150));

        let span = OtelSpan {
            span_id: "1730000000000000".to_string(),
            trace_id: "12345".to_string(),
            parent_span_id: None,
            name: "llm_call".to_string(),
            start_time: 1730000000000000,
            end_time: Some(1730000001500000),
            attributes: attrs,
            events: Vec::new(),
            status: Some("ok".to_string()),
        };

        let edge = convert_otel_span_to_edge(&span, 1, 1).unwrap();

        assert_eq!(edge.agent_id, 42);
        assert_eq!(edge.token_count, 150);
        assert_eq!(edge.duration_us, 1500000);
        assert_eq!(edge.get_span_type(), SpanType::Reasoning);
        assert_eq!(edge.flags, 0);
    }

    #[test]
    fn test_convert_span_with_error() {
        // Use hex format for IDs (OTEL standard)
        let span = OtelSpan {
            span_id: "0x64".to_string(),              // 100 in hex
            trace_id: "0xc8".to_string(),             // 200 in hex
            parent_span_id: Some("0x32".to_string()), // 50 in hex
            name: "agent".to_string(),
            start_time: 1000000,
            end_time: Some(2000000),
            attributes: HashMap::new(),
            events: Vec::new(),
            status: Some("error".to_string()),
        };

        let edge = convert_otel_span_to_edge(&span, 1, 1).unwrap();

        assert_eq!(edge.get_span_type(), SpanType::Planning);
        assert_eq!(edge.flags, 1); // Error flag set
        assert_eq!(edge.causal_parent, 50); // 0x32 = 50
    }
}
