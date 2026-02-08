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

//! WASM Plugin Runtime
//!
//! Language-agnostic WASM runtime that loads and executes plugins compiled from
//! any language (Rust, Python, TypeScript, Go, etc.) using the WASM Component Model.
//!
//! The runtime provides:
//! - Plugin loading and instantiation
//! - Host function bindings (trace queries, HTTP, embeddings, etc.)
//! - Capability enforcement
//! - Resource limits (memory, fuel/instructions)
//! - Sandboxed execution

pub mod component;
pub mod executor;
pub mod host_functions;

pub use component::{LoadedPlugin, PluginInstance};
pub use executor::WasmExecutor;
