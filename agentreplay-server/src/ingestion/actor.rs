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

//! Ingestion Actor - Server-Side Batching for Trace Processing
//!
//! Implements a dedicated actor that collects incoming traces and processes
//! them in batches, maximizing throughput through the sharded governor.
//!
//! ## Architecture
//!
//! ```text
//! HTTP Requests ─┬─► IngestionActor ─► Batch Embeddings ─► ShardedGovernor
//!                │                              │                 │
//!                │        (N items or T ms)     │                 │
//!                └──────────────────────────────┴─────────────────▼
//!                                                              Storage
//! ```
//!
//! ## Benefits
//!
//! 1. **Amortized Embedding Cost**: Batch embedding is 10-30x faster than sequential
//! 2. **Parallel Governor Processing**: Batch goes to 16 shards in parallel
//! 3. **Backpressure**: Channel capacity limits memory under load
//! 4. **Graceful Degradation**: Partial batches flush after timeout

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info};

use crate::governor::{GovernorDecision, ShardedGovernor};

/// Configuration for the Ingestion Actor.
#[derive(Debug, Clone)]
pub struct IngestionConfig {
    /// Maximum batch size before processing
    pub max_batch_size: usize,
    /// Maximum wait time before processing a partial batch
    pub max_wait_time: Duration,
    /// Channel buffer size (backpressure control)
    pub channel_capacity: usize,
    /// Embedding dimension
    pub embedding_dimension: usize,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 64,
            max_wait_time: Duration::from_millis(20),
            channel_capacity: 4096,
            embedding_dimension: 384,
        }
    }
}

/// A trace to be ingested.
#[derive(Debug)]
pub struct TracePayload {
    /// Unique trace ID
    pub trace_id: u128,
    /// Text content for embedding (prompt + completion)
    pub text: String,
    /// Full payload for storage
    pub payload: serde_json::Value,
}

/// Result of ingestion processing.
#[derive(Debug, Clone)]
pub enum IngestionResult {
    /// Trace was stored successfully (includes embedding for vector index)
    Stored {
        trace_id: u128,
        /// The embedding vector to store in the vector index for semantic search
        embedding: Vec<f32>,
    },
    /// Trace was deduplicated
    Deduplicated {
        trace_id: u128,
        similar_to: u128,
        similarity: f32,
    },
    /// Processing failed
    Failed { trace_id: u128, error: String },
}

/// Internal message for the actor.
struct IngestMessage {
    payload: TracePayload,
    response: oneshot::Sender<IngestionResult>,
}

/// Statistics for the ingestion actor.
#[derive(Debug, Clone, Default)]
pub struct IngestionStats {
    pub total_received: u64,
    pub total_stored: u64,
    pub total_deduplicated: u64,
    pub total_failed: u64,
    pub total_batches: u64,
    pub avg_batch_size: f64,
    pub avg_batch_latency_ms: f64,
    pub throughput: f64,
}

/// Handle to interact with the Ingestion Actor.
pub struct IngestionActorHandle {
    sender: mpsc::Sender<IngestMessage>,
    stats: Arc<IngestionStatsInternal>,
}

impl Clone for IngestionActorHandle {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            stats: self.stats.clone(),
        }
    }
}

struct IngestionStatsInternal {
    received: AtomicU64,
    stored: AtomicU64,
    deduplicated: AtomicU64,
    failed: AtomicU64,
    batches: AtomicU64,
    total_batch_size: AtomicU64,
    total_latency_ns: AtomicU64,
    start_time: Instant,
}

