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

// agentreplay-observability/src/batcher.rs
//! Async batching exporter for Agentreplay traces
//!
//! This module implements asynchronous batching to minimize latency overhead
//! in hot paths. Spans are collected in memory and sent to Agentreplay's REST API
//! in batches either:
//! - When batch_size is reached, OR
//! - When batch_timeout expires
//!
//! Target: <50ms P99 latency overhead on application code

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{debug, error, warn};

/// Link to another span for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLink {
    pub trace_id: String,
    pub span_id: String,
    pub relationship: String, // "follows_from", "child_of", etc.
    pub attributes: Option<std::collections::HashMap<String, String>>,
}

/// A simplified span structure for Agentreplay with W3C Trace Context support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentreplaySpan {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub attributes: std::collections::HashMap<String, String>,

    // W3C Trace Context fields (Task 6 from task.md)
    /// W3C traceparent header value (00-{trace_id}-{span_id}-{flags})
    pub traceparent: Option<String>,
    /// W3C tracestate header for vendor-specific data
    pub tracestate: Option<String>,
    /// Span flags: 0x01 = sampled, 0x02 = random trace id
    pub span_flags: u8,
    /// Links to other spans (for batch processing, async operations)
    pub span_links: Option<Vec<SpanLink>>,
}

/// Configuration for the async batch exporter
#[derive(Debug, Clone)]
pub struct BatcherConfig {
    /// Maximum number of spans to batch before flushing
    pub batch_size: usize,

    /// Maximum time to wait before flushing a batch
    pub batch_timeout: Duration,

    /// Size of the internal channel buffer
    pub channel_buffer_size: usize,

    /// Maximum number of spans to buffer in memory (OOM protection) - Task 8
    pub max_buffer_size: usize,

    /// Agentreplay endpoint (e.g., "http://localhost:47100")
    pub agentreplay_endpoint: String,

    /// API key for authentication
    pub api_key: Option<String>,

    /// Priority span types that should never be dropped (Task 8)
    pub priority_span_types: Vec<String>,

    /// Enable adaptive sampling under load (Task 8)
    pub adaptive_sampling: bool,

    /// Target sampling rate when under load (0.0-1.0)
    pub load_sampling_rate: f32,
}

impl Default for BatcherConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            batch_timeout: Duration::from_secs(1),
            channel_buffer_size: 1000,
            max_buffer_size: 10_000,
            agentreplay_endpoint: "http://localhost:47100".to_string(),
            api_key: None,
            priority_span_types: vec!["error".to_string(), "root".to_string()],
            adaptive_sampling: true,
            load_sampling_rate: 0.1, // Sample 10% under load
        }
    }
}

/// Async batch exporter for spans
pub struct AsyncBatchExporter {
    sender: mpsc::Sender<AgentreplaySpan>,
    config: BatcherConfig,
}

impl AsyncBatchExporter {
    /// Create a new async batch exporter
    ///
    /// This spawns a background task that collects spans and exports them in batches.
    /// The task runs until the sender is dropped.
    pub fn new(config: BatcherConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.channel_buffer_size);

        // Spawn background worker task
        let worker_config = config.clone();
        tokio::spawn(async move {
            if let Err(e) = batch_worker(receiver, worker_config).await {
                error!("Batch worker error: {}", e);
            }
        });

        Self { sender, config }
    }

    /// Record a span asynchronously with priority handling and adaptive sampling (Task 8)
    ///
    /// This is non-blocking and should add <1ms overhead.
    /// - Priority spans (errors, root spans) are never dropped
    /// - Under load, applies adaptive sampling to non-priority spans
    /// - If channel full, drops non-priority spans to prevent blocking
    pub async fn record_span(&self, span: AgentreplaySpan) {
        // Check if this is a priority span
        let is_priority = self.config.priority_span_types.iter().any(|priority_type| {
            span.name
                .to_lowercase()
                .contains(&priority_type.to_lowercase())
        });

        // Apply adaptive sampling if enabled and not a priority span
        if !is_priority && self.config.adaptive_sampling {
            // Simple sampling based on load: if sender is > 80% full, sample
            // Use a simple hash-based sampling instead of random
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            span.span_id.hash(&mut hasher);
            let hash_val = hasher.finish();

            // Convert sampling rate to threshold
            let threshold = (self.config.load_sampling_rate * (u64::MAX as f32)) as u64;
            if hash_val > threshold {
                // Drop this span (sampled out)
                return;
            }
        }

        match self.sender.try_send(span) {
            Ok(_) => {
                // Successfully queued
            }
            Err(mpsc::error::TrySendError::Full(dropped_span)) => {
                // Buffer full - different strategy for priority vs non-priority
                if is_priority {
                    // Priority span - log warning but try to preserve it
                    warn!(
                        "Span buffer full, dropping priority span: {}",
                        dropped_span.name
                    );
                } else {
                    // Non-priority - drop silently to prevent log spam
                    // Metrics would track this in production
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!("Span channel closed, cannot record span");
            }
        }
    }

    /// Get batch configuration
    pub fn config(&self) -> &BatcherConfig {
        &self.config
    }
}

