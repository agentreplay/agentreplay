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

//! MCP Router
//!
//! Axum router configuration for MCP endpoints.

use crate::api::AppState;
use crate::mcp::server::MCPServer;
use axum::Router;
use flowtrace_index::CausalIndex;
use std::sync::Arc;

/// Create the MCP router with all endpoints
pub fn mcp_router(app_state: AppState, causal_index: Arc<CausalIndex>) -> Router {
    let mcp_server = MCPServer::new(app_state, causal_index);
    mcp_server.router()
}

/// MCP endpoint paths
pub mod paths {
    /// HTTP POST endpoint for JSON-RPC requests
    pub const MCP_HTTP: &str = "/mcp";
    /// Health check endpoint (GET)
    pub const MCP_HEALTH: &str = "/mcp/health";
    /// WebSocket endpoint for bidirectional communication
    pub const MCP_WS: &str = "/mcp/ws";
    /// SSE endpoint for server-push notifications
    pub const MCP_SSE: &str = "/mcp/sse";
}
