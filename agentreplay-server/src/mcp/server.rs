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

//! MCP Server Implementation
//!
//! The main MCP server that handles client connections and dispatches requests.

use crate::api::AppState;
use crate::mcp::handlers::MCPHandler;
use crate::mcp::protocol::*;
use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
    routing::post,
    Json, Router,
};
use agentreplay_index::CausalIndex;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// MCP Server state
#[derive(Clone)]
pub struct MCPServerState {
    pub handler: Arc<MCPHandler>,
    pub connected_clients: Arc<RwLock<Vec<String>>>,
}

/// MCP Server
pub struct MCPServer {
    state: MCPServerState,
}

impl MCPServer {
    /// Create a new MCP server
    pub fn new(app_state: AppState, causal_index: Arc<CausalIndex>) -> Self {
        let handler = Arc::new(MCPHandler::new(app_state, causal_index));

        Self {
            state: MCPServerState {
                handler,
                connected_clients: Arc::new(RwLock::new(Vec::new())),
            },
        }
    }

    /// Get the Axum router for the MCP server
    pub fn router(&self) -> Router {
        Router::new()
            .route("/mcp", post(handle_mcp_request))
            .route("/mcp/health", axum::routing::get(handle_mcp_health))
            .route("/mcp/ws", axum::routing::get(handle_mcp_websocket))
            .route("/mcp/sse", axum::routing::get(handle_mcp_sse))
            .with_state(self.state.clone())
    }

    /// Get the server state (for embedding in main server)
    pub fn state(&self) -> MCPServerState {
        self.state.clone()
    }
}

/// Handle MCP health check (GET /mcp/health)
/// Returns MCP server status for monitoring
async fn handle_mcp_health(State(state): State<MCPServerState>) -> Json<serde_json::Value> {
    let clients = state.connected_clients.read().await;
    Json(serde_json::json!({
        "status": "ok",
        "protocol_version": MCP_PROTOCOL_VERSION,
        "server_name": "agentreplay-mcp",
        "server_version": env!("CARGO_PKG_VERSION"),
        "connected_clients": clients.len(),
        "capabilities": {
            "resources": true,
            "tools": true,
            "prompts": true,
            "logging": true
        }
    }))
}

/// Handle MCP JSON-RPC request over HTTP POST
async fn handle_mcp_request(
    State(state): State<MCPServerState>,
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let response = state.handler.handle_request(request).await;
    Json(response)
}

/// Handle MCP over WebSocket
async fn handle_mcp_websocket(
    State(state): State<MCPServerState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws_connection(state, socket))
}

/// Handle WebSocket connection
async fn handle_ws_connection(state: MCPServerState, mut socket: axum::extract::ws::WebSocket) {
    use axum::extract::ws::Message;

    let client_id = uuid::Uuid::new_v4().to_string();
    info!(client_id = %client_id, "MCP WebSocket client connected");

    // Track connected client
    {
        let mut clients = state.connected_clients.write().await;
        clients.push(client_id.clone());
    }

    // Handle messages
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Parse JSON-RPC request
                match serde_json::from_str::<JsonRpcRequest>(&text) {
                    Ok(request) => {
                        let response = state.handler.handle_request(request).await;
                        let response_text = serde_json::to_string(&response).unwrap_or_default();

                        if let Err(e) = socket.send(Message::Text(response_text)).await {
                            error!(error = %e, "Failed to send WebSocket response");
                            break;
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Invalid JSON-RPC request");
                        let error_response = JsonRpcResponse::error(
                            JsonRpcId::Null,
                            JsonRpcError::parse_error(format!("Invalid JSON: {}", e)),
                        );
                        let error_text = serde_json::to_string(&error_response).unwrap_or_default();
                        let _ = socket.send(Message::Text(error_text)).await;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!(client_id = %client_id, "MCP WebSocket client disconnected");
                break;
            }
            Ok(Message::Ping(data)) => {
                let _ = socket.send(Message::Pong(data)).await;
            }
            Err(e) => {
                error!(error = %e, "WebSocket error");
                break;
            }
            _ => {}
        }
    }

    // Remove client from tracking
    {
        let mut clients = state.connected_clients.write().await;
        clients.retain(|c| c != &client_id);
    }
}

/// Handle MCP over Server-Sent Events (SSE)
async fn handle_mcp_sse(
    State(state): State<MCPServerState>,
) -> axum::response::Sse<
    impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use futures::stream;
    use std::time::Duration;

    let client_id = uuid::Uuid::new_v4().to_string();
    info!(client_id = %client_id, "MCP SSE client connected");

    // Track connected client
    {
        let mut clients = state.connected_clients.write().await;
        clients.push(client_id.clone());
    }

    // Create initial event with server info
    let init_event = Event::default().event("init").data(
        serde_json::to_string(&serde_json::json!({
            "protocol_version": MCP_PROTOCOL_VERSION,
            "server_name": "agentreplay-mcp",
            "server_version": env!("CARGO_PKG_VERSION"),
        }))
        .unwrap_or_default(),
    );

    // Stream with keepalive
    let stream = stream::once(async move { Ok(init_event) });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(30)))
}

/// Batch request handling for multiple JSON-RPC requests
pub async fn handle_batch_request(
    state: &MCPServerState,
    requests: Vec<JsonRpcRequest>,
) -> Vec<JsonRpcResponse> {
    let mut responses = Vec::with_capacity(requests.len());

    for request in requests {
        let response = state.handler.handle_request(request).await;
        responses.push(response);
    }

    responses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jsonrpc_response_creation() {
        let success =
            JsonRpcResponse::success(JsonRpcId::Number(1), serde_json::json!({"result": "test"}));
        assert!(success.result.is_some());
        assert!(success.error.is_none());

        let error = JsonRpcResponse::error(
            JsonRpcId::String("test".to_string()),
            JsonRpcError::method_not_found("unknown"),
        );
        assert!(error.result.is_none());
        assert!(error.error.is_some());
    }
}
