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

//! Admission Control for High-Throughput Ingestion
//!
//! Implements rate limiting and backpressure signaling to prevent overload.
//!
//! ## Problem
//!
//! Without admission control, clients can overwhelm the server during traffic spikes,
//! causing cascading failures, increased latencies, and potential data loss.
//!
//! ## Solution
//!
//! 1. **Token Bucket Rate Limiter**: Smooth traffic to sustainable rate
//! 2. **Backpressure Headers**: Signal clients to slow down (Retry-After)
//! 3. **Adaptive Throttling**: Adjust limits based on system health
//!
//! ## HTTP Response Codes
//!
//! - 201: Request accepted
//! - 429: Too Many Requests (with Retry-After header)
//! - 503: Service Unavailable (system under heavy load)

use axum::{
    body::Body,
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Configuration for admission control
#[derive(Clone, Debug)]
pub struct AdmissionConfig {
    /// Maximum requests per second
    pub max_rate: u64,
    /// Burst capacity (allows short traffic spikes)
    pub burst_size: u64,
    /// Window size for rate calculation
    pub window_size: Duration,
    /// Enable adaptive throttling
    pub adaptive: bool,
    /// Target P99 latency in microseconds (for adaptive mode)
    pub target_latency_us: u64,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        Self {
            max_rate: 10_000,  // 10K spans/sec
            burst_size: 5_000, // Allow bursts up to 5K
            window_size: Duration::from_secs(1),
            adaptive: true,
            target_latency_us: 50_000, // 50ms P99 target
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    tokens: AtomicU64,
    max_tokens: u64,
    refill_rate: u64, // tokens per second
    last_refill: Mutex<Instant>,
}

impl TokenBucket {
    pub fn new(max_tokens: u64, refill_rate: u64) -> Self {
        Self {
            tokens: AtomicU64::new(max_tokens),
            max_tokens,
            refill_rate,
            last_refill: Mutex::new(Instant::now()),
        }
    }

    /// Try to consume tokens
    ///
    /// Returns (success, retry_after_ms) where:
    /// - success: true if tokens were acquired
    /// - retry_after_ms: suggested wait time if tokens unavailable
    pub fn try_acquire(&self, count: u64) -> (bool, u64) {
        self.refill();

        let current = self.tokens.load(Ordering::Relaxed);
        if current >= count {
            if self
                .tokens
                .compare_exchange_weak(
                    current,
                    current - count,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return (true, 0);
            }
        }

        // Calculate retry-after based on token deficit
        let deficit = count.saturating_sub(current);
        let retry_after_ms = (deficit * 1000) / self.refill_rate.max(1);

        (false, retry_after_ms.max(100)) // Minimum 100ms retry
    }

    fn refill(&self) {
        let mut last_refill = self.last_refill.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);
        *last_refill = now;

        let to_add = (elapsed.as_secs_f64() * self.refill_rate as f64) as u64;
        if to_add > 0 {
            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = (current + to_add).min(self.max_tokens);
            self.tokens.store(new_tokens, Ordering::Relaxed);
        }
    }

    pub fn available(&self) -> u64 {
        self.tokens.load(Ordering::Relaxed)
    }
}

/// Admission controller for rate limiting and backpressure
pub struct AdmissionController {
    config: AdmissionConfig,
    bucket: TokenBucket,
    /// Current system health (0-100, lower = healthier)
    load_score: AtomicU64,
    /// Whether circuit breaker is tripped
    circuit_open: AtomicBool,
    /// Statistics
    stats: Arc<AdmissionStats>,
}

/// Statistics for admission control
#[derive(Debug, Default)]
pub struct AdmissionStats {
    pub requests_accepted: AtomicU64,
    pub requests_rejected: AtomicU64,
    pub total_spans_accepted: AtomicU64,
    pub total_spans_rejected: AtomicU64,
}

impl AdmissionController {
    pub fn new(config: AdmissionConfig) -> Self {
        let bucket = TokenBucket::new(config.burst_size, config.max_rate);

        Self {
            config,
            bucket,
            load_score: AtomicU64::new(0),
            circuit_open: AtomicBool::new(false),
            stats: Arc::new(AdmissionStats::default()),
        }
    }

    /// Check if a request should be admitted
    ///
    /// Returns:
    /// - Ok(()) if request is accepted
    /// - Err(RejectionReason) with details for rejection response
    pub fn should_admit(&self, span_count: usize) -> Result<(), RejectionReason> {
        // Check circuit breaker first
        if self.circuit_open.load(Ordering::Relaxed) {
            self.stats.requests_rejected.fetch_add(1, Ordering::Relaxed);
            self.stats
                .total_spans_rejected
                .fetch_add(span_count as u64, Ordering::Relaxed);
            return Err(RejectionReason::CircuitOpen {
                retry_after_ms: 5000, // 5 seconds
            });
        }

        // Try to acquire tokens
        let (success, retry_after_ms) = self.bucket.try_acquire(span_count as u64);

        if success {
            self.stats.requests_accepted.fetch_add(1, Ordering::Relaxed);
            self.stats
                .total_spans_accepted
                .fetch_add(span_count as u64, Ordering::Relaxed);
            Ok(())
        } else {
            self.stats.requests_rejected.fetch_add(1, Ordering::Relaxed);
            self.stats
                .total_spans_rejected
                .fetch_add(span_count as u64, Ordering::Relaxed);
            Err(RejectionReason::RateLimited { retry_after_ms })
        }
    }

    /// Update system load score (for adaptive throttling)
    ///
    /// Call this periodically with observed latency metrics.
    pub fn update_load(&self, observed_latency_us: u64) {
        if !self.config.adaptive {
            return;
        }

        // Calculate load score as ratio of observed to target latency
        let score = (observed_latency_us * 100) / self.config.target_latency_us.max(1);
        let clamped = score.min(200); // Cap at 200%

        self.load_score.store(clamped, Ordering::Relaxed);

        // Trip circuit breaker if severely overloaded
        if clamped > 150 {
            self.circuit_open.store(true, Ordering::Relaxed);
        } else if clamped < 100 {
            self.circuit_open.store(false, Ordering::Relaxed);
        }
    }

    /// Get current load score (0-200)
    pub fn load_score(&self) -> u64 {
        self.load_score.load(Ordering::Relaxed)
    }

    /// Get statistics
    pub fn stats(&self) -> &AdmissionStats {
        &self.stats
    }

    /// Get available tokens
    pub fn available_capacity(&self) -> u64 {
        self.bucket.available()
    }
}

/// Reason for request rejection
#[derive(Debug)]
pub enum RejectionReason {
    /// Rate limit exceeded
    RateLimited { retry_after_ms: u64 },
    /// System circuit breaker is open
    CircuitOpen { retry_after_ms: u64 },
}

/// Response body for rejected requests
#[derive(Debug, Serialize)]
pub struct RejectionResponse {
    pub error: String,
    pub code: String,
    pub retry_after_ms: u64,
}

impl IntoResponse for RejectionReason {
    fn into_response(self) -> Response<Body> {
        match self {
            RejectionReason::RateLimited { retry_after_ms } => {
                let body = RejectionResponse {
                    error: "Rate limit exceeded. Please slow down.".to_string(),
                    code: "RATE_LIMITED".to_string(),
                    retry_after_ms,
                };

                Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .header(
                        header::RETRY_AFTER,
                        HeaderValue::from_str(&format!("{}", retry_after_ms / 1000 + 1)).unwrap(),
                    )
                    .header(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json"),
                    )
                    .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
                    .unwrap()
            }
            RejectionReason::CircuitOpen { retry_after_ms } => {
                let body = RejectionResponse {
                    error: "Service temporarily unavailable due to high load.".to_string(),
                    code: "SERVICE_UNAVAILABLE".to_string(),
                    retry_after_ms,
                };

                Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .header(
                        header::RETRY_AFTER,
                        HeaderValue::from_str(&format!("{}", retry_after_ms / 1000 + 1)).unwrap(),
                    )
                    .header(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json"),
                    )
                    .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
                    .unwrap()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let bucket = TokenBucket::new(100, 100); // 100 tokens, 100/sec refill

        // Should succeed for first 100 tokens
        for _ in 0..10 {
            let (success, _) = bucket.try_acquire(10);
            assert!(success);
        }

        // Should fail when depleted
        let (success, retry_after) = bucket.try_acquire(10);
        assert!(!success);
        assert!(retry_after >= 100); // Should suggest waiting ~100ms
    }

    #[test]
    fn test_admission_controller() {
        let config = AdmissionConfig {
            max_rate: 1000,
            burst_size: 100,
            ..Default::default()
        };

        let controller = AdmissionController::new(config);

        // First 100 spans should be accepted
        assert!(controller.should_admit(50).is_ok());
        assert!(controller.should_admit(50).is_ok());

        // Next batch should be rejected
        assert!(controller.should_admit(50).is_err());
    }

    #[test]
    fn test_circuit_breaker() {
        let config = AdmissionConfig {
            adaptive: true,
            target_latency_us: 10_000, // 10ms target
            ..Default::default()
        };

        let controller = AdmissionController::new(config);

        // Normal load - circuit closed
        controller.update_load(5_000);
        assert!(controller.should_admit(1).is_ok());

        // Extreme load - circuit should open
        controller.update_load(20_000); // 200% of target
        let result = controller.should_admit(1);
        assert!(matches!(result, Err(RejectionReason::CircuitOpen { .. })));
    }
}
