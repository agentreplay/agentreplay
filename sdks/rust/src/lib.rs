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

//! # Agentreplay SDK for Rust
//!
//! High-performance observability SDK for LLM agents and AI applications.
//!
//! ## Quick Start
//!
//! ```no_run
//! use agentreplay::{AgentreplayClient, ClientConfig, SpanType, CreateTraceOptions};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client
//!     let config = ClientConfig::new("http://localhost:8080", 1)
//!         .with_project_id(0)
//!         .with_agent_id(1);
//!
//!     let client = AgentreplayClient::new(config);
//!
//!     // Create a trace
//!     let trace = client.create_trace(CreateTraceOptions {
//!         agent_id: 1,
//!         session_id: Some(123),
//!         span_type: SpanType::Root,
//!         ..Default::default()
//!     }).await?;
//!
//!     println!("Created trace: {}", trace.edge_id);
//!     Ok(())
//! }
//! ```
//!
//! ## Tracking LLM Calls
//!
//! ```no_run
//! use agentreplay::{AgentreplayClient, ClientConfig, CreateGenAITraceOptions, Message};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = AgentreplayClient::new(ClientConfig::new("http://localhost:8080", 1));
//!
//! let trace = client.create_genai_trace(CreateGenAITraceOptions {
//!     agent_id: 1,
//!     session_id: Some(123),
//!     model: Some("gpt-4o".into()),
//!     input_messages: vec![
//!         Message::system("You are a helpful assistant."),
//!         Message::user("What is the capital of France?"),
//!     ],
//!     output: Some(Message::assistant("The capital of France is Paris.")),
//!     input_usage: Some(25),
//!     output_usage: Some(12),
//!     total_usage: Some(37),
//!     finish_reason: Some("stop".into()),
//!     ..Default::default()
//! }).await?;
//!
//! println!("Created GenAI trace: {}", trace.edge_id);
//! # Ok(())
//! # }
//! ```

mod client;
mod types;

pub use client::{ClientConfig, Result, AgentreplayClient, AgentreplayError};
pub use types::*;
