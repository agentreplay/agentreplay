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

//! Resilience primitives (retry policy + circuit breaker).

use rand::random;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub jitter: f64,
}

impl RetryPolicy {
    pub fn exponential() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: 0.1,
        }
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.initial_delay.as_secs_f64() * self.multiplier.powi(attempt as i32);
        let jitter_factor = 1.0 + (random::<f64>() - 0.5) * 2.0 * self.jitter;
        let jittered = base * jitter_factor;
        let clamped = jittered.min(self.max_delay.as_secs_f64());
        Duration::from_secs_f64(clamped)
    }
}

#[derive(Debug, Clone)]
pub struct CircuitConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub open_duration: Duration,
    pub half_open_max_calls: u32,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_duration: Duration::from_secs(30),
            half_open_max_calls: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

pub struct CircuitBreaker {
    state: RwLock<CircuitState>,
    config: CircuitConfig,
    failure_count: AtomicU32,
    success_count: AtomicU32,
}

impl CircuitBreaker {
    pub fn new(config: CircuitConfig) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            config,
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
        }
    }

    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, CircuitError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::error::Error,
    {
        let state = self.check_state().await;
        if let CircuitState::Open { until } = state {
            return Err(CircuitError::Open {
                retry_after: until.saturating_duration_since(Instant::now()),
            });
        }

        let result = operation().await;
        match &result {
            Ok(_) => self.record_success().await,
            Err(_) => self.record_failure().await,
        }

        result.map_err(CircuitError::Inner)
    }

    async fn check_state(&self) -> CircuitState {
        let mut state = self.state.write().await;
        if let CircuitState::Open { until } = *state {
            if Instant::now() >= until {
                *state = CircuitState::HalfOpen;
                self.success_count.store(0, Ordering::SeqCst);
            }
        }
        *state
    }

    async fn record_success(&self) {
        let mut state = self.state.write().await;
        match *state {
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let successes = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if successes >= self.config.success_threshold {
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::SeqCst);
                }
            }
            CircuitState::Open { .. } => {}
        }
    }

    async fn record_failure(&self) {
        let mut state = self.state.write().await;
        match *state {
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if failures >= self.config.failure_threshold {
                    let until = Instant::now() + self.config.open_duration;
                    *state = CircuitState::Open { until };
                }
            }
            CircuitState::HalfOpen => {
                let until = Instant::now() + self.config.open_duration;
                *state = CircuitState::Open { until };
            }
            CircuitState::Open { .. } => {}
        }
    }
}

#[derive(Debug, Error)]
pub enum CircuitError<E: std::error::Error> {
    #[error("Circuit open, retry after {retry_after:?}")]
    Open { retry_after: Duration },
    #[error("Operation failed: {0}")]
    Inner(E),
}

pub struct Resilient<T> {
    inner: T,
    retry_policy: RetryPolicy,
    circuit_breaker: std::sync::Arc<CircuitBreaker>,
    bulkhead: std::sync::Arc<Bulkhead>,
}

impl<T> Resilient<T> {
    pub fn new(
        inner: T,
        retry_policy: RetryPolicy,
        circuit_breaker: std::sync::Arc<CircuitBreaker>,
        bulkhead: std::sync::Arc<Bulkhead>,
    ) -> Self {
        Self {
            inner,
            retry_policy,
            circuit_breaker,
            bulkhead,
        }
    }

    pub async fn execute<F, Fut, R, E>(&self, mut operation: F) -> Result<R, ResilienceError>
    where
        F: FnMut(&T) -> Fut,
        Fut: std::future::Future<Output = Result<R, E>>,
        E: std::error::Error + Clone,
    {
        let mut last_error: Option<E> = None;

        for attempt in 0..self.retry_policy.max_attempts {
            if attempt > 0 {
                let delay = self.retry_policy.delay_for_attempt(attempt - 1);
                tokio::time::sleep(delay).await;
            }

            let permit = self.bulkhead.acquire().await?;
            let result = self.circuit_breaker.call(|| operation(&self.inner)).await;
            drop(permit);

            match result {
                Ok(value) => return Ok(value),
                Err(CircuitError::Open { retry_after }) => {
                    return Err(ResilienceError::CircuitOpen { retry_after });
                }
                Err(CircuitError::Inner(e)) => {
                    last_error = Some(e);
                }
            }
        }

        Err(ResilienceError::Exhausted {
            attempts: self.retry_policy.max_attempts,
            last_error: last_error.map(|e| e.to_string()),
        })
    }

    pub async fn execute_with_fallback<F, Fut, R, E, FB>(
        &self,
        operation: F,
        fallback: &FB,
    ) -> Result<R, ResilienceError>
    where
        F: FnMut(&T) -> Fut,
        Fut: std::future::Future<Output = Result<R, E>>,
        E: std::error::Error + Clone,
        FB: Fallback<R> + Send + Sync,
    {
        match self.execute(operation).await {
            Ok(result) => Ok(result),
            Err(e) => {
                if let Some(value) = fallback.fallback().await {
                    tracing::warn!("Using fallback after error: {}", e);
                    Ok(value)
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum ResilienceError {
    #[error("Circuit breaker open, retry after {retry_after:?}")]
    CircuitOpen { retry_after: Duration },
    #[error("All {attempts} retry attempts exhausted. Last error: {last_error:?}")]
    Exhausted { attempts: u32, last_error: Option<String> },
    #[error("Bulkhead rejected request")]
    BulkheadRejected,
}

/// Bulkhead isolation for resource pools.
pub struct Bulkhead {
    semaphore: Semaphore,
    name: String,
}

impl Bulkhead {
    pub fn new(name: impl Into<String>, max_concurrent: usize) -> Self {
        Self {
            semaphore: Semaphore::new(max_concurrent),
            name: name.into(),
        }
    }

    pub async fn acquire(&self) -> Result<tokio::sync::SemaphorePermit<'_>, ResilienceError> {
        self.semaphore
            .try_acquire()
            .map_err(|_| ResilienceError::BulkheadRejected)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait::async_trait]
pub trait Fallback<T> {
    async fn fallback(&self) -> Option<T>;
}
