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

//! Model Context Protocol (MCP) Server Implementation
//!
//! This module implements an MCP server that exposes FlowtTrace as a "memory hub"
//! for AI tools like Claude Desktop and Cursor. It allows external agents to query
//! historical traces, solutions, and code snippets.
//!
//! ## Project Isolation
//!
//! MCP operates in a completely isolated context from LLM observability:
//! - **Tenant ID 2**: Dedicated tenant for MCP memory operations
//! - **Project ID 1000**: Auto-created "MCP Memory" project
//!
//! This ensures MCP's vector storage and RAG operations don't conflict
//! with agent tracing data (Tenant 1, Project 0-999).
//!
//! ## MCP Protocol Overview
//!
//! MCP (Model Context Protocol) is an open standard using JSON-RPC 2.0 for connecting
//! AI assistants to data systems. The protocol defines three core primitives:
//!
//! - **Resources**: Data sources that can be queried (trace collections)
//! - **Tools**: Actions that can be executed (search_traces, get_context)
//! - **Prompts**: Template prompts for common operations
//!
//! ## Multi-Signal Relevance Scoring
//!
//! Results are ranked using a composite score:
//!
//! ```text
//! FinalScore(t) = α·S_sem + β·S_time + γ·S_graph
//! ```
//!
//! Where:
//! - `S_sem` = Cosine similarity from HNSW vector search
//! - `S_time` = Temporal decay: e^(-λ(t_now - t_trace))
//! - `S_graph` = PageRank score from causal graph (influential traces)
//!
//! ## Usage
//!
//! The MCP server runs on a configurable port (default 47101) and accepts
//! JSON-RPC 2.0 messages over HTTP or WebSocket.
//!
//! ```rust,ignore
//! let mcp_server = MCPServer::new(app_state.clone());
//! mcp_server.run(47101).await?;
//! ```

pub mod context;
pub mod cache;
pub mod handler;
pub mod handlers;
pub mod protocol;
pub mod prompts;
pub mod relevance;
pub mod resource;
pub mod router;
pub mod server;
pub mod tools;
pub mod transport;

pub use context::{
    MCPCollection, MCPContext, MCPProjectInfo, MCP_DEFAULT_PROJECT_ID, MCP_TENANT_ID,
};
pub use cache::{CacheKey, ContextDocument, InvalidationEvent, ResourceCache};
pub use handler::{McpContextHandler, McpRequest, McpResponse};
pub use handlers::*;
pub use protocol::*;
pub use prompts::*;
pub use relevance::*;
pub use resource::{ContextRequest, ContextResourceConfig, ContextResponse, McpContextResource};
pub use router::mcp_router;
pub use server::MCPServer;
pub use tools::*;
pub use tools::registry::{McpTool, RegistrationError, ToolContext, ToolError, ToolListEntry, ToolRegistry, ToolResult};
pub use transport::{BufferTransport, McpTransport, SseTransport, StdioTransport, TransportError, WebSocketTransport};
