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
