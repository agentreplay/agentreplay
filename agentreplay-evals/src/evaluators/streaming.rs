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

//! Real-time evaluation streaming infrastructure

use crate::EvalResult;
use serde::Serialize;
use tokio::sync::broadcast;

/// Status of an ongoing evaluation
#[derive(Debug, Clone, Serialize)]
pub enum EvalStatus {
    Queued,
    Running { progress: f64 },
    Completed { result: Box<EvalResult> },
    Failed { error: String },
}

/// Update message for real-time streams
#[derive(Debug, Clone, Serialize)]
pub struct EvalUpdate {
    pub trace_id: u128,
    pub evaluator_id: String,
    pub status: EvalStatus,
    pub timestamp: u64,
}

/// Stream manager for real-time evaluation updates
pub struct LiveEvaluationStream {
    tx: broadcast::Sender<EvalUpdate>,
}

impl LiveEvaluationStream {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribe to updates
    pub fn subscribe(&self) -> broadcast::Receiver<EvalUpdate> {
        self.tx.subscribe()
    }

    /// Publish an update
    pub fn publish(
        &self,
        update: EvalUpdate,
    ) -> Result<usize, broadcast::error::SendError<EvalUpdate>> {
        self.tx.send(update)
    }
}

impl Default for LiveEvaluationStream {
    fn default() -> Self {
        Self::new(100)
    }
}
