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

//! Unified Tool Registry Service
//!
//! Provides a centralized registry for tool definitions supporting:
//! - MCP (Model Context Protocol) tools
//! - Native REST API tools
//! - Custom function tools
//! - Semantic versioning with constraint matching
//! - Hierarchical rate limiting
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                   Tool Registry Service                  │
//! ├─────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
//! │  │   Native    │  │     MCP     │  │      REST       │  │
//! │  │  Handlers   │  │   Adapter   │  │    Executor     │  │
//! │  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘  │
//! │         │                │                  │           │
//! │         └────────────────┼──────────────────┘           │
//! │                          ▼                              │
//! │              ┌───────────────────────┐                  │
//! │              │   UnifiedRegistry     │                  │
//! │              │   (DashMap-backed)    │                  │
//! │              └───────────────────────┘                  │
//! │                          │                              │
//! │              ┌───────────▼───────────┐                  │
//! │              │   ToolStore (Storage) │                  │
//! │              └───────────────────────┘                  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Design Decisions
//!
//! 1. **Flattened DashMap Keys**: Uses `(namespace, name, version)` composite keys
//!    instead of nested maps to avoid lock contention issues.
//!
//! 2. **Version Resolution**: Maintains a separate latest-version index for O(1)
//!    lookups when no version constraint is specified.
//!
//! 3. **Rate Limiting**: Implements token bucket algorithm with hierarchical limits
//!    (global → per-kind → per-tool).

mod executor;
mod mcp_adapter;
mod native_handlers;
mod registry;

pub use executor::{ToolExecutor, ToolExecutorConfig};
pub use mcp_adapter::McpToolAdapter;
pub use native_handlers::{NativeHandler, NativeHandlerRegistry};
pub use registry::{ToolLookupResult, ToolRegistry, ToolRegistryConfig, ToolRegistryError};
