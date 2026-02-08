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
//! use agentreplay_plugins::hooks::{HookManager, HookConfig, AgentEvent, HookHandler};
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
