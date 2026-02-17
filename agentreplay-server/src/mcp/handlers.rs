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

//! MCP Request Handlers
//!
//! Handles JSON-RPC 2.0 requests for the MCP protocol.

use crate::api::AppState;
use crate::api::replay::{generate_fork_replay_response, generate_replay_response};
use crate::mcp::context::MCP_TENANT_ID;
use crate::mcp::protocol::*;
use crate::mcp::tools::*;
use agentreplay_index::CausalIndex;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// MCP request handler
pub struct MCPHandler {
    state: AppState,
    causal_index: Arc<CausalIndex>,
    /// Current MCP log level (T14)
    log_level: RwLock<McpLogLevel>,
    /// Active resource subscriptions: URI → set of subscriber IDs (T8)
    subscriptions: RwLock<std::collections::HashMap<String, HashSet<String>>>,
}

impl MCPHandler {
    /// Create a new MCP handler
    pub fn new(state: AppState, causal_index: Arc<CausalIndex>) -> Self {
        Self {
            state,
            causal_index,
            log_level: RwLock::new(McpLogLevel::Info),
            subscriptions: RwLock::new(std::collections::HashMap::new()),
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
            "resources/list" => self.handle_resources_list(request.id, request.params).await,
            "resources/read" => self.handle_resources_read(request.id, request.params).await,
            "resources/templates/list" => {
                self.handle_resource_templates_list(request.id, request.params)
                    .await
            }
            "resources/subscribe" => {
                self.handle_resource_subscribe(request.id, request.params)
                    .await
            }
            "resources/unsubscribe" => {
                self.handle_resource_unsubscribe(request.id, request.params)
                    .await
            }

            // Tools
            "tools/list" => self.handle_tools_list(request.id, request.params).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,

            // Prompts
            "prompts/list" => self.handle_prompts_list(request.id, request.params).await,
            "prompts/get" => self.handle_prompts_get(request.id, request.params).await,

            // Logging (T14)
            "logging/setLevel" => {
                self.handle_logging_set_level(request.id, request.params)
                    .await
            }

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

    /// Handle initialize request with protocol version negotiation (T2)
    async fn handle_initialize(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let init_params: InitializeParams = match params {
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

        // Version negotiation: pick the highest version both sides support
        let negotiated_version = if MCP_SUPPORTED_VERSIONS
            .contains(&init_params.protocol_version.as_str())
        {
            // Client requested a version we support
            init_params.protocol_version.clone()
        } else {
            // Fall back to our latest version
            info!(
                client_version = %init_params.protocol_version,
                server_version = %MCP_PROTOCOL_VERSION,
                "Client requested unsupported protocol version, using server default"
            );
            MCP_PROTOCOL_VERSION.to_string()
        };

        let result = InitializeResult {
            protocol_version: negotiated_version,
            capabilities: ServerCapabilities {
                prompts: Some(PromptsCapability {
                    list_changed: false,
                }),
                resources: Some(ResourcesCapability {
                    subscribe: true,
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

    /// Handle resources/list with pagination (T11)
    async fn handle_resources_list(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let list_params: ListParams = params
            .and_then(|p| serde_json::from_value(p).ok())
            .unwrap_or_default();

        let all_resources = vec![
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

        // Apply cursor-based pagination
        let page_size = 50;
        let offset = list_params
            .cursor
            .as_deref()
            .and_then(decode_cursor)
            .unwrap_or(0);
        let resources: Vec<Resource> = all_resources.into_iter().skip(offset).take(page_size).collect();
        let next_cursor = if offset + page_size < resources.len() + offset {
            None // All resources fit in one page
        } else {
            None
        };

        let result = ListResourcesResult {
            resources,
            next_cursor,
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

    /// Handle tools/list with pagination (T11)
    async fn handle_tools_list(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let list_params: ListParams = params
            .and_then(|p| serde_json::from_value(p).ok())
            .unwrap_or_default();

        let all_tools = get_tool_definitions();
        let page_size = 50;
        let offset = list_params
            .cursor
            .as_deref()
            .and_then(decode_cursor)
            .unwrap_or(0);
        let tools: Vec<Tool> = all_tools.into_iter().skip(offset).take(page_size).collect();
        let next_cursor = if offset + page_size < tools.len() + offset {
            None
        } else {
            None
        };

        let result = ListToolsResult {
            tools,
            next_cursor,
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
                            structured_content: None,
                            is_error: None,
                        })
                    }
                    Err(e) => Err(format!("Failed to get summary: {}", e)),
                }
            }

            "save_memory" => {
                let content = match call_params
                    .arguments
                    .get("content")
                    .and_then(|v| v.as_str()) {
                        Some(c) => c.to_string(),
                        None => {
                            return JsonRpcResponse::error(
                                id,
                                JsonRpcError::invalid_params("Missing content parameter"),
                            );
                        }
                    };
                
                let collection = call_params
                    .arguments
                    .get("collection")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();
                    
                let tags = call_params
                    .arguments
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    });

                execute_save_memory(&self.state, content, collection, tags).await
            }

            "replay_trace" => {
                let trace_id = match call_params
                    .arguments
                    .get("trace_id")
                    .and_then(|v| v.as_str())
                {
                    Some(v) => v,
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params("Missing trace_id parameter"),
                        );
                    }
                };

                let include_payload = call_params
                    .arguments
                    .get("include_payload")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let max_events = call_params
                    .arguments
                    .get("max_events")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10_000) as usize;

                match generate_replay_response(
                    trace_id,
                    &self.state,
                    MCP_TENANT_ID,
                    include_payload,
                    max_events.clamp(1, 50_000),
                )
                .await
                {
                    Ok(replay) => {
                        let structured = serde_json::to_value(&replay)
                            .map_err(|e| format!("Failed to serialize replay result: {}", e));
                        match structured {
                            Ok(structured) => Ok(CallToolResult {
                                content: vec![ToolContent::Text {
                                    text: structured.to_string(),
                                }],
                                structured_content: Some(structured),
                                is_error: None,
                            }),
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(format!("Replay failed: {}", e)),
                }
            }

            "fork_trace_replay" => {
                let trace_id = match call_params
                    .arguments
                    .get("trace_id")
                    .and_then(|v| v.as_str())
                {
                    Some(v) => v,
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params("Missing trace_id parameter"),
                        );
                    }
                };

                let fork_edge_id = match call_params
                    .arguments
                    .get("fork_edge_id")
                    .and_then(|v| v.as_str())
                {
                    Some(v) => v,
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params("Missing fork_edge_id parameter"),
                        );
                    }
                };

                let alternate_tool_response = match call_params.arguments.get("alternate_tool_response") {
                    Some(v) => v.clone(),
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params(
                                "Missing alternate_tool_response parameter",
                            ),
                        );
                    }
                };

