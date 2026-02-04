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

"""Agent Context Tracking for Multi-Agent Systems.

This module provides contextvars-based context propagation for tracking
agent execution in multi-agent systems like CrewAI, AutoGen, and LangGraph.

Example:
    >>> from agentreplay.context import AgentContext
    >>> 
    >>> with AgentContext(agent_id="researcher", session_id="sess-123"):
    ...     # All LLM calls here get tagged with agent_id="researcher"
    ...     response = client.chat.completions.create(...)
"""

from contextvars import ContextVar
from typing import Optional

# Define context variables
_agent_id: ContextVar[Optional[str]] = ContextVar('agent_id', default=None)
_session_id: ContextVar[Optional[str]] = ContextVar('session_id', default=None)
_workflow_id: ContextVar[Optional[str]] = ContextVar('workflow_id', default=None)
_user_id: ContextVar[Optional[str]] = ContextVar('user_id', default=None)


class AgentContext:
    """Context manager for tracking agent execution.
    
    This context manager sets context variables that are automatically
    propagated to all LLM calls made within the context. It works with
    both synchronous and asynchronous code.
    
    Args:
        agent_id: Unique identifier for the agent
        session_id: Session identifier (optional)
        workflow_id: Workflow identifier (optional)
        user_id: User identifier (optional)
    
    Example:
        >>> with AgentContext(agent_id="researcher", session_id="sess-123"):
        ...     # All LLM calls here are automatically tagged
        ...     response = openai_client.chat.completions.create(...)
    """
    
    def __init__(
        self, 
        agent_id: str, 
        session_id: Optional[str] = None,
        workflow_id: Optional[str] = None,
        user_id: Optional[str] = None
    ):
        self.agent_id = agent_id
        self.session_id = session_id
        self.workflow_id = workflow_id
        self.user_id = user_id
        self.tokens = []
    
    def __enter__(self):
        # Set context variables, store tokens for cleanup
        self.tokens.append(_agent_id.set(self.agent_id))
        if self.session_id:
            self.tokens.append(_session_id.set(self.session_id))
        if self.workflow_id:
            self.tokens.append(_workflow_id.set(self.workflow_id))
        if self.user_id:
            self.tokens.append(_user_id.set(self.user_id))
        return self
    
    def __exit__(self, *args):
        # Reset context variables in reverse order
        for token in reversed(self.tokens):
            token.var.reset(token)


def get_current_agent_id() -> Optional[str]:
    """Get the current agent ID from context.
    
    Returns:
        Agent ID if set, None otherwise
    """
    return _agent_id.get()


def get_current_session_id() -> Optional[str]:
    """Get the current session ID from context.
    
    Returns:
        Session ID if set, None otherwise
    """
    return _session_id.get()


def get_current_workflow_id() -> Optional[str]:
    """Get the current workflow ID from context.
    
    Returns:
        Workflow ID if set, None otherwise
    """
    return _workflow_id.get()


def get_current_user_id() -> Optional[str]:
    """Get the current user ID from context.
    
    Returns:
        User ID if set, None otherwise
    """
    return _user_id.get()


def set_agent_id(agent_id: str):
    """Set the agent ID in the current context.
    
    Args:
        agent_id: Agent identifier
    
    Returns:
        Token that can be used to reset the context
    """
    return _agent_id.set(agent_id)


def set_session_id(session_id: str):
    """Set the session ID in the current context.
    
    Args:
        session_id: Session identifier
    
    Returns:
        Token that can be used to reset the context
    """
    return _session_id.set(session_id)


def set_workflow_id(workflow_id: str):
    """Set the workflow ID in the current context.
    
    Args:
        workflow_id: Workflow identifier
    
    Returns:
        Token that can be used to reset the context
    """
    return _workflow_id.set(workflow_id)


def set_user_id(user_id: str):
    """Set the user ID in the current context.
    
    Args:
        user_id: User identifier
    
    Returns:
        Token that can be used to reset the context
    """
    return _user_id.set(user_id)


# =============================================================================
# Ergonomic set_context Function
# =============================================================================

# Global context for all spans (merged with span-level context)
_global_context: dict = {}


def set_context(
    *,
    user_id: Optional[str] = None,
    session_id: Optional[str] = None,
    agent_id: Optional[str] = None,
    workflow_id: Optional[str] = None,
    **extra: str,
) -> None:
    """Set global context that applies to all subsequent spans.
    
    This is a convenience function for setting context that should
    apply to all traces/spans. The context persists until reset.
    
    For request-scoped context, use the `AgentContext` context manager
    or pass context directly to `@traceable` or `trace()`.
    
    Args:
        user_id: User identifier
        session_id: Session identifier
        agent_id: Agent identifier
        workflow_id: Workflow identifier
        **extra: Additional key-value pairs
        
    Example:
        >>> from agentreplay import init, set_context
        >>> 
        >>> init()
        >>> set_context(user_id="user-123", session_id="session-456")
        >>> 
        >>> # All subsequent traces include this context
        >>> @traceable
        >>> def my_function():
        ...     return "hello"
    """
    global _global_context
    
    if user_id is not None:
        _global_context["user_id"] = user_id
        set_user_id(user_id)
    
    if session_id is not None:
        _global_context["session_id"] = session_id
        set_session_id(session_id)
    
    if agent_id is not None:
        _global_context["agent_id"] = agent_id
        set_agent_id(agent_id)
    
    if workflow_id is not None:
        _global_context["workflow_id"] = workflow_id
        set_workflow_id(workflow_id)
    
    # Store extra context
    _global_context.update(extra)


def get_global_context() -> dict:
    """Get the current global context.
    
    Returns:
        Dictionary of global context key-value pairs
    """
    return _global_context.copy()


def clear_context() -> None:
    """Clear all global context."""
    global _global_context
    _global_context = {}


def with_context(**context) -> "ContextScope":
    """Create a context scope for the current async context.
    
    This is like `set_context` but only applies within the current
    async task or thread, and automatically cleans up when the scope exits.
    
    Args:
        **context: Key-value pairs to set in context
        
    Returns:
        ContextScope context manager
        
    Example:
        >>> async def handle_request(user_id: str):
        ...     with with_context(user_id=user_id, request_id="req-123"):
        ...         # Context only applies within this block
        ...         result = await my_traced_function()
        ...     return result
    """
    return ContextScope(**context)


class ContextScope:
    """Context manager for request-scoped context.
    
    Automatically sets and resets context variables.
    """
    
    def __init__(self, **context):
        self.context = context
        self.tokens = []
    
    def __enter__(self):
        if "user_id" in self.context:
            self.tokens.append(_user_id.set(self.context["user_id"]))
        if "session_id" in self.context:
            self.tokens.append(_session_id.set(self.context["session_id"]))
        if "agent_id" in self.context:
            self.tokens.append(_agent_id.set(self.context["agent_id"]))
        if "workflow_id" in self.context:
            self.tokens.append(_workflow_id.set(self.context["workflow_id"]))
        return self
    
    def __exit__(self, *args):
        for token in reversed(self.tokens):
            token.var.reset(token)
    
    async def __aenter__(self):
        return self.__enter__()
    
    async def __aexit__(self, *args):
        return self.__exit__(*args)
