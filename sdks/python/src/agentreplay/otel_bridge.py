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

"""Bridge between Agentreplay spans and OpenTelemetry spans.

This module provides utilities to convert Agentreplay's custom span format
to OpenTelemetry format and vice versa. It also handles the setup of
the OpenTelemetry tracer provider with Agentreplay-specific configuration.

The bridge ensures that:
1. Agentreplay spans are compatible with standard OTLP exporters
2. Agent/session/workflow context is preserved in span attributes
3. Agentreplay's span types map to appropriate OTEL span kinds
"""

from typing import Optional, Dict, Any
import logging
import os
import atexit

from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider, SpanProcessor
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.sdk.resources import Resource, SERVICE_NAME
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.trace import SpanKind, Status, StatusCode

logger = logging.getLogger(__name__)


def setup_tracer_provider(
    service_name: str,
    otlp_endpoint: str,
    tenant_id: int = 1,
    project_id: int = 0,
    debug: bool = False,
) -> TracerProvider:
    """Set up OpenTelemetry tracer provider with Agentreplay configuration.
    
    Args:
        service_name: Service name for resource attributes
        otlp_endpoint: OTLP gRPC endpoint (e.g., 'localhost:47117')
        tenant_id: Agentreplay tenant ID
        project_id: Agentreplay project ID
        debug: Enable debug logging
    
    Returns:
        Configured TracerProvider
    
    Example:
        >>> provider = setup_tracer_provider(
        ...     service_name="my-agent",
        ...     otlp_endpoint="localhost:47117",
        ...     project_id=27986
        ... )
    """
    # Create resource with Agentreplay-specific attributes
    # NOTE: Server expects "tenant_id" and "project_id" (with underscores, not dots)
    resource = Resource.create({
        SERVICE_NAME: service_name,
        "tenant_id": tenant_id,  # Server looks for "tenant.id" or "tenant_id"
        "project_id": project_id,  # Server looks for "project.id" or "project_id"
        "agentreplay.sdk.name": "agentreplay-python",
        "agentreplay.sdk.version": "0.1.0",
    })
    
    # Create tracer provider
    provider = TracerProvider(resource=resource)
    
    # Configure OTLP exporter
    # Note: The endpoint should be gRPC format (no http:// prefix)
    # Standard OTLP uses port 4317 for gRPC, 4318 for HTTP
    if otlp_endpoint.startswith("http://"):
        otlp_endpoint = otlp_endpoint.replace("http://", "")
    if otlp_endpoint.startswith("https://"):
        otlp_endpoint = otlp_endpoint.replace("https://", "")
    
    # Headers for authentication/routing
    headers = {
        "x-agentreplay-tenant-id": str(tenant_id),
        "x-agentreplay-project-id": str(project_id),
    }
    
    # Create OTLP exporter
    # insecure=True because we're using localhost (change for production)
    otlp_exporter = OTLPSpanExporter(
        endpoint=otlp_endpoint,
        headers=headers,
        insecure=True,  # TODO: Make configurable
    )
    
    # Use batch processor for better performance
    span_processor = BatchSpanProcessor(
        otlp_exporter,
        max_queue_size=2048,
        max_export_batch_size=512,
        schedule_delay_millis=5000,  # Export every 5 seconds
    )
    
    provider.add_span_processor(span_processor)
    
    # Set as global tracer provider
    trace.set_tracer_provider(provider)
    
    # Register atexit handler to flush spans when script exits
    # This ensures short-lived scripts export their spans before terminating
    def _flush_on_exit():
        """Flush any pending spans before process exits."""
        try:
            provider.force_flush(timeout_millis=5000)
            if debug:
                logger.debug("✓ Spans flushed on exit")
        except Exception as e:
            logger.debug(f"Failed to flush spans on exit: {e}")
    
    atexit.register(_flush_on_exit)
    
    if debug:
        logger.debug(f"✓ TracerProvider configured:")
        logger.debug(f"  Service: {service_name}")
        logger.debug(f"  Endpoint: {otlp_endpoint}")
        logger.debug(f"  Tenant ID: {tenant_id}")
        logger.debug(f"  Project ID: {project_id}")
    
    return provider


def agentreplay_span_type_to_otel_kind(span_type: str) -> SpanKind:
    """Convert Agentreplay span type to OTEL span kind.
    
    Args:
        span_type: Agentreplay span type (e.g., 'Planning', 'Reasoning', 'ToolCall')
    
    Returns:
        Appropriate SpanKind
    """
    # Map Agentreplay types to OTEL span kinds
    mapping = {
        "Planning": SpanKind.INTERNAL,
        "Reasoning": SpanKind.INTERNAL,
        "ToolCall": SpanKind.CLIENT,
        "Synthesis": SpanKind.INTERNAL,
        "Root": SpanKind.SERVER,
        "Error": SpanKind.INTERNAL,
        "Response": SpanKind.SERVER,
    }
    return mapping.get(span_type, SpanKind.INTERNAL)


def inject_agent_context_to_span(span: trace.Span, context: Dict[str, Any]):
    """Inject Agentreplay agent context into an OTEL span.
    
    Args:
        span: OpenTelemetry span
        context: Context dictionary with agent_id, session_id, etc.
    
    Example:
        >>> with tracer.start_as_current_span("operation") as span:
        ...     inject_agent_context_to_span(span, {
        ...         "agent_id": "researcher",
        ...         "session_id": "sess-123"
        ...     })
    """
    # Add context as span attributes
    if "agent_id" in context:
        span.set_attribute("agentreplay.agent_id", context["agent_id"])
    
    if "session_id" in context:
        span.set_attribute("agentreplay.session_id", context["session_id"])
    
    if "workflow_id" in context:
        span.set_attribute("agentreplay.workflow_id", context["workflow_id"])
    
    if "user_id" in context:
        span.set_attribute("agentreplay.user_id", context["user_id"])


def _inject_agent_context(span: trace.Span):
    """Inject current agent context from contextvars into span.
    
    This reads from the global context variables set by AgentContext
    and adds them to the span automatically.
    
    Args:
        span: OpenTelemetry span to annotate
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
        # Context module not available, skip
        pass


def get_tracer(name: str = "agentreplay") -> trace.Tracer:
    """Get an OpenTelemetry tracer for Agentreplay.
    
    Args:
        name: Tracer name (default: 'agentreplay')
    
    Returns:
        Configured tracer
    
    Example:
        >>> tracer = get_tracer("my-component")
        >>> with tracer.start_as_current_span("operation") as span:
        ...     span.set_attribute("key", "value")
    """
    return trace.get_tracer(name)


def create_span_with_context(
    tracer: trace.Tracer,
    name: str,
    kind: Optional[SpanKind] = None,
    attributes: Optional[Dict[str, Any]] = None,
) -> trace.Span:
    """Create a span with automatic agent context injection.
    
    Args:
        tracer: OpenTelemetry tracer
        name: Span name
        kind: Span kind (default: INTERNAL)
        attributes: Additional attributes
    
    Returns:
        Started span with context
    
    Example:
        >>> tracer = get_tracer()
        >>> span = create_span_with_context(
        ...     tracer,
        ...     "llm_call",
        ...     kind=SpanKind.CLIENT,
        ...     attributes={"model": "gpt-4"}
        ... )
    """
    kind = kind or SpanKind.INTERNAL
    span = tracer.start_span(name, kind=kind)
    
    # Inject agent context
    _inject_agent_context(span)
    
    # Add custom attributes
    if attributes:
        for key, value in attributes.items():
            span.set_attribute(key, str(value))
    
    return span