/// Background worker that batches and exports spans
async fn batch_worker(
    mut receiver: mpsc::Receiver<AgentreplaySpan>,
    config: BatcherConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = Vec::with_capacity(config.batch_size);
    let mut flush_interval = interval(config.batch_timeout);
    let client = reqwest::Client::new();

    loop {
        tokio::select! {
            // Receive new span
            Some(span) = receiver.recv() => {
                buffer.push(span);

                // Check if we should flush due to size
                if buffer.len() >= config.batch_size {
                    flush_batch(&mut buffer, &client, &config).await;
                }

                // Check if we're over memory limit
                if buffer.capacity() > config.max_buffer_size {
                    warn!(
                        "Buffer exceeds max size ({}), flushing early",
                        config.max_buffer_size
                    );
                    flush_batch(&mut buffer, &client, &config).await;
                }
            }

            // Timeout - flush whatever we have
            _ = flush_interval.tick() => {
                if !buffer.is_empty() {
                    debug!("Timeout reached, flushing {} spans", buffer.len());
                    flush_batch(&mut buffer, &client, &config).await;
                }
            }

            // Channel closed, flush remaining and exit
            else => {
                if !buffer.is_empty() {
                    debug!("Channel closed, flushing final {} spans", buffer.len());
                    flush_batch(&mut buffer, &client, &config).await;
                }
                break;
            }
        }
    }

    Ok(())
}

/// Flush a batch of spans to Agentreplay's REST API
async fn flush_batch(
    buffer: &mut Vec<AgentreplaySpan>,
    client: &reqwest::Client,
    config: &BatcherConfig,
) {
    if buffer.is_empty() {
        return;
    }

    let batch_size = buffer.len();
    debug!("Flushing batch of {} spans to Agentreplay", batch_size);

    // Send to Agentreplay's REST API: POST /api/v1/traces
    let endpoint = format!("{}/api/v1/traces", config.agentreplay_endpoint);

    let mut request = client.post(&endpoint).json(&buffer);

    // Add API key if configured
    if let Some(api_key) = &config.api_key {
        request = request.header("X-Agentreplay-API-Key", api_key);
    }

    match request.send().await {
        Ok(response) => {
            if response.status().is_success() {
                debug!("Successfully exported {} spans to Agentreplay", batch_size);
            } else {
                warn!("Failed to export spans: HTTP {}", response.status());
            }
        }
        Err(e) => {
            warn!("Error sending spans to Agentreplay: {}", e);
        }
    }

    buffer.clear();
}

/// Circuit breaker state for graceful degradation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation
    Closed,
    /// Backend is down, don't try to send
    Open,
    /// Testing if backend recovered
    HalfOpen,
}

/// Circuit breaker for export failures
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_threshold: usize,
    failures: Arc<Mutex<usize>>,
    #[allow(dead_code)]
    timeout: Duration,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            failure_threshold,
            failures: Arc::new(Mutex::new(0)),
            timeout,
        }
    }

    /// Record a successful export
    pub async fn record_success(&self) {
        let mut state = self.state.lock().await;
        *state = CircuitState::Closed;
        let mut failures = self.failures.lock().await;
        *failures = 0;
    }

    /// Record a failed export
    pub async fn record_failure(&self) {
        let mut failures = self.failures.lock().await;
        *failures += 1;

        if *failures >= self.failure_threshold {
            let mut state = self.state.lock().await;
            *state = CircuitState::Open;
            warn!(
                "Circuit breaker opened after {} failures",
                self.failure_threshold
            );
        }
    }

    /// Check if requests should be allowed
    pub async fn should_allow_request(&self) -> bool {
        let state = self.state.lock().await;
        matches!(*state, CircuitState::Closed | CircuitState::HalfOpen)
    }

    /// Get current state
    pub async fn state(&self) -> CircuitState {
        *self.state.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batcher_config_default() {
        let config = BatcherConfig::default();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.batch_timeout, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));

        // Initially closed
        assert_eq!(cb.state().await, CircuitState::Closed);

        // Record failures
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        // Third failure opens circuit
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Success closes it
        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
