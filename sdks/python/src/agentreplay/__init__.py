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
Agentreplay Python SDK - Agent Trace Engine for LLM Agents.

Modern, ergonomic SDK for observability in LLM applications.

Quick Start:
    >>> import agentreplay
    >>> 
    >>> # Initialize with env vars (recommended)
    >>> agentreplay.init()
    >>> 
    >>> # Or with explicit config
    >>> agentreplay.init(
    ...     api_key="your-key",
    ...     project_id="my-project",
    ... )
    >>> 
    >>> # Trace functions with decorator
    >>> @agentreplay.traceable
    >>> def my_llm_function(query: str) -> str:
    ...     return call_llm(query)
    >>> 
    >>> # Wrap OpenAI for automatic tracing
    >>> from openai import OpenAI
    >>> client = agentreplay.wrap_openai(OpenAI())
    >>> 
    >>> # Ensure traces are sent before exit
    >>> agentreplay.flush()
"""

from agentreplay.client import AgentreplayClient
from agentreplay.models import SpanType, AgentFlowEdge
from agentreplay.span import Span
from agentreplay.config import AgentreplayConfig, get_config, set_config, reset_config
from agentreplay.batching import BatchingAgentreplayClient
from agentreplay.session import Session
from agentreplay.retry import retry_with_backoff
from agentreplay.exceptions import (
    AgentreplayError,
    AuthenticationError,
    RateLimitError,
    ServerError,
    ValidationError,
    NotFoundError,
    NetworkError,
)

# Agent Context Tracking
from agentreplay.context import (
    AgentContext,
    set_context,
    get_global_context,
    clear_context,
    with_context,
)

# Auto-instrumentation (Pure OpenTelemetry) - Optional
try:
    from agentreplay.auto_instrument import auto_instrument, setup_instrumentation
except ImportError:
    auto_instrument = None  # type: ignore
    setup_instrumentation = None  # type: ignore

# OTEL Bridge & Bootstrap - Optional (requires opentelemetry-sdk)
try:
    from agentreplay.bootstrap import init_otel_instrumentation, is_initialized
    from agentreplay.otel_bridge import get_tracer
except ImportError:
    init_otel_instrumentation = None  # type: ignore
    is_initialized = lambda: False  # type: ignore
    get_tracer = None  # type: ignore

# =============================================================================
# Ergonomic Top-Level API (v0.4+)
# =============================================================================

# SDK Lifecycle
from agentreplay.sdk import (
    init,
    get_client,
    get_batching_client,
    flush,
    shutdown,
    reset,
    get_stats,
    ping,
)

# Decorators & Tracing
from agentreplay.decorators import (
    traceable,
    observe,  # Langfuse-style alias
    trace,
    start_span,
    get_current_span,
    SpanKind,
    ActiveSpan,
)

# Client Wrappers
from agentreplay.wrappers import (
    wrap_openai,
    wrap_anthropic,
    wrap_method,
)

# Privacy
from agentreplay.privacy import (
    configure_privacy,
    redact_payload,
    redact_string,
    hash_pii,
    add_pattern,
    add_scrub_path,
    privacy_context,
)

__version__ = "0.1.2"

__all__ = [
    # ==========================================================================
    # MODERN ERGONOMIC API (Recommended)
    # ==========================================================================
    
    # Initialization (one-liner setup)
    "init",
    "flush",
    "shutdown",
    "reset",
    "get_stats",
    "ping",
    
    # Tracing decorators
    "traceable",
    "observe",  # Langfuse-style alias
    "trace",
    "start_span",
    "get_current_span",
    "SpanKind",
    "ActiveSpan",
    
    # Client wrappers (one-liner instrumentation)
    "wrap_openai",
    "wrap_anthropic",
    "wrap_method",
    
    # Context management
    "set_context",
    "get_global_context",
    "clear_context",
    "with_context",
    "AgentContext",  # Class-based context
    
    # Privacy
    "configure_privacy",
    "redact_payload",
    "redact_string",
    "hash_pii",
    "add_pattern",
    "add_scrub_path",
    "privacy_context",
    
    # ==========================================================================
    # CORE API (Advanced Usage)
    # ==========================================================================
    
    # Core client
    "get_client",
    "get_batching_client",
    "AgentreplayClient",
    "BatchingAgentreplayClient",
    
    # Models
    "SpanType",
    "AgentFlowEdge",
    "Span",
    
    # Configuration
    "AgentreplayConfig",
    "get_config",
    "set_config",
    "reset_config",
    
    # Session management
    "Session",
    
    # Retry utilities
    "retry_with_backoff",
    
    # Auto-instrumentation (Pure OpenTelemetry)
    "auto_instrument",
    "setup_instrumentation",
    
    # OTEL Initialization
    "init_otel_instrumentation",
    "is_initialized",
    
    # OTEL Bridge
    "get_tracer",
    
    # Exceptions
    "AgentreplayError",
    "AuthenticationError",
    "RateLimitError",
    "ServerError",
    "ValidationError",
    "NotFoundError",
    "NetworkError",
]
