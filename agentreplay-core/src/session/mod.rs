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
