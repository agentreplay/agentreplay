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
