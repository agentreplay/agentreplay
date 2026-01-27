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

//! MCP request/response handler.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use super::resource::{ContextRequest, McpContextResource};

/// MCP JSON-RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version.
    pub jsonrpc: String,
    /// Request ID.
    pub id: Option<Value>,
    /// Method name.
    pub method: String,
    /// Parameters.
    #[serde(default)]
    pub params: Value,
}

/// MCP JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON-RPC version.
    pub jsonrpc: String,
    /// Request ID.
    pub id: Option<Value>,
    /// Result (success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error (failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Additional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpResponse {
    /// Create a success response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// MCP context handler.
pub struct McpContextHandler {
    resource: Arc<McpContextResource>,
}

impl McpContextHandler {
    /// Create a new handler.
    pub fn new(resource: Arc<McpContextResource>) -> Self {
        Self { resource }
    }

    /// Handle an MCP request.
    pub fn handle(&self, request: McpRequest) -> McpResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "resources/list" => self.handle_list_resources(request.id),
            "resources/read" => self.handle_read_resource(request.id, request.params),
            "tools/list" => self.handle_list_tools(request.id),
            "tools/call" => self.handle_call_tool(request.id, request.params),
            _ => McpResponse::error(request.id, -32601, "Method not found"),
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> McpResponse {
        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "resources": {
                    "subscribe": false,
                    "listChanged": false
                },
                "tools": {}
            },
            "serverInfo": {
                "name": "flowtrace-context",
                "version": "0.1.0"
            }
        });

        McpResponse::success(id, result)
    }

    fn handle_list_resources(&self, id: Option<Value>) -> McpResponse {
        let result = serde_json::json!({
            "resources": [
                {
                    "uri": "flowtrace://context/current",
                    "name": "Current Project Context",
                    "description": self.resource.resource_description(),
                    "mimeType": self.resource.mime_type()
                }
            ]
        });

        McpResponse::success(id, result)
    }

    fn handle_read_resource(&self, id: Option<Value>, params: Value) -> McpResponse {
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Parse project ID from URI
        let project_id = if uri.starts_with("flowtrace://context/") {
            let id_str = &uri["flowtrace://context/".len()..];
            if id_str == "current" {
                // Use a default/current project
                0u128
            } else {
                u128::from_str_radix(id_str, 16).unwrap_or(0)
            }
        } else {
            return McpResponse::error(id, -32602, "Invalid resource URI");
        };

        let request = ContextRequest {
            project_id,
            session_id: None,
            max_observations: None,
            max_tokens: None,
            concepts: None,
            since: None,
            query: None,
        };

        let response = self.resource.build_context(&request);

        let result = serde_json::json!({
            "contents": [
                {
                    "uri": uri,
                    "mimeType": self.resource.mime_type(),
                    "text": response.context
                }
            ]
        });

        McpResponse::success(id, result)
    }

    fn handle_list_tools(&self, id: Option<Value>) -> McpResponse {
        let result = serde_json::json!({
            "tools": [
                {
                    "name": "get_observations",
                    "description": "Get recent observations for the current project",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "limit": {
                                "type": "number",
                                "description": "Maximum observations to return"
                            },
                            "concepts": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Filter by concepts"
                            }
                        }
                    }
                },
                {
                    "name": "search_observations",
                    "description": "Search observations by query",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Search query"
                            }
                        },
                        "required": ["query"]
                    }
                }
            ]
        });

        McpResponse::success(id, result)
    }

    fn handle_call_tool(&self, id: Option<Value>, params: Value) -> McpResponse {
        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match tool_name {
            "get_observations" => {
                let limit = params
                    .get("arguments")
                    .and_then(|a| a.get("limit"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(10) as usize;

                // Placeholder response
                let result = serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Would return {} recent observations", limit)
                        }
                    ]
                });

                McpResponse::success(id, result)
            }
            "search_observations" => {
                let query = params
                    .get("arguments")
                    .and_then(|a| a.get("query"))
                    .and_then(|q| q.as_str())
                    .unwrap_or("");

                // Placeholder response
                let result = serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Would search for: {}", query)
                        }
                    ]
                });

                McpResponse::success(id, result)
            }
            _ => McpResponse::error(id, -32602, "Unknown tool"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_handler() -> McpContextHandler {
        McpContextHandler::new(Arc::new(McpContextResource::default()))
    }

    #[test]
    fn test_initialize() {
        let handler = create_handler();
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "initialize".to_string(),
            params: Value::Null,
        };

        let response = handler.handle(request);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_list_resources() {
        let handler = create_handler();
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "resources/list".to_string(),
            params: Value::Null,
        };

        let response = handler.handle(request);
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        let resources = result.get("resources").unwrap().as_array().unwrap();
        assert!(!resources.is_empty());
    }

    #[test]
    fn test_unknown_method() {
        let handler = create_handler();
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "unknown/method".to_string(),
            params: Value::Null,
        };

        let response = handler.handle(request);
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }
}
