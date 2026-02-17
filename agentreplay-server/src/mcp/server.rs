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
//! Supports Streamable HTTP (T1), Session Management (T3), and Concurrent Batch (T15).

use crate::api::AppState;
use crate::mcp::handlers::MCPHandler;
use crate::mcp::protocol::*;
use axum::{
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use agentreplay_index::CausalIndex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// MCP Server state
#[derive(Clone)]
pub struct MCPServerState {
    pub handler: Arc<MCPHandler>,
    pub connected_clients: Arc<RwLock<Vec<String>>>,
    /// Active sessions: session_id → session metadata (T3)
    pub sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
}

/// Session metadata (T3)
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub created_at: std::time::Instant,
    pub last_active: std::time::Instant,
    pub client_info: Option<String>,
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
                sessions: Arc::new(RwLock::new(HashMap::new())),
            },
        }
    }

    /// Get the Axum router for the MCP server
    pub fn router(&self) -> Router {
        Router::new()
            .route("/mcp", post(handle_mcp_request).delete(handle_mcp_session_terminate))
            .route("/mcp/health", axum::routing::get(handle_mcp_health))
            .route("/mcp/ws", axum::routing::get(handle_mcp_websocket))
            .route("/mcp/sse", axum::routing::get(handle_mcp_sse))
            .route(
                "/.well-known/oauth-authorization-server",
                axum::routing::get(handle_oauth_authorization_server_metadata),
            )
            .route(
                "/.well-known/openid-configuration",
                axum::routing::get(handle_openid_configuration),
            )
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

/// OAuth 2.1 Authorization Server Metadata (RFC 8414)
async fn handle_oauth_authorization_server_metadata() -> Json<serde_json::Value> {
    let issuer = std::env::var("AGENTREPLAY_OAUTH_ISSUER")
        .unwrap_or_else(|_| "http://127.0.0.1:47101".to_string());
    let token_endpoint = std::env::var("AGENTREPLAY_OAUTH_TOKEN_ENDPOINT")
        .unwrap_or_else(|_| format!("{}/oauth/token", issuer));
    let jwks_uri = std::env::var("AGENTREPLAY_OAUTH_JWKS_URI")
        .unwrap_or_else(|_| format!("{}/.well-known/jwks.json", issuer));

    Json(serde_json::json!({
        "issuer": issuer,
        "token_endpoint": token_endpoint,
        "jwks_uri": jwks_uri,
        "response_types_supported": ["token"],
        "grant_types_supported": ["authorization_code", "refresh_token", "client_credentials"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "private_key_jwt"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["mcp:read", "mcp:write", "replay:read", "replay:fork"]
    }))
}

/// OpenID Connect Discovery metadata
async fn handle_openid_configuration() -> Json<serde_json::Value> {
    let issuer = std::env::var("AGENTREPLAY_OAUTH_ISSUER")
        .unwrap_or_else(|_| "http://127.0.0.1:47101".to_string());
    let token_endpoint = std::env::var("AGENTREPLAY_OAUTH_TOKEN_ENDPOINT")
        .unwrap_or_else(|_| format!("{}/oauth/token", issuer));
    let jwks_uri = std::env::var("AGENTREPLAY_OAUTH_JWKS_URI")
        .unwrap_or_else(|_| format!("{}/.well-known/jwks.json", issuer));

    Json(serde_json::json!({
        "issuer": issuer,
        "token_endpoint": token_endpoint,
        "jwks_uri": jwks_uri,
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["HS256", "RS256"],
        "scopes_supported": ["openid", "profile", "mcp:read", "mcp:write", "replay:read", "replay:fork"],
        "claims_supported": ["sub", "tenant_id", "project_id", "exp", "iat", "iss", "aud", "scope"]
    }))
}

/// Handle MCP JSON-RPC request over HTTP POST
/// Supports single requests, batch requests (T15), and session management (T3)
async fn handle_mcp_request(
    State(state): State<MCPServerState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    // Session management (T3): validate or create session
    let session_id = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let active_session_id = if let Some(sid) = session_id {
        // Validate existing session
        let sessions = state.sessions.read().await;
        if !sessions.contains_key(&sid) {
            return (
                StatusCode::NOT_FOUND,
                [(MCP_SESSION_ID_HEADER, "")],
                Json(JsonRpcResponse::error(
                    JsonRpcId::Null,
                    JsonRpcError::internal_error("Session not found or expired"),
                )),
            )
                .into_response();
        }
        drop(sessions);
        // Update last active
        if let Some(session) = state.sessions.write().await.get_mut(&sid) {
            session.last_active = std::time::Instant::now();
        }
        sid
    } else {
        // Create new session for initialize requests
        let sid = uuid::Uuid::new_v4().to_string();
        state.sessions.write().await.insert(
            sid.clone(),
            SessionInfo {
                created_at: std::time::Instant::now(),
                last_active: std::time::Instant::now(),
                client_info: None,
            },
        );
        sid
    };

    // Try to parse as batch (array) or single request
    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(JsonRpcResponse::error(
                    JsonRpcId::Null,
                    JsonRpcError::parse_error("Invalid UTF-8"),
                )),
            )
                .into_response();
        }
    };

    let trimmed = body_str.trim();
    if trimmed.starts_with('[') {
        // Batch request (T15) — process concurrently
        match serde_json::from_str::<Vec<JsonRpcRequest>>(trimmed) {
            Ok(requests) => {
                let responses = handle_batch_request(&state, requests).await;
                (
                    StatusCode::OK,
                    [(MCP_SESSION_ID_HEADER, active_session_id.as_str())],
                    Json(responses),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(JsonRpcResponse::error(
                    JsonRpcId::Null,
                    JsonRpcError::parse_error(format!("Invalid batch JSON: {}", e)),
                )),
            )
                .into_response(),
        }
    } else {
        // Single request
        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(request) => {
                let response = state.handler.handle_request(request).await;
                (
                    StatusCode::OK,
                    [(MCP_SESSION_ID_HEADER, active_session_id.as_str())],
                    Json(response),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(JsonRpcResponse::error(
                    JsonRpcId::Null,
                    JsonRpcError::parse_error(format!("Invalid JSON: {}", e)),
                )),
            )
                .into_response(),
        }
    }
}

