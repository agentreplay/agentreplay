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

//! Agentreplay Memory System
//!
//! A persistent memory system for LLM agents that provides:
//! - **Session Memory**: Store and retrieve observations from coding sessions
//! - **Context Compression**: Hierarchical summarization to keep context compact
//! - **Semantic Retrieval**: Find relevant context using embeddings + HNSW
//! - **Context Export**: Generate MDC files for injection into editors
//!
//! # Architecture
//!
//! The memory system follows an append-only event log model where each memory
//! write is a record keyed by (workspace_id, session_id, timestamp). This
//! integrates tightly with Agentreplay's tracing system - memory events appear
//! on the same timeline as trace spans.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Memory Engine                             │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
//! │  │ Observation │  │   Session   │  │     Context         │  │
//! │  │   Store     │  │  Summarizer │  │     Packer          │  │
//! │  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
//! │         │                │                     │             │
//! │  ┌──────▼────────────────▼─────────────────────▼──────────┐ │
//! │  │                  Memory Index                           │ │
//! │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │ │
//! │  │  │   HNSW      │  │  Temporal   │  │   Workspace     │  │ │
//! │  │  │  (semantic) │  │   Index     │  │    Index        │  │ │
//! │  │  └─────────────┘  └─────────────┘  └─────────────────┘  │ │
//! │  └─────────────────────────────────────────────────────────┘ │
//! │                              │                               │
//! │  ┌───────────────────────────▼───────────────────────────┐  │
//! │  │              Persistent Storage (LSM)                  │  │
//! │  └────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use agentreplay_memory::{MemoryEngine, MemoryConfig, Observation};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = MemoryConfig::default();
//!     let engine = MemoryEngine::new(config).await?;
//!     
//!     // Write an observation
//!     let obs = Observation::new("project-123", "session-456")
//!         .content("User prefers explicit error handling over Result")
//!         .category(ObservationCategory::Decision)
//!         .tags(vec!["error-handling", "style"]);
//!     engine.write_observation(obs).await?;
//!     
//!     // Retrieve context for a query
//!     let context = engine.retrieve_context("error handling", 5).await?;
//!     
//!     // Export context as MDC file
//!     let mdc = engine.export_mdc("project-123").await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod context;
pub mod engine;
pub mod error;
pub mod observation;
pub mod session;
pub mod storage;

// Re-exports
pub use config::MemoryConfig;
pub use context::{ContextPacker, ContextSection, ContextSpec, PackedContext};
pub use engine::MemoryEngine;
pub use error::{MemoryError, MemoryResult};
pub use observation::{Observation, ObservationCategory, ObservationId, ObservationQuery};
pub use session::{SessionId, SessionMemory, SessionSummary};