impl IngestionStatsInternal {
    fn new() -> Self {
        Self {
            received: AtomicU64::new(0),
            stored: AtomicU64::new(0),
            deduplicated: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            batches: AtomicU64::new(0),
            total_batch_size: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    fn snapshot(&self) -> IngestionStats {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let batches = self.batches.load(Ordering::Relaxed);
        let total_batch_size = self.total_batch_size.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ns.load(Ordering::Relaxed);
        let stored = self.stored.load(Ordering::Relaxed);
        let deduplicated = self.deduplicated.load(Ordering::Relaxed);

        IngestionStats {
            total_received: self.received.load(Ordering::Relaxed),
            total_stored: stored,
            total_deduplicated: deduplicated,
            total_failed: self.failed.load(Ordering::Relaxed),
            total_batches: batches,
            avg_batch_size: if batches > 0 {
                total_batch_size as f64 / batches as f64
            } else {
                0.0
            },
            avg_batch_latency_ms: if batches > 0 {
                (total_latency as f64 / batches as f64) / 1_000_000.0
            } else {
                0.0
            },
            throughput: if elapsed > 0.0 {
                (stored + deduplicated) as f64 / elapsed
            } else {
                0.0
            },
        }
    }
}

impl IngestionActorHandle {
    /// Submit a trace for ingestion.
    pub async fn ingest(&self, payload: TracePayload) -> Result<IngestionResult, String> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(IngestMessage {
                payload,
                response: tx,
            })
            .await
            .map_err(|_| "Ingestion actor channel closed".to_string())?;

        rx.await.map_err(|_| "Response channel dropped".to_string())
    }

    /// Submit multiple traces for ingestion.
    pub async fn ingest_many(
        &self,
        payloads: Vec<TracePayload>,
    ) -> Vec<Result<IngestionResult, String>> {
        let futures: Vec<_> = payloads.into_iter().map(|p| self.ingest(p)).collect();

        futures::future::join_all(futures).await
    }

    /// Get current statistics.
    pub fn stats(&self) -> IngestionStats {
        self.stats.snapshot()
    }
}

/// The Ingestion Actor - runs as a background task.
pub struct IngestionActor {
    config: IngestionConfig,
    governor: Arc<ShardedGovernor>,
    #[allow(dead_code)]
    embedder: Option<Arc<dyn EmbeddingProvider>>,
}

/// Trait for embedding providers (simplified interface).
pub trait EmbeddingProvider: Send + Sync {
    /// Embed a batch of texts.
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;

    /// Get embedding dimension.
    fn dimension(&self) -> usize;
}

impl IngestionActor {
    /// Create a new Ingestion Actor.
    pub fn new(
        config: IngestionConfig,
        governor: Arc<ShardedGovernor>,
        embedder: Option<Arc<dyn EmbeddingProvider>>,
    ) -> Self {
        Self {
            config,
            governor,
            embedder,
        }
    }

    /// Spawn the actor and return a handle for communication.
    pub fn spawn(self) -> IngestionActorHandle {
        let (sender, receiver) = mpsc::channel(self.config.channel_capacity);
        let stats = Arc::new(IngestionStatsInternal::new());

        // Spawn the actor task
        let actor_stats = stats.clone();
        tokio::spawn(async move {
            self.run(receiver, actor_stats).await;
        });

        IngestionActorHandle { sender, stats }
    }

    /// Main actor loop.
    async fn run(
        self,
        mut receiver: mpsc::Receiver<IngestMessage>,
        stats: Arc<IngestionStatsInternal>,
    ) {
        info!(
            "Ingestion actor started with batch_size={}, wait_time={}ms",
            self.config.max_batch_size,
            self.config.max_wait_time.as_millis()
        );

        let mut batch: Vec<IngestMessage> = Vec::with_capacity(self.config.max_batch_size);
        let mut batch_start = Instant::now();

        loop {
            // Calculate remaining wait time for this batch
            let elapsed = batch_start.elapsed();
            let remaining = self.config.max_wait_time.saturating_sub(elapsed);

            let msg = if batch.is_empty() {
                // No batch in progress, wait indefinitely for first message
                receiver.recv().await
            } else if remaining.is_zero() {
                // Timeout expired, process current batch
                None
            } else {
                // Wait for next message or timeout
                tokio::time::timeout(remaining, receiver.recv())
                    .await
                    .ok()
                    .flatten()
            };

            match msg {
                Some(message) => {
                    if batch.is_empty() {
                        batch_start = Instant::now();
                    }

                    stats.received.fetch_add(1, Ordering::Relaxed);
                    batch.push(message);

                    // Process batch if full
                    if batch.len() >= self.config.max_batch_size {
                        self.process_batch(&mut batch, &stats).await;
                        batch.clear();
                    }
                }
                None if !batch.is_empty() => {
                    // Channel closed or timeout with pending batch
                    self.process_batch(&mut batch, &stats).await;
                    batch.clear();
                }
                None => {
                    // Channel closed with no pending batch
                    info!("Ingestion actor shutting down");
                    break;
                }
            }
        }
    }

