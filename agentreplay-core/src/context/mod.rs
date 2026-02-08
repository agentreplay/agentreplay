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

//! Context Builder for Session Injection
//!
//! This module provides context generation with token budget management
//! for injecting relevant historical context into new agent sessions.
//!
//! # Features
//!
//! - **Tiered detail**: Recent observations get full narrative; older get title+subtitle only
//! - **Token budget**: Configurable limit (default: 8000 tokens)
//! - **Timeline ordering**: Chronological with section headers
//!
//! # Token Budget Allocation
//!
//! - 60% for observations
//! - 30% for session summaries
//! - 10% for header/footer
//!
//! # Pruning Strategy
//!
//! Oldest observations first, preserve most recent N.

mod builder;
mod config;
mod token_calculator;

pub use builder::{ContextBuilder, ContextDocument};
pub use config::ContextConfig;
pub use token_calculator::TokenCalculator;