                let max_events = call_params
                    .arguments
                    .get("max_events")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10_000) as usize;

                match generate_fork_replay_response(
                    trace_id,
                    &self.state,
                    MCP_TENANT_ID,
                    fork_edge_id,
                    alternate_tool_response,
                    max_events.clamp(1, 50_000),
                )
                .await
                {
                    Ok(replay) => {
                        let structured = serde_json::to_value(&replay)
                            .map_err(|e| format!("Failed to serialize fork replay result: {}", e));
                        match structured {
                            Ok(structured) => Ok(CallToolResult {
                                content: vec![ToolContent::Text {
                                    text: structured.to_string(),
                                }],
                                structured_content: Some(structured),
                                is_error: None,
                            }),
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(format!("Fork replay failed: {}", e)),
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

    /// Handle prompts/list with pagination (T11)
    async fn handle_prompts_list(
        &self,
        id: JsonRpcId,
        _params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
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
            Prompt {
                name: "replay_trace_debug".to_string(),
                description: Some(
                    "Replay an agent trace step-by-step and identify key breakpoints".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "trace_id".to_string(),
                        description: Some("Trace ID/root edge ID in hex format".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "include_payload".to_string(),
                        description: Some("Include payload in replay events (true/false)".to_string()),
                        required: Some(false),
                    },
                ]),
            },
            Prompt {
                name: "fork_counterfactual".to_string(),
                description: Some(
                    "Fork replay at a specific edge with an alternate tool response to measure behavioral impact".to_string(),
                ),
                arguments: Some(vec![
                    PromptArgument {
                        name: "trace_id".to_string(),
                        description: Some("Trace ID/root edge ID in hex format".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "fork_edge_id".to_string(),
                        description: Some("Edge ID where replay should fork".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "alternate_tool_response".to_string(),
                        description: Some("JSON object with alternate tool response".to_string()),
                        required: Some(true),
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

            "replay_trace_debug" => {
                let trace_id = get_params
                    .arguments
                    .get("trace_id")
                    .cloned()
                    .unwrap_or_default();
                let include_payload = get_params
                    .arguments
                    .get("include_payload")
                    .cloned()
                    .unwrap_or_else(|| "false".to_string());

                GetPromptResult {
                    description: Some("Replay trace and analyze execution flow".to_string()),
                    messages: vec![PromptMessage {
                        role: PromptRole::User,
                        content: PromptContent::Text {
                            text: format!(
                                "Use the replay_trace tool with:\n\
                                 - trace_id: {}\n\
                                 - include_payload: {}\n\
                                 Then provide:\n\
                                 1. Step-by-step timeline of what the agent did\n\
                                 2. Tool-call breakpoints worth inspecting\n\
                                 3. Earliest divergence-risk point where outcome could change\n\
                                 4. A short explanation of the final outcome signature",
                                trace_id, include_payload
                            ),
                        },
                    }],
                }
            }

            "fork_counterfactual" => {
                let trace_id = get_params
                    .arguments
                    .get("trace_id")
                    .cloned()
                    .unwrap_or_default();
                let fork_edge_id = get_params
                    .arguments
                    .get("fork_edge_id")
                    .cloned()
                    .unwrap_or_default();
                let alternate_tool_response = get_params
                    .arguments
                    .get("alternate_tool_response")
                    .cloned()
                    .unwrap_or_else(|| "{}".to_string());

                GetPromptResult {
                    description: Some("Run counterfactual fork replay and quantify impact".to_string()),
                    messages: vec![PromptMessage {
                        role: PromptRole::User,
                        content: PromptContent::Text {
                            text: format!(
                                "Run fork_trace_replay with:\n\
                                 - trace_id: {}\n\
                                 - fork_edge_id: {}\n\
                                 - alternate_tool_response: {}\n\
                                 Then summarize:\n\
                                 1. trajectory_distance\n\
                                 2. sensitivity_score\n\
                                 3. affected_nodes\n\
                                 4. Key behavioral differences between original and forked trajectories",
                                trace_id, fork_edge_id, alternate_tool_response
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

    // =========================================================================
    // Resource Templates (T13)
    // =========================================================================

    /// Handle resources/templates/list
    async fn handle_resource_templates_list(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let _list_params: ListParams = params
            .and_then(|p| serde_json::from_value(p).ok())
            .unwrap_or_default();

        let templates = vec![
            ResourceTemplate {
                uri_template: "agentreplay://traces/{traceId}".to_string(),
                name: "Trace by ID".to_string(),
                description: Some("Get a specific trace by its ID".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "agentreplay://traces/search?q={query}&limit={limit}".to_string(),
                name: "Search Traces".to_string(),
                description: Some("Search traces by query string".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "agentreplay://stats/{statType}".to_string(),
                name: "Statistics".to_string(),
                description: Some("Get statistics by type (summary, errors, latency)".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ];

        let result = ListResourceTemplatesResult {
            resource_templates: templates,
            next_cursor: None,
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    // =========================================================================
    // Resource Subscriptions (T8)
    // =========================================================================

    /// Handle resources/subscribe
    async fn handle_resource_subscribe(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let sub_params: SubscribeParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        JsonRpcError::invalid_params(format!(
                            "Invalid subscribe params: {}",
                            e
                        )),
                    )
                }
            },
            None => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params("Missing subscribe params"),
                )
            }
        };

        // Validate the URI is a known resource pattern
        let valid_prefixes = ["agentreplay://traces/", "agentreplay://stats/"];
        if !valid_prefixes.iter().any(|p| sub_params.uri.starts_with(p)) {
            return JsonRpcResponse::error(
                id,
                JsonRpcError::invalid_params(format!(
                    "Cannot subscribe to unknown resource: {}",
                    sub_params.uri
                )),
            );
        }

        info!(uri = %sub_params.uri, "Resource subscription added");
        let mut subs = self.subscriptions.write().await;
        subs.entry(sub_params.uri)
            .or_insert_with(HashSet::new)
            .insert("default".to_string());

        JsonRpcResponse::success(id, json!({}))
    }

    /// Handle resources/unsubscribe
    async fn handle_resource_unsubscribe(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let unsub_params: UnsubscribeParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        JsonRpcError::invalid_params(format!(
                            "Invalid unsubscribe params: {}",
                            e
                        )),
                    )
                }
            },
            None => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params("Missing unsubscribe params"),
                )
            }
        };

        info!(uri = %unsub_params.uri, "Resource subscription removed");
        let mut subs = self.subscriptions.write().await;
        if let Some(subscribers) = subs.get_mut(&unsub_params.uri) {
            subscribers.remove("default");
            if subscribers.is_empty() {
                subs.remove(&unsub_params.uri);
            }
        }

        JsonRpcResponse::success(id, json!({}))
    }

    // =========================================================================
    // Logging (T14)
    // =========================================================================

    /// Handle logging/setLevel
    async fn handle_logging_set_level(
        &self,
        id: JsonRpcId,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let level_params: SetLevelParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        id,
                        JsonRpcError::invalid_params(format!(
                            "Invalid setLevel params: {}",
                            e
                        )),
                    )
                }
            },
            None => {
                return JsonRpcResponse::error(
                    id,
                    JsonRpcError::invalid_params("Missing setLevel params"),
                )
            }
        };

        info!(level = ?level_params.level, "MCP log level set");
        *self.log_level.write().await = level_params.level;

        JsonRpcResponse::success(id, json!({}))
    }

    /// Get the current MCP log level
    pub async fn current_log_level(&self) -> McpLogLevel {
        *self.log_level.read().await
    }
}
