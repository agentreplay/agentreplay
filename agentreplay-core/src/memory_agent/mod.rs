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

//! Memory Agent Orchestrator (Dual-LLM Architecture)
//!
//! This module implements the observer LLM agent for trace compression.
//! The memory agent is a separate LLM instance that observes the primary agent's
//! tool usage and generates structured observations.
//!
//! # Architecture
//!
//! The dual-LLM architecture consists of:
//! - **Primary Agent**: User's coding session (has tools, runs user prompts)
//! - **Memory Agent**: Separate LLM instance that observes and compresses (NO tools)
//!
//! # Features
//!
//! - Receives raw tool usage events from the primary agent
//! - Generates structured observations via specialized prompts
//! - Maintains conversation continuity via memory session ID
//! - Uses claim-and-delete message queue for reliability
//! - Supports crash recovery through pending message persistence
//!
//! # Example
//!
//! ```rust,ignore
//! use agentreplay_core::memory_agent::{MemoryAgent, MemoryAgentConfig};
//! use std::sync::Arc;
//!
//! let config = MemoryAgentConfig::default();
//! let agent = MemoryAgent::new(llm_client, config);
//!
//! // Process a tool usage event
//! let observation = agent.process_tool_event(tool_event).await?;
//! ```

mod agent;
mod config;
mod message;
mod session;

pub use agent::{MemoryAgent, MemoryAgentError, MemoryAgentStatus};
pub use config::MemoryAgentConfig;
pub use message::{Message, MessageRole, ConversationHistory};
pub use session::{MemorySession, SessionState};
