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

"""
Decorator-based tracing for Agentreplay.

Provides @traceable and @observe decorators for easy function instrumentation.

Example:
    >>> from agentreplay import init, traceable
    >>> 
    >>> init()
    >>> 
    >>> @traceable
    >>> def my_function(query: str) -> str:
    ...     return f"Result for {query}"
    >>> 
    >>> result = my_function("hello")  # Automatically traced!
"""

import functools
import inspect
import time
import logging
from typing import (
    Optional, Callable, TypeVar, Any, Dict, Union, 
    overload, ParamSpec, Awaitable
)
from contextvars import ContextVar

logger = logging.getLogger(__name__)

# Type variables for generic decorators
P = ParamSpec("P")
R = TypeVar("R")

# Context variable for current span
_current_span: ContextVar[Optional[Any]] = ContextVar("current_span", default=None)


# =============================================================================
# Span Kind
# =============================================================================

class SpanKind:
    """Span kind constants for categorizing operations."""
    CHAIN = "chain"
    LLM = "llm"
    TOOL = "tool"
    RETRIEVER = "retriever"
    EMBEDDING = "embedding"
    GUARDRAIL = "guardrail"
    CACHE = "cache"
    HTTP = "http"
    DB = "db"


# =============================================================================
# Active Span
# =============================================================================

