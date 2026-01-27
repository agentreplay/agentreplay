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
