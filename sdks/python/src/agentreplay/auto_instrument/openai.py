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

"""OpenAI-specific instrumentation for streaming and tool calls.

This module provides custom wrappers for OpenAI API calls to:
1. Handle streaming responses (sync and async)
2. Capture tool calls and their results
3. Inject agent context into spans
4. Respect content capture settings

The wrappers are designed to work alongside the official OpenTelemetry
OpenAI instrumentation, adding Agentreplay-specific enhancements.
"""

import logging
from typing import Iterator, AsyncIterator, Optional, Any, Dict, List
import json

logger = logging.getLogger(__name__)

# Configuration from environment
import os
CAPTURE_CONTENT = os.getenv("AGENTREPLAY_CAPTURE_CONTENT", "true").lower() in {
    "1", "true", "yes"
}
MAX_CONTENT_LENGTH = int(os.getenv("AGENTREPLAY_MAX_CONTENT_LENGTH", "10000"))


def is_streaming(response: Any) -> bool:
    """Check if an OpenAI response is a stream.
    
    Args:
        response: OpenAI API response
    
    Returns:
        True if response is a stream, False otherwise
    """
    # Check for stream attribute or iterator protocol
    if hasattr(response, "__iter__") and not isinstance(response, (str, bytes, dict)):
        return True
    if hasattr(response, "__aiter__"):
        return True
    return False


class _StreamWrapper:
    """Wrapper for synchronous OpenAI streaming responses.
    
    This wrapper:
    - Yields chunks to the caller transparently
    - Accumulates content for span attributes
    - Handles tool calls in streaming mode
    - Respects MAX_CONTENT_LENGTH
    
    Example:
        >>> stream = client.chat.completions.create(..., stream=True)
        >>> wrapped = _StreamWrapper(stream, span)
        >>> for chunk in wrapped:
        ...     print(chunk.choices[0].delta.content)
    """
    
    def __init__(self, stream: Iterator, span: Optional[Any] = None):
        """Initialize stream wrapper.
        
        Args:
            stream: Original OpenAI stream
            span: OpenTelemetry span to annotate (optional)
        """
        self.stream = stream
        self.span = span
        self.accumulated_content = []
        # Tool calls accumulator: Dict[int, Dict] keyed by tool call index
        # OpenAI streams tool calls as deltas that need to be merged by index
        self.tool_calls_by_index: Dict[int, Dict[str, Any]] = {}
        self.total_length = 0
        self.chunk_count = 0
        self._capture_content = CAPTURE_CONTENT
    
    def __iter__(self):
        return self
    
    def __next__(self):
        """Get next chunk from stream and capture metadata."""
        try:
            chunk = next(self.stream)
            self.chunk_count += 1
            
            # Extract content if available
            if hasattr(chunk, "choices") and len(chunk.choices) > 0:
                choice = chunk.choices[0]
                
                # Capture text content
                if hasattr(choice, "delta") and hasattr(choice.delta, "content"):
                    content = choice.delta.content
                    if content and self._capture_content:
                        self.accumulated_content.append(content)
                        self.total_length += len(content)
                        
                        # Stop accumulating if we exceed max length
                        if self.total_length > MAX_CONTENT_LENGTH:
                            self.accumulated_content.append(
                                f"... (truncated, total {self.total_length} chars)"
                            )
                            self._capture_content = False  # Disable for rest of stream
                
                # Capture tool calls - merge deltas by index
                if hasattr(choice.delta, "tool_calls") and choice.delta.tool_calls:
                    for tool_call in choice.delta.tool_calls:
                        # Get the index for this tool call (OpenAI sends this)
                        idx = getattr(tool_call, "index", 0)
                        
                        # Initialize if first delta for this index
                        if idx not in self.tool_calls_by_index:
                            self.tool_calls_by_index[idx] = {
                                "id": None,
                                "name": None,
                                "arguments": "",
                            }
                        
                        acc = self.tool_calls_by_index[idx]
                        
                        # Merge id (usually only in first delta)
                        if hasattr(tool_call, "id") and tool_call.id:
                            acc["id"] = tool_call.id
                        
                        # Merge function name and arguments
                        if hasattr(tool_call, "function"):
                            func = tool_call.function
                            if hasattr(func, "name") and func.name:
                                acc["name"] = func.name
                            if hasattr(func, "arguments") and func.arguments:
                                # Arguments come as fragments, concatenate them
                                acc["arguments"] += func.arguments
            
            return chunk
            
        except StopIteration:
            # Stream ended, finalize span
            self._finalize_span()
            raise
    
    def _finalize_span(self):
        """Add accumulated data to span when stream completes."""
        if not self.span:
            return
        
        try:
            # Add accumulated content
            if self.accumulated_content:
                full_content = "".join(self.accumulated_content)
                if self._capture_content:
                    self.span.set_attribute("llm.response.content", full_content[:MAX_CONTENT_LENGTH])
                self.span.set_attribute("llm.response.length", self.total_length)
            
            # Add tool calls (merged by index)
            if self.tool_calls_by_index:
                # Sort by index for consistent ordering
                sorted_tool_calls = [
                    self.tool_calls_by_index[idx] 
                    for idx in sorted(self.tool_calls_by_index.keys())
                ]
                self.span.set_attribute("llm.tool_calls.count", len(sorted_tool_calls))
                for i, tool_call in enumerate(sorted_tool_calls[:10]):  # Max 10
                    if tool_call.get("name"):
                        self.span.set_attribute(f"llm.tool_call.{i}.name", tool_call["name"])
                    if tool_call.get("id"):
                        self.span.set_attribute(f"llm.tool_call.{i}.id", tool_call["id"])
                    if tool_call.get("arguments") and self._capture_content:
                        args = tool_call["arguments"][:500]  # Truncate args
                        self.span.set_attribute(f"llm.tool_call.{i}.arguments", args)
            
            # Add streaming metadata
            self.span.set_attribute("llm.streaming", True)
            self.span.set_attribute("llm.stream.chunks", self.chunk_count)
            
        except Exception as e:
            logger.debug(f"Failed to finalize stream span: {e}")


