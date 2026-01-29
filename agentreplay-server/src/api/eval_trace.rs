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

use std::collections::HashMap;

use agentreplay_core::{
    ContentPartV1, EvalTraceV1, MessageV1, OutcomeV1, OutcomeV2, SpanSummaryV1,
    TranscriptEventV1, TraceStatsV1,
};

use crate::api::payload_extractors::{
    extract_completions, extract_prompts, extract_tool_calls,
};
use crate::api::query::AppState;
use crate::otel_genai::{GenAIPayload, ModelPricing};

fn parse_messages_from_value(value: &serde_json::Value) -> Option<Vec<MessageV1>> {
    let array = value.as_array()?;
    let mut messages = Vec::new();

    for item in array {
        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let name = item.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let tool_call_id = item
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut content_parts = Vec::new();
        if let Some(content) = item.get("content") {
            if let Some(text) = content.as_str() {
                content_parts.push(ContentPartV1::Text {
                    text: text.to_string(),
                });
            } else if let Some(parts) = content.as_array() {
                for part in parts {
                    if let Some(part_type) = part.get("type").and_then(|v| v.as_str()) {
                        match part_type {
                            "text" => {
                                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                    content_parts.push(ContentPartV1::Text {
                                        text: text.to_string(),
                                    });
                                }
                            }
                            "tool_use" => {
                                let id = part
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default();
                                let name = part
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default();
                                let arguments = part
                                    .get("input")
                                    .or_else(|| part.get("arguments"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                content_parts.push(ContentPartV1::ToolUse {
                                    id: id.to_string(),
                                    name: name.to_string(),
                                    arguments,
                                });
                            }
                            "tool_result" => {
                                let tool_call_id = part
                                    .get("tool_call_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default();
                                let content = part
                                    .get("content")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                content_parts.push(ContentPartV1::ToolResult {
                                    tool_call_id: tool_call_id.to_string(),
                                    content,
                                });
                            }
                            _ => content_parts.push(ContentPartV1::Json {
                                value: part.clone(),
                            }),
                        }
                    } else {
                        content_parts.push(ContentPartV1::Json { value: part.clone() });
                    }
                }
            }
        }

        if content_parts.is_empty() {
            if let Some(raw) = item.get("content") {
                content_parts.push(ContentPartV1::Json { value: raw.clone() });
            }
        }

        messages.push(MessageV1 {
            role: role.to_string(),
            content: content_parts,
            name,
            tool_call_id,
            metadata: HashMap::new(),
        });
    }

    Some(messages)
}

fn payload_messages(payload: &GenAIPayload) -> (Vec<MessageV1>, Vec<MessageV1>) {
    let input_messages = payload
        .additional
        .get("gen_ai.input.messages")
        .and_then(parse_messages_from_value)
        .unwrap_or_default();

    let output_messages = payload
        .additional
        .get("gen_ai.output.messages")
        .and_then(parse_messages_from_value)
        .unwrap_or_default();

    (input_messages, output_messages)
}

fn merge_text(parts: &[ContentPartV1]) -> Option<String> {
    let mut text = String::new();
    for part in parts {
        if let ContentPartV1::Text { text: chunk } = part {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(chunk);
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn build_eval_trace_v1(state: &AppState, root_edge: &agentreplay_core::AgentFlowEdge) -> EvalTraceV1 {
    let mut eval_trace = EvalTraceV1::new(format!("0x{:x}", root_edge.edge_id), root_edge.session_id);

    let mut edges: Vec<agentreplay_core::AgentFlowEdge> = state
        .db
        .get_session_edges(root_edge.session_id)
        .into_iter()
        .filter_map(|edge_id| state.db.get(edge_id).ok().flatten())
        .collect();

    if edges.is_empty() {
        edges.push(*root_edge);
    }

    edges.sort_by(|a, b| {
        a.timestamp_us
            .cmp(&b.timestamp_us)
            .then(a.logical_clock.cmp(&b.logical_clock))
            .then(a.edge_id.cmp(&b.edge_id))
    });

    let mut transcript = Vec::new();
    let mut outcome_messages = Vec::new();
    let mut output_text: Option<String> = None;

    let mut total_input_tokens = 0u32;
    let mut total_output_tokens = 0u32;
    let mut total_tokens = 0u32;
    let mut total_cost = 0.0f64;
    let mut has_cost = false;
    let mut total_latency_ms = 0.0f64;

    for edge in &edges {
        let span_id = format!("0x{:x}", edge.edge_id);
        let parent_span_id = if edge.causal_parent != 0 {
            Some(format!("0x{:x}", edge.causal_parent))
        } else {
            None
        };

        let mut status = "completed".to_string();
        let payload = if edge.has_payload == 0 {
            None
        } else {
            state.db.get_payload(edge.edge_id).ok().flatten().and_then(|bytes| serde_json::from_slice::<GenAIPayload>(&bytes).ok())
        };

        if let Some(payload) = payload.as_ref() {
            if payload.error_type.is_some() || edge.get_span_type() == agentreplay_core::SpanType::Error {
                status = "error".to_string();
            }
        } else if edge.get_span_type() == agentreplay_core::SpanType::Error {
            status = "error".to_string();
        }

        eval_trace.spans.push(SpanSummaryV1 {
            span_id: span_id.clone(),
            parent_span_id: parent_span_id.clone(),
            span_type: format!("{:?}", edge.get_span_type()),
            timestamp_us: edge.timestamp_us,
            duration_us: edge.duration_us,
            status: status.clone(),
            attributes: payload.as_ref().and_then(|p| serde_json::to_value(p).ok()),
        });

        transcript.push(TranscriptEventV1::SpanStart {
            span_id: span_id.clone(),
            parent_span_id: parent_span_id.clone(),
            span_type: format!("{:?}", edge.get_span_type()),
            timestamp_us: edge.timestamp_us,
        });

        if let Some(payload) = payload.as_ref() {
            let (input_messages, output_messages) = payload_messages(payload);
            if !input_messages.is_empty() || !output_messages.is_empty() {
                for (idx, message) in input_messages.iter().enumerate() {
                    transcript.push(TranscriptEventV1::Message {
                        id: format!("msg-{span_id}-in-{idx}"),
                        role: message.role.clone(),
                        content: message.content.clone(),
                        timestamp_us: edge.timestamp_us,
                        span_id: Some(span_id.clone()),
                        metadata: message.metadata.clone(),
                    });
                    outcome_messages.push(message.clone());
                }
                for (idx, message) in output_messages.iter().enumerate() {
                    transcript.push(TranscriptEventV1::Message {
                        id: format!("msg-{span_id}-out-{idx}"),
                        role: message.role.clone(),
                        content: message.content.clone(),
                        timestamp_us: edge.timestamp_us,
                        span_id: Some(span_id.clone()),
                        metadata: message.metadata.clone(),
                    });
                    if message.role == "assistant" {
                        output_text = merge_text(&message.content).or(output_text);
                    }
                    outcome_messages.push(message.clone());
                }
            } else {
                let prompts = extract_prompts(payload);
                for (idx, prompt) in prompts.iter().enumerate() {
                    let content = vec![ContentPartV1::Text {
                        text: prompt.content.clone(),
                    }];
                    transcript.push(TranscriptEventV1::Message {
                        id: format!("msg-{span_id}-prompt-{idx}"),
                        role: prompt.role.clone(),
                        content: content.clone(),
                        timestamp_us: edge.timestamp_us,
                        span_id: Some(span_id.clone()),
                        metadata: HashMap::new(),
                    });
                    outcome_messages.push(MessageV1 {
                        role: prompt.role.clone(),
                        content,
                        name: None,
                        tool_call_id: None,
                        metadata: HashMap::new(),
                    });
                }

                let completions = extract_completions(payload);
                for (idx, completion) in completions.iter().enumerate() {
                    let content = vec![ContentPartV1::Text {
                        text: completion.content.clone(),
                    }];
                    transcript.push(TranscriptEventV1::Message {
                        id: format!("msg-{span_id}-completion-{idx}"),
                        role: completion.role.clone(),
                        content: content.clone(),
                        timestamp_us: edge.timestamp_us,
                        span_id: Some(span_id.clone()),
                        metadata: HashMap::new(),
                    });
                    if completion.role == "assistant" {
                        output_text = Some(completion.content.clone());
                    }
                    outcome_messages.push(MessageV1 {
                        role: completion.role.clone(),
                        content,
                        name: None,
                        tool_call_id: None,
                        metadata: HashMap::new(),
                    });
                }
            }

            let tool_calls = extract_tool_calls(payload);
            for (idx, tool_call) in tool_calls.iter().enumerate() {
                let tool_call_id = format!("tool-{span_id}-{idx}");
                transcript.push(TranscriptEventV1::ToolCall {
                    id: tool_call_id.clone(),
                    name: tool_call.name.clone(),
                    arguments: Some(tool_call.arguments.clone()),
                    timestamp_us: edge.timestamp_us,
                    span_id: Some(span_id.clone()),
                    metadata: HashMap::new(),
                });

                transcript.push(TranscriptEventV1::ToolResult {
                    id: format!("tool-result-{span_id}-{idx}"),
                    tool_call_id: tool_call_id.clone(),
                    content: tool_call.result.clone(),
                    timestamp_us: edge.timestamp_us,
                    span_id: Some(span_id.clone()),
                    metadata: HashMap::new(),
                });
            }

            if let Some(model) = payload.request_model.as_ref().or(payload.response_model.as_ref()) {
                let provider = payload.system.clone().or(payload.provider_name.clone()).unwrap_or_else(|| "openai".to_string());
                let pricing = ModelPricing::for_model(&provider, model);
                total_cost += payload.calculate_cost(&pricing);
                has_cost = true;
            }

            total_input_tokens += payload.input_tokens.unwrap_or(0);
            total_output_tokens += payload.output_tokens.unwrap_or(0);
            total_tokens += payload.total_tokens.unwrap_or(0);
        } else {
            total_tokens += edge.token_count;
        }

        total_latency_ms += edge.duration_us as f64 / 1000.0;

        transcript.push(TranscriptEventV1::SpanEnd {
            span_id: span_id.clone(),
            timestamp_us: edge.timestamp_us.saturating_add(edge.duration_us as u64),
            duration_us: edge.duration_us,
            status,
        });
    }

    let outcome = OutcomeV1 {
        status: if eval_trace
            .spans
            .iter()
            .any(|span| span.status == "error")
        {
            "error".to_string()
        } else {
            "completed".to_string()
        },
        error: None,
        messages: outcome_messages.clone(),
        output_text,
        metadata: HashMap::new(),
    };

    let outcome_v2 = OutcomeV2 {
        status: outcome.status.clone(),
        error: outcome.error.clone(),
        messages: outcome_messages,
        output_text: outcome.output_text.clone(),
        metadata: HashMap::new(),
        state_before: None,
        state_after: None,
        side_effects: Vec::new(),
    };

    eval_trace.transcript = transcript;
    eval_trace.outcome = outcome;
    eval_trace.outcome_v2 = Some(outcome_v2);
    eval_trace.stats = TraceStatsV1 {
        total_tokens,
        input_tokens: total_input_tokens,
        output_tokens: total_output_tokens,
        cost_usd: if has_cost { Some(total_cost) } else { None },
        latency_ms: Some(total_latency_ms),
    };

    let trace_hash = eval_trace.content_hash();
    eval_trace.trace_ref = Some(agentreplay_core::TraceRefV1 {
        schema_version: agentreplay_core::EVAL_TRACE_SCHEMA_VERSION_V1.to_string(),
        trace_id: eval_trace.trace_id.clone(),
        export_uri: None,
        hash: Some(trace_hash),
    });

    eval_trace
}
