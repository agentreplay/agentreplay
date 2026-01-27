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

//! Plugin Hook Infrastructure for LLM Agent Events
//!
//! This module provides an event-driven hook system for intercepting agent lifecycle events.
//! It enables memory capture at agent lifecycle boundaries with support for:
//!
//! - **SessionStart**: Initialize memory session, prepare context injection
//! - **UserPromptSubmit**: Capture original user request, start memory agent
//! - **PostToolUse**: Observe tool inputs/outputs for compression
//! - **Stop**: Trigger session summarization
//!
//! # Architecture
//!
//! The hook system uses an Observer pattern with priority ordering:
//! - Event dispatch: O(k) where k = registered hooks per event
//! - Hook execution: async with configurable timeout
//! - Lower priority values execute first
//!
//! # Example
//!
//! ```rust,ignore
//! use flowtrace_plugins::hooks::{HookManager, HookConfig, AgentEvent, HookHandler};
//!
//! let config = HookConfig::from_json(r#"{
//!     "hooks": [
//!         {"event": "SessionStart", "command": "init_memory"},
//!         {"event": "Stop", "command": "summarize"}
//!     ]
//! }"#)?;
//!
//! let manager = HookManager::new(config);
//! manager.dispatch(AgentEvent::SessionStart { session_id: 123, project_id: 456 }).await?;
//! ```

mod config;
mod dispatcher;
mod events;
mod handlers;
mod registry;

pub use config::{HookConfig, HookDefinition, HookCommand};
pub use dispatcher::{HookDispatcher, DispatchResult, DispatchError};
pub use events::{AgentEvent, EventContext, ToolUsageData, SessionData};
pub use handlers::{HookHandler, HookResult, AsyncHookHandler, SyncHookHandler};
pub use registry::{HookRegistry, RegisteredHook, HookPriority};
