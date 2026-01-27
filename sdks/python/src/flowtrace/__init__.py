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

"""Flowtrace Python SDK - Agent Trace Engine for LLM Agents."""

from flowtrace.client import FlowtraceClient
from flowtrace.models import SpanType, AgentFlowEdge
from flowtrace.span import Span
from flowtrace.config import FlowtraceConfig, get_config, set_config, reset_config
from flowtrace.batching import BatchingFlowtraceClient
from flowtrace.session import Session
from flowtrace.retry import retry_with_backoff
from flowtrace.exceptions import (
    FlowtraceError,
    AuthenticationError,
    RateLimitError,
    ServerError,
    ValidationError,
    NotFoundError,
    NetworkError,
)

# Agent Context Tracking
from flowtrace.context import AgentContext

# Auto-instrumentation (Pure OpenTelemetry)
from flowtrace.auto_instrument import auto_instrument, setup_instrumentation

# OTEL Bridge & Bootstrap
from flowtrace.bootstrap import init_otel_instrumentation, is_initialized
from flowtrace.otel_bridge import get_tracer

__version__ = "0.1.0"

__all__ = [
    # Core client
    "FlowtraceClient",
    "BatchingFlowtraceClient",
    # Models
    "SpanType",
    "AgentFlowEdge",
    "Span",
    # Configuration
    "FlowtraceConfig",
    "get_config",
    "set_config",
    "reset_config",
    # Session management
    "Session",
    # Retry utilities
    "retry_with_backoff",
    # Agent Context
    "AgentContext",
    # Auto-instrumentation (Pure OpenTelemetry)
    "auto_instrument",
    "setup_instrumentation",
    # OTEL Initialization
    "init_otel_instrumentation",
    "is_initialized",
    # OTEL Bridge
    "get_tracer",
    # Exceptions
    "FlowtraceError",
    "AuthenticationError",
    "RateLimitError",
    "ServerError",
    "ValidationError",
    "NotFoundError",
    "NetworkError",
]
