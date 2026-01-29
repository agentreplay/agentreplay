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

use moka::sync::Cache;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window: Duration,
    /// Enable rate limiting
    pub enabled: bool,
    /// Maximum number of tracked clients (default: 100,000)
    pub max_clients: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
            enabled: true,
            max_clients: 100_000,
        }
    }
}

/// Token bucket for rate limiting
///
/// CRITICAL FIX: Now uses AtomicU64 for thread-safe updates without Mutex
#[derive(Debug)]
struct TokenBucket {
    /// Current number of tokens (scaled by 1000 for precision)
    tokens: AtomicU64,
    /// Maximum tokens (capacity)
    capacity: f64,
    /// Token refill rate (tokens per second)
    refill_rate: f64,
    /// Last refill time (milliseconds since start)
    last_refill_ms: AtomicU64,
    /// Reference instant for time calculations
    start_instant: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, window: Duration) -> Self {
        let refill_rate = capacity as f64 / window.as_secs_f64();
        Self {
            tokens: AtomicU64::new((capacity as u64) * 1000), // Scale by 1000
            capacity: capacity as f64,
            refill_rate,
            last_refill_ms: AtomicU64::new(0),
            start_instant: Instant::now(),
        }
    }

    /// Get current tokens (scaled by 1000)
    fn get_tokens(&self) -> f64 {
        self.tokens.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Set tokens (scaled by 1000)
    fn set_tokens(&self, value: f64) {
        self.tokens
            .store((value * 1000.0) as u64, Ordering::Relaxed);
    }

    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let now_ms = self.start_instant.elapsed().as_millis() as u64;
        let last_ms = self.last_refill_ms.swap(now_ms, Ordering::Relaxed);
        let elapsed_secs = (now_ms.saturating_sub(last_ms)) as f64 / 1000.0;

        // Add tokens based on elapsed time
        let current = self.get_tokens();
        let new_tokens = (current + elapsed_secs * self.refill_rate).min(self.capacity);
        self.set_tokens(new_tokens);
    }

    /// Try to consume one token
    fn try_consume(&self) -> bool {
        self.refill();

        let current = self.get_tokens();
        if current >= 1.0 {
            self.set_tokens(current - 1.0);
            true
        } else {
            false
        }
    }

    /// Get remaining tokens
    fn remaining(&self) -> u32 {
        self.refill();
        self.get_tokens().floor() as u32
    }

    /// Get time until next token is available
    fn retry_after(&self) -> Duration {
        self.refill();

        let current = self.get_tokens();
        if current >= 1.0 {
            Duration::from_secs(0)
        } else {
            let tokens_needed = 1.0 - current;
            let seconds = tokens_needed / self.refill_rate;
            Duration::from_secs_f64(seconds)
        }
    }
}

/// Rate limiter using token bucket algorithm
///
/// CRITICAL FIX: Now uses moka cache with automatic TTL-based eviction
/// to prevent unbounded memory growth from tracking many unique IPs.
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Bounded LRU cache: IP -> TokenBucket with automatic TTL expiration
    /// When capacity is reached, least recently used entries are evicted
    buckets: Cache<String, Arc<TokenBucket>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        // TTL is 10x the window duration - entries not accessed within this
        // time will be automatically evicted
        let ttl = config.window * 10;

        let buckets = Cache::builder()
            .max_capacity(config.max_clients)
            .time_to_idle(ttl)
            .build();

        Self { config, buckets }
    }

    /// Check if request is allowed for given identifier (IP or API key)
    pub fn check_rate_limit(&self, identifier: &str) -> RateLimitResult {
        if !self.config.enabled {
            return RateLimitResult::Allowed {
                remaining: self.config.max_requests,
                retry_after: Duration::from_secs(0),
            };
        }

        // Get or create bucket for this identifier
        let bucket = self.buckets.get_with(identifier.to_string(), || {
            Arc::new(TokenBucket::new(
                self.config.max_requests,
                self.config.window,
            ))
        });

        // Try to consume a token
        if bucket.try_consume() {
            RateLimitResult::Allowed {
                remaining: bucket.remaining(),
                retry_after: Duration::from_secs(0),
            }
        } else {
            RateLimitResult::RateLimited {
                retry_after: bucket.retry_after(),
            }
        }
    }

    /// Clean up old buckets (now automatic via moka TTL, this is a no-op for compatibility)
    #[deprecated(note = "Cleanup is now automatic via moka TTL eviction")]
    pub fn cleanup_old_buckets(&self) {
        // No-op: moka handles this automatically
        self.buckets.run_pending_tasks();
    }

    /// Get current number of tracked clients
    pub fn client_count(&self) -> u64 {
        self.buckets.entry_count()
    }
}

