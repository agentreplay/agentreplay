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

"""Session management for conversational applications."""

import time
from typing import Optional
from agentreplay.client import AgentreplayClient
from agentreplay.models import SpanType
from agentreplay.span import Span


class Session:
    """Session manager for chatbot and conversational applications.
    
    Automatically tracks session_id and message counts, simplifying
    session lifecycle management for multi-turn conversations.
    
    Args:
        client: Agentreplay client instance
        session_id: Optional session identifier (auto-generated if not provided)
        agent_id: Optional agent identifier (uses client default if not provided)
        
    Attributes:
        session_id: Session identifier
        message_count: Number of messages/traces in this session
        start_time: Session start timestamp (microseconds)
        
    Example:
        >>> client = AgentreplayClient(url="http://localhost:8080", tenant_id=1)
        >>> session = Session(client)
        >>> 
        >>> # Track conversation turns
        >>> with session.trace(SpanType.LLM) as turn1:
        ...     turn1.set_attribute("prompt", "Hello")
        ...     turn1.set_token_count(50)
        ...
        >>> with session.trace(SpanType.LLM) as turn2:
        ...     turn2.set_attribute("prompt", "How are you?")
        ...     turn2.set_token_count(60)
        ...
        >>> print(f"Session {session.session_id} had {session.message_count} turns")
        >>> session.end()
    """

    def __init__(
        self,
        client: AgentreplayClient,
        session_id: Optional[int] = None,
        agent_id: Optional[int] = None,
    ):
        """Initialize session manager.
        
        Args:
            client: Agentreplay client
            session_id: Session identifier (auto-generated if None)
            agent_id: Agent identifier (uses client default if None)
        """
        self.client = client
        self.session_id = session_id if session_id is not None else int(time.time() * 1000)
        self.agent_id = agent_id if agent_id is not None else client.agent_id
        self.message_count = 0
        self.start_time = int(time.time() * 1_000_000)
        self._end_time: Optional[int] = None  # Track when session ended
        self._ended = False

    def __enter__(self) -> "Session":
        """Context manager entry."""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit - end session."""
        self.end()

    def trace(
        self,
        span_type: SpanType = SpanType.ROOT,
        **metadata,
    ) -> Span:
        """Create a trace within this session.
        
        Automatically sets session_id and tracks message count.
        
        Args:
            span_type: Type of span (default: ROOT)
            **metadata: Additional metadata to attach to span
            
        Returns:
            Span context manager
            
        Example:
            >>> session = Session(client)
            >>> with session.trace(SpanType.LLM, model="gpt-4") as span:
            ...     span.set_token_count(100)
        """
        if self._ended:
            raise RuntimeError("Cannot create trace in ended session")

        self.message_count += 1

        # Add session metadata
        full_metadata = {
            "message_num": self.message_count,
            "session_duration_us": int(time.time() * 1_000_000) - self.start_time,
            **metadata,
        }

        return self.client.trace(
            span_type=span_type,
            agent_id=self.agent_id,
            session_id=self.session_id,
        )

    def end(self) -> None:
        """Mark session as ended.
        
        Optionally send session summary metrics to backend.
        """
        if self._ended:
            return

        self._end_time = int(time.time() * 1_000_000)
        self._ended = True

        # Compute session statistics
        duration_us = self._end_time - self.start_time

        # Could send session summary span
        # with self.client.trace(
        #     span_type=SpanType.ROOT,
        #     agent_id=self.agent_id,
        #     session_id=self.session_id
        # ) as summary:
        #     summary.set_attribute("session_ended", True)
        #     summary.set_attribute("total_messages", self.message_count)
        #     summary.set_attribute("duration_us", duration_us)

    @property
    def is_active(self) -> bool:
        """Check if session is still active."""
        return not self._ended

    @property
    def duration_seconds(self) -> float:
        """Get session duration in seconds.
        
        Returns accurate duration even after session has ended.
        """
        if self._ended and self._end_time is not None:
            # Return actual session duration
            return (self._end_time - self.start_time) / 1_000_000
        # Session still active, return current duration
        return (int(time.time() * 1_000_000) - self.start_time) / 1_000_000
