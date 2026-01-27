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

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::auth::AuthContext;

/// Rate limiter state
#[derive(Clone)]
pub struct RateLimiter {
    /// tenant_id → (request_count, window_start)
    limits: Arc<DashMap<u64, (u32, Instant)>>,
    /// Maximum requests per minute per tenant
    max_requests_per_minute: u32,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `max_requests_per_minute` - Maximum requests allowed per tenant per minute
    ///
    /// # Example
    /// ```
    /// use flowtrace_server::middleware::rate_limit::RateLimiter;
    /// let limiter = RateLimiter::new(1000);  // 1000 req/min
    /// ```
    pub fn new(max_requests_per_minute: u32) -> Self {
        Self {
            limits: Arc::new(DashMap::new()),
            max_requests_per_minute,
        }
    }

    /// Check if request should be rate limited
    ///
    /// # Arguments
    /// * `tenant_id` - Tenant making the request
    ///
    /// # Returns
    /// * `Ok(())` - Request allowed
    /// * `Err(retry_after)` - Request should be rate limited, retry after N seconds
    pub fn check_limit(&self, tenant_id: u64) -> Result<(), u64> {
        let now = Instant::now();

        let mut entry = self.limits.entry(tenant_id).or_insert((0, now));

        // Reset window if expired (1 minute)
        if now.duration_since(entry.1) > Duration::from_secs(60) {
            entry.0 = 0;
            entry.1 = now;
        }

        entry.0 += 1;

        if entry.0 > self.max_requests_per_minute {
            // Calculate seconds until window resets
            let elapsed = now.duration_since(entry.1).as_secs();
            let retry_after = 60u64.saturating_sub(elapsed);
            return Err(retry_after);
        }

        Ok(())
    }

    /// Get current request count for a tenant
    pub fn get_request_count(&self, tenant_id: u64) -> u32 {
        self.limits
            .get(&tenant_id)
            .map(|entry| entry.0)
            .unwrap_or(0)
    }

    /// Get remaining requests for a tenant in current window
    pub fn get_remaining(&self, tenant_id: u64) -> u32 {
        let count = self.get_request_count(tenant_id);
        self.max_requests_per_minute.saturating_sub(count)
    }
}

/// Rate limiting middleware
///
/// Enforces per-tenant rate limits and returns 429 with Retry-After header
/// when limit is exceeded.
pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    Extension(auth): Extension<AuthContext>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Check rate limit
    match limiter.check_limit(auth.tenant_id) {
        Ok(()) => {
            // Request allowed
            let remaining = limiter.get_remaining(auth.tenant_id);

            // Add rate limit headers to response
            let mut response = next.run(request).await;
            let headers = response.headers_mut();
            headers.insert(
                "X-RateLimit-Limit",
                limiter.max_requests_per_minute.to_string().parse().unwrap(),
            );
            headers.insert(
                "X-RateLimit-Remaining",
                remaining.to_string().parse().unwrap(),
            );
            response
        }
        Err(retry_after) => {
            // Rate limited
            (
                StatusCode::TOO_MANY_REQUESTS,
                [
                    ("Retry-After", retry_after.to_string()),
                    (
                        "X-RateLimit-Limit",
                        limiter.max_requests_per_minute.to_string(),
                    ),
                    ("X-RateLimit-Remaining", "0".to_string()),
                ],
                format!("Rate limit exceeded. Retry after {} seconds", retry_after),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_under_limit() {
        let limiter = RateLimiter::new(10);

        // First 10 requests should succeed
        for _ in 0..10 {
            assert!(limiter.check_limit(1).is_ok());
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(10);

        // Use up the limit
        for _ in 0..10 {
            limiter.check_limit(1).unwrap();
        }

        // 11th request should be rate limited
        assert!(limiter.check_limit(1).is_err());
    }

    #[test]
    fn test_rate_limiter_per_tenant() {
        let limiter = RateLimiter::new(5);

        // Tenant 1 uses limit
        for _ in 0..5 {
            limiter.check_limit(1).unwrap();
        }
        assert!(limiter.check_limit(1).is_err());

        // Tenant 2 should still have full limit
        assert!(limiter.check_limit(2).is_ok());
    }

    #[test]
    fn test_rate_limiter_remaining() {
        let limiter = RateLimiter::new(10);

        assert_eq!(limiter.get_remaining(1), 10);

        limiter.check_limit(1).unwrap();
        assert_eq!(limiter.get_remaining(1), 9);

        limiter.check_limit(1).unwrap();
        assert_eq!(limiter.get_remaining(1), 8);
    }
}