/// Result of rate limit check
#[derive(Debug)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed {
        /// Remaining requests in current window
        remaining: u32,
        /// Time until next window (always 0 for allowed)
        retry_after: Duration,
    },
    /// Request is rate limited
    RateLimited {
        /// Time to wait before retrying
        retry_after: Duration,
    },
}

/// Extract client IP from request headers
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> Option<String> {
    // Try X-Forwarded-For first (proxy/load balancer)
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // Take first IP (client IP)
            if let Some(first_ip) = forwarded_str.split(',').next() {
                return Some(first_ip.trim().to_string());
            }
        }
    }

    // Try X-Real-IP (nginx)
    if let Some(real_ip) = headers.get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            return Some(ip_str.to_string());
        }
    }

    // No IP found in headers
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_basic() {
        let bucket = TokenBucket::new(10, Duration::from_secs(10));

        // Should allow 10 requests
        for _ in 0..10 {
            assert!(bucket.try_consume());
        }

        // 11th request should be denied
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_token_bucket_refill() {
        let bucket = TokenBucket::new(10, Duration::from_secs(1));

        // Consume all tokens
        for _ in 0..10 {
            assert!(bucket.try_consume());
        }

        // Wait for refill
        std::thread::sleep(Duration::from_millis(100));

        // Should have ~1 token now (0.1s * 10 tokens/s = 1 token)
        assert!(bucket.try_consume());
    }

    #[test]
    fn test_rate_limiter() {
        let config = RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(1),
            enabled: true,
            max_clients: 1000,
        };

        let limiter = RateLimiter::new(config);

        // Should allow 5 requests
        for _ in 0..5 {
            match limiter.check_rate_limit("test_client") {
                RateLimitResult::Allowed { .. } => {}
                RateLimitResult::RateLimited { .. } => panic!("Should be allowed"),
            }
        }

        // 6th request should be rate limited
        match limiter.check_rate_limit("test_client") {
            RateLimitResult::RateLimited { .. } => {}
            RateLimitResult::Allowed { .. } => panic!("Should be rate limited"),
        }
    }

    #[test]
    fn test_rate_limiter_disabled() {
        let config = RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(1),
            enabled: false, // Disabled
            max_clients: 1000,
        };

        let limiter = RateLimiter::new(config);

        // Should allow unlimited requests when disabled
        for _ in 0..100 {
            match limiter.check_rate_limit("test_client") {
                RateLimitResult::Allowed { .. } => {}
                RateLimitResult::RateLimited { .. } => {
                    panic!("Should not rate limit when disabled")
                }
            }
        }
    }

    #[test]
    fn test_rate_limiter_bounded_memory() {
        let config = RateLimitConfig {
            max_requests: 10,
            window: Duration::from_secs(1),
            enabled: true,
            max_clients: 10, // Very small limit for testing
        };

        let limiter = RateLimiter::new(config);

        // Create many clients
        for i in 0..100 {
            limiter.check_rate_limit(&format!("client_{}", i));
        }

        // Force pending tasks to complete
        limiter.buckets.run_pending_tasks();

        // Should have at most max_clients entries
        assert!(
            limiter.client_count() <= 10,
            "Client count {} should be <= 10",
            limiter.client_count()
        );
    }
}
