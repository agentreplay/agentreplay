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

"""Span context manager for convenient trace logging."""

from typing import Optional, TYPE_CHECKING
import time
import threading
from contextlib import contextmanager
from agentreplay.models import AgentFlowEdge, SpanType

if TYPE_CHECKING:
    from agentreplay.client import AgentreplayClient


class Span:
    """Context manager for tracking agent execution spans.
    
    Automatically creates parent-child relationships and logs
    edges to Agentreplay on context exit.
    
    Example:
        >>> with client.trace() as root:
        ...     root.set_token_count(100)
        ...     
        ...     with root.child(SpanType.PLANNING) as planning:
        ...         planning.set_confidence(0.95)
        ...         
        ...         with planning.child(SpanType.TOOL_CALL) as tool:
        ...             tool.set_duration_ms(150)
    """

    def __init__(
        self,
        client: "AgentreplayClient",
        span_type: SpanType,
        tenant_id: int,
        project_id: int,
        agent_id: int,
        session_id: int,
        parent_id: int = 0,
    ):
        """Initialize span."""
        self.client = client
        self.span_type = span_type
        self.tenant_id = tenant_id
        self.project_id = project_id
        self.agent_id = agent_id
        self.session_id = session_id
        self.parent_id = parent_id
        self._lock = threading.RLock()  # Thread-safe operations

        # Edge fields
        self.edge_id: Optional[int] = None
        self.start_time_us = int(time.time() * 1_000_000)
        
        # Validate timestamp (must be >= 2020-01-01 in microseconds)
        MIN_VALID_TIMESTAMP = 1_577_836_800_000_000  # 2020-01-01 00:00:00 UTC
        if self.start_time_us < MIN_VALID_TIMESTAMP:
            import warnings
            warnings.warn(
                f"WARNING: Timestamp {self.start_time_us} is too old/small! "
                f"Check microsecond conversion. Expected >= {MIN_VALID_TIMESTAMP}."
            )
        
        self.confidence = 1.0
        self.token_count = 0
        self.duration_us = 0
        self.sampling_rate = 1.0
        self.sensitivity_flags = 0
        self.flags = 0

    def __enter__(self) -> "Span":
        """Enter span context - create initial edge."""
        with self._lock:
            # Create initial edge on entry so children can reference parent_id
            edge = AgentFlowEdge(
                edge_id=0,  # Will be assigned by client
                causal_parent=self.parent_id,
                timestamp_us=self.start_time_us,
                logical_clock=0,
                tenant_id=self.tenant_id,
                project_id=self.project_id,
                schema_version=2,
                sensitivity_flags=self.sensitivity_flags,
                agent_id=self.agent_id,
                session_id=self.session_id,
                span_type=self.span_type,
                parent_count=1 if self.parent_id != 0 else 0,
                confidence=self.confidence,
                token_count=0,  # Will be updated on exit
                duration_us=0,  # Will be calculated on exit
                sampling_rate=self.sampling_rate,
                compression_type=0,
                has_payload=False,
                flags=self.flags,
                checksum=0,
            )

            # Insert initial edge to get assigned ID
            inserted = self.client.insert(edge)
            self.edge_id = inserted.edge_id
            
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Exit span context - update edge with final metrics."""
        with self._lock:
            # Calculate duration
            end_time_us = int(time.time() * 1_000_000)
            self.duration_us = end_time_us - self.start_time_us

            # Handle errors
            if exc_type is not None:
                self.span_type = SpanType.ERROR
                self.confidence = 0.0

            # Create completion edge with final metrics
            edge = AgentFlowEdge(
                edge_id=self.edge_id,  # Use same ID for completion
                causal_parent=self.parent_id,
                timestamp_us=self.start_time_us,
                logical_clock=0,
                tenant_id=self.tenant_id,
                project_id=self.project_id,
                schema_version=2,
                sensitivity_flags=self.sensitivity_flags,
                agent_id=self.agent_id,
                session_id=self.session_id,
                span_type=self.span_type,
                parent_count=1 if self.parent_id != 0 else 0,
                confidence=self.confidence,
                token_count=self.token_count,  # Final token count
                duration_us=self.duration_us,  # Final duration
                sampling_rate=self.sampling_rate,
                compression_type=0,
                has_payload=False,
                flags=self.flags,
                checksum=0,
            )

            # Update edge with completion metrics
            self.client.insert(edge)

    def child(self, span_type: SpanType) -> "Span":
        """Create a child span.
        
        Thread-safe child span creation.
        
        Args:
            span_type: Type of child span
            
        Returns:
            Child span context manager
            
        Raises:
            RuntimeError: If called before parent span has been entered
        """
        with self._lock:
            if self.edge_id is None:
                raise RuntimeError("Parent span must be entered before creating children")

            return Span(
                client=self.client,
                span_type=span_type,
                tenant_id=self.tenant_id,
                project_id=self.project_id,
                agent_id=self.agent_id,
                session_id=self.session_id,
                parent_id=self.edge_id,
            )

    # Fluent API for setting properties

    def set_token_count(self, count: int) -> "Span":
        """Set token count (thread-safe)."""
        with self._lock:
            self.token_count = count
        return self

    def set_confidence(self, confidence: float) -> "Span":
        """Set confidence score (0.0 to 1.0, thread-safe)."""
        if not 0.0 <= confidence <= 1.0:
            raise ValueError("Confidence must be between 0.0 and 1.0")
        with self._lock:
            self.confidence = confidence
        return self

    def set_duration_ms(self, duration_ms: int) -> "Span":
        """Set duration in milliseconds (thread-safe).

        IMPORTANT: This method accepts milliseconds and converts to microseconds.
        If you accidentally pass microseconds, the duration will be 1000x too large!

        Examples:
            span.set_duration_ms(150)  # 150 milliseconds = 150,000 microseconds ✓
            span.set_duration_ms(150_000)  # ERROR! This is 150 seconds, not milliseconds! ✗

        Use set_duration_us() if you have microseconds already.
        """
        with self._lock:
            # DESKTOP APP FIX: Validate input to catch common mistakes
            if duration_ms > 600_000:  # > 10 minutes in ms
                import warnings
                warnings.warn(
                    f"WARNING: duration_ms={duration_ms} seems suspiciously large (>{duration_ms/1000}s). "
                    f"Did you mean to use set_duration_us({duration_ms}) instead? "
                    f"If this duration is correct, ignore this warning.",
                    UserWarning,
                    stacklevel=2
                )
            self.duration_us = duration_ms * 1_000
        return self

    def set_duration_us(self, duration_us: int) -> "Span":
        """Set duration in microseconds (thread-safe).

        Use this when you already have duration in microseconds.
        For milliseconds, use set_duration_ms() instead.

        Examples:
            span.set_duration_us(150_000)  # 150 milliseconds = 150,000 microseconds ✓
            span.set_duration_us(150)  # 150 microseconds = 0.15 milliseconds ✓
        """
        with self._lock:
            self.duration_us = duration_us
        return self

    def set_sampling_rate(self, rate: float) -> "Span":
        """Set sampling rate (0.0 to 1.0, thread-safe)."""
        if not 0.0 <= rate <= 1.0:
            raise ValueError("Sampling rate must be between 0.0 and 1.0")
        with self._lock:
            self.sampling_rate = rate
        return self

    def mark_pii(self) -> "Span":
        """Mark this span as containing PII (thread-safe)."""
        from agentreplay.models import SensitivityFlags

        with self._lock:
            self.sensitivity_flags |= SensitivityFlags.PII
        return self

    def mark_secret(self) -> "Span":
        """Mark this span as containing secrets (thread-safe)."""
        from agentreplay.models import SensitivityFlags

        with self._lock:
            self.sensitivity_flags |= SensitivityFlags.SECRET
        return self

    def mark_no_embed(self) -> "Span":
        """Mark this span to never be embedded in vector index (thread-safe)."""
        from agentreplay.models import SensitivityFlags

        with self._lock:
            self.sensitivity_flags |= SensitivityFlags.NO_EMBED
        return self