class _AsyncStreamWrapper:
    """Wrapper for asynchronous OpenAI streaming responses.
    
    Similar to _StreamWrapper but for async/await code.
    
    Example:
        >>> stream = await client.chat.completions.create(..., stream=True)
        >>> wrapped = _AsyncStreamWrapper(stream, span)
        >>> async for chunk in wrapped:
        ...     print(chunk.choices[0].delta.content)
    """
    
    def __init__(self, stream: AsyncIterator, span: Optional[Any] = None):
        """Initialize async stream wrapper.
        
        Args:
            stream: Original OpenAI async stream
            span: OpenTelemetry span to annotate (optional)
        """
        self.stream = stream
        self.span = span
        self.accumulated_content = []
        # Tool calls accumulator: Dict[int, Dict] keyed by tool call index
        # OpenAI streams tool calls as deltas that need to be merged by index
        self.tool_calls_by_index: Dict[int, Dict[str, Any]] = {}
        self.total_length = 0
        self.chunk_count = 0
        self._capture_content = CAPTURE_CONTENT
    
    def __aiter__(self):
        return self
    
    async def __anext__(self):
        """Get next chunk from async stream and capture metadata."""
        try:
            chunk = await self.stream.__anext__()
            self.chunk_count += 1
            
            # Extract content if available
            if hasattr(chunk, "choices") and len(chunk.choices) > 0:
                choice = chunk.choices[0]
                
                # Capture text content
                if hasattr(choice, "delta") and hasattr(choice.delta, "content"):
                    content = choice.delta.content
                    if content and self._capture_content:
                        self.accumulated_content.append(content)
                        self.total_length += len(content)
                        
                        # Stop accumulating if we exceed max length
                        if self.total_length > MAX_CONTENT_LENGTH:
                            self.accumulated_content.append(
                                f"... (truncated, total {self.total_length} chars)"
                            )
                            self._capture_content = False
                
                # Capture tool calls - merge deltas by index
                if hasattr(choice.delta, "tool_calls") and choice.delta.tool_calls:
                    for tool_call in choice.delta.tool_calls:
                        # Get the index for this tool call (OpenAI sends this)
                        idx = getattr(tool_call, "index", 0)
                        
                        # Initialize if first delta for this index
                        if idx not in self.tool_calls_by_index:
                            self.tool_calls_by_index[idx] = {
                                "id": None,
                                "name": None,
                                "arguments": "",
                            }
                        
                        acc = self.tool_calls_by_index[idx]
                        
                        # Merge id (usually only in first delta)
                        if hasattr(tool_call, "id") and tool_call.id:
                            acc["id"] = tool_call.id
                        
                        # Merge function name and arguments
                        if hasattr(tool_call, "function"):
                            func = tool_call.function
                            if hasattr(func, "name") and func.name:
                                acc["name"] = func.name
                            if hasattr(func, "arguments") and func.arguments:
                                # Arguments come as fragments, concatenate them
                                acc["arguments"] += func.arguments
            
            return chunk
            
        except StopAsyncIteration:
            # Stream ended, finalize span
            self._finalize_span()
            raise
    
    def _finalize_span(self):
        """Add accumulated data to span when stream completes."""
        if not self.span:
            return
        
        try:
            # Add accumulated content
            if self.accumulated_content:
                full_content = "".join(self.accumulated_content)
                if self._capture_content:
                    self.span.set_attribute("llm.response.content", full_content[:MAX_CONTENT_LENGTH])
                self.span.set_attribute("llm.response.length", self.total_length)
            
            # Add tool calls (merged by index)
            if self.tool_calls_by_index:
                # Sort by index for consistent ordering
                sorted_tool_calls = [
                    self.tool_calls_by_index[idx] 
                    for idx in sorted(self.tool_calls_by_index.keys())
                ]
                self.span.set_attribute("llm.tool_calls.count", len(sorted_tool_calls))
                for i, tool_call in enumerate(sorted_tool_calls[:10]):  # Max 10
                    if tool_call.get("name"):
                        self.span.set_attribute(f"llm.tool_call.{i}.name", tool_call["name"])
                    if tool_call.get("id"):
                        self.span.set_attribute(f"llm.tool_call.{i}.id", tool_call["id"])
                    if tool_call.get("arguments") and self._capture_content:
                        args = tool_call["arguments"][:500]  # Truncate args
                        self.span.set_attribute(f"llm.tool_call.{i}.arguments", args)
            
            # Add streaming metadata
            self.span.set_attribute("llm.streaming", True)
            self.span.set_attribute("llm.stream.chunks", self.chunk_count)
            
        except Exception as e:
            logger.debug(f"Failed to finalize async stream span: {e}")