/// Handle session termination via DELETE /mcp (T3)
async fn handle_mcp_session_terminate(
    State(state): State<MCPServerState>,
    headers: HeaderMap,
) -> Response {
    let session_id = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(sid) = session_id {
        let removed = state.sessions.write().await.remove(&sid).is_some();
        if removed {
            info!(session_id = %sid, "MCP session terminated");
            StatusCode::OK.into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    } else {
        StatusCode::BAD_REQUEST.into_response()
    }
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

/// Batch request handling with concurrent processing (T15)
/// Uses tokio::JoinSet to process requests concurrently
pub async fn handle_batch_request(
    state: &MCPServerState,
    requests: Vec<JsonRpcRequest>,
) -> Vec<JsonRpcResponse> {
    use tokio::task::JoinSet;

    let mut join_set = JoinSet::new();

    for (idx, request) in requests.into_iter().enumerate() {
        let handler = state.handler.clone();
        join_set.spawn(async move {
            let response = handler.handle_request(request).await;
            (idx, response)
        });
    }

    let mut indexed_responses = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((idx, response)) => indexed_responses.push((idx, response)),
            Err(e) => {
                error!(error = %e, "Batch request task panicked");
                indexed_responses.push((
                    usize::MAX,
                    JsonRpcResponse::error(
                        JsonRpcId::Null,
                        JsonRpcError::internal_error("Request processing failed"),
                    ),
                ));
            }
        }
    }

    // Sort by original order
    indexed_responses.sort_by_key(|(idx, _)| *idx);
    indexed_responses.into_iter().map(|(_, r)| r).collect()
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
