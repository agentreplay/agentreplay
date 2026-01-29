# Copyright 2025 Sushanth (https://github.com/sushanthpy)
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Circuit Breaker pattern for Agentreplay backend resilience.

This module implements a circuit breaker to prevent cascading failures when
the Agentreplay backend is unavailable. The circuit breaker has three states:

- CLOSED: Normal operation, requests pass through
- OPEN: Backend is failing, requests are rejected immediately
- HALF_OPEN: Testing recovery, allowing limited requests through

The circuit breaker helps maintain application responsiveness during backend
outages by failing fast rather than blocking on retries.

Usage:
    >>> from agentreplay.circuit_breaker import CircuitBreaker, CircuitBreakerOpen
    >>> 
    >>> breaker = CircuitBreaker()
    >>> 
    >>> try:
    ...     with breaker:
    ...         send_spans_to_backend()
    ... except CircuitBreakerOpen:
    ...     logger.warning("Agentreplay backend unavailable, dropping spans")
"""

import time
import threading
import logging
from enum import Enum
from typing import Optional, Callable, Any
from functools import wraps

logger = logging.getLogger(__name__)


class CircuitState(Enum):
    """Circuit breaker states."""
    CLOSED = "closed"       # Normal operation
    OPEN = "open"           # Failing, reject requests
    HALF_OPEN = "half_open" # Testing recovery


class CircuitBreakerOpen(Exception):
    """Exception raised when circuit breaker is open."""
    pass


class CircuitBreaker:
    """Thread-safe circuit breaker for backend resilience.
    
    Args:
        failure_threshold: Number of failures before opening circuit (default: 5)
        recovery_timeout: Seconds before attempting recovery (default: 30)
        success_threshold: Successes needed to close circuit from half-open (default: 3)
        failure_window: Time window in seconds for counting failures (default: 60)
    
    Example:
        >>> breaker = CircuitBreaker(failure_threshold=3, recovery_timeout=60)
        >>> 
        >>> @breaker.protect
        ... def send_to_backend():
        ...     response = requests.post(...)
        ...     response.raise_for_status()
        ...     return response
    """
    
    def __init__(
        self,
        failure_threshold: int = 5,
        recovery_timeout: float = 30.0,
        success_threshold: int = 3,
        failure_window: float = 60.0,
    ):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.success_threshold = success_threshold
        self.failure_window = failure_window
        
        self._state = CircuitState.CLOSED
        self._failure_count = 0
        self._success_count = 0
        self._last_failure_time: Optional[float] = None
        self._last_state_change: float = time.time()
        self._lock = threading.Lock()
        
        # Track failure times for windowed counting
        self._failure_times: list[float] = []
    
    @property
    def state(self) -> CircuitState:
        """Get current circuit state."""
        with self._lock:
            self._check_state_transition()
            return self._state
    
    @property
    def is_closed(self) -> bool:
        """Check if circuit is closed (normal operation)."""
        return self.state == CircuitState.CLOSED
    
    @property
    def is_open(self) -> bool:
        """Check if circuit is open (rejecting requests)."""
        return self.state == CircuitState.OPEN
    
    def _check_state_transition(self) -> None:
        """Check if state should transition (called with lock held)."""
        now = time.time()
        
        if self._state == CircuitState.OPEN:
            # Check if recovery timeout has passed
            if now - self._last_state_change >= self.recovery_timeout:
                self._transition_to(CircuitState.HALF_OPEN)
        
        elif self._state == CircuitState.CLOSED:
            # Clean up old failures outside the window
            cutoff = now - self.failure_window
            self._failure_times = [t for t in self._failure_times if t > cutoff]
            self._failure_count = len(self._failure_times)
    
    def _transition_to(self, new_state: CircuitState) -> None:
        """Transition to a new state (called with lock held)."""
        old_state = self._state
        self._state = new_state
        self._last_state_change = time.time()
        
        if new_state == CircuitState.CLOSED:
            self._failure_count = 0
            self._failure_times.clear()
            self._success_count = 0
        elif new_state == CircuitState.HALF_OPEN:
            self._success_count = 0
        
        logger.info(
            f"Circuit breaker state change: {old_state.value} -> {new_state.value}"
        )
    
    def record_success(self) -> None:
        """Record a successful request."""
        with self._lock:
            if self._state == CircuitState.HALF_OPEN:
                self._success_count += 1
                if self._success_count >= self.success_threshold:
                    self._transition_to(CircuitState.CLOSED)
    
    def record_failure(self, error: Optional[Exception] = None) -> None:
        """Record a failed request."""
        now = time.time()
        
        with self._lock:
            self._last_failure_time = now
            
            if self._state == CircuitState.CLOSED:
                self._failure_times.append(now)
                self._failure_count = len(self._failure_times)
                
                if self._failure_count >= self.failure_threshold:
                    self._transition_to(CircuitState.OPEN)
                    logger.warning(
                        f"Circuit breaker opened after {self._failure_count} failures. "
                        f"Will retry after {self.recovery_timeout}s."
                    )
            
            elif self._state == CircuitState.HALF_OPEN:
                # Any failure in half-open state reopens the circuit
                self._transition_to(CircuitState.OPEN)
                logger.warning("Circuit breaker reopened after recovery test failure.")
    
    def allow_request(self) -> bool:
        """Check if a request should be allowed through.
        
        Returns:
            True if request is allowed, False if circuit is open
        """
        with self._lock:
            self._check_state_transition()
            
            if self._state == CircuitState.CLOSED:
                return True
            elif self._state == CircuitState.OPEN:
                return False
            else:  # HALF_OPEN
                return True  # Allow test requests through
    
    def __enter__(self) -> "CircuitBreaker":
        """Context manager entry - check if request is allowed."""
        if not self.allow_request():
            raise CircuitBreakerOpen(
                f"Circuit breaker is open. Recovery in "
                f"{self.recovery_timeout - (time.time() - self._last_state_change):.1f}s"
            )
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        """Context manager exit - record success or failure."""
        if exc_type is None:
            self.record_success()
        else:
            self.record_failure(exc_val)
        return False  # Don't suppress exceptions
    
    def protect(self, func: Callable) -> Callable:
        """Decorator to protect a function with the circuit breaker.
        
        Args:
            func: Function to protect
        
        Returns:
            Wrapped function that respects circuit breaker state
        
        Example:
            >>> @breaker.protect
            ... def send_spans():
            ...     pass
        """
        @wraps(func)
        def wrapper(*args, **kwargs) -> Any:
            with self:
                return func(*args, **kwargs)
        return wrapper
    
    def reset(self) -> None:
        """Manually reset the circuit breaker to closed state."""
        with self._lock:
            self._transition_to(CircuitState.CLOSED)
            logger.info("Circuit breaker manually reset")
    
    def stats(self) -> dict:
        """Get circuit breaker statistics.
        
        Returns:
            Dictionary with current state and counters
        """
        with self._lock:
            return {
                "state": self._state.value,
                "failure_count": self._failure_count,
                "success_count": self._success_count,
                "last_failure_time": self._last_failure_time,
                "last_state_change": self._last_state_change,
                "seconds_in_state": time.time() - self._last_state_change,
            }


# Global circuit breaker instance for the Agentreplay backend
_default_breaker: Optional[CircuitBreaker] = None


def get_circuit_breaker() -> CircuitBreaker:
    """Get the default circuit breaker instance.
    
    Creates one if it doesn't exist with default settings.
    
    Returns:
        Default CircuitBreaker instance
    """
    global _default_breaker
    if _default_breaker is None:
        _default_breaker = CircuitBreaker()
    return _default_breaker


def configure_circuit_breaker(
    failure_threshold: int = 5,
    recovery_timeout: float = 30.0,
    success_threshold: int = 3,
    failure_window: float = 60.0,
) -> CircuitBreaker:
    """Configure the default circuit breaker.
    
    Args:
        failure_threshold: Number of failures before opening circuit
        recovery_timeout: Seconds before attempting recovery
        success_threshold: Successes needed to close circuit from half-open
        failure_window: Time window in seconds for counting failures
    
    Returns:
        Configured CircuitBreaker instance
    """
    global _default_breaker
    _default_breaker = CircuitBreaker(
        failure_threshold=failure_threshold,
        recovery_timeout=recovery_timeout,
        success_threshold=success_threshold,
        failure_window=failure_window,
    )
    return _default_breaker