    /// Process a batch of messages.
    async fn process_batch(
        &self,
        batch: &mut Vec<IngestMessage>,
        stats: &Arc<IngestionStatsInternal>,
    ) {
        let batch_size = batch.len();
        let start = Instant::now();

        debug!("Processing batch of {} traces", batch_size);

        // Step 1: Generate embeddings for all traces
        let embeddings = self.generate_embeddings(batch).await;

        // Step 2: Process through governor in parallel
        let items: Vec<(u128, Vec<f32>)> = batch
            .iter()
            .zip(embeddings.iter())
            .filter_map(|(msg, emb)| emb.as_ref().ok().map(|e| (msg.payload.trace_id, e.clone())))
            .collect();

        let decisions = self.governor.process_batch(items).await;

        // Step 3: Send results back to callers
        let mut decision_iter = decisions.into_iter();

        for (msg, emb_result) in batch.drain(..).zip(embeddings.into_iter()) {
            let result = match emb_result {
                Err(e) => {
                    stats.failed.fetch_add(1, Ordering::Relaxed);
                    IngestionResult::Failed {
                        trace_id: msg.payload.trace_id,
                        error: e,
                    }
                }
                Ok(embedding) => {
                    match decision_iter.next() {
                        Some(GovernorDecision::Store { trace_id }) => {
                            stats.stored.fetch_add(1, Ordering::Relaxed);
                            // Return embedding for storage in vector index
                            IngestionResult::Stored {
                                trace_id,
                                embedding,
                            }
                        }
                        Some(GovernorDecision::Drop {
                            similar_to,
                            similarity,
                            ..
                        }) => {
                            stats.deduplicated.fetch_add(1, Ordering::Relaxed);
                            IngestionResult::Deduplicated {
                                trace_id: msg.payload.trace_id,
                                similar_to,
                                similarity,
                            }
                        }
                        None => {
                            stats.failed.fetch_add(1, Ordering::Relaxed);
                            IngestionResult::Failed {
                                trace_id: msg.payload.trace_id,
                                error: "No governor decision".to_string(),
                            }
                        }
                    }
                }
            };

            // Send result (ignore if receiver dropped)
            let _ = msg.response.send(result);
        }

        // Update stats
        let latency = start.elapsed();
        stats.batches.fetch_add(1, Ordering::Relaxed);
        stats
            .total_batch_size
            .fetch_add(batch_size as u64, Ordering::Relaxed);
        stats
            .total_latency_ns
            .fetch_add(latency.as_nanos() as u64, Ordering::Relaxed);

        debug!("Batch processed: {} traces in {:?}", batch_size, latency);
    }

    /// Generate embeddings for a batch of messages.
    async fn generate_embeddings(&self, batch: &[IngestMessage]) -> Vec<Result<Vec<f32>, String>> {
        if let Some(embedder) = &self.embedder {
            // Use real embedder if available
            let texts: Vec<String> = batch.iter().map(|m| m.payload.text.clone()).collect();

            match embedder.embed_batch(&texts) {
                Ok(embeddings) => embeddings.into_iter().map(Ok).collect(),
                Err(e) => batch.iter().map(|_| Err(e.clone())).collect(),
            }
        } else {
            // Generate deterministic fake embeddings for testing
            batch
                .iter()
                .map(|msg| {
                    let dim = self.config.embedding_dimension;
                    let embedding = generate_fake_embedding(&msg.payload.text, dim);
                    Ok(embedding)
                })
                .collect()
        }
    }
}

/// Generate a deterministic fake embedding based on text hash.
fn generate_fake_embedding(text: &str, dimension: usize) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let seed = hasher.finish();

