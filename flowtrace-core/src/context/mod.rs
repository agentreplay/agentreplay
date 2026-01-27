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
