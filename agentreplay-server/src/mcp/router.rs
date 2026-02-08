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

//! MCP Router
//!
//! Axum router configuration for MCP endpoints.

use crate::api::AppState;
use crate::mcp::server::MCPServer;
use axum::Router;
use agentreplay_index::CausalIndex;
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