    let mut embedding = Vec::with_capacity(dimension);
    let mut state = seed;

    for _ in 0..dimension {
        state = state.wrapping_mul(0x5851f42d4c957f2d).wrapping_add(1);
        let val = (state as f32 / u64::MAX as f32) * 2.0 - 1.0;
        embedding.push(val);
    }

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }

    embedding
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governor::GovernorConfig;

    #[tokio::test]
    async fn test_ingestion_actor_basic() {
        let governor_config = GovernorConfig {
            epsilon: 0.1,
            dimension: 128,
            ..Default::default()
        };
        let governor = ShardedGovernor::new_shared(governor_config);

        let actor_config = IngestionConfig {
            max_batch_size: 4,
            max_wait_time: Duration::from_millis(100),
            channel_capacity: 100,
            embedding_dimension: 128,
        };

        let actor = IngestionActor::new(actor_config, governor, None);
        let handle = actor.spawn();

        // Submit some traces
        let payloads: Vec<_> = (0..10)
            .map(|i| TracePayload {
                trace_id: i as u128,
                text: format!("Test trace {}", i),
                payload: serde_json::json!({"test": i}),
            })
            .collect();

        let results = handle.ingest_many(payloads).await;

        // All should succeed
        assert_eq!(results.len(), 10);
        for result in &results {
            assert!(result.is_ok());
        }

        // Check stats
        let stats = handle.stats();
        assert_eq!(stats.total_received, 10);
        assert!(stats.total_stored + stats.total_deduplicated == 10);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let governor_config = GovernorConfig {
            epsilon: 0.1,
            dimension: 128,
            ..Default::default()
        };
        let governor = ShardedGovernor::new_shared(governor_config);

        let actor_config = IngestionConfig {
            max_batch_size: 2,
            max_wait_time: Duration::from_millis(10),
            channel_capacity: 100,
            embedding_dimension: 128,
        };

        let actor = IngestionActor::new(actor_config, governor, None);
        let handle = actor.spawn();

        // Submit the same text twice
        let payload1 = TracePayload {
            trace_id: 1,
            text: "Same content here".to_string(),
            payload: serde_json::json!({}),
        };
        let payload2 = TracePayload {
            trace_id: 2,
            text: "Same content here".to_string(),
            payload: serde_json::json!({}),
        };

        let result1 = handle.ingest(payload1).await.unwrap();
        let result2 = handle.ingest(payload2).await.unwrap();

        // First should be stored, second deduplicated
        assert!(matches!(result1, IngestionResult::Stored { .. }));
        assert!(matches!(result2, IngestionResult::Deduplicated { .. }));
    }

    #[tokio::test]
    async fn test_batch_timeout() {
        let governor_config = GovernorConfig {
            epsilon: 0.1,
            dimension: 128,
            ..Default::default()
        };
        let governor = ShardedGovernor::new_shared(governor_config);

        let actor_config = IngestionConfig {
            max_batch_size: 100,                      // High batch size
            max_wait_time: Duration::from_millis(50), // Short timeout
            channel_capacity: 100,
            embedding_dimension: 128,
        };

        let actor = IngestionActor::new(actor_config, governor, None);
        let handle = actor.spawn();

        // Submit just one trace (won't fill batch)
        let payload = TracePayload {
            trace_id: 1,
            text: "Single trace".to_string(),
            payload: serde_json::json!({}),
        };

        let start = Instant::now();
        let result = handle.ingest(payload).await;
        let elapsed = start.elapsed();

        // Should complete within timeout + some buffer
        assert!(result.is_ok());
        assert!(elapsed < Duration::from_millis(200));

        // Stats should show one batch
        let stats = handle.stats();
        assert_eq!(stats.total_batches, 1);
    }
}
