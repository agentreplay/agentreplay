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
//! use flowtrace_core::memory_agent::{MemoryAgent, MemoryAgentConfig};
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