def _inject_agent_context(span: Any):
    """Inject current agent context into span attributes.
    
    Reads from contextvars set by AgentContext and adds them to the span.
    
    Args:
        span: OpenTelemetry span
    """
    try:
        from agentreplay.context import (
            get_current_agent_id,
            get_current_session_id,
            get_current_workflow_id,
            get_current_user_id,
        )
        
        agent_id = get_current_agent_id()
        if agent_id:
            span.set_attribute("agentreplay.agent_id", agent_id)
        
        session_id = get_current_session_id()
        if session_id:
            span.set_attribute("agentreplay.session_id", session_id)
        
        workflow_id = get_current_workflow_id()
        if workflow_id:
            span.set_attribute("agentreplay.workflow_id", workflow_id)
        
        user_id = get_current_user_id()
        if user_id:
            span.set_attribute("agentreplay.user_id", user_id)
    
    except ImportError:
        # Context module not available
        pass
    except Exception as e:
        logger.debug(f"Failed to inject agent context: {e}")


def extract_tool_calls(response: Any) -> List[Dict[str, Any]]:
    """Extract tool calls from OpenAI response.
    
    Args:
        response: OpenAI chat completion response
    
    Returns:
        List of tool call dictionaries with id, name, arguments
    """
    tool_calls = []
    
    try:
        if not hasattr(response, "choices") or not response.choices:
            return tool_calls
        
        choice = response.choices[0]
        if not hasattr(choice, "message") or not hasattr(choice.message, "tool_calls"):
            return tool_calls
        
        if not choice.message.tool_calls:
            return tool_calls
        
        for tool_call in choice.message.tool_calls:
            if hasattr(tool_call, "function"):
                tool_calls.append({
                    "id": getattr(tool_call, "id", None),
                    "type": getattr(tool_call, "type", "function"),
                    "name": getattr(tool_call.function, "name", None),
                    "arguments": getattr(tool_call.function, "arguments", None),
                })
    
    except Exception as e:
        logger.debug(f"Failed to extract tool calls: {e}")
    
    return tool_calls


def annotate_span_with_tool_calls(span: Any, tool_calls: List[Dict[str, Any]]):
    """Add tool call information to span attributes.
    
    Args:
        span: OpenTelemetry span
        tool_calls: List of tool call dictionaries
    """
    if not tool_calls:
        return
    
    try:
        span.set_attribute("llm.tool_calls.count", len(tool_calls))
        
        for i, tool_call in enumerate(tool_calls[:10]):  # Max 10 tool calls
            prefix = f"llm.tool_call.{i}"
            
            if tool_call.get("id"):
                span.set_attribute(f"{prefix}.id", tool_call["id"])
            
            if tool_call.get("name"):
                span.set_attribute(f"{prefix}.name", tool_call["name"])
            
            if tool_call.get("type"):
                span.set_attribute(f"{prefix}.type", tool_call["type"])
            
            if tool_call.get("arguments") and CAPTURE_CONTENT:
                args = tool_call["arguments"]
                if isinstance(args, str):
                    # Truncate if too long
                    args = args[:500]
                    span.set_attribute(f"{prefix}.arguments", args)
    
    except Exception as e:
        logger.debug(f"Failed to annotate span with tool calls: {e}")