class ActiveSpan:
    """Active span with methods to add data.
    
    This is yielded by the trace() context manager and passed to
    decorated functions.
    """
    
    def __init__(
        self,
        name: str,
        kind: str = SpanKind.CHAIN,
        span_id: Optional[str] = None,
        parent_id: Optional[str] = None,
        trace_id: Optional[str] = None,
    ):
        self.name = name
        self.kind = kind
        self.span_id = span_id or self._generate_id()
        self.parent_id = parent_id
        self.trace_id = trace_id or self._generate_id()
        self.start_time = time.time()
        self.end_time: Optional[float] = None
        self.attributes: Dict[str, Any] = {}
        self.events: list = []
        self.input_data: Optional[Any] = None
        self.output_data: Optional[Any] = None
        self.error: Optional[Exception] = None
        self.token_usage: Dict[str, int] = {}
        self._ended = False
    
    @staticmethod
    def _generate_id() -> str:
        """Generate unique span ID."""
        import uuid
        return uuid.uuid4().hex[:16]
    
    def set_input(self, data: Any) -> "ActiveSpan":
        """Set input data."""
        self.input_data = data
        return self
    
    def set_output(self, data: Any) -> "ActiveSpan":
        """Set output data."""
        self.output_data = data
        return self
    
    def set_attribute(self, key: str, value: Any) -> "ActiveSpan":
        """Set a span attribute."""
        self.attributes[key] = value
        return self
    
    def set_attributes(self, attributes: Dict[str, Any]) -> "ActiveSpan":
        """Set multiple attributes."""
        self.attributes.update(attributes)
        return self
    
    def add_event(self, name: str, attributes: Optional[Dict[str, Any]] = None) -> "ActiveSpan":
        """Add an event to the span."""
        self.events.append({
            "name": name,
            "timestamp": time.time(),
            "attributes": attributes or {},
        })
        return self
    
    def set_error(self, error: Exception) -> "ActiveSpan":
        """Set error on span."""
        self.error = error
        self.attributes["error.type"] = type(error).__name__
        self.attributes["error.message"] = str(error)
        import traceback
        self.attributes["error.stack"] = traceback.format_exc()
        return self
    
    def set_token_usage(
        self,
        prompt_tokens: Optional[int] = None,
        completion_tokens: Optional[int] = None,
        total_tokens: Optional[int] = None,
    ) -> "ActiveSpan":
        """Set token usage for LLM calls."""
        if prompt_tokens is not None:
            self.token_usage["prompt"] = prompt_tokens
            self.attributes["gen_ai.usage.prompt_tokens"] = prompt_tokens
        if completion_tokens is not None:
            self.token_usage["completion"] = completion_tokens
            self.attributes["gen_ai.usage.completion_tokens"] = completion_tokens
        if total_tokens is not None:
            self.token_usage["total"] = total_tokens
            self.attributes["gen_ai.usage.total_tokens"] = total_tokens
        return self
    
    def set_model(self, model: str, provider: Optional[str] = None) -> "ActiveSpan":
        """Set model information."""
        self.attributes["gen_ai.request.model"] = model
        if provider:
            self.attributes["gen_ai.system"] = provider
        return self
    
    def end(self) -> None:
        """End the span and send to backend."""
        if self._ended:
            return
        
        self._ended = True
        self.end_time = time.time()
        
        # Send to backend
        self._send()
    
    def _send(self) -> None:
        """Send span to Agentreplay backend."""
        try:
            from agentreplay.sdk import get_batching_client, is_initialized, get_config
            
            if not is_initialized():
                return
            
            config = get_config()
            if not config.enabled:
                return
            
            # Build edge
            from agentreplay.models import AgentFlowEdge, SpanType
            
            # Map kind to SpanType
            span_type_map = {
                SpanKind.CHAIN: SpanType.ROOT,
                SpanKind.LLM: SpanType.TOOL_CALL,
                SpanKind.TOOL: SpanType.TOOL_CALL,
                SpanKind.RETRIEVER: SpanType.TOOL_CALL,
                SpanKind.EMBEDDING: SpanType.TOOL_CALL,
            }
            span_type = span_type_map.get(self.kind, SpanType.ROOT)
            
            # Calculate duration
            duration_us = int((self.end_time - self.start_time) * 1_000_000) if self.end_time else 0
            
            # Build payload
            payload = {}
            if self.input_data is not None and config.capture_input:
                payload["input"] = self._safe_serialize(self.input_data)
            if self.output_data is not None and config.capture_output:
                payload["output"] = self._safe_serialize(self.output_data)
            if self.attributes:
                payload["attributes"] = self.attributes
            if self.events:
                payload["events"] = self.events
            if self.error:
                payload["error"] = {
                    "type": type(self.error).__name__,
                    "message": str(self.error),
                }
            
            edge = AgentFlowEdge(
                tenant_id=config.tenant_id,
                project_id=config.project_id,
                agent_id=config.agent_id,
                session_id=int(self.trace_id[:8], 16) if self.trace_id else 0,
                span_type=span_type,
                timestamp_us=int(self.start_time * 1_000_000),
                duration_us=duration_us,
                token_count=self.token_usage.get("total", 0),
                payload=payload,
            )
            
            # Send via batching client
            client = get_batching_client()
            client.insert(edge)
            
        except Exception as e:
            logger.debug(f"Failed to send span: {e}")
    
    def _safe_serialize(self, data: Any, max_size: int = 10000) -> Any:
        """Safely serialize data with size limits."""
        import json
        
        try:
            serialized = json.dumps(data, default=str)
            if len(serialized) > max_size:
                return {"__truncated": True, "__preview": serialized[:1000]}
            return data
        except Exception:
            return str(data)[:max_size]
    
    def __enter__(self) -> "ActiveSpan":
        """Context manager entry."""
        # Set as current span
        self._token = _current_span.set(self)
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit."""
        # Capture error if any
        if exc_val is not None:
            self.set_error(exc_val)
        
        # End span
        self.end()
        
        # Reset current span
        _current_span.reset(self._token)


# =============================================================================
# Get Current Span
# =============================================================================

def get_current_span() -> Optional[ActiveSpan]:
    """Get the currently active span.
    
    Returns:
        ActiveSpan if inside a traced context, None otherwise
    """
    return _current_span.get()


# =============================================================================
# Traceable Decorator
# =============================================================================

@overload
def traceable(func: Callable[P, R]) -> Callable[P, R]: ...

@overload
def traceable(
    *,
    name: Optional[str] = None,
    kind: str = SpanKind.CHAIN,
    capture_input: bool = True,
    capture_output: bool = True,
    metadata: Optional[Dict[str, Any]] = None,
) -> Callable[[Callable[P, R]], Callable[P, R]]: ...


def traceable(
    func: Optional[Callable[P, R]] = None,
    *,
    name: Optional[str] = None,
    kind: str = SpanKind.CHAIN,
    capture_input: bool = True,
    capture_output: bool = True,
    metadata: Optional[Dict[str, Any]] = None,
) -> Union[Callable[P, R], Callable[[Callable[P, R]], Callable[P, R]]]:
    """Decorator to trace a function.
    
    Works with both sync and async functions. Automatically captures
    inputs, outputs, errors, and timing.
    
    Args:
        func: Function to decorate (when used without parentheses)
        name: Span name (default: function name)
        kind: Span kind (chain, llm, tool, retriever, etc.)
        capture_input: Whether to capture function inputs
        capture_output: Whether to capture function output
        metadata: Additional metadata to attach
        
    Returns:
        Decorated function
        
    Example:
        >>> @traceable
        >>> def simple_function():
        ...     return "hello"
        
        >>> @traceable(name="my_operation", kind="tool")
        >>> def tool_function(query: str):
        ...     return search(query)
        
        >>> @traceable(capture_input=False)  # Don't capture sensitive inputs
        >>> def sensitive_function(password: str):
        ...     return authenticate(password)
    """
    def decorator(fn: Callable[P, R]) -> Callable[P, R]:
        span_name = name or fn.__name__
        
        # Check if async
        if inspect.iscoroutinefunction(fn):
            @functools.wraps(fn)
            async def async_wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
                # Get parent span
                parent = get_current_span()
                
                # Create span
                span = ActiveSpan(
                    name=span_name,
                    kind=kind,
                    parent_id=parent.span_id if parent else None,
                    trace_id=parent.trace_id if parent else None,
                )
                
                # Add metadata
                if metadata:
                    span.set_attributes(metadata)
                
                # Capture input
                if capture_input:
                    try:
                        input_data = _capture_args(fn, args, kwargs)
                        span.set_input(input_data)
                    except Exception:
                        pass
                
                # Execute with span context
                with span:
                    try:
                        result = await fn(*args, **kwargs)
                        
                        # Capture output
                        if capture_output:
                            span.set_output(result)
                        
                        return result
                    except Exception as e:
                        span.set_error(e)
                        raise
            
            return async_wrapper
        else:
            @functools.wraps(fn)
            def sync_wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
                # Get parent span
                parent = get_current_span()
                
                # Create span
                span = ActiveSpan(
                    name=span_name,
                    kind=kind,
                    parent_id=parent.span_id if parent else None,
                    trace_id=parent.trace_id if parent else None,
                )
                
                # Add metadata
                if metadata:
                    span.set_attributes(metadata)
                
                # Capture input
                if capture_input:
                    try:
                        input_data = _capture_args(fn, args, kwargs)
                        span.set_input(input_data)
                    except Exception:
                        pass
                
                # Execute with span context
                with span:
                    try:
                        result = fn(*args, **kwargs)
                        
                        # Capture output
                        if capture_output:
                            span.set_output(result)
                        
                        return result
                    except Exception as e:
                        span.set_error(e)
                        raise
            
            return sync_wrapper
    
    # Handle @traceable vs @traceable()
    if func is not None:
        return decorator(func)
    return decorator


# Alias for Langfuse-style API
observe = traceable


def _capture_args(fn: Callable, args: tuple, kwargs: dict) -> Dict[str, Any]:
    """Capture function arguments as a dict."""
    sig = inspect.signature(fn)
    params = list(sig.parameters.keys())
    
    result = {}
    for i, arg in enumerate(args):
        if i < len(params):
            result[params[i]] = arg
        else:
            result[f"arg_{i}"] = arg
    
    result.update(kwargs)
    return result


# =============================================================================
# Trace Context Manager
# =============================================================================

def trace(
    name: str,
    *,
    kind: str = SpanKind.CHAIN,
    input: Optional[Any] = None,
    metadata: Optional[Dict[str, Any]] = None,
) -> ActiveSpan:
    """Create a trace span as a context manager.
    
    Args:
        name: Span name
        kind: Span kind (chain, llm, tool, retriever, etc.)
        input: Input data to record
        metadata: Additional metadata
        
    Returns:
        ActiveSpan context manager
        
    Example:
        >>> with trace("retrieve_documents", kind="retriever") as span:
        ...     docs = vector_db.search(query)
        ...     span.set_output({"count": len(docs)})
        ...     return docs
    """
    # Get parent span
    parent = get_current_span()
    
    # Create span
    span = ActiveSpan(
        name=name,
        kind=kind,
        parent_id=parent.span_id if parent else None,
        trace_id=parent.trace_id if parent else None,
    )
    
    # Set input
    if input is not None:
        span.set_input(input)
    
    # Set metadata
    if metadata:
        span.set_attributes(metadata)
    
    return span


def start_span(
    name: str,
    *,
    kind: str = SpanKind.CHAIN,
    input: Optional[Any] = None,
    metadata: Optional[Dict[str, Any]] = None,
) -> ActiveSpan:
    """Start a manual span (must call span.end()).
    
    Use trace() context manager when possible. This is for cases
    where you need manual control over span lifetime.
    
    Args:
        name: Span name
        kind: Span kind
        input: Input data
        metadata: Additional metadata
        
    Returns:
        ActiveSpan (call .end() when done)
        
    Example:
        >>> span = start_span("long_operation", kind="tool")
        >>> try:
        ...     result = do_something()
        ...     span.set_output(result)
        >>> except Exception as e:
        ...     span.set_error(e)
        ...     raise
        >>> finally:
        ...     span.end()
    """
    return trace(name, kind=kind, input=input, metadata=metadata)
