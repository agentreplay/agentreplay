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

//! Session Continuity Manager
//!
//! Cross-session state management for memory agent continuity.
//!
//! # Session IDs
//!
//! - `contentSessionId`: Agent's session ID (from hooks)
//! - `memorySessionId`: Memory agent's conversation ID (for multi-turn)
//!
//! # State Persistence
//!
//! Session state is persisted to enable:
//! - Worker restart recovery
//! - Session timeout handling
//! - Multi-turn observation generation

mod continuity;
mod state;
mod state_machine;

pub use continuity::{SessionContinuity, ContinuityManager, ContinuityConfig};
pub use state::{SessionStateStore, PersistedSessionState};
pub use state_machine::{InvalidTransition, SessionEvent, SessionState};
