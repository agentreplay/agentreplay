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

//! Tool event batching with deadline-driven flush.

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::Instant;
use tokio_util::time::DelayQueue;
use tokio_stream::StreamExt;

/// Batcher configuration.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Max time window for batching.
    pub max_window: Duration,
    /// Maximum number of events in a batch.
    pub max_batch_size: usize,
    /// Channel buffer for flush commands.
    pub flush_buffer: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_window: Duration::from_secs(2),
            max_batch_size: 10,
            flush_buffer: 256,
        }
    }
}

/// Flush commands emitted by the batcher.
#[derive(Debug, Clone)]
pub enum FlushCommand {
    /// Deadline elapsed for a session batch.
    SessionTimeout(u128),
    /// Force immediate flush (size or explicit).
    Immediate(u128),
}

struct BatchState<T> {
    events: Vec<T>,
    first_event_at: Instant,
    last_event_at: Instant,
}

/// Event batcher with bounded delivery latency.
pub struct EventBatcher<T> {
    pending: std::sync::Arc<RwLock<HashMap<u128, BatchState<T>>>>,
    flush_tx: mpsc::Sender<FlushCommand>,
    config: BatchConfig,
}

impl<T: Send + 'static> EventBatcher<T> {
    /// Create a new batcher and its background timer loop.
    pub fn new(config: BatchConfig) -> (Self, impl std::future::Future<Output = ()>) {
        let pending = std::sync::Arc::new(RwLock::new(HashMap::new()));
        let (flush_tx, flush_rx) = mpsc::channel(config.flush_buffer);

        let loop_pending = std::sync::Arc::clone(&pending);
        let loop_config = config.clone();
        let timer_loop = async move {
            Self::timer_loop(loop_pending, flush_rx, loop_config).await;
        };

        (
            Self {
                pending,
                flush_tx,
                config,
            },
            timer_loop,
        )
    }

    /// Add an event to a session batch.
    ///
    /// Returns a batch when flush conditions are met.
    pub async fn add_event(&self, session_id: u128, event: T) -> Option<Vec<T>> {
        let mut pending = self.pending.write().await;
        let now = Instant::now();

        let state = pending.entry(session_id).or_insert_with(|| {
            let _ = self.flush_tx.try_send(FlushCommand::SessionTimeout(session_id));
            BatchState {
                events: Vec::with_capacity(self.config.max_batch_size),
                first_event_at: now,
                last_event_at: now,
            }
        });

        state.events.push(event);
        state.last_event_at = now;

        let size_exceeded = state.events.len() >= self.config.max_batch_size;
        let deadline_exceeded = state.first_event_at.elapsed() >= self.config.max_window;

        if size_exceeded || deadline_exceeded {
            Some(std::mem::take(&mut state.events))
        } else {
            None
        }
    }

    async fn timer_loop(
        pending: std::sync::Arc<RwLock<HashMap<u128, BatchState<T>>>>,
        mut flush_rx: mpsc::Receiver<FlushCommand>,
        config: BatchConfig,
    ) {
        let mut deadlines: DelayQueue<u128> = DelayQueue::new();
        let mut keys: HashMap<u128, tokio_util::time::delay_queue::Key> = HashMap::new();

        loop {
            tokio::select! {
                Some(cmd) = flush_rx.recv() => {
                    match cmd {
                        FlushCommand::SessionTimeout(session_id) => {
                            let key = deadlines.insert(session_id, config.max_window);
                            keys.insert(session_id, key);
                        }
                        FlushCommand::Immediate(session_id) => {
                            Self::flush_session(&pending, session_id).await;
                        }
                    }
                }
                Some(expired) = deadlines.next() => {
                    let session_id = expired.into_inner();
                    keys.remove(&session_id);
                    let _ = Self::flush_session(&pending, session_id).await;
                }
            }
        }
    }

    async fn flush_session(
        pending: &RwLock<HashMap<u128, BatchState<T>>>,
        session_id: u128,
    ) -> Option<Vec<T>> {
        let mut guard = pending.write().await;
        let state = guard.remove(&session_id)?;
        if state.events.is_empty() {
            return None;
        }
        Some(state.events)
    }

    /// Drain all pending batches and return them for processing.
    pub async fn shutdown(&self) -> Vec<(u128, Vec<T>)> {
        let session_ids: Vec<u128> = {
            let guard = self.pending.read().await;
            guard.keys().copied().collect()
        };

        let mut drained = Vec::new();
        for session_id in session_ids {
            if let Some(events) = Self::flush_session(&self.pending, session_id).await {
                drained.push((session_id, events));
            }
        }
        drained
    }
}
