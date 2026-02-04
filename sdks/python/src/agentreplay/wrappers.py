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
SDK client wrappers for automatic instrumentation.

Provides one-liner wrappers for popular LLM SDKs that automatically
trace all API calls without code changes.

Example:
    >>> from openai import OpenAI
    >>> from agentreplay import init, wrap_openai
    >>> 
    >>> init()
    >>> client = wrap_openai(OpenAI())
    >>> 
    >>> # All calls are now traced automatically!
    >>> response = client.chat.completions.create(
    ...     model="gpt-4",
    ...     messages=[{"role": "user", "content": "Hello!"}]
    ... )
"""

import functools
import time
import logging
from typing import TypeVar, Any, Optional, Callable, Dict

logger = logging.getLogger(__name__)

T = TypeVar("T")


# =============================================================================
# OpenAI Wrapper
# =============================================================================

def wrap_openai(client: T, *, capture_content: bool = True) -> T:
    """Wrap an OpenAI client for automatic tracing.
    
    Traces all chat.completions.create() and completions.create() calls,
    automatically capturing:
    - Model name
    - Messages/prompt
    - Response content
    - Token usage
    - Latency
    - Errors
    
    Args:
        client: OpenAI client instance
        capture_content: Whether to capture message content (disable for privacy)
        
    Returns:
        Wrapped client with same interface
        
    Example:
        >>> from openai import OpenAI
        >>> from agentreplay import wrap_openai
        >>> 
        >>> client = wrap_openai(OpenAI())
        >>> 
        >>> response = client.chat.completions.create(
        ...     model="gpt-4",
        ...     messages=[{"role": "user", "content": "Hello!"}]
        ... )  # Automatically traced!
    """
    try:
        from agentreplay.decorators import ActiveSpan, SpanKind, _current_span
        
        # Get original methods
        original_chat_create = client.chat.completions.create
        original_chat_create_async = getattr(
            getattr(client, "chat", None),
            "completions", None
        )
        
        # Check if async client
        is_async = hasattr(client, "_async_client") or "AsyncOpenAI" in type(client).__name__
        
        if is_async:
            return _wrap_openai_async(client, capture_content)
        
        # Wrap sync chat.completions.create
        @functools.wraps(original_chat_create)
        def wrapped_chat_create(*args, **kwargs):
            # Get parent span
            parent = _current_span.get()
            
            # Create span
            span = ActiveSpan(
                name="openai.chat.completions.create",
                kind=SpanKind.LLM,
                parent_id=parent.span_id if parent else None,
                trace_id=parent.trace_id if parent else None,
            )
            
            # Set attributes
            model = kwargs.get("model", "unknown")
            span.set_model(model, provider="openai")
            span.set_attribute("llm.request.type", "chat")
            
            # Capture input
            if capture_content:
                messages = kwargs.get("messages", [])
                span.set_input({"messages": messages})
            
            with span:
                try:
                    response = original_chat_create(*args, **kwargs)
                    
                    # Capture output
                    if capture_content and hasattr(response, "choices") and response.choices:
                        output_content = response.choices[0].message.content
                        span.set_output({"content": output_content})
                    
                    # Capture token usage
                    if hasattr(response, "usage") and response.usage:
                        span.set_token_usage(
                            prompt_tokens=response.usage.prompt_tokens,
                            completion_tokens=response.usage.completion_tokens,
                            total_tokens=response.usage.total_tokens,
                        )
                    
                    # Capture finish reason
                    if hasattr(response, "choices") and response.choices:
                        span.set_attribute(
                            "llm.response.finish_reason",
                            response.choices[0].finish_reason
                        )
                    
                    return response
                    
                except Exception as e:
                    span.set_error(e)
                    raise
        
        # Monkey-patch
        client.chat.completions.create = wrapped_chat_create
        
        # Also wrap embeddings if present
        if hasattr(client, "embeddings"):
            _wrap_openai_embeddings(client, capture_content)
        
        return client
        
    except Exception as e:
        logger.warning(f"Failed to wrap OpenAI client: {e}. Returning unwrapped client.")
        return client


def _wrap_openai_async(client: T, capture_content: bool) -> T:
    """Wrap async OpenAI client."""
    try:
        from agentreplay.decorators import ActiveSpan, SpanKind, _current_span
        
        original_chat_create = client.chat.completions.create
        
        @functools.wraps(original_chat_create)
        async def wrapped_chat_create(*args, **kwargs):
            parent = _current_span.get()
            
            span = ActiveSpan(
                name="openai.chat.completions.create",
                kind=SpanKind.LLM,
                parent_id=parent.span_id if parent else None,
                trace_id=parent.trace_id if parent else None,
            )
            
            model = kwargs.get("model", "unknown")
            span.set_model(model, provider="openai")
            span.set_attribute("llm.request.type", "chat")
            
            if capture_content:
                messages = kwargs.get("messages", [])
                span.set_input({"messages": messages})
            
            with span:
                try:
                    response = await original_chat_create(*args, **kwargs)
                    
                    if capture_content and hasattr(response, "choices") and response.choices:
                        output_content = response.choices[0].message.content
                        span.set_output({"content": output_content})
                    
                    if hasattr(response, "usage") and response.usage:
                        span.set_token_usage(
                            prompt_tokens=response.usage.prompt_tokens,
                            completion_tokens=response.usage.completion_tokens,
                            total_tokens=response.usage.total_tokens,
                        )
                    
                    if hasattr(response, "choices") and response.choices:
                        span.set_attribute(
                            "llm.response.finish_reason",
                            response.choices[0].finish_reason
                        )
                    
                    return response
                    
                except Exception as e:
                    span.set_error(e)
                    raise
        
        client.chat.completions.create = wrapped_chat_create
        return client
        
    except Exception as e:
        logger.warning(f"Failed to wrap async OpenAI client: {e}")
        return client


def _wrap_openai_embeddings(client: T, capture_content: bool) -> None:
    """Wrap OpenAI embeddings."""
    try:
        from agentreplay.decorators import ActiveSpan, SpanKind, _current_span
        
        original_create = client.embeddings.create
        
        @functools.wraps(original_create)
        def wrapped_create(*args, **kwargs):
            parent = _current_span.get()
            
            span = ActiveSpan(
                name="openai.embeddings.create",
                kind=SpanKind.EMBEDDING,
                parent_id=parent.span_id if parent else None,
                trace_id=parent.trace_id if parent else None,
            )
            
            model = kwargs.get("model", "text-embedding-ada-002")
            span.set_model(model, provider="openai")
            span.set_attribute("llm.request.type", "embedding")
            
            # Capture input count (not content for privacy)
            input_data = kwargs.get("input", [])
            if isinstance(input_data, str):
                span.set_attribute("embedding.input_count", 1)
            else:
                span.set_attribute("embedding.input_count", len(input_data))
            
            with span:
                try:
                    response = original_create(*args, **kwargs)
                    
                    if hasattr(response, "usage") and response.usage:
                        span.set_token_usage(
                            prompt_tokens=response.usage.prompt_tokens,
                            total_tokens=response.usage.total_tokens,
                        )
                    
                    if hasattr(response, "data"):
                        span.set_attribute("embedding.output_count", len(response.data))
                    
                    return response
                    
                except Exception as e:
                    span.set_error(e)
                    raise
        
        client.embeddings.create = wrapped_create
        
    except Exception as e:
        logger.debug(f"Failed to wrap embeddings: {e}")


# =============================================================================
# Anthropic Wrapper
# =============================================================================

def wrap_anthropic(client: T, *, capture_content: bool = True) -> T:
    """Wrap an Anthropic client for automatic tracing.
    
    Traces all messages.create() calls, automatically capturing:
    - Model name
    - Messages/prompt
    - Response content
    - Token usage
    - Latency
    - Errors
    
    Args:
        client: Anthropic client instance
        capture_content: Whether to capture message content
        
    Returns:
        Wrapped client with same interface
        
    Example:
        >>> from anthropic import Anthropic
        >>> from agentreplay import wrap_anthropic
        >>> 
        >>> client = wrap_anthropic(Anthropic())
        >>> 
        >>> message = client.messages.create(
        ...     model="claude-3-opus-20240229",
        ...     messages=[{"role": "user", "content": "Hello!"}]
        ... )  # Automatically traced!
    """
    try:
        from agentreplay.decorators import ActiveSpan, SpanKind, _current_span
        
        # Check if async
        is_async = "AsyncAnthropic" in type(client).__name__
        
        if is_async:
            return _wrap_anthropic_async(client, capture_content)
        
        original_messages_create = client.messages.create
        
        @functools.wraps(original_messages_create)
        def wrapped_messages_create(*args, **kwargs):
            parent = _current_span.get()
            
            span = ActiveSpan(
                name="anthropic.messages.create",
                kind=SpanKind.LLM,
                parent_id=parent.span_id if parent else None,
                trace_id=parent.trace_id if parent else None,
            )
            
            model = kwargs.get("model", "claude-3")
            span.set_model(model, provider="anthropic")
            span.set_attribute("llm.request.type", "chat")
            
            if capture_content:
                messages = kwargs.get("messages", [])
                system = kwargs.get("system")
                input_data = {"messages": messages}
                if system:
                    input_data["system"] = system
                span.set_input(input_data)
            
            with span:
                try:
                    response = original_messages_create(*args, **kwargs)
                    
                    if capture_content and hasattr(response, "content") and response.content:
                        content = response.content[0]
                        if hasattr(content, "text"):
                            span.set_output({"content": content.text})
                    
                    if hasattr(response, "usage"):
                        span.set_token_usage(
                            prompt_tokens=response.usage.input_tokens,
                            completion_tokens=response.usage.output_tokens,
                        )
                    
                    if hasattr(response, "stop_reason"):
                        span.set_attribute("llm.response.finish_reason", response.stop_reason)
                    
                    return response
                    
                except Exception as e:
                    span.set_error(e)
                    raise
        
        client.messages.create = wrapped_messages_create
        return client
        
    except Exception as e:
        logger.warning(f"Failed to wrap Anthropic client: {e}. Returning unwrapped client.")
        return client


def _wrap_anthropic_async(client: T, capture_content: bool) -> T:
    """Wrap async Anthropic client."""
    try:
        from agentreplay.decorators import ActiveSpan, SpanKind, _current_span
        
        original_messages_create = client.messages.create
        
        @functools.wraps(original_messages_create)
        async def wrapped_messages_create(*args, **kwargs):
            parent = _current_span.get()
            
            span = ActiveSpan(
                name="anthropic.messages.create",
                kind=SpanKind.LLM,
                parent_id=parent.span_id if parent else None,
                trace_id=parent.trace_id if parent else None,
            )
            
            model = kwargs.get("model", "claude-3")
            span.set_model(model, provider="anthropic")
            span.set_attribute("llm.request.type", "chat")
            
            if capture_content:
                messages = kwargs.get("messages", [])
                system = kwargs.get("system")
                input_data = {"messages": messages}
                if system:
                    input_data["system"] = system
                span.set_input(input_data)
            
            with span:
                try:
                    response = await original_messages_create(*args, **kwargs)
                    
                    if capture_content and hasattr(response, "content") and response.content:
                        content = response.content[0]
                        if hasattr(content, "text"):
                            span.set_output({"content": content.text})
                    
                    if hasattr(response, "usage"):
                        span.set_token_usage(
                            prompt_tokens=response.usage.input_tokens,
                            completion_tokens=response.usage.output_tokens,
                        )
                    
                    if hasattr(response, "stop_reason"):
                        span.set_attribute("llm.response.finish_reason", response.stop_reason)
                    
                    return response
                    
                except Exception as e:
                    span.set_error(e)
                    raise
        
        client.messages.create = wrapped_messages_create
        return client
        
    except Exception as e:
        logger.warning(f"Failed to wrap async Anthropic client: {e}")
        return client


# =============================================================================
# Generic Wrapper
# =============================================================================

def wrap_method(
    obj: Any,
    method_name: str,
    *,
    span_name: Optional[str] = None,
    kind: str = "chain",
    capture_input: bool = True,
    capture_output: bool = True,
) -> None:
    """Wrap a specific method on an object for tracing.
    
    Low-level utility for custom instrumentation.
    
    Args:
        obj: Object containing the method
        method_name: Name of the method to wrap
        span_name: Name for the span (default: method name)
        kind: Span kind
        capture_input: Whether to capture method args
        capture_output: Whether to capture return value
        
    Example:
        >>> wrap_method(my_service, "call_api", span_name="api.call", kind="http")
    """
    from agentreplay.decorators import ActiveSpan, _current_span
    import inspect
    
    original_method = getattr(obj, method_name)
    name = span_name or method_name
    
    if inspect.iscoroutinefunction(original_method):
        @functools.wraps(original_method)
        async def async_wrapped(*args, **kwargs):
            parent = _current_span.get()
            span = ActiveSpan(
                name=name,
                kind=kind,
                parent_id=parent.span_id if parent else None,
                trace_id=parent.trace_id if parent else None,
            )
            
            if capture_input:
                span.set_input({"args": args, "kwargs": kwargs})
            
            with span:
                try:
                    result = await original_method(*args, **kwargs)
                    if capture_output:
                        span.set_output(result)
                    return result
                except Exception as e:
                    span.set_error(e)
                    raise
        
        setattr(obj, method_name, async_wrapped)
    else:
        @functools.wraps(original_method)
        def sync_wrapped(*args, **kwargs):
            parent = _current_span.get()
            span = ActiveSpan(
                name=name,
                kind=kind,
                parent_id=parent.span_id if parent else None,
                trace_id=parent.trace_id if parent else None,
            )
            
            if capture_input:
                span.set_input({"args": args, "kwargs": kwargs})
            
            with span:
                try:
                    result = original_method(*args, **kwargs)
                    if capture_output:
                        span.set_output(result)
                    return result
                except Exception as e:
                    span.set_error(e)
                    raise
        
        setattr(obj, method_name, sync_wrapped)
