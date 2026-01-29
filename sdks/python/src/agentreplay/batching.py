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

"""Batching client for high-throughput trace ingestion."""

import threading
import time
from collections import deque
from typing import Deque, List, Optional
from agentreplay.client import AgentreplayClient
from agentreplay.models import AgentFlowEdge


class BatchingAgentreplayClient:
    """Client wrapper that batches spans for efficient ingestion.

    Automatically buffers spans and flushes when batch size is reached
    or flush interval expires, reducing HTTP overhead by 100x.

    **CRITICAL FIX**: Now includes max_buffer_size to prevent OOM.
    When buffer is full, oldest edges are dropped (sampling behavior).

    Args:
        client: Underlying AgentreplayClient
        batch_size: Number of spans to buffer before flushing (default: 100)
        flush_interval: Seconds between automatic flushes (default: 5.0)
        max_buffer_size: Maximum buffer size before dropping edges (default: 10000)

    Example:
        >>> client = AgentreplayClient(url="http://localhost:8080", tenant_id=1)
        >>> batching_client = BatchingAgentreplayClient(
        ...     client,
        ...     batch_size=100,
        ...     max_buffer_size=10000  # Prevent OOM
        ... )
        >>>
        >>> # Spans are buffered
        >>> for i in range(1000):
        ...     edge = AgentFlowEdge(
        ...         tenant_id=1,
        ...         agent_id=1,
        ...         session_id=42,
        ...         span_type=SpanType.ROOT
        ...     )
        ...     batching_client.insert(edge)  # Buffered, not sent immediately
        ...
        >>> # Flush remaining spans
        >>> batching_client.flush()
        >>> batching_client.close()
    """

    def __init__(
        self,
        client: AgentreplayClient,
        batch_size: int = 100,
        flush_interval: float = 5.0,
        max_buffer_size: int = 10000,
    ):
        """Initialize batching client."""
        self.client = client
        self.batch_size = batch_size
        self.flush_interval = flush_interval
        self.max_buffer_size = max_buffer_size
        self._buffer: List[AgentFlowEdge] = []
        self._retry_queue: Deque[List[AgentFlowEdge]] = deque()  # Failed batches awaiting retry
        self._max_retry_batches = 10  # Limit retry queue to prevent unbounded growth
        self._lock = threading.Lock()
        self._running = True
        self._dropped_count = 0  # Track dropped edges for monitoring
        self._flush_thread = threading.Thread(target=self._auto_flush, daemon=True)
        self._flush_thread.start()

    def __enter__(self) -> "BatchingAgentreplayClient":
        """Context manager entry."""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit - flush and close."""
        self.flush()
        self.close()

    def insert(self, edge: AgentFlowEdge) -> AgentFlowEdge:
        """Buffer a single edge for batched insertion.

        **CRITICAL FIX**: Now enforces max_buffer_size to prevent OOM.
        If buffer is full, drops oldest edges (FIFO sampling).

        Args:
            edge: Edge to buffer

        Returns:
            The same edge (for consistency with AgentreplayClient API)
        """
        with self._lock:
            # CRITICAL FIX: Enforce max buffer size to prevent OOM
            if len(self._buffer) >= self.max_buffer_size:
                # Drop oldest edge (FIFO sampling)
                self._buffer.pop(0)
                self._dropped_count += 1

                # Log warning every 1000 drops
                if self._dropped_count % 1000 == 0:
                    print(
                        f"WARNING: Dropped {self._dropped_count} edges due to full buffer. "
                        f"Backend may be slow or down. Consider increasing max_buffer_size "
                        f"or reducing ingestion rate."
                    )

            self._buffer.append(edge)

            # Flush if batch size reached
            if len(self._buffer) >= self.batch_size:
                self._flush_unlocked()

        return edge

    def flush(self) -> int:
        """Manually flush all buffered spans.
        
        Returns:
            Number of spans flushed
        """
        with self._lock:
            return self._flush_unlocked()

    def _flush_unlocked(self) -> int:
        """Flush buffer without acquiring lock (caller must hold lock).
        
        Returns:
            Number of spans flushed
        """
        if not self._buffer:
            return 0

        # Send entire batch in one HTTP request
        try:
            self.client.insert_batch(self._buffer)
            count = len(self._buffer)
            self._buffer = []
            return count
        except Exception as e:
            # Log error but don't lose spans
            print(f"Error flushing batch: {e}")
            return 0

    def _auto_flush(self) -> None:
        """Background thread that flushes buffer periodically.
        
        CRITICAL FIX: Transactional buffer management - data is only removed
        from buffer after successful network I/O. Failed batches are queued
        for retry to prevent data loss.
        """
        while self._running:
            time.sleep(self.flush_interval)
            if self._running:  # Check again after sleep
                # 1. First, try to send any previously failed batches
                self._process_retry_queue()
                
                # 2. Grab data to send (under lock) but DON'T clear buffer yet
                batch_to_send = []
                with self._lock:
                    if self._buffer:
                        batch_to_send = self._buffer[:]  # Copy for sending
                
                # 3. Send I/O outside the lock (won't block application threads)
                if batch_to_send:
                    success = False
                    try:
                        self.client.insert_batch(batch_to_send)
                        success = True
                    except Exception as e:
                        # Log error but don't crash the thread
                        print(f"Error flushing batch: {e}")
                    
                    # 4. ONLY clear buffer after confirmed success (transactional)
                    with self._lock:
                        if success:
                            # Remove only the items we successfully sent
                            # (new items may have been added during I/O)
                            self._buffer = self._buffer[len(batch_to_send):]
                        else:
                            # Failed: queue batch for retry, clear from main buffer
                            # to prevent duplicate sends
                            self._buffer = self._buffer[len(batch_to_send):]
                            self._queue_for_retry(batch_to_send)

    def _process_retry_queue(self) -> None:
        """Process failed batches from retry queue."""
        while self._retry_queue:
            # Pop from front (FIFO)
            with self._lock:
                if not self._retry_queue:
                    break
                batch = self._retry_queue.popleft()
            
            try:
                self.client.insert_batch(batch)
                # Success - batch is now sent, continue to next
            except Exception as e:
                # Still failing - re-queue at the back for later retry
                print(f"Retry failed for batch of {len(batch)} edges: {e}")
                with self._lock:
                    if len(self._retry_queue) < self._max_retry_batches:
                        self._retry_queue.append(batch)
                    else:
                        # Drop batch to prevent unbounded growth
                        self._dropped_count += len(batch)
                        print(f"WARNING: Dropped batch of {len(batch)} edges after max retries")
                break  # Stop processing retry queue on failure

    def _queue_for_retry(self, batch: List[AgentFlowEdge]) -> None:
        """Add failed batch to retry queue (caller must NOT hold lock)."""
        if len(self._retry_queue) < self._max_retry_batches:
            self._retry_queue.append(batch)
        else:
            # Drop oldest retry batch to make room
            dropped = self._retry_queue.popleft()
            self._dropped_count += len(dropped)
            self._retry_queue.append(batch)
            print(f"WARNING: Dropped oldest retry batch ({len(dropped)} edges) to make room")

    def close(self) -> None:
        """Stop auto-flush thread and flush remaining spans including retry queue."""
        self._running = False
        if self._flush_thread.is_alive():
            self._flush_thread.join(timeout=self.flush_interval + 1.0)
        
        # Flush main buffer
        self.flush()
        
        # Attempt to flush retry queue (best effort)
        retry_attempts = 0
        max_close_retries = 3
        while self._retry_queue and retry_attempts < max_close_retries:
            retry_attempts += 1
            with self._lock:
                if not self._retry_queue:
                    break
                batch = self._retry_queue.popleft()
            try:
                self.client.insert_batch(batch)
            except Exception as e:
                print(f"Failed to flush retry queue on close (attempt {retry_attempts}): {e}")
                # Re-queue for next attempt
                with self._lock:
                    self._retry_queue.appendleft(batch)
                break
        
        # Report any remaining data that couldn't be sent
        remaining = sum(len(b) for b in self._retry_queue)
        if remaining > 0:
            print(f"WARNING: {remaining} edges in retry queue could not be sent on close")

    def __del__(self) -> None:
        """Destructor - ensure spans are flushed."""
        try:
            self.close()
        except:
            pass  # Ignore errors during cleanup
