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

"""Bootstrap module for zero-code auto-instrumentation.

This module is called by the .pth file on Python startup when AGENTREPLAY_ENABLED=true.
It initializes OpenTelemetry instrumentation with minimal overhead.

Environment Variables:
    AGENTREPLAY_ENABLED: Set to 'true' to enable auto-instrumentation
    AGENTREPLAY_SERVICE_NAME: Service name for traces (default: 'agentreplay-app')
    AGENTREPLAY_OTLP_ENDPOINT: OTLP gRPC endpoint (default: 'localhost:47117')
    AGENTREPLAY_PROJECT_ID: Project ID for traces
    AGENTREPLAY_TENANT_ID: Tenant ID for traces (default: 1)
    AGENTREPLAY_DEBUG: Enable debug logging (default: false)
    AGENTREPLAY_CAPTURE_CONTENT: Capture LLM request/response content (default: true)
    OTEL_EXPORTER_OTLP_ENDPOINT: Standard OTEL endpoint override

Example:
    # Option 1: Automatic via .pth file
    $ export AGENTREPLAY_ENABLED=true
    $ export AGENTREPLAY_PROJECT_ID=27986
    $ python my_app.py  # Auto-instrumented!
    
    # Option 2: Manual initialization
    >>> from agentreplay.bootstrap import init_otel_instrumentation
    >>> init_otel_instrumentation()
"""

import os
import logging
from typing import Optional

logger = logging.getLogger(__name__)

# Global flag to prevent double-initialization
_initialized = False


def init_otel_instrumentation(
    service_name: Optional[str] = None,
    otlp_endpoint: Optional[str] = None,
    project_id: Optional[int] = None,
    tenant_id: Optional[int] = None,
    capture_content: Optional[bool] = None,
    debug: Optional[bool] = None,
) -> bool:
    """Initialize OpenTelemetry instrumentation.
    
    This function sets up the OpenTelemetry SDK with OTLP exporter and
    automatically instruments all available libraries.
    
    Args:
        service_name: Service name (default: from env or 'agentreplay-app')
        otlp_endpoint: OTLP endpoint (default: from env or 'localhost:47117')
        project_id: Project ID (default: from env)
        tenant_id: Tenant ID (default: from env or 1)
        capture_content: Capture LLM content (default: from env or True)
        debug: Enable debug logging (default: from env or False)
    
    Returns:
        True if initialization succeeded, False if already initialized
    
    Example:
        >>> from agentreplay.bootstrap import init_otel_instrumentation
        >>> init_otel_instrumentation(
        ...     service_name="my-agent",
        ...     project_id=27986
        ... )
    """
    global _initialized
    
    if _initialized:
        logger.debug("Agentreplay already initialized, skipping")
        return False
    
    # Read from environment with fallbacks
    service_name = service_name or os.getenv("AGENTREPLAY_SERVICE_NAME", "agentreplay-app")
    otlp_endpoint = otlp_endpoint or os.getenv(
        "AGENTREPLAY_OTLP_ENDPOINT",
        os.getenv("OTEL_EXPORTER_OTLP_ENDPOINT", "localhost:47117")
    )
    
    # Project/tenant IDs
    if project_id is None:
        project_id_str = os.getenv("AGENTREPLAY_PROJECT_ID", "0")
        try:
            project_id = int(project_id_str)
        except ValueError:
            logger.warning(f"Invalid AGENTREPLAY_PROJECT_ID: {project_id_str}, using 0")
            project_id = 0
    
    if tenant_id is None:
        tenant_id_str = os.getenv("AGENTREPLAY_TENANT_ID", "1")
        try:
            tenant_id = int(tenant_id_str)
        except ValueError:
            logger.warning(f"Invalid AGENTREPLAY_TENANT_ID: {tenant_id_str}, using 1")
            tenant_id = 1
    
    # Flags
    if capture_content is None:
        capture_content = os.getenv("AGENTREPLAY_CAPTURE_CONTENT", "true").lower() in {
            "1", "true", "yes"
        }
    
    if debug is None:
        debug = os.getenv("AGENTREPLAY_DEBUG", "false").lower() in {"1", "true", "yes"}
    
    if debug:
        logging.basicConfig(level=logging.DEBUG)
        logger.setLevel(logging.DEBUG)
    
    try:
        # Import here to avoid loading OTEL on every Python startup
        from agentreplay.auto_instrument import setup_instrumentation
        
        logger.info(f"🚀 Initializing Agentreplay for service: {service_name}")
        logger.debug(f"   OTLP Endpoint: {otlp_endpoint}")
        logger.debug(f"   Project ID: {project_id}")
        logger.debug(f"   Tenant ID: {tenant_id}")
        logger.debug(f"   Capture Content: {capture_content}")
        
        setup_instrumentation(
            service_name=service_name,
            otlp_endpoint=otlp_endpoint,
            tenant_id=tenant_id,
            project_id=project_id,
            capture_content=capture_content,
            debug=debug,
        )
        
        _initialized = True
        logger.info("✅ Agentreplay initialization complete")
        return True
        
    except Exception as e:
        logger.error(f"❌ Failed to initialize Agentreplay: {e}", exc_info=debug)
        # Don't crash the user's app - fail open
        return False


def _auto_init():
    """Called by the .pth file on Python startup.
    
    Only initializes if AGENTREPLAY_ENABLED=true to avoid overhead.
    This is the entry point for zero-code auto-instrumentation.
    
    Automatically loads .env file if present for developer convenience.
    """
    # Try to load .env file first (if python-dotenv is available)
    if os.path.exists('.env'):
        try:
            from dotenv import load_dotenv
            load_dotenv('.env', override=False)  # Don't override existing env vars
        except ImportError:
            pass  # python-dotenv not installed, no problem
        except Exception as e:
            pass  # Any other error, fail silently
    
    if not os.getenv("AGENTREPLAY_ENABLED", "").lower() in {"1", "true", "yes"}:
        # Not enabled, skip silently
        return
    
    try:
        init_otel_instrumentation(debug=True)  # Enable debug to see what's happening
    except Exception as e:
        # Fail open - don't break user's app if SDK has issues
        import sys
        print(f"Agentreplay auto-init failed: {e}", file=sys.stderr)
        pass


def is_initialized() -> bool:
    """Check if Agentreplay has been initialized.
    
    Returns:
        True if initialized, False otherwise
    """
    return _initialized


def reset_initialization():
    """Reset initialization state (primarily for testing).
    
    Warning:
        This does not actually tear down the OTEL SDK, it only resets
        the initialization flag. Use only in tests.
    """
    global _initialized
    _initialized = False
