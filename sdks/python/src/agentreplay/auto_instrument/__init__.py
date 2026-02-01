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

"""Automatic instrumentation using OpenTelemetry.

This module provides zero-code observability by leveraging the official
OpenTelemetry instrumentation ecosystem. We don't maintain framework-specific
integrations - we use the community-maintained OTEL instrumentations.

Example:
    >>> from agentreplay import auto_instrument
    >>> auto_instrument("my-service")
    >>> 
    >>> # Now all instrumented libraries automatically emit spans!
    >>> from openai import OpenAI
    >>> client = OpenAI()
    >>> response = client.chat.completions.create(...)  # ✓ Traced via OTEL
"""

from typing import Optional
import logging
import os

logger = logging.getLogger(__name__)

_instrumented = False


def auto_instrument(
    service_name: str,
    otlp_endpoint: str = "localhost:47117",
    tenant_id: int = 1,
    project_id: int = 0,
    capture_content: bool = True,
    debug: bool = False,
) -> None:
    """Automatically instrument using OpenTelemetry.
    
    Sets up OTLP exporter and automatically instruments all available
    libraries using their official OTEL instrumentations.
    
    Args:
        service_name: Name of your service
        otlp_endpoint: OTLP gRPC endpoint (default: localhost:47117)
        tenant_id: Tenant ID (added to resource attributes)
        project_id: Project ID (added to resource attributes)
        capture_content: Capture LLM request/response content (default: True)
        debug: Enable debug logging (default: False)
    """
    global _instrumented
    
    if _instrumented:
        logger.warning("Already instrumented. Skipping.")
        return
    
    if debug:
        logging.basicConfig(level=logging.DEBUG)
        logger.setLevel(logging.DEBUG)
    
    logger.info(f"🚀 Setting up Agentreplay OpenTelemetry for service: {service_name}")
    
    try:
        # Use the otel_bridge to set up tracer provider
        from agentreplay.otel_bridge import setup_tracer_provider
        
        setup_tracer_provider(
            service_name=service_name,
            otlp_endpoint=otlp_endpoint,
            tenant_id=tenant_id,
            project_id=project_id,
            debug=debug,
        )
        
        logger.info(f"✓ OTLP exporter configured: {otlp_endpoint}")
        
        # Set environment for OpenAI instrumentation
        if capture_content:
            os.environ["AGENTREPLAY_CAPTURE_CONTENT"] = "true"
        else:
            os.environ["AGENTREPLAY_CAPTURE_CONTENT"] = "false"
        
        # Auto-discover and instrument everything
        _auto_instrument_all()
        
        _instrumented = True
        logger.info("✅ Auto-instrumentation complete")
        
    except Exception as e:
        logger.error(f"❌ Failed to setup OpenTelemetry: {e}")
        raise


def _auto_instrument_all():
    """Auto-instrument all available libraries using official OTEL instrumentations."""
    
    instrumented = []
    
    # Enable content capture for GenAI instrumentations (OTEL standard)
    import os
    capture_content = os.getenv("AGENTREPLAY_CAPTURE_CONTENT", "true").lower() in {"1", "true", "yes"}
    if capture_content:
        os.environ["OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT"] = "true"
    
    # Try OpenAI
    try:
        from opentelemetry.instrumentation.openai import OpenAIInstrumentor
        OpenAIInstrumentor().instrument()
        instrumented.append("OpenAI")
    except ImportError:
        pass
    except Exception as e:
        logger.debug(f"OpenAI instrumentation failed: {e}")
    
    # Try Anthropic
    try:
        from opentelemetry.instrumentation.anthropic import AnthropicInstrumentor
        AnthropicInstrumentor().instrument()
        instrumented.append("Anthropic")
    except ImportError:
        pass
    except Exception as e:
        logger.debug(f"Anthropic instrumentation failed: {e}")
    
    # Try LangChain - use custom Agentreplay callback handler for hierarchy
    try:
        from agentreplay.langchain_tracer import get_agentreplay_callback
        
        callback = get_agentreplay_callback()
        if callback:
            # Register globally for all LangChain operations
            import langchain_core.callbacks.manager as manager
            if hasattr(manager, '_default_callback_handlers'):
                if not hasattr(manager._default_callback_handlers, '__iter__'):
                    manager._default_callback_handlers = []
                manager._default_callback_handlers.append(callback)
            instrumented.append("LangChain")
    except ImportError:
        pass  # LangChain not installed
    except Exception as e:
        logger.debug(f"LangChain instrumentation failed: {e}")
    
    # Try LlamaIndex
    try:
        from opentelemetry.instrumentation.llamaindex import LlamaIndexInstrumentor
        LlamaIndexInstrumentor().instrument()
        instrumented.append("LlamaIndex")
    except ImportError:
        pass
    except Exception as e:
        logger.debug(f"LlamaIndex instrumentation failed: {e}")
    
    # Try HTTP clients (requests, httpx, urllib3)
    try:
        from opentelemetry.instrumentation.requests import RequestsInstrumentor
        RequestsInstrumentor().instrument()
        instrumented.append("requests")
    except ImportError:
        pass
    except Exception as e:
        logger.debug(f"requests instrumentation failed: {e}")
    
    try:
        from opentelemetry.instrumentation.httpx import HTTPXClientInstrumentor
        HTTPXClientInstrumentor().instrument()
        instrumented.append("httpx")
    except ImportError:
        pass
    except Exception as e:
        logger.debug(f"httpx instrumentation failed: {e}")
    
    if instrumented:
        logger.info(f"✓ Instrumented: {', '.join(instrumented)}")
    else:
        logger.warning("No instrumentable libraries found. Install opentelemetry-instrumentation-* packages.")


def setup_instrumentation(
    service_name: Optional[str] = None,
    otlp_endpoint: Optional[str] = None,
    tenant_id: Optional[int] = None,
    project_id: Optional[int] = None,
    capture_content: Optional[bool] = None,
    debug: Optional[bool] = None,
) -> None:
    """Setup instrumentation with environment variable fallbacks.
    
    This is a convenience wrapper around auto_instrument that reads from
    environment variables if parameters aren't provided.
    
    Args:
        service_name: Service name (default: from env or 'default-service')
        otlp_endpoint: OTLP endpoint (default: from env or 'localhost:47117')
        tenant_id: Tenant ID (default: from env or 1)
        project_id: Project ID (default: from env or 0)
        capture_content: Capture LLM content (default: from env or True)
        debug: Enable debug logging (default: from env or False)
    """
    # Read from environment
    service_name = service_name or os.getenv("AGENTREPLAY_SERVICE_NAME", "default-service")
    otlp_endpoint = otlp_endpoint or os.getenv("AGENTREPLAY_OTLP_ENDPOINT", "localhost:47117")
    
    if tenant_id is None:
        tenant_id = int(os.getenv("AGENTREPLAY_TENANT_ID", "1"))
    
    if project_id is None:
        project_id = int(os.getenv("AGENTREPLAY_PROJECT_ID", "0"))
    
    if capture_content is None:
        capture_content = os.getenv("AGENTREPLAY_CAPTURE_CONTENT", "true").lower() in {
            "1", "true", "yes"
        }
    
    if debug is None:
        debug = os.getenv("AGENTREPLAY_DEBUG", "false").lower() in {"1", "true", "yes"}
    
    auto_instrument(
        service_name=service_name,
        otlp_endpoint=otlp_endpoint,
        tenant_id=tenant_id,
        project_id=project_id,
        capture_content=capture_content,
        debug=debug,
    )


__all__ = ["auto_instrument", "setup_instrumentation"]
