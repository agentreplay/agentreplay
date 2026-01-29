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

//! MCP Request Handlers
//!
//! Handles JSON-RPC 2.0 requests for the MCP protocol.

use crate::api::AppState;
use crate::mcp::protocol::*;
use crate::mcp::tools::*;
use agentreplay_index::CausalIndex;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

/// MCP request handler
pub struct MCPHandler {
    state: AppState,
    causal_index: Arc<CausalIndex>,
}

impl MCPHandler {
    /// Create a new MCP handler
    pub fn new(state: AppState, causal_index: Arc<CausalIndex>) -> Self {
        Self {
            state,
            causal_index,
        }
    }

    /// Handle a JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!(method = %request.method, "MCP request received");

        match request.method.as_str() {
            // Health check (MCP protocol standard)
            "ping" => self.handle_ping(request.id).await,

            // Initialization
            "initialize" => self.handle_initialize(request.id, request.params).await,
            "initialized" => self.handle_initialized(request.id).await,

            // Resources
            "resources/list" => self.handle_resources_list(request.id).await,
            "resources/read" => self.handle_resources_read(request.id, request.params).await,

            // Tools
            "tools/list" => self.handle_tools_list(request.id).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,

            // Prompts
            "prompts/list" => self.handle_prompts_list(request.id).await,
            "prompts/get" => self.handle_prompts_get(request.id, request.params).await,

            // Unknown method
            _ => {
                warn!(method = %request.method, "Unknown MCP method");
                JsonRpcResponse::error(request.id, JsonRpcError::method_not_found(&request.method))
            }
        }
    }

    /// Handle ping request (MCP health check)
    /// Returns empty object per MCP protocol specification
    async fn handle_ping(&self, id: JsonRpcId) -> JsonRpcResponse {
        info!("MCP ping received - server healthy");
        JsonRpcResponse::success(id, json!({}))
    }

    /// Handle initialize request
    async fn handle_initialize(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let _init_params: InitializeParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        JsonRpcError::invalid_params(format!("Invalid initialize params: {}", e)),
                    )
                }
            },
            None => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params("Missing initialize params"),
                )
            }
        };

        let result = InitializeResult {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                prompts: Some(PromptsCapability {
                    list_changed: false,
                }),
                resources: Some(ResourcesCapability {
                    subscribe: false,
                    list_changed: false,
                }),
                tools: Some(ToolsCapability {
                    list_changed: false,
                }),
                logging: Some(LoggingCapability {}),
            },
            server_info: ServerInfo {
                name: "agentreplay-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    /// Handle initialized notification
    async fn handle_initialized(&self, id: JsonRpcId) -> JsonRpcResponse {
        info!("MCP client initialized");
        JsonRpcResponse::success(id, json!({}))
    }

    /// Handle resources/list
    async fn handle_resources_list(&self, id: JsonRpcId) -> JsonRpcResponse {
        let resources = vec![
            Resource {
                uri: "agentreplay://traces/recent".to_string(),
                name: "Recent Traces".to_string(),
                description: Some("Most recent trace data from the last 24 hours".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            Resource {
                uri: "agentreplay://traces/errors".to_string(),
                name: "Error Traces".to_string(),
                description: Some("Traces containing errors or failures".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            Resource {
                uri: "agentreplay://stats/summary".to_string(),
                name: "Statistics Summary".to_string(),
                description: Some("High-level statistics about trace data".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ];

        let result = ListResourcesResult {
            resources,
            next_cursor: None,
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    /// Handle resources/read
    async fn handle_resources_read(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let read_params: ReadResourceParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        JsonRpcError::invalid_params(format!("Invalid read params: {}", e)),
                    )
                }
            },
            None => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params("Missing read params"),
                )
            }
        };

        let content = match read_params.uri.as_str() {
            "agentreplay://traces/recent" => {
                // Get recent traces (last 24 hours)
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_micros() as u64)
                    .unwrap_or(0);
                let start = now.saturating_sub(86_400_000_000); // 24 hours

                match self.state.db.query_temporal_range(start, now) {
                    Ok(edges) => {
                        let traces: Vec<serde_json::Value> = edges
                            .iter()
                            .take(100)
                            .map(|e| {
                                json!({
                                    "edge_id": format!("{:#x}", e.edge_id),
                                    "timestamp": e.timestamp_us,
                                    "span_type": format!("{:?}", e.get_span_type()),
                                    "duration_ms": e.duration_us as f64 / 1000.0,
                                    "tokens": e.token_count,
                                })
                            })
                            .collect();
                        json!({ "traces": traces, "count": traces.len() }).to_string()
                    }
                    Err(e) => json!({ "error": e.to_string() }).to_string(),
                }
            }
            "agentreplay://traces/errors" => {
                // Get error traces
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_micros() as u64)
                    .unwrap_or(0);
                let start = now.saturating_sub(86_400_000_000 * 7); // Last week

                match self.state.db.query_temporal_range(start, now) {
                    Ok(edges) => {
                        let errors: Vec<serde_json::Value> = edges
                            .iter()
                            .filter(|e| {
                                matches!(e.get_span_type(), agentreplay_core::SpanType::Error)
                            })
                            .take(50)
                            .map(|e| {
                                json!({
                                    "edge_id": format!("{:#x}", e.edge_id),
                                    "timestamp": e.timestamp_us,
                                    "duration_ms": e.duration_us as f64 / 1000.0,
                                })
                            })
                            .collect();
                        json!({ "errors": errors, "count": errors.len() }).to_string()
                    }
                    Err(e) => json!({ "error": e.to_string() }).to_string(),
                }
            }
            "agentreplay://stats/summary" => {
                // Get statistics summary
                let stats = self.state.db.stats();
                json!({
                    "causal_nodes": stats.causal_nodes,
                    "causal_edges": stats.causal_edges,
                    "vector_count": stats.vector_count,
                })
                .to_string()
            }
            _ => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params(format!(
                        "Unknown resource URI: {}",
                        read_params.uri
                    )),
                )
            }
        };

        let result = ReadResourceResult {
            contents: vec![ResourceContent {
                uri: read_params.uri,
                mime_type: Some("application/json".to_string()),
                text: Some(content),
                blob: None,
            }],
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    /// Handle tools/list
    async fn handle_tools_list(&self, id: JsonRpcId) -> JsonRpcResponse {
        let result = ListToolsResult {
            tools: get_tool_definitions(),
            next_cursor: None,
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    /// Handle tools/call
    async fn handle_tools_call(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let call_params: CallToolParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        JsonRpcError::invalid_params(format!("Invalid tool call params: {}", e)),
                    )
                }
            },
            None => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params("Missing tool call params"),
                )
            }
        };

        info!(tool = %call_params.name, "Executing MCP tool");

        let result = match call_params.name.as_str() {
            "search_traces" => {
                let search_params: TraceSearchParams = match serde_json::from_value(
                    serde_json::Value::Object(
                        call_params
                            .arguments
                            .into_iter()
                            .collect::<serde_json::Map<String, serde_json::Value>>(),
                    ),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params(format!("Invalid search params: {}", e)),
                        )
                    }
                };

                execute_search_traces(&self.state, search_params, self.causal_index.clone()).await
            }

            "get_context" => {
                let context_params: GetContextParams = match serde_json::from_value(
                    serde_json::Value::Object(
                        call_params
                            .arguments
                            .into_iter()
                            .collect::<serde_json::Map<String, serde_json::Value>>(),
                    ),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params(format!("Invalid context params: {}", e)),
                        )
                    }
                };

                execute_get_context(&self.state, context_params, self.causal_index.clone()).await
            }

            "get_trace_details" => {
                let edge_id = call_params
                    .arguments
                    .get("edge_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                execute_get_trace_details(&self.state, edge_id).await
            }

            "get_related_traces" => {
                let edge_id = call_params
                    .arguments
                    .get("edge_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let direction = call_params
                    .arguments
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("both");
                let max_depth = call_params
                    .arguments
                    .get("max_depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10) as usize;

                execute_get_related_traces(
                    &self.state,
                    self.causal_index.clone(),
                    edge_id,
                    direction,
                    max_depth,
                )
                .await
            }

            "get_trace_summary" => {
                // Implement summary tool
                let time_range = call_params
                    .arguments
                    .get("time_range")
                    .and_then(|v| v.as_str())
                    .unwrap_or("last_day");

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_micros() as u64)
                    .unwrap_or(0);

                let start = match time_range {
                    "last_hour" => now.saturating_sub(3_600_000_000),
                    "last_day" => now.saturating_sub(86_400_000_000),
                    "last_week" => now.saturating_sub(86_400_000_000 * 7),
                    "last_month" => now.saturating_sub(86_400_000_000 * 30),
                    _ => now.saturating_sub(86_400_000_000),
                };

                match self.state.db.query_temporal_range(start, now) {
                    Ok(edges) => {
                        let total = edges.len();
                        let errors = edges
                            .iter()
                            .filter(|e| {
                                matches!(e.get_span_type(), agentreplay_core::SpanType::Error)
                            })
                            .count();
                        let total_tokens: u64 = edges.iter().map(|e| e.token_count as u64).sum();
                        let avg_duration: f64 = if total > 0 {
                            edges.iter().map(|e| e.duration_us as f64).sum::<f64>() / total as f64
                        } else {
                            0.0
                        };

                        Ok(CallToolResult {
                            content: vec![ToolContent::Text {
                                text: json!({
                                    "time_range": time_range,
                                    "total_traces": total,
                                    "error_count": errors,
                                    "error_rate": if total > 0 { errors as f64 / total as f64 } else { 0.0 },
                                    "total_tokens": total_tokens,
                                    "avg_duration_ms": avg_duration / 1000.0,
                                })
                                .to_string(),
                            }],
                            is_error: None,
                        })
                    }
                    Err(e) => Err(format!("Failed to get summary: {}", e)),
                }
            }

            _ => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::method_not_found(&call_params.name),
                )
            }
        };

        match result {
            Ok(tool_result) => {
                JsonRpcResponse::success(id, serde_json::to_value(tool_result).unwrap())
            }
            Err(e) => JsonRpcResponse::error(id, JsonRpcError::internal_error(e)),
        }
    }

    /// Handle prompts/list
    async fn handle_prompts_list(&self, id: JsonRpcId) -> JsonRpcResponse {
        let prompts = vec![
            Prompt {
                name: "analyze_error".to_string(),
                description: Some(
                    "Analyze an error and find similar past issues with resolutions".to_string(),
                ),
                arguments: Some(vec![PromptArgument {
                    name: "error_message".to_string(),
                    description: Some("The error message to analyze".to_string()),
                    required: Some(true),
                }]),
            },
            Prompt {
                name: "summarize_session".to_string(),
                description: Some("Generate a summary of a trace session".to_string()),
                arguments: Some(vec![PromptArgument {
                    name: "session_id".to_string(),
                    description: Some("The session ID to summarize".to_string()),
                    required: Some(true),
                }]),
            },
            Prompt {
                name: "find_patterns".to_string(),
                description: Some(
                    "Find patterns in recent traces (errors, slow calls, etc.)".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "pattern_type".to_string(),
                        description: Some(
                            "Type of pattern: 'errors', 'slow', 'high_cost'".to_string(),
                        ),
                        required: Some(false),
                    },
                    PromptArgument {
                        name: "time_range".to_string(),
                        description: Some(
                            "Time range: 'last_hour', 'last_day', 'last_week'".to_string(),
                        ),
                        required: Some(false),
                    },
                ]),
            },
        ];

        let result = ListPromptsResult {
            prompts,
            next_cursor: None,
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    /// Handle prompts/get
    async fn handle_prompts_get(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let get_params: GetPromptParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        JsonRpcError::invalid_params(format!("Invalid prompt params: {}", e)),
                    )
                }
            },
            None => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params("Missing prompt params"),
                )
            }
        };

        let result = match get_params.name.as_str() {
            "analyze_error" => {
                let error_msg = get_params
                    .arguments
                    .get("error_message")
                    .cloned()
                    .unwrap_or_default();

                GetPromptResult {
                    description: Some("Analyze the error and find similar past issues".to_string()),
                    messages: vec![PromptMessage {
                        role: PromptRole::User,
                        content: PromptContent::Text {
                            text: format!(
                                "I encountered this error:\n\n```\n{}\n```\n\n\
                                     Please use the search_traces tool to find similar past errors \
                                     and their resolutions. Then provide:\n\
                                     1. Similar errors from the past\n\
                                     2. How they were resolved\n\
                                     3. Recommended actions for this error",
                                error_msg
                            ),
                        },
                    }],
                }
            }

            "summarize_session" => {
                let session_id = get_params
                    .arguments
                    .get("session_id")
                    .cloned()
                    .unwrap_or_default();

                GetPromptResult {
                    description: Some("Generate a session summary".to_string()),
                    messages: vec![PromptMessage {
                        role: PromptRole::User,
                        content: PromptContent::Text {
                            text: format!(
                                "Please analyze session {} and provide:\n\
                                     1. Overall session summary\n\
                                     2. Key operations performed\n\
                                     3. Any errors or issues\n\
                                     4. Performance metrics (latency, tokens used)\n\
                                     5. Recommendations for improvement",
                                session_id
                            ),
                        },
                    }],
                }
            }

            "find_patterns" => {
                let pattern_type = get_params
                    .arguments
                    .get("pattern_type")
                    .cloned()
                    .unwrap_or_else(|| "all".to_string());
                let time_range = get_params
                    .arguments
                    .get("time_range")
                    .cloned()
                    .unwrap_or_else(|| "last_day".to_string());

                GetPromptResult {
                    description: Some("Find patterns in traces".to_string()),
                    messages: vec![PromptMessage {
                        role: PromptRole::User,
                        content: PromptContent::Text {
                            text: format!(
                                "Please analyze traces from the {} and find {} patterns:\n\n\
                                     Use the get_trace_summary tool first to get an overview, \
                                     then use search_traces to find specific examples.\n\n\
                                     Provide:\n\
                                     1. Most common patterns\n\
                                     2. Anomalies or outliers\n\
                                     3. Trends over time\n\
                                     4. Actionable recommendations",
                                time_range, pattern_type
                            ),
                        },
                    }],
                }
            }

            _ => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params(format!("Unknown prompt: {}", get_params.name)),
                )
            }
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }
}
