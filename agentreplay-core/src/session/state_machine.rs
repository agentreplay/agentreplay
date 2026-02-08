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

//! Session lifecycle state machine.

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Created,
    Initializing,
    Active,
    Processing,
    Summarizing,
    Completed,
    TimedOut,
    Failed,
}

impl SessionState {
    pub fn is_active(self) -> bool {
        matches!(self, SessionState::Active | SessionState::Processing | SessionState::Summarizing)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Initialize,
    Start,
    ToolEvent,
    ToolComplete,
    Stop,
    SummaryComplete,
    Timeout,
    Error,
}

#[derive(Debug, Error)]
#[error("Invalid transition: {current:?} -> {event:?}")]
pub struct InvalidTransition {
    pub current: SessionState,
    pub event: SessionEvent,
}

impl SessionState {
    pub fn transition(self, event: SessionEvent) -> Result<SessionState, InvalidTransition> {
        use SessionEvent::*;
        use SessionState::*;

        let next = match (self, event) {
            (Created, Initialize) => Initializing,
            (Initializing, Start) => Active,
            (Active, ToolEvent) => Processing,
            (Processing, ToolComplete) => Active,
            (Active, Stop) => Summarizing,
            (Summarizing, SummaryComplete) => Completed,
            (s, Timeout) if s.is_active() => TimedOut,
            (_, Error) => Failed,
            (TimedOut, Start) => Active,
            (Completed, Start) => Active,
            _ => {
                return Err(InvalidTransition {
                    current: self,
                    event,
                })
            }
        };

        Ok(next)
    }
}
